//! Monde cote serveur : chunks, conteneurs et sauvegarde sur disque.
use serde::{Deserialize, Serialize};
use shared::blocks;
use shared::chunk::{idx, Chunk, H};
use shared::gen;
use shared::items::Slot;
use shared::physics::BlockSource;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

/// Fichier qui signale un monde deja au format de 384 blocs de haut.
const FORMAT_MARK: &str = "format384";

pub struct SChunk {
    pub c: Chunk,
    pub modified: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Furnace {
    pub input: Slot,
    pub fuel: Slot,
    pub output: Slot,
    pub burn: f64,
    pub burn_max: f64,
    pub cook: f64,
}

/// Animal conserve d'une session a l'autre.
#[derive(Serialize, Deserialize, Clone)]
pub struct MobSave {
    pub kind: u8,
    pub pos: [f64; 3],
    pub hp: f64,
    pub age: f64,
}

#[derive(Serialize, Deserialize, Default)]
struct WorldSave {
    seed: u32,
    time: f64,
    furnaces: Vec<([i32; 3], Furnace)>,
    chests: Vec<([i32; 3], Vec<Slot>)>,
    mobs: Vec<MobSave>,
}

/// En-tete des sauvegardes en double precision (voir `SAVE_MAGIC` du serveur).
const MAGIC: &[u8; 4] = crate::SAVE_MAGIC;

/// Four, creature et monde des sauvegardes en simple precision.
#[derive(Deserialize)]
struct Furnace32 {
    input: Slot,
    fuel: Slot,
    output: Slot,
    burn: f32,
    burn_max: f32,
    cook: f32,
}

#[derive(Deserialize)]
struct MobSave32 {
    kind: u8,
    pos: [f32; 3],
    hp: f32,
    age: f32,
}

#[derive(Deserialize)]
struct WorldSaveV2 {
    seed: u32,
    time: f32,
    furnaces: Vec<([i32; 3], Furnace32)>,
    chests: Vec<([i32; 3], Vec<Slot>)>,
    mobs: Vec<MobSave32>,
}

/// Format d'avant les creatures, encore accepte a la lecture.
#[derive(Deserialize)]
struct WorldSaveV1 {
    seed: u32,
    time: f32,
    furnaces: Vec<([i32; 3], Furnace32)>,
    chests: Vec<([i32; 3], Vec<Slot>)>,
}

fn read_save(data: &[u8]) -> Option<WorldSave> {
    if let Some(body) = data.strip_prefix(MAGIC) {
        return bincode::deserialize::<WorldSave>(body).ok();
    }
    let old = |f: Vec<([i32; 3], Furnace32)>| {
        f.into_iter()
            .map(|(p, x)| (p, Furnace { input: x.input, fuel: x.fuel, output: x.output, burn: x.burn as f64, burn_max: x.burn_max as f64, cook: x.cook as f64 }))
            .collect()
    };
    if let Ok(v) = bincode::deserialize::<WorldSaveV2>(data) {
        let mobs = v.mobs.into_iter().map(|m| MobSave { kind: m.kind, pos: [m.pos[0] as f64, m.pos[1] as f64, m.pos[2] as f64], hp: m.hp as f64, age: m.age as f64 }).collect();
        return Some(WorldSave { seed: v.seed, time: v.time as f64, furnaces: old(v.furnaces), chests: v.chests, mobs });
    }
    let v1: WorldSaveV1 = bincode::deserialize(data).ok()?;
    Some(WorldSave { seed: v1.seed, time: v1.time as f64, furnaces: old(v1.furnaces), chests: v1.chests, mobs: Vec::new() })
}

/// Changement de circuit programme pour un battement a venir.
#[derive(Clone, Copy)]
pub struct Pending {
    pub due: u64,
    pub pos: [i32; 3],
    pub kind: u8,
    pub val: u8,
}

pub struct World {
    pub seed: u32,
    pub time: f64,
    /// Dossier de sauvegarde ; None = monde en memoire seulement.
    pub dir: Option<PathBuf>,
    pub chunks: HashMap<(i32, i32), SChunk>,
    /// Chunks generes d'avance, en attente d'etre charges par `ensure`.
    pub pregen: HashMap<(i32, i32), Chunk>,
    pub furnaces: HashMap<[i32; 3], Furnace>,
    pub chests: HashMap<[i32; 3], Vec<Slot>>,
    /// Foyers irori : aliments en train de cuire et secondes ecoulees (non sauvegarde).
    pub irori: HashMap<[i32; 3], Vec<(shared::items::Stack, f64)>>,
    /// Changements de blocs a diffuser aux clients.
    pub changes: Vec<([i32; 3], u8, u8)>,
    /// Blocs a reexaminer (fluides, sable, plantes) parce qu'un voisin a change.
    pub updates: HashSet<[i32; 3]>,
    /// Animaux lus dans la sauvegarde, a faire reapparaitre au demarrage.
    pub saved_mobs: Vec<MobSave>,
    /// 0 surface, 1 Abime, 2 Zenith.
    pub dim: u8,
    /// Blocs de circuit charges, et drapeau "a recalculer".
    pub signal: HashSet<[i32; 3]>,
    pub signal_dirty: bool,
    /// Battements de circuit ecoules (un toutes les 0,1 s).
    pub rt: u64,
    pub pending: Vec<Pending>,
    /// Dalles enfoncees et battement du dernier passage.
    pub plates: HashMap<[i32; 3], u64>,
    /// Lanternes hitodama grillees, jusqu'a quel battement.
    pub burnt: HashMap<[i32; 3], u64>,
    /// Extinctions recentes de chaque lanterne.
    pub toggles: HashMap<[i32; 3], Vec<u64>>,
    /// Blocs changes depuis le dernier battement, pour les daruma guetteurs.
    pub observed: Vec<[i32; 3]>,
    /// Futons des maisons que la generation vient de poser : un villageois nait pres de chacun.
    pub new_homes: Vec<[i32; 3]>,
    /// Explosifs amorces : (position, secondes restantes).
    pub primed: Vec<([i32; 3], f64)>,
    /// Le boss du Zenith a ete vaincu.
    pub boss_dead: bool,
    /// Sauvegarde d'avant le monde de 384 blocs, qui vient d'etre remontee : les joueurs restent a remonter.
    pub lifted: bool,
}

fn key_of(p: [i32; 3]) -> (i32, i32) {
    (p[0] >> 4, p[2] >> 4)
}

impl World {
    /// Ouvre le monde du dossier s'il existe ; sinon en cree un avec `seed`.
    pub fn open(seed: u32, dir: Option<PathBuf>, dim: u8) -> World {
        let mut w = World {
            seed,
            time: 0.05,
            dir,
            chunks: HashMap::new(),
            pregen: HashMap::new(),
            furnaces: HashMap::new(),
            chests: HashMap::new(),
            irori: HashMap::new(),
            changes: Vec::new(),
            updates: HashSet::new(),
            saved_mobs: Vec::new(),
            dim,
            signal: HashSet::new(),
            signal_dirty: false,
            rt: 0,
            pending: Vec::new(),
            plates: HashMap::new(),
            burnt: HashMap::new(),
            toggles: HashMap::new(),
            observed: Vec::new(),
            new_homes: Vec::new(),
            primed: Vec::new(),
            boss_dead: false,
            lifted: false,
        };
        if let Some(d) = w.dir.clone() {
            if let Err(e) = fs::create_dir_all(d.join("joueurs")) {
                eprintln!("Sauvegarde desactivee ({}) : {e}", d.display());
                w.dir = None;
            } else if let Some(s) = fs::read(d.join(Self::meta_name(dim))).ok().and_then(|b| read_save(&b)) {
                w.boss_dead = d.join("wyrm_vaincu").exists();
                w.saved_mobs = s.mobs;
                w.seed = s.seed;
                w.time = s.time;
                w.furnaces = s.furnaces.into_iter().collect();
                w.chests = s.chests.into_iter().collect();
                // Monde d'avant les 384 blocs de haut : tout ce qui est a la surface remonte de 64 blocs,
                // comme les chunks eux-memes au moment ou ils sont relus.
                if dim == 0 && !d.join(FORMAT_MARK).exists() {
                    let up = shared::chunk::LEGACY_SHIFT as i32;
                    w.furnaces = w.furnaces.drain().map(|(p, f)| ([p[0], p[1] + up, p[2]], f)).collect();
                    w.chests = w.chests.drain().map(|(p, c)| ([p[0], p[1] + up, p[2]], c)).collect();
                    for m in w.saved_mobs.iter_mut() {
                        m.pos[1] += up as f64;
                    }
                    w.lifted = true;
                }
            }
            if dim == 0 && w.dir.is_some() {
                let _ = fs::write(d.join(FORMAT_MARK), b"384");
            }
        }
        w
    }

