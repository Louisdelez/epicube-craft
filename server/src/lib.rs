//! Serveur Epicube Craft : fait autorite sur le monde, les inventaires et la survie.
mod chat;
mod combat;
mod farm;
mod mobs;
mod rails;
mod realms;
mod signal;
mod sim;
mod village;
mod world;

use glam::DVec3 as Vec3;
use serde::{Deserialize, Serialize};
use shared::blocks::{self, AIR, CHEST, CRAFT, FURNACE, LAVA, WATER};
use shared::chunk::H;
use shared::craft;
use shared::items::{self, add_to, Slot, Stack};
use shared::physics::d::step_drop;
use shared::proto::{read_msg, write_msg, ClientMsg, ServerMsg, SlotRef, WindowKind, PROTOCOL_VERSION};
use shared::util::Rng;
use shared::{gen, DAY_SECONDS, TPS};
use std::collections::{HashMap, HashSet};
use std::io::{BufReader, BufWriter};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use world::World;

/// Chunks envoyes a chaque joueur par tick, au maximum.
const CHUNKS_PER_TICK: usize = 12;
const MAX_VIEW: i32 = 12;
const REACH: f64 = 7.0;
const DT: f64 = 1.0 / TPS as f64;

enum Event {
    Join(u32, TcpStream),
    Msg(u32, ClientMsg),
    Leave(u32),
    Shutdown,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Win {
    None,
    Inventory,
    Table,
    Furnace([i32; 3]),
    Chest([i32; 3]),
    /// Echange avec le villageois de cet identifiant.
    Trade(u32),
}

impl Win {
    fn kind(self) -> WindowKind {
        match self {
            Win::None => WindowKind::None,
            Win::Inventory => WindowKind::Inventory,
            Win::Table => WindowKind::Table,
            Win::Furnace(_) => WindowKind::Furnace,
            Win::Chest(_) => WindowKind::Chest,
            Win::Trade(_) => WindowKind::Trade,
        }
    }
    fn pos(self) -> Option<[i32; 3]> {
        match self {
            Win::Furnace(p) | Win::Chest(p) => Some(p),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct PlayerSave {
    pos: [f64; 3],
    hp: f64,
    hunger: f64,
    inv: Vec<Slot>,
    /// Point de reapparition choisi en dormant.
    home: Option<[f64; 3]>,
    dim: u8,
}

/// En-tete des sauvegardes en double precision. Les fichiers sans en-tete sont les anciens, en simple precision.
pub(crate) const SAVE_MAGIC: &[u8; 4] = b"VX64";

/// Format d'avant la double precision.
#[derive(Deserialize)]
struct PlayerSaveV3 {
    pos: [f32; 3],
    hp: f32,
    hunger: f32,
    inv: Vec<Slot>,
    home: Option<[f32; 3]>,
    dim: u8,
}

/// Format d'avant les dimensions.
#[derive(Deserialize)]
struct PlayerSaveV2 {
    pos: [f32; 3],
    hp: f32,
    hunger: f32,
    inv: Vec<Slot>,
    home: Option<[f32; 3]>,
}

/// Format d'avant les lits, encore accepte a la lecture.
#[derive(Deserialize)]
struct PlayerSaveV1 {
    pos: [f32; 3],
    hp: f32,
    hunger: f32,
    inv: Vec<Slot>,
}

fn wide(p: [f32; 3]) -> [f64; 3] {
    [p[0] as f64, p[1] as f64, p[2] as f64]
}

fn encode_player(p: &PlayerSave) -> Option<Vec<u8>> {
    let mut data = SAVE_MAGIC.to_vec();
    data.extend(bincode::serialize(p).ok()?);
    Some(data)
}

/// Remonte de 64 blocs les joueurs sauvegardes a la surface d'un ancien monde.
fn lift_players(world: &World) {
    let Some(dir) = world.dir.as_ref().map(|d| d.join("joueurs")) else { return };
    let Ok(files) = std::fs::read_dir(dir) else { return };
    let up = shared::chunk::LEGACY_SHIFT as f64;
    for f in files.flatten() {
        let Some(mut p) = std::fs::read(f.path()).ok().and_then(|d| read_player(&d)) else { continue };
        if p.dim & 15 == 0 {
            p.pos[1] += up;
        }
        // Le point de reapparition (le futon) est toujours a la surface.
        if let Some(h) = p.home.as_mut() {
            h[1] += up;
        }
        if let Some(data) = encode_player(&p) {
            let _ = std::fs::write(f.path(), data);
        }
    }
}

fn read_player(data: &[u8]) -> Option<PlayerSave> {
    if let Some(body) = data.strip_prefix(SAVE_MAGIC) {
        return bincode::deserialize::<PlayerSave>(body).ok();
    }
    // Anciens formats, du plus recent au plus ancien.
    if let Ok(v) = bincode::deserialize::<PlayerSaveV3>(data) {
        return Some(PlayerSave { pos: wide(v.pos), hp: v.hp as f64, hunger: v.hunger as f64, inv: v.inv, home: v.home.map(wide), dim: v.dim });
    }
    if let Ok(v) = bincode::deserialize::<PlayerSaveV2>(data) {
        return Some(PlayerSave { pos: wide(v.pos), hp: v.hp as f64, hunger: v.hunger as f64, inv: v.inv, home: v.home.map(wide), dim: 0 });
    }
    let v1: PlayerSaveV1 = bincode::deserialize(data).ok()?;
    Some(PlayerSave { pos: wide(v1.pos), hp: v1.hp as f64, hunger: v1.hunger as f64, inv: v1.inv, home: None, dim: 0 })
}

struct Client {
    tx: Sender<ServerMsg>,
    name: String,
    pos: [f64; 3],
    prev: [f64; 3],
    yaw: f64,
    pitch: f64,
    view: i32,
    ready: bool,
    moved: bool,
    sent: HashSet<(i32, i32)>,
    inv: [Slot; 36],
    /// Equipement porte : tete, torse, jambes, pieds.
    armor: [Slot; 4],
    /// Derniere tenue annoncee aux autres joueurs.
    gear_sent: [u16; 4],
    cursor: Slot,
    craft: [Slot; 9],
    window: Win,
    sel: usize,
    hp: f64,
    hunger: f64,
    exhaustion: f64,
    on_ground: bool,
    fly: bool,
    /// Mode de jeu (voir `shared::rules`).
    mode: u8,
    fall_peak: f64,
    dead: bool,
    regen_t: f64,
    hurt_t: f64,
    last_dig: Instant,
    /// Envois de voix recus pendant la seconde en cours, et debut de cette seconde.
    voice_n: u32,
    voice_t: Instant,
    next_eat: Instant,
    inv_dirty: bool,
    stats_dirty: bool,
    /// Dimension ou se trouve le joueur.
    dim: u8,
    /// Secondes de souffle restantes sous l'eau.
    air: f64,
    /// Invulnerabilite juste apres un coup.
    inv_t: f64,
    home: Option<[f64; 3]>,
    last_hit: Instant,
    /// Chariot dans lequel le joueur est assis.
    riding: Option<u32>,
    /// Reserve cachee qui se vide avant la faim.
    saturation: f64,
    /// Effets en cours, en secondes restantes : nausee, poison, bienfait du the.
    queasy_t: f64,
    poison_t: f64,
    tea_t: f64,
    effect_acc: f64,
    fishing: Option<farm::Fishing>,
    /// Bouclier leve.
    blocking: bool,
    /// Secondes passees dans le voile d'un torii (negatif : vient d'arriver, doit d'abord en sortir).
    veil_t: f64,
    /// Robe de plumes deployee.
    gliding: bool,
    glide_acc: f64,
}

impl Client {
    /// Encaisse un coup de `dmg` a travers l'armure : les pieces s'usent (et cassent), le reste passe.
    /// Renvoie les degats subis.
    fn absorb(&mut self, dmg: f64) -> f64 {
        let (points, toughness, _) = items::defense(&self.armor);
        let wear = items::armor_wear(dmg);
        for s in self.armor.iter_mut() {
            if let Some(st) = s.as_mut().filter(|st| items::armor(st.id).is_some()) {
                st.dur = st.dur.saturating_sub(wear);
                if st.dur == 0 {
                    *s = None;
                }
                self.inv_dirty = true;
            }
        }
        items::mitigate(dmg, points, toughness)
    }

    fn chunk(&self) -> (i32, i32) {
        ((self.pos[0].floor() as i32) >> 4, (self.pos[2].floor() as i32) >> 4)
    }
    fn eye(&self) -> Vec3 {
        Vec3::from(self.pos) + Vec3::Y * 1.62
    }
    fn held(&self) -> Option<u16> {
        self.inv[self.sel].map(|s| s.id)
    }
    fn send(&self, msg: ServerMsg) {
        let _ = self.tx.send(msg);
    }
}

struct Drop {
    id: u32,
    pos: Vec3,
    vel: Vec3,
    stack: Stack,
    age: f64,
    /// Delai avant de pouvoir etre ramasse.
    delay: f64,
}

struct Server {
    /// Regles du monde : difficulte, mode extreme, mode des nouveaux venus, droit de changer de mode.
    rules: shared::rules::Rules,
    /// Dossier de la sauvegarde, ou les regles sont notees.
    save_dir: Option<PathBuf>,
    world: World,
    tick: u64,
    /// Cadence mesuree (ticks par seconde) et duree moyenne d'un tick (ms), pour l'ecran d'informations des joueurs.
    tps: f64,
    mspt: f64,
    clients: HashMap<u32, Client>,
    spawn: [f64; 3],
    drops: Vec<Drop>,
    next_drop: u32,
    rng: Rng,
    /// Conteneurs dont le contenu doit etre renvoye a ceux qui les regardent.
    cont_dirty: HashSet<[i32; 3]>,
    mobs: Vec<mobs::Mob>,
    projs: Vec<mobs::Proj>,
    /// Compteur des cycles de mise a jour des blocs.
    cycle: u64,
    /// Heure du jour, commune a toutes les dimensions.
    time: f64,
    /// Dimension active : `world`, `drops`, `mobs`, `projs` et `cont_dirty` sont les siens.
    dim: u8,
    /// Les autres dimensions, en attente de leur tour.
    parked: Vec<Option<Realm>>,
}

/// Tout ce qui appartient a une dimension.
struct Realm {
    world: World,
    drops: Vec<Drop>,
    mobs: Vec<mobs::Mob>,
    projs: Vec<mobs::Proj>,
    cont_dirty: HashSet<[i32; 3]>,
}

impl Realm {
    fn new(world: World) -> Realm {
        Realm { world, drops: Vec::new(), mobs: Vec::new(), projs: Vec::new(), cont_dirty: HashSet::new() }
    }
}

impl Server {
    fn new(seed: u32, dir: Option<PathBuf>) -> Server {
        // Regles : celles du fichier ; a defaut, un monde deja cree garde sa liberte d'avant, un monde neuf est en survie normale.
        let rules = match dir.as_ref().map(|d| (d.join("regles.txt"), d.join("monde.bin").exists())) {
            Some((file, old)) => match std::fs::read_to_string(&file) {
                Ok(text) => shared::rules::Rules::parse(&text),
                Err(_) if old => shared::rules::Rules::legacy(),
                Err(_) => shared::rules::Rules::default(),
            },
            None => shared::rules::Rules::default(),
        };
        let save_dir = dir.clone();
        let world = World::open(seed, dir.clone(), 0);
        if world.lifted {
            lift_players(&world);
        }
        let parked = vec![
            None,
            Some(Realm::new(World::open(world.seed, dir.clone(), 1))),
            Some(Realm::new(World::open(world.seed, dir, 2))),
        ];
        let mut srv = Server {
            rng: Rng::new(world.seed as u64 ^ 0xABCD),
            time: world.time,
            dim: 0,
            parked,
            rules,
            save_dir,
            world,
            tick: 0,
            tps: TPS as f64,
            mspt: 0.0,
            clients: HashMap::new(),
            spawn: [0.0; 3],
            drops: Vec::new(),
            next_drop: 1,
            cont_dirty: HashSet::new(),
            mobs: Vec::new(),
            projs: Vec::new(),
            cycle: 0,
        };
        srv.spawn = srv.find_spawn();
        for dim in 0..3 {
            srv.enter(dim);
            for m in std::mem::take(&mut srv.world.saved_mobs) {
                srv.restore_mob(m);
            }
        }
        srv.enter(0);
        // Aide au developpement : EPICUBE_DEMO=zoo place une creature de chaque genre devant le joueur.
        if std::env::var("EPICUBE_DEMO").is_ok_and(|v| v == "zoo") {
            let o = Vec3::from(srv.spawn);
            for kind in 0..9u8 {
                let pos = o + Vec3::new((kind as f64 - 4.0) * 1.6, 6.0, 7.0);
                srv.spawn_mob(kind, pos, 0.0, shared::mobs::max_hp(kind));
            }
        }
        // Aide au developpement : EPICUBE_DEMO=grotte fait apparaitre le joueur dans une grotte moussue.
        if std::env::var("EPICUBE_DEMO").is_ok_and(|v| v == "grotte") {
            let seed = srv.world.seed;
            'search: for i in 0..4000 {
                let (x, z) = ((i % 63) * 9 - 280, (i / 63) * 9 - 280);
                if gen::cavern(seed, x, z) != gen::Cavern::Mossy {
                    continue;
                }
                srv.world.ensure((x >> 4, z >> 4));
                for y in 16..40 {
                    let at = |dy: i32| srv.world.get([x, y + dy, z]).map(|b| b.0);
                    if at(-1) == Some(blocks::MOSS) && at(0) == Some(AIR) && at(1) == Some(AIR) && at(2) == Some(AIR) && at(3) == Some(AIR) {
                        srv.spawn = [x as f64 + 0.5, y as f64 + 0.05, z as f64 + 0.5];
                        // Quelques lanternes pour y voir.
                        srv.world.set([x, y, z + 1], blocks::TORCH, 0);
                        srv.world.set([x + 1, y, z], blocks::TORCH, 0);
                        for (dx, dz) in [(2, 3), (-3, 5), (4, 8), (-2, 10), (1, 14), (6, 4), (-6, 7)] {
                            for dy in -4..5 {
                                let q = [x + dx, y + dy, z + dz];
                                srv.world.ensure((q[0] >> 4, q[2] >> 4));
                                let solid = srv.world.get([q[0], q[1] - 1, q[2]]).is_some_and(|b| blocks::opaque(b.0));
                                if solid && srv.world.get(q).is_some_and(|b| b.0 == AIR) {
                                    srv.world.set(q, blocks::TORCH, 0);
                                    break;
                                }
                            }
                        }
                        break 'search;
                    }
                }
            }
        }
        // Aide au developpement : EPICUBE_DEMO=village fait apparaitre le joueur a l'entree du village le plus proche.
        if std::env::var("EPICUBE_DEMO").is_ok_and(|v| v == "village") {
            let seed = srv.world.seed;
            let found = (0..40)
                .flat_map(|r: i32| (-r..=r).flat_map(move |a| [(a, -r), (a, r), (-r, a), (r, a)]))
                .find_map(|(rx, rz)| shared::structures::site(seed, rx, rz).filter(|s| s.kind == shared::structures::Kind::Village));
            if let Some(v) = found {
                let (x, z) = (v.x + 7, v.z - 26);
                let (h, _) = gen::column(seed, x, z);
                srv.spawn = [x as f64 + 0.5, h as f64 + 3.0, z as f64 + 0.5];
            }
        }
        // Aide au developpement : EPICUBE_DEMO=lointain fait apparaitre le joueur tout pres de la bordure du monde.
        if std::env::var("EPICUBE_DEMO").is_ok_and(|v| v == "lointain") {
            let seed = srv.world.seed;
            let start = shared::WORLD_BORDER as i32 - 10_000;
            let open = |x: i32| {
                let h = gen::column(seed, x, 8).0;
                let c = gen::generate(0, seed, x >> 4, 0);
                (h > shared::chunk::SEA + 2 && h < 150 && c.top((x & 15) as usize, 8) == Some(h as usize)).then_some((x, h))
            };
            if let Some((x, h)) = (0..2000).find_map(|i| open(start + i * 3)) {
                srv.spawn = [x as f64 + 0.5, h as f64 + 1.05, 8.5];
            }
        }
        if std::env::var("EPICUBE_DEMO").is_ok_and(|v| v == "ferme") {
            srv.demo_farm();
        }
        if std::env::var("EPICUBE_DEMO").is_ok_and(|v| v == "circuits") {
            srv.demo_circuits();
        }
        srv
    }
}

impl Client {
    fn new(tx: Sender<ServerMsg>, spawn: [f64; 3]) -> Client {
        let now = Instant::now();
        Client {
                        tx,
                        name: String::new(),
                        pos: spawn,
                        prev: spawn,
                        yaw: 0.0,
                        pitch: 0.0,
                        view: 8,
                        ready: false,
                        moved: false,
                        sent: HashSet::new(),
                        inv: [None; 36],
                        armor: [None; 4],
                        gear_sent: [0; 4],
                        cursor: None,
                        craft: [None; 9],
                        window: Win::None,
                        sel: 0,
                        hp: 20.0,
                        hunger: 20.0,
                        exhaustion: 0.0,
                        on_ground: true,
                        fly: false,
                        mode: 0,
                        fall_peak: spawn[1],
                        dead: false,
                        regen_t: 0.0,
                        hurt_t: 0.0,
                        last_dig: now,
                        voice_n: 0,
                        voice_t: now,
                        next_eat: now,
                        inv_dirty: false,
                        stats_dirty: false,
                        dim: 0,
                        air: 10.0,
                        inv_t: 0.0,
                        home: None,
                        last_hit: now,
                        riding: None,
                        saturation: 5.0,
                        queasy_t: 0.0,
                        poison_t: 0.0,
                        tea_t: 0.0,
                        effect_acc: 0.0,
                        fishing: None,
                        blocking: false,
                        veil_t: 0.0,
                        gliding: false,
                        glide_acc: 0.0,
                    }
    }
}

/// Serveur lance dans un fil ; `stop` sauvegarde puis attend la fin.
pub struct ServerHandle {
    tx: Sender<Event>,
    join: Option<JoinHandle<()>>,
    pub port: u16,
}

impl ServerHandle {
    pub fn stop(&mut self) {
        let _ = self.tx.send(Event::Shutdown);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
    /// Bloque jusqu'a l'arret du serveur.
    pub fn wait(&mut self) {
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
    /// Fonction a appeler depuis un autre fil (Ctrl+C) pour demander l'arret.
    pub fn stopper(&self) -> impl Fn() + Send + 'static {
        let tx = self.tx.clone();
        move || {
            let _ = tx.send(Event::Shutdown);
        }
    }
}

/// Demarre un serveur local sur un port libre (mode solo).
/// Avec `lan`, la partie ecoute sur le port par defaut et accepte les autres machines du reseau.
pub fn spawn_local(seed: u32, dir: Option<PathBuf>, lan: bool) -> std::io::Result<ServerHandle> {
    let listener = if lan {
        TcpListener::bind(("0.0.0.0", shared::DEFAULT_PORT))?
    } else {
        TcpListener::bind("127.0.0.1:0")?
    };
    start(listener, seed, dir)
}

/// Demarre le serveur sur `listener`. `dir` = dossier de sauvegarde.
pub fn start(listener: TcpListener, seed: u32, dir: Option<PathBuf>) -> std::io::Result<ServerHandle> {
    let port = listener.local_addr()?.port();
    let (ev_tx, ev_rx) = mpsc::channel::<Event>();
    let accept_tx = ev_tx.clone();
    thread::spawn(move || accept_loop(listener, accept_tx));
    let join = thread::Builder::new().name("serveur".into()).spawn(move || {
        Server::new(seed, dir).run(ev_rx);
    })?;
    Ok(ServerHandle { tx: ev_tx, join: Some(join), port })
}

fn accept_loop(listener: TcpListener, ev_tx: Sender<Event>) {
    let mut next_id = 1u32;
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let _ = stream.set_nodelay(true);
        let id = next_id;
        next_id += 1;
        let Ok(writer) = stream.try_clone() else { continue };
        if ev_tx.send(Event::Join(id, writer)).is_err() {
            return;
        }
        let tx = ev_tx.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stream);
            loop {
                match read_msg::<_, ClientMsg>(&mut reader) {
                    Ok(msg) => {
                        if tx.send(Event::Msg(id, msg)).is_err() {
                            return;
                        }
                    }
                    Err(_) => {
                        let _ = tx.send(Event::Leave(id));
                        return;
                    }
                }
            }
        });
    }
}

/// Clic sur une case avec la pile tenue par le curseur. `take_only` : case de sortie.
fn slot_click(t: &mut Slot, cur: &mut Slot, right: bool, take_only: bool) {
    match (*cur, *t) {
        (None, None) => {}
        (None, Some(s)) => {
            if right && s.n > 1 && !take_only {
                let half = s.n.div_ceil(2);
                *cur = Some(Stack { n: half, ..s });
                *t = Some(Stack { n: s.n - half, ..s });
            } else {
                *cur = Some(s);
                *t = None;
            }
        }
        (Some(c), None) => {
            if take_only {
                return;
            }
            if right && c.n > 1 {
                *t = Some(Stack { n: 1, ..c });
                *cur = Some(Stack { n: c.n - 1, ..c });
            } else {
                *t = Some(c);
                *cur = None;
            }
        }
        (Some(c), Some(s)) => {
            let max = items::max_stack(s.id);
            if c.id == s.id && max > 1 {
                if take_only {
                    if c.n + s.n <= max {
                        *cur = Some(Stack { n: c.n + s.n, ..c });
                        *t = None;
                    }
                } else {
                    let mv = if right { 1 } else { c.n }.min(max - s.n);
                    *t = Some(Stack { n: s.n + mv, ..s });
                    *cur = if c.n > mv { Some(Stack { n: c.n - mv, ..c }) } else { None };
                }
            } else if !take_only {
                std::mem::swap(t, cur);
            }
        }
    }
}

/// Case designee par `slot` pour ce joueur, si elle est accessible dans sa fenetre.
fn slot_mut<'a>(c: &'a mut Client, world: &'a mut World, slot: SlotRef) -> Option<&'a mut Slot> {
    let win = c.window;
    match slot {
        SlotRef::Inv(i) => c.inv.get_mut(i as usize),
        SlotRef::Armor(i) => c.armor.get_mut(i as usize),
        SlotRef::Craft(i) => {
            let ok = win == Win::Table || (win == Win::Inventory && [0, 1, 3, 4].contains(&i));
            if ok {
                c.craft.get_mut(i as usize)
            } else {
                None
            }
        }
        SlotRef::Cont(i) => match win {
            Win::Furnace(p) => world.furnaces.get_mut(&p).and_then(|f| match i {
                0 => Some(&mut f.input),
                1 => Some(&mut f.fuel),
                2 => Some(&mut f.output),
                _ => None,
            }),
            Win::Chest(p) => world.chests.get_mut(&p).and_then(|v| v.get_mut(i as usize)),
            _ => None,
        },
        SlotRef::CraftOut => None,
    }
}

impl Server {
    fn run(&mut self, ev_rx: Receiver<Event>) {
        let tick_len = Duration::from_secs_f64(1.0 / TPS as f64);
        let mut next = Instant::now() + tick_len;
        let mut last_tick = Instant::now() - tick_len;
        loop {
            loop {
                let now = Instant::now();
                if now >= next {
                    break;
                }
                match ev_rx.recv_timeout(next - now) {
                    Ok(Event::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                        self.save_all();
                        return;
                    }
                    Ok(ev) => {
                        self.handle(ev);
                        self.flush();
                    }
                    Err(RecvTimeoutError::Timeout) => break,
                }
            }
            let began = Instant::now();
            self.tick();
            self.flush();
            // Moyennes glissantes : le temps de calcul d'un tick, et le rythme reellement tenu.
            self.mspt += (began.elapsed().as_secs_f64() * 1000.0 - self.mspt) * 0.1;
            let gap = began.duration_since(last_tick).as_secs_f64();
            last_tick = began;
            if gap > 0.0 {
                self.tps += ((1.0 / gap).min(TPS as f64 * 2.0) - self.tps) * 0.05;
            }
            next += tick_len;
            let now = Instant::now();
            if next < now {
                next = now + tick_len;
            }
        }
    }

    fn handle(&mut self, ev: Event) {
        match ev {
            Event::Join(id, stream) => {
                let (tx, rx) = mpsc::channel::<ServerMsg>();
                thread::spawn(move || {
                    let mut w = BufWriter::new(stream);
                    while let Ok(msg) = rx.recv() {
                        if write_msg(&mut w, &msg).is_err() {
                            break;
                        }
                    }
                });
                self.clients.insert(
                    id,
                    Client::new(tx, self.spawn),
                );
            }
            Event::Msg(id, msg) => self.on_msg(id, msg),
            Event::Leave(id) => {
                if let Some(dim) = self.clients.get(&id).map(|c| c.dim) {
                    self.enter(dim);
                }
                self.close_window(id);
                self.save_player(id);
                if let Some(c) = self.clients.remove(&id) {
                    if c.ready {
                        println!("{} a quitte la partie", c.name);
                        self.broadcast_all(ServerMsg::PlayerLeave { id }, None);
                        self.broadcast_all(ServerMsg::Roster { name: c.name.clone(), here: false }, None);
                        self.announce(&format!("{} a quitté la partie", c.name), shared::proto::chat::INFO);
                    }
                }
            }
            Event::Shutdown => {}
        }
    }

    fn on_msg(&mut self, id: u32, msg: ClientMsg) {
        if !matches!(msg, ClientMsg::Hello { .. }) && !self.clients.get(&id).is_some_and(|c| c.ready) {
            return;
        }
        // Un spectateur regarde : rien de ce qui touche au monde ne passe. En aventure, on ne creuse pas.
        let mode = self.clients.get(&id).map_or(0, |c| c.mode);
        let acts = matches!(
            msg,
            ClientMsg::Dig { .. }
                | ClientMsg::Use { .. }
                | ClientMsg::Attack { .. }
                | ClientMsg::Interact { .. }
                | ClientMsg::Click { .. }
                | ClientMsg::Craft { .. }
                | ClientMsg::DropItem { .. }
                | ClientMsg::Shoot { .. }
                | ClientMsg::Block(_)
                | ClientMsg::Trade { .. }
                | ClientMsg::OpenInventory
                | ClientMsg::Pick { .. }
        );
        if (acts && !shared::rules::touches(mode)) || (matches!(msg, ClientMsg::Dig { .. }) && !shared::rules::builds(mode)) {
            return;
        }
        if let Some(dim) = self.clients.get(&id).map(|c| c.dim) {
            self.enter(dim);
        }
        match msg {
            ClientMsg::Hello { version, name, view } => self.hello(id, version, name, view),
            ClientMsg::Pos { pos, yaw, pitch, on_ground } => self.on_pos(id, pos, yaw, pitch, on_ground),
            ClientMsg::Chat { text } => self.chat(id, text),
            ClientMsg::Voice { data } => self.voice(id, data),
            ClientMsg::SetMode(m) => {
                // Changer de mode en cours de partie est une triche : le monde doit l'autoriser.
                if self.rules.cheats && !self.rules.hardcore && m <= shared::rules::SPECTATOR {
                    self.set_mode(id, m);
                }
            }
            ClientMsg::SetDifficulty(d) => {
                if !self.rules.hardcore && d <= shared::rules::HARD {
                    self.rules.difficulty = d;
                    self.rules_changed();
                }
            }
            ClientMsg::Pick { id: item } => {
                // Inventaire creatif : tout objet, a volonte.
                if let Some(c) = self.clients.get_mut(&id).filter(|c| c.mode == shared::rules::CREATIVE && c.window == Win::Inventory) {
                    c.cursor = (item != 0 && c.cursor.is_none() && (items::place_block(item).is_some() || item >= 256)).then(|| Stack::new(item, items::max_stack(item)));
                    c.inv_dirty = true;
                }
            }
            ClientMsg::SetFly(f) => {
                if let Some(c) = self.clients.get_mut(&id) {
                    c.fly = f && shared::rules::flies(c.mode);
                    c.fall_peak = c.pos[1];
                }
            }
            ClientMsg::SetView(v) => {
                if let Some(c) = self.clients.get_mut(&id) {
                    c.view = (v as i32).clamp(2, MAX_VIEW);
                }
            }
            ClientMsg::Dig { pos } => self.dig(id, pos),
            ClientMsg::Use { target } => self.use_item(id, target),
            ClientMsg::Select(s) => {
                if let Some(c) = self.clients.get_mut(&id) {
                    c.sel = (s as usize).min(8);
                }
            }
            ClientMsg::OpenInventory => self.open_window(id, Win::Inventory),
            ClientMsg::CloseWindow => self.close_window(id),
            ClientMsg::Click { slot, right, shift } => self.click(id, slot, right, shift),
            ClientMsg::Craft { recipe } => self.craft_recipe(id, recipe as usize),
            ClientMsg::DropItem { all } => self.drop_selected(id, all),
            ClientMsg::Attack { id: mob } => self.attack(id, mob),
            ClientMsg::Interact { id: mob } => self.interact(id, mob),
            ClientMsg::Respawn => self.respawn(id),
            ClientMsg::Shoot { charge } => self.shoot(id, charge),
            ClientMsg::Trade { index } => self.trade(id, index),
            ClientMsg::Rebase => self.resync(id),
            ClientMsg::Ping { t } => {
                if let Some(c) = self.clients.get(&id) {
                    c.send(ServerMsg::Pong { t, tps: self.tps, mspt: self.mspt });
                }
            }
            ClientMsg::Glide(on) => {
                if let Some(c) = self.clients.get_mut(&id) {
                    c.gliding = on && c.inv.iter().chain(c.armor.iter()).flatten().any(|st| st.id == items::HAGOROMO);
                }
            }
            ClientMsg::Block(up) => {
                if let Some(c) = self.clients.get_mut(&id) {
                    c.blocking = up && c.held() == Some(items::SHIELD);
                }
            }
        }
    }