    fn meta_name(dim: u8) -> String {
        if dim == 0 {
            "monde.bin".to_string()
        } else {
            format!("monde_d{dim}.bin")
        }
    }

    fn chunk_path(&self, key: (i32, i32)) -> Option<PathBuf> {
        let prefix = if self.dim == 0 { String::new() } else { format!("d{}_", self.dim) };
        self.dir.as_ref().map(|d| d.join(format!("{prefix}c_{}_{}.bin", key.0, key.1)))
    }

    pub fn set_boss_dead(&mut self) {
        self.boss_dead = true;
        if let Some(d) = &self.dir {
            let _ = fs::write(d.join("wyrm_vaincu"), b"1");
        }
    }

    /// Charge le chunk depuis le disque, ou le genere.
    /// Genere d'avance, sur plusieurs coeurs, ceux de ces chunks qui n'existent pas encore (ni en memoire,
    /// ni sur le disque). `ensure` les trouvera prets : le tick du serveur n'attend plus la generation.
    pub fn pregenerate(&mut self, keys: &[(i32, i32)]) {
        let missing: Vec<(i32, i32)> = keys
            .iter()
            .copied()
            .filter(|k| !self.chunks.contains_key(k) && !self.pregen.contains_key(k) && !self.chunk_path(*k).is_some_and(|p| p.exists()))
            .collect();
        if missing.len() < 2 {
            return;
        }
        let (dim, seed) = (self.dim, self.seed);
        let made: Vec<((i32, i32), Chunk)> = std::thread::scope(|s| {
            let jobs: Vec<_> = missing.iter().map(|&k| s.spawn(move || (k, gen::generate(dim, seed, k.0, k.1)))).collect();
            jobs.into_iter().filter_map(|j| j.join().ok()).collect()
        });
        self.pregen.extend(made);
    }