    fn hello(&mut self, id: u32, version: u32, name: String, view: u8) {
        if version != PROTOCOL_VERSION {
            if let Some(c) = self.clients.remove(&id) {
                c.send(ServerMsg::Refused {
                    reason: format!("version du protocole {version}, attendu {PROTOCOL_VERSION}"),
                });
            }
            return;
        }
        let mut name: String = name.chars().filter(|c| !c.is_control()).take(16).collect();
        if name.is_empty() || self.clients.values().any(|c| c.ready && c.name == name) {
            name = format!("{name}{id}");
        }
        let save: Option<PlayerSave> = self
            .world
            .player_path(&name)
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|d| read_player(&d));
        // L'octet de dimension du fichier porte aussi le mode de jeu (quartet haut : mode + 1, 0 = jamais fixe).
        let saved_mode = save.as_ref().map(|s| s.dim >> 4).filter(|m| *m > 0).map(|m| (m - 1).min(shared::rules::SPECTATOR));
        let dim = save.as_ref().map(|s| s.dim & 15).filter(|d| *d < 3).unwrap_or(0);
        self.enter(dim);
        // Les joueurs deja la se presentent ; leur position suivra au prochain tick.
        for c in self.clients.values_mut() {
            c.moved = true;
        }
        let others: Vec<ServerMsg> = self
            .clients
            .iter()
            .filter(|(oid, c)| **oid != id && c.ready)
            .flat_map(|(oid, c)| [ServerMsg::PlayerJoin { id: *oid, name: c.name.clone() }, ServerMsg::Gear { id: *oid, armor: c.armor.map(|s| s.map_or(0, |st| st.id)) }])
            .collect();
        let drops: Vec<ServerMsg> = self
            .drops
            .iter()
            .map(|d| ServerMsg::DropSpawn { id: d.id, pos: d.pos.to_array(), vel: d.vel.to_array(), item: d.stack.id })
            .collect();
        let drops: Vec<ServerMsg> = drops.into_iter().chain(self.mob_msgs()).collect();
        let (seed, time, rules) = (self.world.seed, self.time, self.rules);
        let Some(c) = self.clients.get_mut(&id) else { return };
        c.dim = dim;
        if let Some(s) = save {
            c.home = s.home;
            if s.pos.iter().all(|v| v.is_finite()) {
                c.pos = s.pos;
            }
            c.hp = s.hp.clamp(1.0, 20.0);
            c.hunger = s.hunger.clamp(0.0, 20.0);
            // Les quatre pieces portees suivent les 36 cases du sac dans le fichier.
            for (dst, src) in c.inv.iter_mut().chain(c.armor.iter_mut()).zip(s.inv) {
                *dst = src;
            }
        }
        c.prev = c.pos;
        c.fall_peak = c.pos[1];
        c.name = name.clone();
        c.view = (view as i32).clamp(2, MAX_VIEW);
        c.ready = true;
        c.moved = true;
        // Aide au developpement : EPICUBE_DEMO=armes remplit l'inventaire d'armes et d'outils.
        // EPICUBE_DEMO=armure : une armure de fer sur le dos, les autres tenues dans le sac.
        if std::env::var("EPICUBE_DEMO").is_ok_and(|v| v == "armure") {
            for slot in 0..4u8 {
                c.armor[slot as usize] = Some(Stack::new(items::armor_id(slot, 2), 1));
            }
            for (k, tier) in [0u8, 1, 3, 4, 5].into_iter().enumerate() {
                for slot in 0..4u8 {
                    c.inv[9 + k * 4 + slot as usize] = Some(Stack::new(items::armor_id(slot, tier), 1));
                }
            }
            c.inv[0] = Some(Stack::new(items::tool_id(items::SWORD, 2), 1));
            c.inv[1] = Some(Stack::new(items::HAGOROMO, 1));
        }
        if std::env::var("EPICUBE_DEMO").is_ok_and(|v| v == "armes" || v == "main") {
            let kit = [
                items::tool_id(items::SWORD, 2), items::YUMI, items::YARI, items::NAGINATA, items::TANTO, items::TETSUBO, items::SHIELD,
                items::tool_id(items::PICK, 4), items::tool_id(items::SWORD, 4), items::tool_id(items::SWORD, 0), items::tool_id(items::AXE, 4),
                items::tool_id(items::SHOVEL, 4), items::tool_id(items::HOE, 4), items::TAMAHAGANE, items::SHEARS, items::FISHING_ROD,
            ];
            for (slot, id) in kit.into_iter().enumerate() {
                c.inv[slot] = Some(Stack::new(id, 1));
            }
            c.inv[16] = Some(Stack::new(items::KUNAI, 16));
            c.inv[17] = Some(Stack::new(items::ARROW, 32));
        }
        c.inv_dirty = true;
        c.stats_dirty = true;
        c.mode = saved_mode.unwrap_or(rules.mode);
        // Aide au developpement : EPICUBE_DEMO=creatif ouvre la partie en mode creatif.
        if std::env::var("EPICUBE_DEMO").is_ok_and(|v| v == "creatif") {
            c.mode = shared::rules::CREATIVE;
        }
        c.send(ServerMsg::Welcome { id, seed, spawn: c.pos, time, dim });
        c.send(ServerMsg::Rules { mode: c.mode, rules });
        for m in others.into_iter().chain(drops) {
            c.send(m);
        }
        println!("{name} a rejoint la partie");
        self.info(id, &format!("Bienvenue, {name} !"));
        // Le chat propose les noms des joueurs : le nouveau recoit ceux des presents, et tous recoivent le sien.
        for other in self.clients.values().filter(|c| c.ready && c.name != name) {
            if let Some(c) = self.clients.get(&id) {
                c.send(ServerMsg::Roster { name: other.name.clone(), here: true });
            }
        }
        self.broadcast_all(ServerMsg::Roster { name: name.clone(), here: true }, None);
        self.announce(&format!("{name} a rejoint la partie"), shared::proto::chat::INFO);
        self.broadcast_all(ServerMsg::PlayerJoin { id, name }, Some(id));
        // Aide au developpement : EPICUBE_DEMO=abime ou zenith envoie le joueur dans la dimension.
        match std::env::var("EPICUBE_DEMO").as_deref() {
            Ok("abime") => self.travel(id, 1),
            Ok("zenith") => self.travel(id, 2),
            _ => {}
        }
    }

    fn on_pos(&mut self, id: u32, pos: [f64; 3], yaw: f64, pitch: f64, on_ground: bool) {
        if !pos.iter().all(|v| v.is_finite()) {
            return;
        }
        let feet = [pos[0].floor() as i32, pos[1].floor() as i32, pos[2].floor() as i32];
        let in_water = self.world.get(feet).is_some_and(|(b, _)| b == WATER);
        // Retomber sur une terre labouree la pietine : chance = hauteur de chute - 0,5.
        let fall = self.clients.get(&id).filter(|c| on_ground && !c.on_ground && !c.fly && !c.dead).map(|c| c.fall_peak - pos[1]);
        if let Some(fall) = fall {
            let under = [feet[0], (pos[1] - 0.2).floor() as i32, feet[2]];
            if self.world.get(under).is_some_and(|(b, _)| b == blocks::FARMLAND) && self.rng.d() < fall - 0.5 {
                self.world.set(under, blocks::DIRT, 0);
            }
        }
        let Some(c) = self.clients.get_mut(&id) else { return };
        if c.dead {
            return;
        }
        // Bordure du monde : on ne la franchit pas.
        if pos[0].abs() > shared::WORLD_BORDER || pos[2].abs() > shared::WORLD_BORDER {
            let b = shared::WORLD_BORDER - 0.5;
            c.pos = [pos[0].clamp(-b, b), c.pos[1], pos[2].clamp(-b, b)];
            c.send(ServerMsg::Teleport { pos: c.pos });
            return;
        }
        // Un deplacement impossible en un seul message est refuse : le client est remis a sa place.
        if Vec3::from(c.pos).distance(Vec3::from(pos)) > 10.0 {
            c.send(ServerMsg::Teleport { pos: c.pos });
            return;
        }
        c.pos = pos;
        c.yaw = yaw;
        c.pitch = pitch;
        c.moved = true;
        // Degats de chute : hauteur perdue depuis le point le plus haut atteint en l'air.
        if on_ground {
            // Au sol, la robe de plumes se replie ; en vol, elle efface la chute.
            if c.gliding {
                c.fall_peak = pos[1];
            }
            c.gliding = false;
        }
        if c.fly || in_water || c.riding.is_some() || c.gliding {
            c.fall_peak = pos[1];
        } else if on_ground {
            let fall = c.fall_peak - pos[1];
            if !c.on_ground && fall > 3.5 {
                c.hp -= (fall - 3.0).floor();
                c.stats_dirty = true;
            }
            c.fall_peak = pos[1];
        } else {
            c.fall_peak = c.fall_peak.max(pos[1]);
        }
        c.on_ground = on_ground;
    }

    /// Rend la dimension `dim` active en echangeant son contenu avec celui de la dimension courante.
    fn enter(&mut self, dim: u8) {
        if dim == self.dim {
            return;
        }
        let Some(mut r) = self.parked.get_mut(dim as usize).and_then(|p| p.take()) else { return };
        std::mem::swap(&mut self.world, &mut r.world);
        std::mem::swap(&mut self.drops, &mut r.drops);
        std::mem::swap(&mut self.mobs, &mut r.mobs);
        std::mem::swap(&mut self.projs, &mut r.projs);
        std::mem::swap(&mut self.cont_dirty, &mut r.cont_dirty);
        self.parked[self.dim as usize] = Some(r);
        self.dim = dim;
    }

    /// Fait entendre un son a tous ceux qui sont dans ce monde (voir `shared::proto::sound`).
    pub(crate) fn sound(&self, pos: [i32; 3], kind: u8, pitch: u8) {
        self.broadcast(ServerMsg::Sound { pos, kind, pitch }, None);
    }

    fn broadcast_all(&self, msg: ServerMsg, except: Option<u32>) {
        for (id, c) in &self.clients {
            if c.ready && Some(*id) != except {
                c.send(msg.clone());
            }
        }
    }

    /// Envoie aux joueurs de la dimension active.
    fn broadcast(&self, msg: ServerMsg, except: Option<u32>) {
        for (id, c) in &self.clients {
            if c.ready && c.dim == self.dim && Some(*id) != except {
                c.send(msg.clone());
            }
        }
    }

    fn info(&self, id: u32, text: &str) {
        if let Some(c) = self.clients.get(&id) {
            c.send(ServerMsg::Info { text: text.to_string() });
        }
    }

    // ---------------------------------------------------------------- objets au sol

    fn spawn_drop(&mut self, pos: Vec3, vel: Vec3, stack: Stack, delay: f64) {
        let id = self.next_drop;
        self.next_drop += 1;
        self.broadcast(ServerMsg::DropSpawn { id, pos: pos.to_array(), vel: vel.to_array(), item: stack.id }, None);
        self.drops.push(Drop { id, pos, vel, stack, age: 0.0, delay });
    }

    /// Fait tomber une pile a l'emplacement d'un bloc.
    fn drop_at_block(&mut self, p: [i32; 3], stack: Stack) {
        let pos = Vec3::new(p[0] as f64 + 0.5, p[1] as f64 + 0.4, p[2] as f64 + 0.5);
        let vel = Vec3::new(self.rng.d() - 0.5, 2.0 + self.rng.d(), self.rng.d() - 0.5) * 1.5;
        self.spawn_drop(pos, vel, stack, 0.4);
    }

    /// Donne une pile au joueur ; ce qui ne rentre pas tombe a ses pieds.
    fn give(&mut self, id: u32, stack: Stack) {
        let Some(c) = self.clients.get_mut(&id) else { return };
        c.inv_dirty = true;
        if let Some(left) = add_to(&mut c.inv, stack) {
            let pos = c.eye() - Vec3::Y * 0.4;
            self.spawn_drop(pos, Vec3::ZERO, left, 1.0);
        }
    }

    fn drop_selected(&mut self, id: u32, all: bool) {
        let Some(c) = self.clients.get_mut(&id) else { return };
        if c.dead {
            return;
        }
        let Some(st) = c.inv[c.sel] else { return };
        let n = if all { st.n } else { 1 };
        c.inv[c.sel] = if st.n > n { Some(Stack { n: st.n - n, ..st }) } else { None };
        c.inv_dirty = true;
        let dir = Vec3::new(c.yaw.sin() * c.pitch.cos(), c.pitch.sin(), c.yaw.cos() * c.pitch.cos());
        let pos = c.eye() - Vec3::Y * 0.3;
        self.spawn_drop(pos, dir * 6.0 + Vec3::Y * 1.5, Stack { n, ..st }, 1.5);
    }

    fn tick_drops(&mut self) {
        let dim = self.dim;
        let mut removed = Vec::new();
        let mut drops = std::mem::take(&mut self.drops);
        for d in drops.iter_mut() {
            d.age += DT;
            step_drop(&self.world, &mut d.pos, &mut d.vel, DT);
            if d.age > 300.0 || d.pos.y < -10.0 {
                removed.push(d.id);
                continue;
            }
            if d.age < d.delay {
                continue;
            }
            for c in self.clients.values_mut() {
                if !c.ready || c.dead || c.dim != dim || !shared::rules::touches(c.mode) {
                    continue;
                }
                let center = Vec3::from(c.pos) + Vec3::Y * 0.9;
                if center.distance(d.pos) < 1.5 {
                    match add_to(&mut c.inv, d.stack) {
                        None => {
                            c.inv_dirty = true;
                            removed.push(d.id);
                            break;
                        }
                        Some(left) if left.n < d.stack.n => {
                            c.inv_dirty = true;
                            d.stack = left;
                        }
                        Some(_) => {}
                    }
                }
            }
        }
        drops.retain(|d| !removed.contains(&d.id));
        drops.append(&mut self.drops);
        self.drops = drops;
        for id in removed {
            self.broadcast(ServerMsg::DropRemove { id }, None);
        }
    }

    // ---------------------------------------------------------------- blocs

    fn in_reach(&self, id: u32, p: [i32; 3]) -> bool {
        self.clients.get(&id).is_some_and(|c| {
            let center = Vec3::new(p[0] as f64 + 0.5, p[1] as f64 + 0.5, p[2] as f64 + 0.5);
            !c.dead && c.eye().distance(center) <= REACH
        })
    }

    /// Fait passer un joueur dans un autre mode de jeu et le lui annonce.
    pub(crate) fn set_mode(&mut self, id: u32, mode: u8) {
        let rules = self.rules;
        let Some(c) = self.clients.get_mut(&id) else { return };
        let was = c.mode;
        c.mode = mode;
        if !shared::rules::flies(mode) {
            c.fly = false;
        }
        c.fall_peak = c.pos[1];
        c.moved = true;
        c.stats_dirty = true;
        c.send(ServerMsg::Rules { mode, rules });
        // Un spectateur disparait aux yeux des autres ; il reparait en quittant ce mode.
        if mode == shared::rules::SPECTATOR && was != mode {
            self.close_window(id);
            self.broadcast_all(ServerMsg::PlayerHide { id }, Some(id));
        }
    }

    /// Les regles du monde ont change : on les note dans la sauvegarde et on previent tout le monde.
    fn rules_changed(&mut self) {
        self.rules = self.rules.checked();
        if self.rules.difficulty == shared::rules::PEACEFUL {
            // Paisible : les creatures hostiles deja la s'evanouissent.
            let gone: Vec<u32> = self.mobs.iter().filter(|m| shared::mobs::hostile(m.kind) && m.kind != shared::mobs::WYRM).map(|m| m.id).collect();
            self.mobs.retain(|m| !gone.contains(&m.id));
            for id in gone {
                self.broadcast(ServerMsg::MobRemove { id }, None);
            }
        }
        if let Some(dir) = &self.save_dir {
            let _ = std::fs::write(dir.join("regles.txt"), self.rules.to_text());
        }
        let rules = self.rules;
        for c in self.clients.values().filter(|c| c.ready) {
            c.send(ServerMsg::Rules { mode: c.mode, rules });
        }
    }

    fn dig(&mut self, id: u32, pos: [i32; 3]) {
        if !self.in_reach(id, pos) {
            return;
        }
        let Some((b, _)) = self.world.get(pos) else { return };
        let Some(c) = self.clients.get_mut(&id) else { return };
        if c.mode == shared::rules::CREATIVE {
            // Creatif : tout casse d'un coup, sans user l'outil ni rien laisser tomber.
            let now = Instant::now();
            if now.duration_since(c.last_dig).as_secs_f64() < 0.05 {
                return;
            }
            c.last_dig = now;
            return self.break_block(pos, false);
        }
        let held = c.held();
        let Some(time) = items::break_time(b, held) else { return };
        // Tolerance sur la duree : le client mesure le temps de son cote.
        let now = Instant::now();
        if now.duration_since(c.last_dig).as_secs_f64() < time.max(0.15) * 0.7 {
            return;
        }
        c.last_dig = now;
        c.exhaustion += 0.005;
        if time > 0.0 {
            // Un outil s'use d'un cran par bloc ; une arme detournee en outil, de deux.
            let wear = match c.held().and_then(items::tool) {
                Some((items::SWORD, _)) => 2,
                Some(_) => 1,
                None => items::weapon(c.held()).wear * 2,
            };
            if let Some(st) = c.inv[c.sel].as_mut().filter(|_| wear > 0) {
                st.dur = st.dur.saturating_sub(wear);
                if st.dur == 0 {
                    c.inv[c.sel] = None;
                }
                c.inv_dirty = true;
            }
        }
        self.break_block(pos, items::can_harvest(b, held));
    }

    /// Retire un bloc, fait tomber son butin et ce qui reposait dessus.
    fn break_block(&mut self, pos: [i32; 3], harvest: bool) {
        let Some((b, m)) = self.world.get(pos) else { return };
        if b == AIR {
            return;
        }
        let mut loot: Vec<Stack> = Vec::new();
        if let Some(f) = self.world.furnaces.remove(&pos) {
            loot.extend([f.input, f.fuel, f.output].into_iter().flatten());
        }
        if let Some(ch) = self.world.chests.remove(&pos) {
            loot.extend(ch.into_iter().flatten());
        }
        if let Some(grid) = self.world.irori.remove(&pos) {
            loot.extend(grid.into_iter().map(|x| x.0));
        }
        self.world.set(pos, AIR, 0);
        if shared::signal::is_rail(b) {
            self.fix_rails_around(pos);
        }
        // Un poussoir et sa tete partent ensemble.
        let d = shared::signal::facing6(m);
        match b {
            blocks::PISTON | blocks::STICKY_PISTON if m & 8 != 0 => {
                let head = shared::signal::add(pos, d);
                if self.world.get(head).is_some_and(|(hb, _)| hb == blocks::PISTON_HEAD) {
                    self.world.set(head, AIR, 0);
                }
            }
            blocks::PISTON_HEAD => {
                let body = shared::signal::add(pos, shared::signal::neg(d));
                if self.world.get(body).is_some_and(|(bb, bm)| matches!(bb, blocks::PISTON | blocks::STICKY_PISTON) && bm & 8 != 0) {
                    self.break_block(body, harvest);
                }
            }
            _ => {}
        }
        if harvest {
            loot.extend(items::drops(b, m, &mut self.rng));
        }
        for st in loot {
            self.drop_at_block(pos, st);
        }
        let up = [pos[0], pos[1] + 1, pos[2]];
        if self.world.get(up).is_some_and(|(ub, _)| items::needs_support(ub)) {
            self.break_block(up, true);
        }
    }

    /// Enfile la piece tenue en main ; celle qui etait portee revient dans la main. Vrai si c'est fait.
    pub(crate) fn don_held(&mut self, id: u32) -> bool {
        let Some(c) = self.clients.get_mut(&id) else { return false };
        let Some(slot) = c.inv[c.sel].and_then(|st| items::wear_slot(st.id)) else { return false };
        std::mem::swap(&mut c.inv[c.sel], &mut c.armor[slot as usize]);
        c.inv_dirty = true;
        true
    }

    fn use_item(&mut self, id: u32, target: Option<([i32; 3], [i32; 3])>) {
        if self.clients.get(&id).is_none_or(|c| c.dead || c.window != Win::None) {
            return;
        }
        // Une piece d'armure en main : le clic droit l'enfile, a moins de viser un bloc qui s'utilise.
        let busy = target.and_then(|(pos, _)| self.world.get(pos)).is_some_and(|(b, _)| {
            use blocks::*;
            matches!(b, CHEST | CRAFT | FURNACE | SMOKER | LEVER | BUTTON | DOOR | BED | DROPPER | HOPPER | NOTE | REPEATER | COMPARATOR | COMPOSTER | ABYSS_PORTAL | ZENITH_PORTAL | GATEWAY)
        });
        if !busy && self.don_held(id) {
            return;
        }
        if let Some((pos, place)) = target {
            if !self.in_reach(id, pos) {
                return;
            }
            let Some((b, bm)) = self.world.get(pos) else { return };
            if self.farm_use(id, pos, place, b, bm) {
                return;
            }
            match b {
                blocks::BUTTON => return self.press_button(pos, bm),
                // Relais : le retard passe de 1 a 4 battements, puis revient a 1.
                blocks::REPEATER => {
                    self.world.set(pos, b, (bm & !12) | ((((bm >> 2) + 1) & 3) << 2));
                    return;
                }
                // Balance : comparaison ou soustraction.
                blocks::COMPARATOR => {
                    self.world.set(pos, b, bm ^ 4);
                    return;
                }
                // Cadran : jour ou nuit.
                blocks::SUNDIAL => {
                    self.world.set(pos, b, bm ^ 16);
                    self.tick_sundials();
                    return;
                }
                // Tambour : chaque clic monte la note d'un demi-ton (25 notes), et la joue.
                blocks::NOTE => {
                    let nm = (bm & 32) | ((bm & 31) + 1) % 25;
                    self.world.set(pos, b, nm);
                    self.drum(pos, nm);
                    return;
                }
                _ if shared::signal::is_rail(b) && self.place_cart(id, pos) => return,
                blocks::DROPPER | blocks::HOPPER => {
                    self.world.chests.entry(pos).or_insert_with(|| vec![None; 27]);
                    return self.open_window(id, Win::Chest(pos));
                }
                CRAFT => return self.open_window(id, Win::Table),
                FURNACE => {
                    self.world.furnaces.entry(pos).or_default();
                    return self.open_window(id, Win::Furnace(pos));
                }
                CHEST => {
                    self.world.chests.entry(pos).or_insert_with(|| vec![None; 27]);
                    return self.open_window(id, Win::Chest(pos));
                }
                blocks::BED => return self.sleep(id, pos),
                blocks::ABYSS_PORTAL => return self.travel(id, if self.dim == 1 { 0 } else { 1 }),
                blocks::ZENITH_PORTAL => return self.travel(id, if self.dim == 2 { 0 } else { 2 }),
                blocks::GATEWAY => return self.use_gateway(id),
                blocks::LEVER | blocks::DOOR => {
                    self.sound(pos, if b == blocks::LEVER { shared::proto::sound::LEVER } else { shared::proto::sound::CHEST_OPEN }, 0);
                    let m = self.world.get(pos).map(|x| x.1).unwrap_or(0);
                    self.world.set(pos, b, m ^ 1);
                    return;
                }
                _ => {}
            }
            if self.use_tool(id, pos, place) || self.place(id, pos, place) {
                return;
            }
        }
        self.eat(id);
    }

    /// Franchit un portail vers la dimension `to`.
    fn travel(&mut self, id: u32, to: u8) {
        let from = self.dim;
        self.close_window(id);
        let Some(c) = self.clients.get(&id) else { return };
        let (p, home) = (c.pos, c.home.unwrap_or(self.spawn));
        self.enter(to);
        if self.dim != to {
            return;
        }
        let (px, pz) = (p[0].floor() as i32, p[2].floor() as i32);
        let (x, z) = match (from, to) {
            (_, 2) => (0, 12),
            (2, _) => (home[0].floor() as i32, home[2].floor() as i32),
            // Un pas dans Yomi en vaut huit a la surface.
            (0, 1) => (px.div_euclid(realms::YOMI_SCALE), pz.div_euclid(realms::YOMI_SCALE)),
            (1, 0) => (px * realms::YOMI_SCALE, pz * realms::YOMI_SCALE),
            _ => (px, pz),
        };
        for dz in -1..=1 {
            for dx in -1..=1 {
                self.world.ensure(((x >> 4) + dx, (z >> 4) + dz));
            }
        }
        // Entre la surface et Yomi, on ressort par le torii le plus proche, ou par un torii neuf.
        if to == 1 || from == 1 {
            let pos = self.arrive_torii(x, z);
            self.move_to_dim(id, to, pos);
            if let Some(c) = self.clients.get_mut(&id) {
                c.veil_t = -1.0;
            }
            return;
        }
        let y = self.arrival_y(x, z);
        self.move_to_dim(id, to, [x as f64 + 0.5, y as f64 + 0.05, z as f64 + 0.5]);
    }

    /// Hauteur ou poser un voyageur dans la dimension active.
    fn arrival_y(&mut self, x: i32, z: i32) -> i32 {
        let air = |w: &World, y: i32| w.get([x, y, z]).is_some_and(|(b, _)| b == AIR);
        if self.dim == 1 {
            let spot = (34..108).find(|&y| {
                air(&self.world, y) && air(&self.world, y + 1) && self.world.get([x, y - 1, z]).is_some_and(|(b, m)| blocks::solid(b, m))
            });
            if let Some(y) = spot {
                return y;
            }
            // Rien de libre : on creuse une petite salle.
            for dy in -1..=2 {
                for dz in -1..=1 {
                    for dx in -1..=2 {
                        let b = if dy < 0 { blocks::ABYSS_ROCK } else { AIR };
                        self.world.set([x + dx, 64 + dy, z + dz], b, 0);
                    }
                }
            }
            return 64;
        }
        (1..H as i32)
            .rev()
            .find(|&y| self.world.get([x, y, z]).is_some_and(|(b, m)| blocks::solid(b, m)))
            .map(|y| y + 1)
            .unwrap_or(80)
    }

    /// Renvoie a un joueur tout ce qui l'entoure, sans le deplacer : son client vient de changer
    /// d'origine d'affichage et a tout oublie.
    fn resync(&mut self, id: u32) {
        let Some(dim) = self.clients.get(&id).map(|c| c.dim) else { return };
        self.enter(dim);
        let mut msgs: Vec<ServerMsg> = self
            .drops
            .iter()
            .map(|d| ServerMsg::DropSpawn { id: d.id, pos: d.pos.to_array(), vel: d.vel.to_array(), item: d.stack.id })
            .collect();
        msgs.extend(self.mob_msgs());
        for c in self.clients.values_mut().filter(|c| c.dim == dim) {
            c.moved = true;
        }
        let Some(c) = self.clients.get_mut(&id) else { return };
        c.sent.clear();
        c.send(ServerMsg::Dimension { dim, pos: c.pos });
        for m in msgs {
            c.send(m);
        }
    }

    /// Place le joueur dans la dimension active (deja rendue courante) et lui envoie son contenu.
    fn move_to_dim(&mut self, id: u32, to: u8, pos: [f64; 3]) {
        self.broadcast_all(ServerMsg::PlayerHide { id }, Some(id));
        let mut msgs: Vec<ServerMsg> = self
            .drops
            .iter()
            .map(|d| ServerMsg::DropSpawn { id: d.id, pos: d.pos.to_array(), vel: d.vel.to_array(), item: d.stack.id })
            .collect();
        msgs.extend(self.mob_msgs());
        for c in self.clients.values_mut().filter(|c| c.dim == to) {
            c.moved = true;
        }
        let Some(c) = self.clients.get_mut(&id) else { return };
        c.dim = to;
        c.pos = pos;
        c.prev = pos;
        c.fall_peak = pos[1];
        c.on_ground = true;
        c.air = 10.0;
        c.sent.clear();
        c.moved = true;
        c.stats_dirty = true;
        c.send(ServerMsg::Dimension { dim: to, pos });
        c.send(ServerMsg::Boss { hp: None });
        for m in msgs {
            c.send(m);
        }
    }

    /// Dormir : fixe le point de reapparition et, la nuit, passe au matin.
    fn sleep(&mut self, id: u32, bed: [i32; 3]) {
        // Hors de la surface, nul repos : le futon explose.
        if self.dim != 0 {
            self.world.set(bed, AIR, 0);
            self.explode(Vec3::new(bed[0] as f64 + 0.5, bed[1] as f64 + 0.5, bed[2] as f64 + 0.5), 5.0);
            return;
        }
        let night = !self.is_day();
        let Some(c) = self.clients.get_mut(&id) else { return };
        c.home = Some([bed[0] as f64 + 0.5, bed[1] as f64 + 0.6, bed[2] as f64 + 0.5]);
        if night {
            self.time = 0.02;
            self.broadcast_all(ServerMsg::Time { time: 0.02 }, None);
            self.info(id, "Bonne nuit ! Point de reapparition defini");
        } else {
            self.info(id, "Point de reapparition defini (on ne dort que la nuit)");
        }
    }

    /// Houe et seaux. Renvoie vrai si l'objet tenu a servi.
    fn use_tool(&mut self, id: u32, pos: [i32; 3], place: [i32; 3]) -> bool {
        let Some(st) = self.clients.get(&id).and_then(|c| c.inv[c.sel]) else { return false };
        let Some((b, m)) = self.world.get(pos) else { return false };
        let up = [pos[0], pos[1] + 1, pos[2]];
        let mut wear = false;
        let new_item = match st.id {
            _ if matches!(items::tool(st.id), Some((items::HOE, _))) => {
                let free = self.world.get(up).is_some_and(|(a, _)| a == AIR);
                if !matches!(b, blocks::GRASS | blocks::DIRT) || !free {
                    return false;
                }
                self.world.set(pos, blocks::FARMLAND, 0);
                wear = true;
                st.id
            }
            items::BUCKET => {
                if !matches!(b, WATER | LAVA) || m != 0 {
                    return false;
                }
                self.world.set(pos, AIR, 0);
                if b == WATER {
                    items::WATER_BUCKET
                } else {
                    items::LAVA_BUCKET
                }
            }
            items::LIGHTER => {
                if b == blocks::EXPLOSIVE && m == 0 {
                    self.prime(pos, 2.5);
                } else if !(b == blocks::OBSIDIAN && self.light_torii(place)) {
                    // Le briquet allume aussi un torii d'obsidienne, par l'interieur du cadre.
                    return false;
                }
                wear = true;
                st.id
            }
            items::WATER_BUCKET | items::LAVA_BUCKET => {
                let cell = if items::replaceable(b) { pos } else { place };
                let dist: i32 = (0..3).map(|a| (cell[a] - pos[a]).abs()).sum();
                if dist > 1 || !self.world.get(cell).is_some_and(|(cb, _)| items::replaceable(cb)) {
                    return false;
                }
                if st.id == items::WATER_BUCKET && self.dim == 1 {
                    // A Yomi, l'eau s'evapore avant de toucher le sol.
                    self.info(id, "L'eau s'evapore dans un sifflement");
                } else {
                    self.world.set(cell, if st.id == items::WATER_BUCKET { WATER } else { LAVA }, 0);
                }
                items::BUCKET
            }
            _ => return false,
        };
        if let Some(c) = self.clients.get_mut(&id) {
            let mut held = Stack { id: new_item, ..st };
            if new_item != st.id {
                held.dur = items::max_dur(new_item);
            }
            if wear {
                held.dur = held.dur.saturating_sub(1);
            }
            c.inv[c.sel] = (!wear || held.dur > 0).then_some(held);
            c.inv_dirty = true;
        }
        true
    }

    /// Pose le bloc tenu. Renvoie vrai si un bloc a ete pose.
    fn place(&mut self, id: u32, pos: [i32; 3], place: [i32; 3]) -> bool {
        if !self.clients.get(&id).is_some_and(|c| shared::rules::builds(c.mode)) {
            return false;
        }
        let Some(st) = self.clients.get(&id).and_then(|c| c.inv[c.sel]) else { return false };
        let Some(nb) = items::place_block(st.id) else { return false };
        let Some((target, _)) = self.world.get(pos) else { return false };
        let cell = if items::replaceable(target) { pos } else { place };
        let dist: i32 = (0..3).map(|a| (cell[a] - pos[a]).abs()).sum();
        if dist > 1 {
            return false;
        }
        let Some((cur, _)) = self.world.get(cell) else { return false };
        // Orientation : la face visee pour ce qui s'accroche, le regard du joueur pour le reste.
        let normal = if cell == pos { [0, 1, 0] } else { [cell[0] - pos[0], cell[1] - pos[1], cell[2] - pos[2]] };
        let look = self.clients.get(&id).map(|c| [c.yaw.sin() * c.pitch.cos(), c.pitch.sin(), c.yaw.cos() * c.pitch.cos()]).unwrap_or([0.0, 0.0, 1.0]);
        let back = [-look[0], -look[1], -look[2]];
        let attach = shared::signal::attach_index(shared::signal::neg(normal));
        let meta = match nb {
            // Une lanterne ne tient pas au plafond.
            blocks::SIGTORCH if attach == 5 => return false,
            blocks::SIGTORCH => 1 | attach << 1,
            blocks::LEVER | blocks::BUTTON => attach << 1,
            // Un piquet de shimenawa ne se plante que dans un mur.
            blocks::HOOK if attach == 0 || attach == 5 => return false,
            blocks::HOOK => attach << 1,
            // La tremie se deverse vers le bloc vise, jamais vers le haut.
            blocks::HOPPER => {
                let a = shared::signal::neg(normal);
                if a[1] != 0 {
                    3
                } else {
                    shared::signal::index6([a[0] as f64, 0.0, a[2] as f64])
                }
            }
            blocks::REPEATER | blocks::COMPARATOR => shared::signal::index4(look),
            blocks::PISTON | blocks::STICKY_PISTON | blocks::DROPPER => shared::signal::index6(back),
            blocks::OBSERVER => shared::signal::index6(look),
            _ => 0,
        };
        let below = self.world.get(shared::signal::add(cell, items::support_dir(nb, meta))).map(|(b, _)| b).unwrap_or(AIR);
        if !items::replaceable(cur) || !items::support_ok(nb, below) {
            return false;
        }
        if blocks::solid(nb, 0) {
            // Pas de bloc plein dans un joueur.
            let blocked = self.clients.values().any(|c| {
                let p = c.pos;
                c.ready
                    && !c.dead
                    && c.dim == self.dim
                    && p[0] + 0.3 > cell[0] as f64
                    && p[0] - 0.3 < cell[0] as f64 + 1.0
                    && p[1] + 1.8 > cell[1] as f64
                    && p[1] < cell[1] as f64 + 1.0
                    && p[2] + 0.3 > cell[2] as f64
                    && p[2] - 0.3 < cell[2] as f64 + 1.0
            });
            if blocked {
                return false;
            }
        }
        if !self.world.set(cell, nb, meta) {
            return false;
        }
        if shared::signal::is_rail(nb) {
            self.fix_rails_around(cell);
        }
        // En creatif, la pile ne diminue pas.
        if let Some(c) = self.clients.get_mut(&id).filter(|c| c.mode != shared::rules::CREATIVE) {
            c.inv[c.sel] = if st.n > 1 { Some(Stack { n: st.n - 1, ..st }) } else { None };
            c.inv_dirty = true;
        }
        true
    }

    // ---------------------------------------------------------------- fenetres

    fn open_window(&mut self, id: u32, win: Win) {
        self.close_window(id);
        let Some(c) = self.clients.get_mut(&id) else { return };
        if c.dead {
            return;
        }
        c.window = win;
        c.inv_dirty = true;
        c.send(ServerMsg::Window { kind: win.kind() });
        if let Some(p) = win.pos() {
            self.cont_dirty.insert(p);
        }
        if let Win::Chest(p) = win {
            self.sound(p, shared::proto::sound::CHEST_OPEN, 0);
        }
    }

    /// Ferme la fenetre : la grille d'artisanat et le curseur retournent a l'inventaire.
    fn close_window(&mut self, id: u32) {
        let Some(c) = self.clients.get_mut(&id) else { return };
        if c.window == Win::None && c.cursor.is_none() {
            return;
        }
        let chest = if let Win::Chest(p) = c.window { Some(p) } else { None };
        c.window = Win::None;
        let back: Vec<Stack> = c.craft.iter_mut().chain(std::iter::once(&mut c.cursor)).filter_map(|s| s.take()).collect();
        c.send(ServerMsg::Window { kind: WindowKind::None });
        for st in back {
            self.give(id, st);
        }
        if let Some(p) = chest {
            self.sound(p, shared::proto::sound::CHEST_CLOSE, 0);
        }
    }

    fn click(&mut self, id: u32, slot: SlotRef, right: bool, shift: bool) {
        let Some(c) = self.clients.get_mut(&id) else { return };
        if c.dead || c.window == Win::None {
            return;
        }
        let win = c.window;
        c.inv_dirty = true;
        if let Some(p) = win.pos() {
            self.cont_dirty.insert(p);
        }

        if slot == SlotRef::CraftOut {
            if !matches!(win, Win::Inventory | Win::Table) {
                return;
            }
            // Clic simple : une fabrication. Maj : autant que possible, vers l'inventaire.
            loop {
                let Some(out) = craft::result(&c.craft) else { break };
                if shift {
                    let mut test = c.inv;
                    if add_to(&mut test, out).is_some() {
                        break;
                    }
                    c.inv = test;
                } else {
                    match c.cursor {
                        None => c.cursor = Some(out),
                        Some(cur) if cur.id == out.id && cur.n + out.n <= items::max_stack(out.id) => {
                            c.cursor = Some(Stack { n: cur.n + out.n, ..cur });
                        }
                        _ => break,
                    }
                }
                for s in c.craft.iter_mut() {
                    if let Some(st) = s {
                        st.n -= 1;
                        if st.n == 0 {
                            *s = None;
                        }
                    }
                }
                if !shift {
                    break;
                }
            }
            return;
        }

        if let SlotRef::Armor(i) = slot {
            // L'equipement ne se touche que depuis l'inventaire, et chaque case n'accepte que sa piece.
            let i = i as usize;
            if win != Win::Inventory || i >= 4 {
                return;
            }
            if shift {
                if let Some(st) = c.armor[i].take() {
                    c.armor[i] = add_to(&mut c.inv, st);
                }
            } else if c.cursor.is_none_or(|st| items::wear_slot(st.id) == Some(i as u8)) {
                std::mem::swap(&mut c.cursor, &mut c.armor[i]);
            }
            return;
        }
        if let (true, Win::Inventory, SlotRef::Inv(i)) = (shift, win, slot) {
            // Maj-clic sur une piece d'armure : elle s'enfile si sa place est libre.
            let i = i as usize;
            if let Some(w) = c.inv.get(i).copied().flatten().and_then(|st| items::wear_slot(st.id)) {
                if c.armor[w as usize].is_none() {
                    c.armor[w as usize] = c.inv[i].take();
                    return;
                }
            }
        }
        let take_only = matches!((slot, win), (SlotRef::Cont(2), Win::Furnace(_)));
        if shift {
            // Deplacement rapide vers l'autre zone.
            let Some(st) = slot_mut(c, &mut self.world, slot).and_then(|s| s.take()) else { return };
            let left = match (slot, win) {
                (SlotRef::Inv(_), Win::Chest(p)) => match self.world.chests.get_mut(&p) {
                    Some(ch) => add_to(ch, st),
                    None => Some(st),
                },
                (SlotRef::Inv(_), Win::Furnace(p)) => match self.world.furnaces.get_mut(&p) {
                    Some(f) if craft::smelt(st.id).is_some() => add_to(std::slice::from_mut(&mut f.input), st),
                    Some(f) if craft::fuel(st.id) > 0.0 => add_to(std::slice::from_mut(&mut f.fuel), st),
                    _ => Some(st),
                },
                (SlotRef::Inv(i), _) if i < 9 => add_to(&mut c.inv[9..], st),
                (SlotRef::Inv(_), _) => add_to(&mut c.inv[..9], st),
                _ => add_to(&mut c.inv, st),
            };
            if let Some(s) = slot_mut(c, &mut self.world, slot) {
                *s = left;
            }
        } else {
            let mut cur = c.cursor.take();
            if let Some(t) = slot_mut(c, &mut self.world, slot) {
                slot_click(t, &mut cur, right, take_only);
            }
            c.cursor = cur;
        }
    }

    /// Fabrique une recette directement depuis l'inventaire (liste des recettes).
    fn craft_recipe(&mut self, id: u32, index: usize) {
        let Some(c) = self.clients.get_mut(&id) else { return };
        let Some(r) = craft::recipes().get(index) else { return };
        let allowed = match c.window {
            Win::Table => true,
            Win::Inventory => r.w <= 2 && r.h <= 2,
            _ => false,
        };
        if c.dead || !allowed || !craft::can_craft(r, &c.inv) {
            return;
        }
        for (item, n) in craft::ingredients(r) {
            items::remove(&mut c.inv, item, n);
        }
        self.give(id, Stack::new(r.out, r.n));
    }

    fn tick_furnaces(&mut self) {
        let mut lit_changes = Vec::new();
        // Le gril shichirin ne cuit que la cuisine, mais deux fois plus vite (et brule d'autant).
        let grills: HashSet<[i32; 3]> =
            self.world.furnaces.keys().filter(|p| self.world.get(**p).is_some_and(|(b, _)| b == blocks::SMOKER)).copied().collect();
        for (pos, f) in self.world.furnaces.iter_mut() {
            let grill = grills.contains(pos);
            let dt = if grill { DT * 2.0 } else { DT };
            let was_lit = f.burn > 0.0;
            let result = f.input.filter(|s| !grill || craft::grills(s.id)).and_then(|s| craft::smelt(s.id));
            let can_cook = result.is_some_and(|r| match f.output {
                None => true,
                Some(o) => o.id == r && o.n < items::max_stack(r),
            });
            if f.burn > 0.0 {
                f.burn -= dt;
            }
            if f.burn <= 0.0 && can_cook {
                if let Some(fu) = f.fuel.filter(|s| craft::fuel(s.id) > 0.0) {
                    f.burn = craft::fuel(fu.id);
                    f.burn_max = f.burn;
                    f.fuel = if fu.id == items::LAVA_BUCKET {
                        Some(Stack::new(items::BUCKET, 1))
                    } else if fu.n > 1 {
                        Some(Stack { n: fu.n - 1, ..fu })
                    } else {
                        None
                    };
                }
            }
            if f.burn > 0.0 && can_cook {
                f.cook += dt;
                if f.cook >= craft::COOK_TIME {
                    f.cook = 0.0;
                    if let (Some(inp), Some(r)) = (f.input, result) {
                        f.input = if inp.n > 1 { Some(Stack { n: inp.n - 1, ..inp }) } else { None };
                        f.output = Some(match f.output {
                            Some(o) => Stack { n: o.n + 1, ..o },
                            None => Stack::new(r, 1),
                        });
                    }
                }
            } else {
                f.cook = 0.0;
            }
            let lit = f.burn > 0.0;
            if lit != was_lit {
                lit_changes.push((*pos, lit));
            }
            if lit || was_lit {
                self.cont_dirty.insert(*pos);
            }
        }
        for (pos, lit) in lit_changes {
            let b = if grills.contains(&pos) { blocks::SMOKER } else { FURNACE };
            self.world.set(pos, b, lit as u8);
        }
    }

    // ---------------------------------------------------------------- survie

    fn tick_survival(&mut self) {
        let dim = self.dim;
        let (difficulty, second) = (self.rules.difficulty, self.tick % TPS as u64 == 0);
        let mut dead = Vec::new();
        for (id, c) in self.clients.iter_mut() {
            if !c.ready || c.dead || c.dim != dim {
                continue;
            }
            let (p, q) = (Vec3::from(c.pos), Vec3::from(c.prev));
            c.prev = c.pos;
            if !shared::rules::invulnerable(c.mode) {
                // Fatigue : marcher coute peu, courir beaucoup plus.
                let d = Vec3::new(p.x - q.x, 0.0, p.z - q.z).length();
                if d < 2.0 {
                    c.exhaustion += d * if d / DT > 5.0 { 0.1 } else { 0.01 };
                }
                // Nausee : la fatigue monte toute seule. Poison : un point de vie toutes les 1,25 s,
                // sans jamais tuer. The vert : un point de vie rendu toutes les 2,5 s.
                let before = (c.queasy_t > 0.0, c.poison_t > 0.0);
                c.effect_acc += DT;
                let beat = c.effect_acc >= 1.25;
                if beat {
                    c.effect_acc = 0.0;
                }
                if c.queasy_t > 0.0 {
                    c.queasy_t -= DT;
                    c.exhaustion += 0.3 * DT;
                }
                if c.poison_t > 0.0 {
                    c.poison_t -= DT;
                    if beat && c.hp > 1.0 {
                        c.hp -= 1.0;
                        c.stats_dirty = true;
                    }
                }
                if c.tea_t > 0.0 {
                    c.tea_t -= DT;
                    if beat && c.hp < 20.0 {
                        c.hp = (c.hp + 0.5).min(20.0);
                        c.stats_dirty = true;
                    }
                }
                if before != (c.queasy_t > 0.0, c.poison_t > 0.0) {
                    c.stats_dirty = true;
                }
                // La fatigue entame d'abord la saturation, puis la faim.
                while c.exhaustion >= 4.0 {
                    c.exhaustion -= 4.0;
                    if c.saturation > 0.0 {
                        c.saturation = (c.saturation - 1.0).max(0.0);
                    } else {
                        c.hunger = (c.hunger - 1.0).max(0.0);
                        c.stats_dirty = true;
                    }
                }
                c.regen_t += DT;
                if c.hunger >= 20.0 && c.saturation > 0.0 && c.hp < 20.0 {
                    // Bien nourri : la vie remonte vite, en puisant dans la saturation.
                    if c.regen_t >= 0.5 {
                        c.regen_t = 0.0;
                        c.hp = (c.hp + 1.0).min(20.0);
                        c.exhaustion += 6.0;
                        c.stats_dirty = true;
                    }
                } else if c.regen_t >= 4.0 {
                    c.regen_t = 0.0;
                    if c.hunger >= 18.0 && c.hp < 20.0 {
                        c.hp = (c.hp + 1.0).min(20.0);
                        c.exhaustion += 6.0;
                        c.stats_dirty = true;
                    } else if c.hunger <= 0.0 && c.hp > shared::rules::starve_floor(difficulty) {
                        c.hp -= 1.0;
                        c.stats_dirty = true;
                    }
                }
                // Paisible : la faim ne descend pas, et la vie remonte d'un demi-coeur par seconde.
                if difficulty == shared::rules::PEACEFUL {
                    c.exhaustion = 0.0;
                    if second && (c.hp < 20.0 || c.hunger < 20.0) {
                        c.hp = (c.hp + 1.0).min(20.0);
                        c.hunger = (c.hunger + 1.0).min(20.0);
                        c.stats_dirty = true;
                    }
                }
                // Souffle : dix secondes sous l'eau, puis la noyade.
                c.inv_t -= DT;
                let head = [p.x.floor() as i32, (p.y + 1.62).floor() as i32, p.z.floor() as i32];
                let before = c.air;
                if self.world.get(head).is_some_and(|(b, _)| b == WATER) {
                    c.air -= DT;
                    if c.air <= -1.0 {
                        c.air = 0.0;
                        c.hp -= 2.0;
                        c.stats_dirty = true;
                    }
                } else {
                    c.air = (c.air + DT * 4.0).min(10.0);
                }
                if before.ceil() != c.air.ceil() {
                    c.stats_dirty = true;
                }
                // Lave et vide.
                c.hurt_t -= DT;
                let feet = [p.x.floor() as i32, (p.y + 0.2).floor() as i32, p.z.floor() as i32];
                let in_lava = self.world.get(feet).is_some_and(|(b, _)| b == LAVA);
                if (in_lava || p.y < -20.0) && c.hurt_t <= 0.0 {
                    c.hurt_t = 0.5;
                    // L'armure protege de la lave, pas du vide.
                    c.hp -= if in_lava { c.absorb(4.0) } else { 4.0 };
                    c.stats_dirty = true;
                }
            }
            if c.hp <= 0.0 {
                dead.push(*id);
            }
        }
        for id in dead {
            self.kill(id);
        }
    }

    fn kill(&mut self, id: u32) {
        self.close_window(id);
        let Some(c) = self.clients.get_mut(&id) else { return };
        if c.dead {
            return;
        }
        c.dead = true;
        c.hp = 0.0;
        c.stats_dirty = true;
        c.inv_dirty = true;
        let pos = Vec3::from(c.pos) + Vec3::Y * 0.8;
        let stuff: Vec<Stack> = c.inv.iter_mut().chain(c.armor.iter_mut()).filter_map(|s| s.take()).collect();
        println!("{} est mort", c.name);
        let name = c.name.clone();
        for st in stuff {
            let vel = Vec3::new(self.rng.d() - 0.5, 1.0 + self.rng.d(), self.rng.d() - 0.5) * 3.0;
            self.spawn_drop(pos, vel, st, 1.5);
        }
        self.announce(&format!("{name} est mort"), shared::proto::chat::INFO);
    }

    fn respawn(&mut self, id: u32) {
        let spawn = self.spawn;
        let Some(c) = self.clients.get_mut(&id) else { return };
        if !c.dead {
            return;
        }
        if self.rules.hardcore {
            // Mode extreme : pas de seconde vie. On revient en spectateur, la ou l'on est tombe.
            c.dead = false;
            c.hp = 20.0;
            c.hunger = 20.0;
            c.air = 10.0;
            return self.set_mode(id, shared::rules::SPECTATOR);
        }
        let spawn = c.home.unwrap_or(spawn);
        c.air = 10.0;
        c.dead = false;
        c.hp = 20.0;
        c.hunger = 20.0;
        c.saturation = 5.0;
        (c.queasy_t, c.poison_t, c.tea_t) = (0.0, 0.0, 0.0);
        c.exhaustion = 0.0;
        c.pos = spawn;
        c.prev = spawn;
        c.fall_peak = spawn[1];
        c.on_ground = true;
        c.moved = true;
        c.stats_dirty = true;
        if c.dim == 0 {
            c.send(ServerMsg::Teleport { pos: spawn });
            return;
        }
        // Mort dans une autre dimension : retour a la surface.
        self.enter(0);
        self.move_to_dim(id, 0, spawn);
    }

    // ---------------------------------------------------------------- boucle

    fn tick(&mut self) {
        self.tick += 1;
        self.time = (self.time + 1.0 / (DAY_SECONDS * TPS as f64)).fract();
        for dim in 0..3u8 {
            let used = self.clients.values().any(|c| c.ready && c.dim == dim);
            self.enter(dim);
            if used || !self.world.chunks.is_empty() || !self.world.updates.is_empty() {
                self.tick_realm();
                self.flush();
            }
        }
        self.tick_fishing();
        self.tick_gliders();
        self.tick_veils();
        if self.tick % TPS as u64 == 0 {
            self.broadcast_all(ServerMsg::Time { time: self.time }, None);
        }
        if self.tick % (30 * TPS as u64) == 0 {
            self.save_all();
        }
    }

    /// Un tick de la dimension active.
    fn tick_realm(&mut self) {
        self.tick_furnaces();
        self.tick_irori();
        self.tick_drops();
        self.tick_survival();
        self.tick_mobs();
        self.tick_projs();
        self.random_ticks();
        if self.tick % 5 == 0 {
            self.tick_updates();
        }
        if self.tick % 2 == 0 {
            self.tick_signal();
        }
        self.tick_primed();
        if self.tick % TPS as u64 == 0 {
            self.tick_sundials();
            self.spawn_cycle();
            self.breed_cycle();
        }

        // Ferme les fenetres dont le bloc a disparu ou est trop loin.
        let mut stale: Vec<u32> = self
            .clients
            .iter()
            .filter(|(_, c)| {
                c.dim == self.dim
                    && c.window.pos().is_some_and(|p| {
                    let gone = !self.world.furnaces.contains_key(&p) && !self.world.chests.contains_key(&p);
                    let center = Vec3::new(p[0] as f64 + 0.5, p[1] as f64 + 0.5, p[2] as f64 + 0.5);
                    gone || c.eye().distance(center) > REACH + 2.0
                })
            })
            .map(|(id, _)| *id)
            .collect();
        // Et les echanges dont le villageois s'est eloigne ou n'est plus.
        stale.extend(self.clients.iter().filter(|(_, c)| matches!(c.window, Win::Trade(_)) && c.dim == self.dim && self.trade_partner(c).is_none()).map(|(id, _)| *id));
        for id in stale {
            self.close_window(id);
        }

        // Positions des joueurs.
        let dim = self.dim;
        let moved: Vec<(u32, ServerMsg)> = self
            .clients
            .iter_mut()
            .filter(|(_, c)| c.ready && c.moved && c.dim == dim && c.mode != shared::rules::SPECTATOR)
            .map(|(id, c)| {
                c.moved = false;
                (*id, ServerMsg::PlayerPos { id: *id, pos: c.pos, yaw: c.yaw, pitch: c.pitch })
            })
            .collect();
        for (id, msg) in moved {
            self.broadcast(msg, Some(id));
        }

        // Chunks : les plus proches d'abord.
        let ids: Vec<u32> = self.clients.keys().copied().collect();
        for id in ids {
            let (center, view) = match self.clients.get(&id) {
                Some(c) if c.ready && c.dim == dim => (c.chunk(), c.view),
                _ => continue,
            };
            let mut wanted: Vec<(i32, (i32, i32))> = Vec::new();
            {
                let c = &self.clients[&id];
                for dz in -view..=view {
                    for dx in -view..=view {
                        let d = dx * dx + dz * dz;
                        let key = (center.0 + dx, center.1 + dz);
                        if d <= view * view && !c.sent.contains(&key) {
                            wanted.push((d, key));
                        }
                    }
                }
            }
            wanted.sort_unstable();
            wanted.truncate(CHUNKS_PER_TICK);
            let keys: Vec<(i32, i32)> = wanted.iter().map(|w| w.1).collect();
            self.world.pregenerate(&keys);
            for (_, key) in wanted {
                let data = self.world.ensure(key).c.encode();
                let c = self.clients.get_mut(&id).unwrap();
                c.sent.insert(key);
                c.send(ServerMsg::Chunk { cx: key.0, cz: key.1, data });
            }
            let c = self.clients.get_mut(&id).unwrap();
            let far = (view + 2) * (view + 2);
            let tx = c.tx.clone();
            c.sent.retain(|&(cx, cz)| {
                let (dx, dz) = (cx - center.0, cz - center.1);
                let keep = dx * dx + dz * dz <= far;
                if !keep {
                    let _ = tx.send(ServerMsg::Unload { cx, cz });
                }
                keep
            });
        }

        if self.tick % TPS as u64 == 0 {
            let clients = &self.clients;
            self.world.unload_unused(|key| clients.values().any(|c| c.dim == dim && c.sent.contains(key)));
        }
    }

    /// Envoie aux clients tout ce qui a change depuis le dernier appel.
    fn flush(&mut self) {
        for (pos, b, m) in std::mem::take(&mut self.world.changes) {
            let key = (pos[0] >> 4, pos[2] >> 4);
            for c in self.clients.values() {
                if c.dim == self.dim && c.sent.contains(&key) {
                    c.send(ServerMsg::BlockChange { pos, b, m });
                }
            }
        }
        // Un conteneur qui change peut faire bouger une balance onmyo.
        if !self.cont_dirty.is_empty() {
            self.world.signal_dirty = true;
        }
        let dirty = std::mem::take(&mut self.cont_dirty);
        // Tenues qui ont change : tout le monde doit voir la nouvelle.
        let mut gear = Vec::new();
        for (id, c) in self.clients.iter_mut().filter(|(_, c)| c.ready) {
            let worn = c.armor.map(|s| s.map_or(0, |st| st.id));
            if worn != c.gear_sent {
                c.gear_sent = worn;
                gear.push(ServerMsg::Gear { id: *id, armor: worn });
            }
        }
        for msg in gear {
            self.broadcast_all(msg, None);
        }
        for c in self.clients.values_mut() {
            if c.inv_dirty {
                c.inv_dirty = false;
                c.send(ServerMsg::Inventory {
                    inv: c.inv.to_vec(),
                    armor: c.armor.to_vec(),
                    cursor: c.cursor,
                    craft: c.craft.to_vec(),
                    craft_out: craft::result(&c.craft),
                });
            }
            if c.stats_dirty {
                c.stats_dirty = false;
                let effects = (c.poison_t > 0.0) as u8 | ((c.queasy_t > 0.0) as u8) << 1;
                c.send(ServerMsg::Stats { hp: c.hp.max(0.0), hunger: c.hunger, air: c.air, effects });
            }
            if c.dim != self.dim {
                continue;
            }
            match c.window {
                Win::Furnace(p) if dirty.contains(&p) => {
                    if let Some(f) = self.world.furnaces.get(&p) {
                        let flame = if f.burn_max > 0.0 { (f.burn / f.burn_max).clamp(0.0, 1.0) } else { 0.0 };
                        c.send(ServerMsg::Container {
                            slots: vec![f.input, f.fuel, f.output],
                            progress: [flame, f.cook / craft::COOK_TIME],
                        });
                    }
                }
                Win::Chest(p) if dirty.contains(&p) => {
                    if let Some(ch) = self.world.chests.get(&p) {
                        c.send(ServerMsg::Container { slots: ch.clone(), progress: [0.0; 2] });
                    }
                }
                _ => {}
            }
        }
    }

    fn save_player(&self, id: u32) {
        let Some(c) = self.clients.get(&id).filter(|c| c.ready) else { return };
        let Some(path) = self.world.player_path(&c.name) else { return };
        // Un joueur mort reapparaitra au point de depart, en pleine forme.
        let save = if c.dead {
            let pos = c.home.unwrap_or(self.spawn);
            PlayerSave { pos, hp: 20.0, hunger: 20.0, inv: c.inv.iter().chain(c.armor.iter()).copied().collect(), home: c.home, dim: (c.mode + 1) << 4 }
        } else {
            PlayerSave { pos: c.pos, hp: c.hp, hunger: c.hunger, inv: c.inv.iter().chain(c.armor.iter()).copied().collect(), home: c.home, dim: c.dim | (c.mode + 1) << 4 }
        };
        match encode_player(&save) {
            Some(data) => {
                if let Err(e) = std::fs::write(&path, data) {
                    eprintln!("Echec de la sauvegarde de {} : {e}", c.name);
                }
            }
            None => eprintln!("Echec de la sauvegarde de {}", c.name),
        }
    }

    fn save_all(&mut self) {
        // Rien ne doit rester dans une grille ou un curseur au moment d'ecrire.
        let ids: Vec<u32> = self.clients.keys().copied().collect();
        for id in &ids {
            self.save_player(*id);
        }
        let back = self.dim;
        for dim in 0..3 {
            self.enter(dim);
            if dim == 0 {
                self.world.time = self.time;
            }
            let mobs = self.mob_saves();
            self.world.save(mobs);
        }
        self.enter(back);
    }

    /// Cherche un point d'apparition sur la terre ferme pres de l'origine.
    fn find_spawn(&mut self) -> [f64; 3] {
        for i in 0..400 {
            let (x, z) = (8 + i * 13, 8 + (i * 7) % 31);
            let (h, _) = gen::column(self.world.seed, x, z);
            if h > shared::chunk::SEA + 1 && h < 160 {
                // Sur le sol nu : ni au sommet d'un arbre, ni sur un toit, ni au bord d'une grotte.
                let chunk = gen::generate(0, self.world.seed, x >> 4, z >> 4);
                let (lx, lz, y) = ((x & 15) as usize, (z & 15) as usize, h as usize);
                let ground = matches!(chunk.get(lx, y, lz), blocks::GRASS | blocks::SAND | blocks::SNOW | blocks::DIRT | blocks::STONE);
                if ground && chunk.top(lx, lz) == Some(y) && y + 3 < H {
                    return [x as f64 + 0.5, y as f64 + 1.05, z as f64 + 0.5];
                }
            }
        }
        [8.5, 170.0, 8.5]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physics_is_exact_at_the_world_border() {
        // Une dalle a 29 999 000 blocs de l'origine : la ou un flottant simple avance par pas de 2 blocs.
        let far = shared::WORLD_BORDER as i32 - 984;
        let mut srv = Server::new(7, None);
        srv.world.ensure((far >> 4, far >> 4));
        let y = 320;
        for dz in -3..=3 {
            for dx in -3..=3 {
                srv.world.set([far + dx, y, far + dz], blocks::STONE, 0);
            }
        }
        // Un objet lache un quart de bloc a cote du centre retombe exactement sur la dalle, sans glisser.
        let start = Vec3::new(far as f64 + 0.25, y as f64 + 4.0, far as f64 + 0.75);
        srv.spawn_drop(start, Vec3::ZERO, Stack::new(items::COAL, 1), 5.0);
        for _ in 0..80 {
            srv.tick_drops();
        }
        let p = srv.drops[0].pos;
        assert!((p.y - (y as f64 + 1.0)).abs() < 1e-6, "pose sur la dalle : y = {}", p.y);
        assert!((p.x - start.x).abs() < 1e-6 && (p.z - start.z).abs() < 1e-6, "tombe droit : {p:?}");
        // Une creature y marche par petits pas, pas par bonds de deux blocs.
        srv.spawn_mob(shared::mobs::COW, Vec3::new(far as f64 + 0.5, y as f64 + 1.0, far as f64 + 0.5), 0.0, 10.0);
        let mut steps = Vec::new();
        let mut last = srv.mobs[0].pos;
        for _ in 0..200 {
            srv.tick_mobs();
            let now = srv.mobs[0].pos;
            steps.push((now.x - last.x).hypot(now.z - last.z));
            last = now;
        }
        assert!(steps.iter().all(|d| *d < 0.5), "aucun bond : {:?}", steps.iter().cloned().fold(0.0, f64::max));
        assert!((last.y - (y as f64 + 1.0)).abs() < 1e-6 || last.y < y as f64, "elle tient sur la dalle (ou en est tombee)");

        // La bordure ne se franchit pas.
        let (tx, rx) = mpsc::channel();
        let edge = shared::WORLD_BORDER - 2.0;
        let mut c = Client::new(tx, [edge, y as f64 + 1.0, 0.5]);
        c.ready = true;
        srv.clients.insert(1, c);
        srv.on_pos(1, [edge + 5.0, y as f64 + 1.0, 0.5], 0.0, 0.0, true);
        assert!(srv.clients[&1].pos[0] <= shared::WORLD_BORDER);
        assert!(rx.try_iter().any(|m| matches!(m, ServerMsg::Teleport { .. })));
        srv.on_pos(1, [edge + 1.0, y as f64 + 1.0, 0.5], 0.0, 0.0, true);
        assert_eq!(srv.clients[&1].pos[0], edge + 1.0, "juste avant la bordure, on avance librement");
    }

    #[test]
    fn ping_is_echoed_with_server_pace() {
        let mut srv = Server::new(7, None);
        let (tx, rx) = mpsc::channel();
        let mut c = Client::new(tx, [0.5, 200.0, 0.5]);
        c.ready = true;
        srv.clients.insert(1, c);
        srv.on_msg(1, ClientMsg::Ping { t: 123_456 });
        let (t, tps, mspt) = rx
            .try_iter()
            .find_map(|m| match m {
                ServerMsg::Pong { t, tps, mspt } => Some((t, tps, mspt)),
                _ => None,
            })
            .expect("le serveur repond aussitot");
        assert_eq!(t, 123_456, "l'horodatage revient tel quel");
        assert!(tps > 0.0 && tps <= 40.0 && mspt >= 0.0);
    }

    #[test]
    fn old_player_files_still_load() {
        // Fichier d'avant la double precision : memes champs, en f32, sans en-tete.
        let inv: Vec<Slot> = vec![Some(Stack::new(items::COAL, 5)); 36];
        let old = bincode::serialize(&([10.5f32, 70.0f32, -3.25f32], 17.0f32, 12.0f32, inv.clone(), Some([1.0f32, 2.0f32, 3.0f32]), 1u8)).unwrap();
        let p = read_player(&old).expect("ancien format");
        assert_eq!((p.pos, p.hp, p.hunger, p.dim), ([10.5, 70.0, -3.25], 17.0, 12.0, 1));
        assert_eq!(p.home, Some([1.0, 2.0, 3.0]));
        assert_eq!(p.inv, inv);
        // Reecrit, il porte l'en-tete et garde toute la precision, meme tres loin de l'origine.
        let far = PlayerSave { pos: [29_999_900.125, 140.0, -29_999_900.875], ..p };
        let data = encode_player(&far).unwrap();
        assert!(data.starts_with(SAVE_MAGIC));
        assert_eq!(read_player(&data).unwrap().pos, [29_999_900.125, 140.0, -29_999_900.875]);
        // Un fichier illisible n'est pas pris pour un joueur.
        assert!(read_player(&[1, 2, 3]).is_none());
    }

    #[test]
    fn click_rules() {
        let dirt = |n| Some(Stack::new(blocks::DIRT as u16, n));
        // Clic droit sur une pile : on en prend la moitie arrondie au-dessus.
        let (mut t, mut cur) = (dirt(5), None);
        slot_click(&mut t, &mut cur, true, false);
        assert_eq!((t, cur), (dirt(2), dirt(3)));
        // Clic droit avec une pile en main : on en pose un seul.
        slot_click(&mut t, &mut cur, true, false);
        assert_eq!((t, cur), (dirt(3), dirt(2)));
        // Clic gauche : fusion limitee a 64.
        let (mut t, mut cur) = (dirt(60), dirt(10));
        slot_click(&mut t, &mut cur, false, false);
        assert_eq!((t, cur), (dirt(64), dirt(6)));
        // Case de sortie : impossible d'y poser.
        let (mut t, mut cur) = (None, dirt(1));
        slot_click(&mut t, &mut cur, false, true);
        assert_eq!((t, cur), (None, dirt(1)));
    }
}

#[cfg(test)]
mod sim_tests {
    use super::*;
    use shared::blocks::*;
    use shared::mobs as mk;

    /// Serveur sans reseau, avec une dalle de pierre en l'air pour experimenter.
    fn lab() -> (Server, [i32; 3]) {
        let mut srv = Server::new(7, None);
        srv.world.ensure((0, 0));
        let c = [8, 300, 8];
        for dz in -3..=3 {
            for dx in -3..=3 {
                srv.world.set([c[0] + dx, c[1], c[2] + dz], STONE, 0);
            }
        }
        srv.world.updates.clear();
        (srv, c)
    }

    fn player(srv: &mut Server, pos: [f64; 3]) -> (u32, Receiver<ServerMsg>) {
        let (tx, rx) = mpsc::channel();
        let mut c = Client::new(tx, pos);
        c.ready = true;
        c.name = "Test".into();
        srv.clients.insert(1, c);
        (1, rx)
    }

    fn cycles(srv: &mut Server, n: usize) {
        for _ in 0..n {
            srv.tick_updates();
        }
    }

    /// Mesure, pas un vrai test : `cargo test --release -p server -- --ignored --nocapture vitesse`.
    #[test]
    #[ignore]
    fn vitesse_de_generation() {
        let mut srv = Server::new(5, None);
        let t = Instant::now();
        let mut n = 0;
        for cz in -6..6 {
            for cx in -6..6 {
                srv.world.ensure((cx, cz));
                n += 1;
            }
        }
        let gen = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
        let t = Instant::now();
        let mut bytes = 0;
        for cz in -6..6 {
            for cx in -6..6 {
                bytes += srv.world.ensure((cx, cz)).c.encode().len();
            }
        }
        println!("generation : {gen:.2} ms par chunk ; encodage : {:.2} ms, {} octets en moyenne", t.elapsed().as_secs_f64() * 1000.0 / n as f64, bytes / n);
    }