    pub fn ensure(&mut self, key: (i32, i32)) -> &SChunk {
        if !self.chunks.contains_key(&key) {
            let saved = self.chunk_path(key).and_then(|p| fs::read(p).ok()).and_then(|d| {
                // Sauvegarde d'avant le monde de 384 blocs : la surface est remontee de 64, pas les autres mondes.
                let shift = if self.dim == 0 { shared::chunk::LEGACY_SHIFT } else { 0 };
                Chunk::decode(&d).or_else(|| Chunk::decode_legacy(&d, shift))
            });
            let fresh = saved.is_none();
            // Le chunk a pu etre genere d'avance, en parallele, par `pregenerate`.
            let ahead = self.pregen.remove(&key);
            let c = saved.or(ahead).unwrap_or_else(|| gen::generate(self.dim, self.seed, key.0, key.1));
            // On recense les blocs de circuit, et sur un chunk neuf on garnit ce que la generation a pose :
            // coffres (leur metadonnee dit quel butin), distributeurs des pieges, maisons a peupler.
            let (seed, dim) = (self.seed, self.dim);
            let mut settled = false;
            for (i, &b) in c.b.iter().enumerate() {
                if b == 0 || !(blocks::is_signal(b) || (fresh && matches!(b, blocks::CHEST | blocks::BED))) {
                    continue;
                }
                let (x, z, y) = ((i % 16) as i32, ((i / 16) % 16) as i32, (i / 256) as i32);
                let p = [key.0 * 16 + x, y, key.1 * 16 + z];
                if blocks::is_signal(b) {
                    self.signal.insert(p);
                    self.signal_dirty = true;
                }
                if !fresh {
                    continue;
                }
                match b {
                    // Un coffre deja connu (ouvert, vide) garde son contenu.
                    blocks::CHEST => {
                        let table = c.m[i];
                        self.chests.entry(p).or_insert_with(|| gen::chest_loot(dim, seed, p, table));
                    }
                    blocks::DROPPER => {
                        self.chests.entry(p).or_insert_with(|| {
                            let mut slots = vec![None; 27];
                            slots[4] = Some(shared::items::Stack::new(shared::items::ARROW, 16));
                            slots
                        });
                    }
                    blocks::BED if dim == 0 => {
                        self.new_homes.push(p);
                        settled = true;
                    }
                    _ => {}
                }
            }
            // Un chunk peuple est aussitot tenu pour modifie : relu du disque, il ne repeuplera pas.
            self.chunks.insert(key, SChunk { c, modified: settled });
        }
        &self.chunks[&key]
    }