    #[test]
    fn game_modes_and_difficulty() {
        use shared::rules::*;
        let (mut srv, c) = lab();
        let (id, rx) = player(&mut srv, [c[0] as f64 + 0.5, c[1] as f64 + 1.0, c[2] as f64 + 0.5]);
        let stone = [c[0] + 1, c[1], c[2]];
        let give = |srv: &mut Server, st: Stack| srv.clients.get_mut(&1).unwrap().inv[0] = Some(st);
        // Sans triche, on ne change pas de mode.
        srv.on_msg(id, ClientMsg::SetMode(CREATIVE));
        assert_eq!(srv.clients[&id].mode, SURVIVAL);
        srv.rules.cheats = true;

        // Aventure : ni casser, ni poser.
        srv.on_msg(id, ClientMsg::SetMode(ADVENTURE));
        assert!(rx.try_iter().any(|m| matches!(m, ServerMsg::Rules { mode: ADVENTURE, .. })), "le mode est annonce");
        srv.clients.get_mut(&id).unwrap().last_dig = Instant::now() - Duration::from_secs(60);
        srv.on_msg(id, ClientMsg::Dig { pos: stone });
        assert_eq!(srv.world.get(stone).map(|x| x.0), Some(STONE));
        give(&mut srv, Stack::new(DIRT as u16, 4));
        assert!(!srv.place(id, stone, [stone[0], stone[1] + 1, stone[2]]));

        // Creatif : la pierre casse d'un coup et ne laisse rien ; poser ne coute rien ; on vole ; on ne craint rien.
        srv.on_msg(id, ClientMsg::SetMode(CREATIVE));
        srv.clients.get_mut(&id).unwrap().last_dig = Instant::now() - Duration::from_secs(60);
        let before = srv.drops.len();
        srv.on_msg(id, ClientMsg::Dig { pos: stone });
        assert_eq!(srv.world.get(stone).map(|x| x.0), Some(AIR));
        assert_eq!(srv.drops.len(), before, "rien ne tombe");
        assert!(srv.place(id, [c[0] + 2, c[1], c[2]], [c[0] + 2, c[1] + 1, c[2]]));
        assert_eq!(srv.clients[&id].inv[0].map(|s| s.n), Some(4), "la pile ne diminue pas");
        srv.on_msg(id, ClientMsg::SetFly(true));
        assert!(srv.clients[&id].fly);
        srv.hurt_player(id, 8.0, Vec3::ZERO);
        assert_eq!(srv.clients[&id].hp, 20.0);
        // Inventaire creatif : n'importe quel objet, par pile entiere ; un second clic vide le curseur.
        srv.open_window(id, Win::Inventory);
        srv.on_msg(id, ClientMsg::Pick { id: items::DIAMOND });
        assert_eq!(srv.clients[&id].cursor, Some(Stack::new(items::DIAMOND, 64)));
        srv.on_msg(id, ClientMsg::Pick { id: 0 });
        assert!(srv.clients[&id].cursor.is_none());
        srv.close_window(id);

        // Spectateur : rien ne passe, et le vol cesse en revenant en survie.
        srv.on_msg(id, ClientMsg::SetMode(SPECTATOR));
        srv.on_msg(id, ClientMsg::OpenInventory);
        assert_eq!(srv.clients[&id].window, Win::None);
        srv.hurt_player(id, 8.0, Vec3::ZERO);
        assert_eq!(srv.clients[&id].hp, 20.0);
        srv.on_msg(id, ClientMsg::SetMode(SURVIVAL));
        assert!(!srv.clients[&id].fly);
        srv.on_msg(id, ClientMsg::Pick { id: items::DIAMOND });
        assert!(srv.clients[&id].cursor.is_none(), "pas d'objets gratuits en survie");

        // Difficulte : le meme coup de 6 coute 4, 6 ou 9 ; en paisible, rien.
        for (d, cost) in [(EASY, 4.0), (NORMAL, 6.0), (HARD, 9.0), (PEACEFUL, 0.0)] {
            srv.on_msg(id, ClientMsg::SetDifficulty(d));
            let cl = srv.clients.get_mut(&id).unwrap();
            (cl.hp, cl.inv_t) = (20.0, 0.0);
            srv.hurt_player(id, 6.0, Vec3::ZERO);
            assert_eq!(20.0 - srv.clients[&id].hp, cost, "difficulte {d}");
        }
        // Paisible : aucune creature hostile ne nait, les autres si.
        let at = Vec3::new(c[0] as f64, c[1] as f64 + 1.0, c[2] as f64);
        let n = srv.mobs.len();
        srv.spawn_mob(mk::ZOMBIE, at, 0.0, 20.0);
        assert_eq!(srv.mobs.len(), n);
        srv.spawn_mob(mk::COW, at, 0.0, 10.0);
        assert_eq!(srv.mobs.len(), n + 1);

        // Mode extreme : difficile impose, plus de triche ; apres la mort on reste spectateur.
        srv.rules.hardcore = true;
        srv.rules_changed();
        assert_eq!((srv.rules.difficulty, srv.rules.cheats), (HARD, false));
        srv.on_msg(id, ClientMsg::SetDifficulty(EASY));
        assert_eq!(srv.rules.difficulty, HARD, "la difficulte est verrouillee");
        srv.kill(id);
        srv.respawn(id);
        let cl = &srv.clients[&id];
        assert!(!cl.dead && cl.mode == SPECTATOR);
        srv.on_msg(id, ClientMsg::SetMode(SURVIVAL));
        assert_eq!(srv.clients[&id].mode, SPECTATOR, "pas de retour a la vie");
    }

    #[test]
    fn armor_is_worn_protects_and_wears_out() {
        let (mut srv, c) = lab();
        let (id, rx) = player(&mut srv, [c[0] as f64 + 0.5, c[1] as f64 + 1.0, c[2] as f64 + 0.5]);
        let iron = |slot: u8| items::armor_id(slot, 2);
        // Clic droit, piece en main : elle s'enfile ; la suivante prend sa place et l'ancienne revient en main.
        srv.clients.get_mut(&id).unwrap().inv[0] = Some(Stack::new(iron(items::BODY), 1));
        srv.use_item(id, None);
        assert_eq!(srv.clients[&id].armor[1].map(|s| s.id), Some(iron(items::BODY)));
        assert!(srv.clients[&id].inv[0].is_none());
        srv.clients.get_mut(&id).unwrap().inv[0] = Some(Stack::new(items::armor_id(items::BODY, 4), 1));
        srv.use_item(id, None);
        assert_eq!(srv.clients[&id].armor[1].map(|s| s.id), Some(items::armor_id(items::BODY, 4)));
        assert_eq!(srv.clients[&id].inv[0].map(|s| s.id), Some(iron(items::BODY)));
        // Dans l'inventaire : chaque case n'accepte que sa piece ; Maj-clic enfile et retire.
        srv.open_window(id, Win::Inventory);
        srv.clients.get_mut(&id).unwrap().cursor = Some(Stack::new(iron(items::HEAD), 1));
        srv.click(id, SlotRef::Armor(3), false, false);
        assert!(srv.clients[&id].armor[3].is_none(), "un casque ne se met pas aux pieds");
        srv.click(id, SlotRef::Armor(0), false, false);
        assert_eq!(srv.clients[&id].armor[0].map(|s| s.id), Some(iron(items::HEAD)));
        assert!(srv.clients[&id].cursor.is_none());
        srv.clients.get_mut(&id).unwrap().inv[5] = Some(Stack::new(iron(items::FEET), 1));
        srv.click(id, SlotRef::Inv(5), false, true);
        assert_eq!(srv.clients[&id].armor[3].map(|s| s.id), Some(iron(items::FEET)));
        srv.click(id, SlotRef::Armor(1), false, true);
        assert!(srv.clients[&id].armor[1].is_none(), "Maj-clic retire la piece");
        srv.close_window(id);
        // Tenue de fer complete : 15 points.
        for slot in 0..4u8 {
            srv.clients.get_mut(&id).unwrap().armor[slot as usize] = Some(Stack::new(iron(slot), 1));
        }
        srv.flush();
        let msgs: Vec<ServerMsg> = rx.try_iter().collect();
        assert!(msgs.iter().any(|m| matches!(m, ServerMsg::Gear { armor, .. } if armor[0] == iron(0) && armor[3] == iron(3))), "la tenue est annoncee");
        assert!(msgs.iter().any(|m| matches!(m, ServerMsg::Inventory { armor, .. } if armor.iter().flatten().count() == 4)));
        // Un coup de 5 : sans armure il coute 5 coeurs-demis, en fer nettement moins, et chaque piece s'use.
        srv.hurt_player(id, 5.0, Vec3::ZERO);
        let taken = 20.0 - srv.clients[&id].hp;
        assert!((taken - items::mitigate(5.0, 15.0, 0.0)).abs() < 1e-9 && taken < 2.6, "degats subis : {taken}");
        for slot in 0..4u8 {
            assert_eq!(srv.clients[&id].armor[slot as usize].map(|s| s.dur), Some(items::max_dur(iron(slot)) - 1));
        }
        // Une piece a bout de souffle casse au coup suivant.
        let cl = srv.clients.get_mut(&id).unwrap();
        cl.armor[0].as_mut().unwrap().dur = 1;
        cl.inv_t = 0.0;
        srv.hurt_player(id, 5.0, Vec3::ZERO);
        assert!(srv.clients[&id].armor[0].is_none(), "le casque use se brise");
        // La chute, elle, ignore l'armure ; et a la mort tout tombe au sol, armure comprise.
        let before = srv.drops.len();
        srv.kill(id);
        assert!(srv.clients[&id].armor.iter().all(|s| s.is_none()));
        assert!(srv.drops.len() >= before + 3);
    }