    /// (bloc, metadonnee), ou None si la zone n'est pas chargee.
    pub fn get(&self, p: [i32; 3]) -> Option<(u8, u8)> {
        if p[1] < 0 || p[1] >= H as i32 {
            return None;
        }
        let c = self.chunks.get(&key_of(p))?;
        let i = idx((p[0] & 15) as usize, p[1] as usize, (p[2] & 15) as usize);
        Some((c.c.b[i], c.c.m[i]))
    }

    pub fn set(&mut self, p: [i32; 3], b: u8, m: u8) -> bool {
        if p[1] < 1 || p[1] >= H as i32 {
            return false;
        }
        let Some(c) = self.chunks.get_mut(&key_of(p)) else { return false };
        let i = idx((p[0] & 15) as usize, p[1] as usize, (p[2] & 15) as usize);
        if c.c.b[i] == b && c.c.m[i] == m {
            return true;
        }
        let old = c.c.b[i];
        c.c.b[i] = b;
        c.c.m[i] = m;
        c.modified = true;
        if blocks::is_signal(old) || blocks::is_signal(b) {
            if blocks::is_signal(b) {
                self.signal.insert(p);
            } else {
                self.signal.remove(&p);
            }
            self.signal_dirty = true;
        }
        self.changes.push((p, b, m));
        if self.observed.len() < 8192 {
            self.observed.push(p);
        }
        self.updates.insert(p);
        for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
            self.updates.insert([p[0] + d[0], p[1] + d[1], p[2] + d[2]]);
        }
        true
    }

    fn write_chunk(&self, key: (i32, i32), c: &SChunk) {
        if let Some(p) = self.chunk_path(key) {
            if let Err(e) = fs::write(&p, c.c.encode()) {
                eprintln!("Echec de l'ecriture de {} : {e}", p.display());
            }
        }
    }

    /// Libere les chunks que personne ne regarde, en sauvant ceux qui ont change.
    pub fn unload_unused(&mut self, mut used: impl FnMut(&(i32, i32)) -> bool) {
        let drop: Vec<(i32, i32)> = self.chunks.keys().filter(|k| !used(k)).copied().collect();
        for key in drop {
            if let Some(c) = self.chunks.remove(&key) {
                if c.modified {
                    self.write_chunk(key, &c);
                }
            }
        }
    }

    /// Ecrit tout ce qui a change depuis la derniere sauvegarde.
    pub fn save(&mut self, mobs: Vec<MobSave>) {
        let Some(dir) = self.dir.clone() else { return };
        let keys: Vec<(i32, i32)> = self.chunks.iter().filter(|(_, c)| c.modified).map(|(k, _)| *k).collect();
        for key in keys {
            self.write_chunk(key, &self.chunks[&key]);
            if let Some(c) = self.chunks.get_mut(&key) {
                c.modified = false;
            }
        }
        let save = WorldSave {
            seed: self.seed,
            time: self.time,
            furnaces: self.furnaces.iter().map(|(k, v)| (*k, v.clone())).collect(),
            chests: self.chests.iter().map(|(k, v)| (*k, v.clone())).collect(),
            mobs,
        };
        match bincode::serialize(&save) {
            Ok(body) => {
                let mut data = MAGIC.to_vec();
                data.extend(body);
                if let Err(e) = fs::write(dir.join(Self::meta_name(self.dim)), data) {
                    eprintln!("Echec de la sauvegarde du monde : {e}");
                }
            }
            Err(e) => eprintln!("Echec de la sauvegarde du monde : {e}"),
        }
    }

    pub fn player_path(&self, name: &str) -> Option<PathBuf> {
        let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        self.dir.as_ref().map(|d| d.join("joueurs").join(format!("{safe}.bin")))
    }
}

impl BlockSource for World {
    fn solid_at(&self, x: i32, y: i32, z: i32) -> bool {
        if y < 0 || y >= H as i32 {
            return false;
        }
        match self.get([x, y, z]) {
            Some((b, m)) => blocks::solid(b, m),
            None => true,
        }
    }
}