    #[test]
    fn water_spreads_then_drains() {
        let (mut srv, c) = lab();
        let src = [c[0], c[1] + 1, c[2]];
        srv.world.set(src, WATER, 0);
        cycles(&mut srv, 40);
        assert_eq!(srv.world.get([src[0] + 1, src[1], src[2]]), Some((WATER, 1)));
        assert_eq!(srv.world.get([src[0] + 3, src[1], src[2]]), Some((WATER, 3)));
        // Au bord de la dalle, l'eau tombe.
        assert_eq!(srv.world.get([src[0] + 4, src[1] - 1, src[2]]), Some((WATER, 8)));
        // Sans source, tout finit par disparaitre.
        srv.world.set(src, AIR, 0);
        cycles(&mut srv, 400);
        for dx in -4..=4 {
            assert_ne!(srv.world.get([src[0] + dx, src[1], src[2]]).unwrap().0, WATER, "reste de l'eau en {dx}");
        }
    }

    #[test]
    fn two_sources_make_a_third() {
        let (mut srv, c) = lab();
        let y = c[1] + 1;
        srv.world.set([c[0] - 1, y, c[2]], WATER, 0);
        srv.world.set([c[0] + 1, y, c[2]], WATER, 0);
        cycles(&mut srv, 10);
        assert_eq!(srv.world.get([c[0], y, c[2]]), Some((WATER, 0)));
    }

    #[test]
    fn sand_falls_and_lava_meets_water() {
        let (mut srv, c) = lab();
        srv.world.set([c[0] - 2, c[1] + 9, c[2]], SAND, 0);
        cycles(&mut srv, 20);
        assert_eq!(srv.world.get([c[0] - 2, c[1] + 1, c[2]]).unwrap().0, SAND);
        assert_eq!(srv.world.get([c[0] - 2, c[1] + 9, c[2]]).unwrap().0, AIR);

        srv.world.set([c[0] + 2, c[1] + 1, c[2] + 2], LAVA, 0);
        srv.world.set([c[0] + 1, c[1] + 1, c[2] + 2], WATER, 0);
        cycles(&mut srv, 8);
        assert_eq!(srv.world.get([c[0] + 2, c[1] + 1, c[2] + 2]).unwrap().0, OBSIDIAN);
    }

    #[test]
    fn crops_and_trees_grow() {
        let (mut srv, c) = lab();
        let soil = [c[0], c[1], c[2]];
        srv.world.set(soil, FARMLAND, 0);
        let crop = [c[0], c[1] + 1, c[2]];
        srv.world.set(crop, WHEAT, 0);
        for _ in 0..200 {
            srv.random_tick(crop);
        }
        assert_eq!(srv.world.get(crop), Some((WHEAT, 3)));
        // Sans la terre labouree, la plante tombe.
        srv.world.set(soil, STONE, 0);
        cycles(&mut srv, 2);
        assert_eq!(srv.world.get(crop).unwrap().0, AIR);
        assert!(srv.drops.iter().any(|d| d.stack.id == items::WHEAT_ITEM));

        let base = [c[0] + 2, c[1] + 1, c[2] - 2];
        srv.world.set([base[0], base[1] - 1, base[2]], DIRT, 0);
        srv.world.set(base, SAPLING, 0);
        srv.grow_tree(base);
        assert_eq!(srv.world.get(base).unwrap().0, LOG);
        // Le tronc fait 4 a 6 blocs ; juste au-dessus, le feuillage.
        let above = (1..9).map(|dy| srv.world.get([base[0], base[1] + dy, base[2]]).unwrap().0).find(|b| *b != LOG);
        assert_eq!(above, Some(LEAVES));
    }

    #[test]
    fn hoe_and_bucket() {
        let (mut srv, c) = lab();
        let (id, _rx) = player(&mut srv, [c[0] as f64 + 0.5, c[1] as f64 + 1.0, c[2] as f64 + 0.5]);
        let dirt = [c[0] + 1, c[1], c[2]];
        srv.world.set(dirt, DIRT, 0);
        srv.clients.get_mut(&id).unwrap().inv[0] = Some(Stack::new(items::tool_id(items::HOE, 0), 1));
        srv.use_item(id, Some((dirt, [dirt[0], dirt[1] + 1, dirt[2]])));
        assert_eq!(srv.world.get(dirt).unwrap().0, FARMLAND);
        assert_eq!(srv.clients[&id].inv[0].unwrap().dur, items::max_dur(items::tool_id(items::HOE, 0)) - 1);

        let pool = [c[0] - 1, c[1] + 1, c[2]];
        srv.world.set(pool, WATER, 0);
        srv.clients.get_mut(&id).unwrap().inv[0] = Some(Stack::new(items::BUCKET, 1));
        srv.use_item(id, Some((pool, pool)));
        assert_eq!(srv.world.get(pool).unwrap().0, AIR);
        assert_eq!(srv.clients[&id].inv[0].unwrap().id, items::WATER_BUCKET);
        srv.use_item(id, Some(([pool[0], pool[1] - 1, pool[2]], pool)));
        assert_eq!(srv.world.get(pool), Some((WATER, 0)));
        assert_eq!(srv.clients[&id].inv[0].unwrap().id, items::BUCKET);
    }

    #[test]
    fn combat_and_breeding() {
        let (mut srv, c) = lab();
        let stand = Vec3::new(c[0] as f64 + 0.5, c[1] as f64 + 1.0, c[2] as f64 + 0.5);
        let (id, rx) = player(&mut srv, stand.to_array());

        // Un zombie au contact blesse le joueur et le repousse.
        srv.spawn_mob(mk::ZOMBIE, stand + Vec3::X * 1.2, 0.0, mk::max_hp(mk::ZOMBIE));
        srv.time = 0.75; // minuit : il ne brule pas
        for _ in 0..40 {
            srv.tick_mobs();
        }
        assert!(srv.clients[&id].hp < 20.0, "le zombie aurait du frapper");
        assert!(rx.try_iter().any(|m| matches!(m, ServerMsg::Knock { .. })));

        // Le joueur le tue a l'epee ; il laisse (parfois) du butin et disparait.
        srv.clients.get_mut(&id).unwrap().inv[0] = Some(Stack::new(items::tool_id(items::SWORD, 3), 1));
        let zid = srv.mobs[0].id;
        for _ in 0..12 {
            if let Some(m) = srv.mobs.iter_mut().find(|m| m.id == zid) {
                m.pos = stand + Vec3::X * 1.2;
            }
            srv.clients.get_mut(&id).unwrap().last_hit = Instant::now() - Duration::from_secs(1);
            srv.attack(id, zid);
            for _ in 0..8 {
                srv.tick_mobs();
            }
        }
        assert!(srv.mobs.iter().all(|m| m.id != zid), "le zombie devrait etre mort");
        assert!(srv.clients[&id].inv[0].unwrap().dur < 1562, "l'epee s'use");

        // Deux vaches nourries au ble font un veau.
        srv.clients.get_mut(&id).unwrap().inv[0] = Some(Stack::new(items::WHEAT_ITEM, 2));
        srv.spawn_mob(mk::COW, stand + Vec3::Z * 1.5, 0.0, 10.0);
        srv.spawn_mob(mk::COW, stand - Vec3::Z * 1.5, 0.0, 10.0);
        let cows: Vec<u32> = srv.mobs.iter().map(|m| m.id).collect();
        for cow in &cows {
            srv.interact(id, *cow);
        }
        assert!(srv.clients[&id].inv[0].is_none(), "le ble est consomme");
        srv.breed_cycle();
        assert_eq!(srv.mobs.len(), 3);
        assert!(srv.mobs.iter().any(|m| m.age < 0.0), "un petit est ne");
        // Pas de second petit tout de suite.
        srv.breed_cycle();
        assert_eq!(srv.mobs.len(), 3);
    }
}

#[cfg(test)]
mod dim_tests {
    use super::*;
    use shared::blocks::*;
    use shared::mobs as mk;

    fn lab() -> (Server, [i32; 3]) {
        let mut srv = Server::new(7, None);
        srv.world.ensure((0, 0));
        let c = [8, 300, 8];
        for dz in -4..=4 {
            for dx in -4..=4 {
                srv.world.set([c[0] + dx, c[1], c[2] + dz], STONE, 0);
            }
        }
        srv.world.updates.clear();
        (srv, c)
    }

    fn player(srv: &mut Server, pos: [f64; 3]) -> Receiver<ServerMsg> {
        let (tx, rx) = mpsc::channel();
        let mut c = Client::new(tx, pos);
        c.ready = true;
        c.name = "Test".into();
        srv.clients.insert(1, c);
        rx
    }

    fn settle(srv: &mut Server) {
        for _ in 0..12 {
            srv.tick_signal();
        }
    }

    #[test]
    fn lever_wire_lamp_and_inverter() {
        let (mut srv, c) = lab();
        let y = c[1] + 1;
        let (lever, lamp) = ([c[0] - 3, y, c[2]], [c[0] + 2, y, c[2]]);
        srv.world.set(lever, LEVER, 0);
        for dx in -2..=1 {
            srv.world.set([c[0] + dx, y, c[2]], WIRE, 0);
        }
        srv.world.set(lamp, LAMP, 0);
        settle(&mut srv);
        assert_eq!(srv.world.get(lamp), Some((LAMP, 0)));

        srv.world.set(lever, LEVER, 1);
        settle(&mut srv);
        assert_eq!(srv.world.get([c[0] - 2, y, c[2]]), Some((WIRE, 15)));
        assert_eq!(srv.world.get([c[0] + 1, y, c[2]]), Some((WIRE, 12)), "le signal perd 1 par bloc");
        assert_eq!(srv.world.get(lamp), Some((LAMP, 1)));

        srv.world.set(lever, LEVER, 0);
        settle(&mut srv);
        assert_eq!(srv.world.get(lamp), Some((LAMP, 0)));
    }

    #[test]
    fn powered_explosive_blows_up() {
        let (mut srv, c) = lab();
        let y = c[1] + 1;
        let (lever, bomb) = ([c[0] - 1, y, c[2]], [c[0], y, c[2]]);
        srv.world.set(bomb, EXPLOSIVE, 0);
        srv.world.set(lever, LEVER, 1);
        settle(&mut srv);
        assert_eq!(srv.world.get(bomb), Some((EXPLOSIVE, 1)), "amorce, pas encore explose");
        for _ in 0..100 {
            srv.tick_primed();
        }
        assert_eq!(srv.world.get(bomb).unwrap().0, AIR);
        assert_eq!(srv.world.get([c[0], c[1], c[2]]).unwrap().0, AIR, "la dalle sous l'explosif est detruite");
    }

    #[test]
    fn portals_and_boss() {
        let (mut srv, c) = lab();
        let start = [c[0] as f64 + 0.5, c[1] as f64 + 1.0, c[2] as f64 + 0.5];
        let rx = player(&mut srv, start);

        // Aller a Yomi : on arrive dans le voile d'un torii neuf, les pieds sur son seuil.
        srv.travel(1, 1);
        let cl = &srv.clients[&1];
        assert_eq!((cl.dim, srv.dim), (1, 1));
        let p = [cl.pos[0].floor() as i32, cl.pos[1].floor() as i32, cl.pos[2].floor() as i32];
        assert_eq!(srv.world.get(p), Some((ABYSS_PORTAL, 2)));
        assert_eq!(srv.world.get([p[0], p[1] - 1, p[2]]).unwrap().0, OBSIDIAN);
        assert!(rx.try_iter().any(|m| matches!(m, ServerMsg::Dimension { dim: 1, .. })));

        // Un clic sur le voile ramene a la surface, au meme endroit (a huit blocs pres).
        srv.use_item(1, Some((p, p)));
        assert_eq!((srv.clients[&1].dim, srv.dim), (0, 0));
        assert_eq!(srv.clients[&1].pos[0].floor() as i32, c[0]);

        // Zenith : arrivee sur l'ile, le Wyrm s'eveille et sa barre de vie est envoyee.
        srv.travel(1, 2);
        let pos = Vec3::from(srv.clients[&1].pos);
        assert_eq!(srv.clients[&1].dim, 2);
        assert_eq!(srv.world.get([0, pos.y.floor() as i32 - 1, 12]).unwrap().0, ZENITH_STONE);
        srv.spawn_cycle();
        assert!(srv.mobs.iter().any(|m| m.kind == mk::WYRM));
        assert!(rx.try_iter().any(|m| matches!(m, ServerMsg::Boss { hp: Some(_) })));

        // Il vole sans tomber, puis meurt : butin au centre et plus jamais de reapparition.
        for _ in 0..100 {
            srv.tick_mobs();
        }
        let wyrm = srv.mobs.iter_mut().find(|m| m.kind == mk::WYRM).unwrap();
        assert!(wyrm.pos.y > 70.0, "le Wyrm doit rester en l'air");
        assert!(wyrm.pos.distance(Vec3::new(42.0, 95.0, 0.0)) > 5.0, "le Wyrm doit se deplacer");
        wyrm.hp = 0.0;
        srv.tick_mobs();
        assert!(srv.world.boss_dead);
        assert!(srv.drops.iter().any(|d| d.stack.id == items::WYRM_HEART));
        srv.spawn_cycle();
        assert!(!srv.mobs.iter().any(|m| m.kind == mk::WYRM));

        // Le portail central ramene a la surface.
        let top = (1..H as i32).rev().find(|&y| srv.world.get([0, y, 0]).is_some_and(|(b, _)| b == ZENITH_PORTAL)).unwrap();
        srv.clients.get_mut(&1).unwrap().pos = [0.5, top as f64, 2.5];
        srv.use_item(1, Some(([0, top, 0], [0, top, 1])));
        assert_eq!(srv.clients[&1].dim, 0);
    }

    #[test]
    fn dens_hold_loot_and_summon_oni() {
        const SEED: u32 = 20251;
        let mut srv = Server::new(SEED, None);
        let (cx, cz) = (0..40)
            .flat_map(|cx| (0..40).map(move |cz| (cx, cz)))
            .find(|(cx, cz)| shared::gen::generate(0, SEED, *cx, *cz).b.contains(&SPAWNER))
            .expect("une taniere");
        srv.world.ensure((cx, cz));
        // Les coffres poses par la generation sont garnis des que le chunk existe.
        assert!(srv.world.chests.len() >= 2);
        assert!(srv.world.chests.values().all(|c| c.iter().flatten().count() >= 1));
        // Un coffre vide par un joueur ne se regarnit pas.
        let key = *srv.world.chests.keys().next().unwrap();
        srv.world.chests.insert(key, vec![None; 27]);
        srv.world.chunks.remove(&(cx, cz));
        srv.world.ensure((cx, cz));
        assert_eq!(srv.world.chests[&key].iter().flatten().count(), 0);

        // La stele fait surgir des oni tant qu'un joueur est proche, quatre au plus autour d'elle.
        let stele = *srv.world.signal.iter().find(|p| srv.world.get(**p).is_some_and(|x| x.0 == SPAWNER)).expect("stele recensee");
        let at = Vec3::new(stele[0] as f64 + 0.5, stele[1] as f64, stele[2] as f64 + 0.5);
        let near = |srv: &Server| srv.mobs.iter().filter(|m| m.kind == mk::ZOMBIE && m.pos.distance(at) < 9.0).count();
        for _ in 0..100 {
            srv.spawn_cycle();
        }
        assert_eq!(near(&srv), 0, "personne autour : rien ne sort");
        let _rx = player(&mut srv, [at.x + 1.0, at.y, at.z]);
        for _ in 0..300 {
            srv.spawn_cycle();
        }
        assert!((1..=4).contains(&near(&srv)), "oni autour de la stele : {}", near(&srv));
    }

    #[test]
    fn dimensions_generate_differently() {
        let abyss = shared::gen::generate(1, 7, 0, 0);
        let count = |c: &shared::chunk::Chunk, b: u8| c.b.iter().filter(|x| **x == b).count();
        assert!(count(&abyss, ABYSS_ROCK) > 5000 && count(&abyss, AIR) > 2000);
        assert_eq!(abyss.get(3, 127, 3), BEDROCK);
        let zenith = shared::gen::generate(2, 7, 0, 0);
        assert_eq!(count(&zenith, ZENITH_PORTAL), 1);
        assert!(count(&zenith, ZENITH_STONE) > 1000);
        // Loin de l'ile centrale mais avant les ilots : le vide.
        assert_eq!(count(&shared::gen::generate(2, 7, 7, 0), AIR), shared::chunk::VOL);
    }
}
