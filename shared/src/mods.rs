//! Mods : le paquet d'un mod, le registre des blocs qu'ils ajoutent, et le bac a sable Lua.
//!
//! Un mod est une archive zip (ou, pendant qu'on l'ecrit, un dossier) qui contient :
//!   mod.json      son identite : id, nom, version, description, auteur
//!   serveur.lua   ce qu'il fait dans le monde (facultatif)
//!   client.lua    ce qu'il fait a l'ecran du joueur (facultatif)
//!   textures/     les images PNG de ses blocs
//! Le serveur d'une partie envoie ses mods aux joueurs qui ne les ont pas : rien a installer a la main.
use mlua::{HookTriggers, Lua, LuaOptions, StdLib, Table, Value, VmState};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Dossier des mods installes, a cote du jeu ou du serveur.
pub const DIR: &str = "mods";
/// Mods recus des serveurs rejoints, ranges sous leur empreinte.
pub const CACHE: &str = "mods/cache";
/// Taille maximale d'un mod, et de ce qu'il contient une fois decompresse.
pub const MAX_SIZE: usize = 32 << 20;
const MAX_UNPACKED: u64 = 64 << 20;
const MAX_FILES: usize = 512;
/// Taille d'un morceau de mod envoye sur la connexion du jeu.
pub const PART: usize = 128 << 10;
/// Premier numero de bloc reserve aux mods ; les suivants vont jusqu'a 255.
pub const FIRST_BLOCK: u8 = 160;
/// Tuiles reservees aux textures des mods : elles occupent un second carre, sous l'atlas du jeu.
pub const FIRST_TILE: u16 = 1024;
pub const TILES: u16 = 768;
/// Modeles de creatures de mods par partie : leurs textures occupent le bas du carre des mods.
pub const MODELS: usize = 16;
/// Longueur maximale du nom et du contenu d'un message entre le script serveur et le script client.
pub const MSG_NAME: usize = 64;
pub const MSG_DATA: usize = 16 << 10;

pub type Hash = [u8; 32];

pub fn hash_of(data: &[u8]) -> Hash {
    Sha256::digest(data).into()
}

pub fn hex(h: &Hash) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}

/// Identite d'un mod, lue dans son fichier mod.json.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub id: String,
    #[serde(alias = "nom")]
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, alias = "auteur")]
    pub author: String,
}

/// Un identifiant de mod : minuscules, chiffres, tirets, de 2 a 32 caracteres.
pub fn valid_id(id: &str) -> bool {
    (2..=32).contains(&id.len()) && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

impl Manifest {
    fn parse(text: &str) -> Result<Manifest, String> {
        let mut m: Manifest = serde_json::from_str(text).map_err(|e| format!("mod.json illisible : {e}"))?;
        if !valid_id(&m.id) {
            return Err(format!("identifiant de mod invalide « {} » (minuscules, chiffres, tirets, 2 à 32 caractères)", m.id));
        }
        let trim = |s: &str, n: usize| s.trim().chars().filter(|c| !c.is_control()).take(n).collect::<String>();
        m.name = trim(&m.name, 48);
        m.version = trim(&m.version, 24);
        m.description = trim(&m.description, 200);
        m.author = trim(&m.author, 48);
        if m.name.is_empty() || m.version.is_empty() {
            return Err("mod.json : « name » et « version » sont obligatoires".into());
        }
        Ok(m)
    }
}

/// Un mod charge en memoire : son archive telle qu'on l'echange, et ses fichiers.
#[derive(Clone)]
pub struct Pack {
    pub manifest: Manifest,
    pub hash: Hash,
    /// L'archive zip, telle qu'elle est envoyee aux joueurs.
    pub bytes: Vec<u8>,
    files: BTreeMap<String, Vec<u8>>,
}

impl Pack {
    /// Lit une archive zip. Les fichiers peuvent etre a la racine ou dans un dossier unique.
    pub fn from_zip(bytes: Vec<u8>) -> Result<Pack, String> {
        if bytes.len() > MAX_SIZE {
            return Err(format!("mod trop gros ({} Mo au plus)", MAX_SIZE >> 20));
        }
        let mut zip = zip::ZipArchive::new(Cursor::new(&bytes[..])).map_err(|e| format!("archive illisible : {e}"))?;
        if zip.len() > MAX_FILES {
            return Err(format!("trop de fichiers dans le mod ({MAX_FILES} au plus)"));
        }
        let mut files = BTreeMap::new();
        let mut total = 0u64;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| format!("archive illisible : {e}"))?;
            if f.is_dir() {
                continue;
            }
            let name = f.name().replace('\\', "/");
            // Fichiers caches et residus de macOS : ignores.
            if name.split('/').any(|part| part.starts_with('.') || part == "__MACOSX") {
                continue;
            }
            total += f.size();
            if total > MAX_UNPACKED {
                return Err("le contenu du mod est trop gros".into());
            }
            let mut data = Vec::new();
            (&mut f).take(MAX_UNPACKED).read_to_end(&mut data).map_err(|e| format!("archive illisible : {e}"))?;
            files.insert(name, data);
        }
        // mod.json fixe la racine : on retire le dossier qui l'entoure, s'il y en a un.
        let root = files
            .keys()
            .filter(|k| *k == "mod.json" || k.ends_with("/mod.json"))
            .min_by_key(|k| k.len())
            .map(|k| k[..k.len() - "mod.json".len()].to_string())
            .ok_or("mod.json absent de l'archive")?;
        let files: BTreeMap<String, Vec<u8>> = files.into_iter().filter_map(|(k, v)| k.strip_prefix(&root).map(|k| (k.to_string(), v))).collect();
        let text = std::str::from_utf8(&files["mod.json"]).map_err(|_| "mod.json n'est pas du texte")?;
        let manifest = Manifest::parse(text)?;
        Ok(Pack { manifest, hash: hash_of(&bytes), bytes, files })
    }

    /// Fabrique l'archive d'un mod a partir de son dossier. Le meme dossier donne toujours la meme archive.
    pub fn from_dir(dir: &Path) -> Result<Pack, String> {
        let mut files = BTreeMap::new();
        collect(dir, "", &mut files).map_err(|e| format!("{} : {e}", dir.display()))?;
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default());
        for (name, data) in &files {
            w.start_file(name.as_str(), opts).map_err(|e| e.to_string())?;
            w.write_all(data).map_err(|e| e.to_string())?;
        }
        Pack::from_zip(w.finish().map_err(|e| e.to_string())?.into_inner())
    }

    pub fn file(&self, name: &str) -> Option<&[u8]> {
        self.files.get(name).map(|v| v.as_slice())
    }

    /// Noms des fichiers du mod.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(|k| k.as_str())
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        self.file(name).and_then(|d| std::str::from_utf8(d).ok())
    }

    pub fn info(&self) -> Info {
        let m = &self.manifest;
        Info { id: m.id.clone(), name: m.name.clone(), version: m.version.clone(), hash: self.hash, size: self.bytes.len() as u32 }
    }
}

/// Dossiers d'un mod qui partent dans son archive ; le reste (sources, compilation) reste chez son createur.
const FOLDERS: [&str; 4] = ["textures", "sons", "modeles", "donnees"];
/// Fichiers de la racine d'un mod qui partent dans son archive.
const ROOT_FILES: [&str; 5] = [".json", ".lua", ".wasm", ".png", ".txt"];

fn collect(dir: &Path, prefix: &str, out: &mut BTreeMap<String, Vec<u8>>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            if !prefix.is_empty() || FOLDERS.contains(&name.as_str()) {
                collect(&path, &format!("{prefix}{name}/"), out)?;
            }
        } else if !prefix.is_empty() || ROOT_FILES.iter().any(|ext| name.ends_with(ext)) {
            out.insert(format!("{prefix}{name}"), std::fs::read(&path)?);
        }
    }
    Ok(())
}

/// Charge tous les mods d'un dossier : les archives .zip et les dossiers qui contiennent un mod.json.
/// Renvoie les mods, dans l'ordre de leur identifiant, et les problemes rencontres.
pub fn load_dir(dir: &Path) -> (Vec<Pack>, Vec<String>) {
    let (mut packs, mut problems): (Vec<Pack>, Vec<String>) = (Vec::new(), Vec::new());
    let Ok(entries) = std::fs::read_dir(dir) else { return (packs, problems) };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let pack = if path.is_dir() && path.join("mod.json").exists() {
            Pack::from_dir(&path)
        } else if path.is_file() && name.ends_with(".zip") {
            std::fs::read(&path).map_err(|e| e.to_string()).and_then(Pack::from_zip)
        } else {
            continue;
        };
        match pack {
            Ok(p) if packs.iter().any(|q| q.manifest.id == p.manifest.id) => problems.push(format!("{name} : le mod « {} » est présent deux fois", p.manifest.id)),
            Ok(p) => packs.push(p),
            Err(e) => problems.push(format!("{name} : {e}")),
        }
    }
    packs.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    (packs, problems)
}

/// Ce que le serveur annonce d'un mod aux joueurs qui arrivent.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Info {
    pub id: String,
    pub name: String,
    pub version: String,
    pub hash: Hash,
    pub size: u32,
}

/// Image a peindre dans une tuile de l'atlas : un fichier du mod.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TileDef {
    pub tile: u16,
    pub mod_id: String,
    pub file: String,
}

/// Outil qui accelere le minage d'un bloc de mod.
pub mod tool {
    pub const NONE: u8 = 0;
    pub const PICK: u8 = 1;
    pub const AXE: u8 = 2;
    pub const SHOVEL: u8 = 3;
}

/// Un bloc ajoute par un mod.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BlockDef {
    /// Numero du bloc dans le monde, de FIRST_BLOCK a 255.
    pub id: u8,
    /// Nom complet : « mod:bloc ».
    pub key: String,
    /// Nom affiche.
    pub name: String,
    /// Tuiles du dessus, des cotes et du dessous.
    pub tiles: [u16; 3],
    pub hardness: f32,
    /// Lumiere emise, de 0 a 15.
    pub light: u8,
    pub tool: u8,
    /// Laisse passer la lumiere (verre, grille...).
    pub clear: bool,
    /// Objet laisse quand on le casse ; 0 = le bloc lui-meme, 65535 = rien.
    pub drop: u16,    /// Forme : 0 un cube plein, 1 une plante (deux plans croises, que l'on traverse).
    #[serde(default)]
    pub shape: u8,
}

pub const DROP_NOTHING: u16 = u16::MAX;

/// Premier numero d'objet reserve aux mods, et leur nombre.
pub const FIRST_ITEM: u16 = 1000;
pub const ITEMS: u16 = 1000;

/// Genre d'outil d'un objet de mod (0 : ce n'est pas un outil).
pub mod kind {
    pub const NONE: u8 = 0;
    pub const PICK: u8 = 1;
    pub const AXE: u8 = 2;
    pub const SHOVEL: u8 = 3;
    pub const SWORD: u8 = 4;
    pub const HOE: u8 = 5;
}

/// Un objet ajoute par un mod.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ItemDef {
    /// Numero de l'objet, a partir de FIRST_ITEM.
    pub id: u16,
    /// Nom complet : « mod:objet ».
    pub key: String,
    pub name: String,
    /// Tuile de son icone.
    pub tile: u16,
    /// Taille d'une pile, de 1 a 64.
    pub stack: u8,
    /// Faim et reserve rendues quand on le mange (0 : ne se mange pas).
    pub food: f32,
    pub saturation: f32,
    /// Secondes de combustion dans un four (0 : pas un combustible).
    pub fuel: f32,
    /// Nombre d'usages (0 : ne s'use pas).
    pub durability: u16,
    /// Genre d'outil (voir `kind`) et niveau, de 0 (bois) a 4 (tamahagane).
    pub tool: u8,
    pub tier: u8,
    /// Degats au corps a corps (0 : ceux du genre d'outil, ou de la main).
    pub damage: f32,
}

/// Une recette d'artisanat ajoutee par un mod : `cells` compte `w * h` objets, 0 pour une case vide.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RecipeDef {
    pub w: u8,
    pub h: u8,
    pub cells: Vec<u16>,
    pub out: u16,
    pub n: u8,
}

/// Une cuisson au four ajoutee par un mod.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SmeltDef {
    pub input: u16,
    pub out: u16,
}

/// Un filon que la generation du monde seme : des amas de `block` a la place de `replace`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct OreDef {
    pub block: u8,
    /// Bloc remplace ; 0 : la roche du lieu (pierre, ardoise, roche de Yomi ou de Takamagahara).
    pub replace: u8,
    /// Monde concerne : 0 le monde ordinaire, 1 Yomi, 2 Takamagahara.
    pub dim: u8,
    /// Amas par chunk, et blocs par amas.
    pub veins: u8,
    pub size: u8,
    pub min_y: u16,
    pub max_y: u16,
}

/// Une creature ajoutee par un mod : elle se comporte comme une creature du jeu (`base`), avec ses
/// propres points de vie, degats, taille, couleur et butin.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MobDef {
    /// Numero de l'apparence, a partir de 1 (0 : une creature du jeu).
    pub id: u8,
    /// Nom complet : « mod:creature ».
    pub key: String,
    pub name: String,
    /// Genre de la creature du jeu dont elle a le comportement et la silhouette.
    pub base: u8,
    pub hp: f32,
    /// Degats au contact (0 : ceux de la creature de base).
    pub damage: f32,
    /// Taille par rapport a la creature de base.
    pub scale: f32,
    /// Teinte donnee a son modele.
    pub color: [u8; 3],
    /// Part des creatures de base qui naissent sous cette forme, de 0 a 1.
    pub rarity: f32,
    /// Butin : (objet, minimum, maximum).
    pub loot: Vec<(u16, u8, u8)>,
    /// Son propre modele : un fichier Blockbench du mod (« modeles/golem.bbmodel »), ou vide pour
    /// garder la silhouette de la creature de base.
    pub model: String,
}

/// Tout ce que les mods d'une partie ajoutent au jeu. Le serveur l'etablit, les joueurs le recoivent.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Content {
    pub blocks: Vec<BlockDef>,
    pub items: Vec<ItemDef>,
    pub recipes: Vec<RecipeDef>,
    pub smelts: Vec<SmeltDef>,
    pub ores: Vec<OreDef>,
    pub mobs: Vec<MobDef>,
    /// Commandes des mods, pour que le chat les propose : (nom, aide).
    pub commands: Vec<(String, String)>,
}

static CONTENT: RwLock<Option<std::sync::Arc<Content>>> = RwLock::new(None);

/// Remplace ce que les mods ajoutent au jeu (pour la partie en cours).
pub fn set_content(mut c: Content) {
    c.blocks.retain(|d| d.id >= FIRST_BLOCK);
    c.items.retain(|d| d.id >= FIRST_ITEM);
    crate::craft::set_mod_recipes(&c.recipes);
    if let Ok(mut slot) = CONTENT.write() {
        *slot = Some(std::sync::Arc::new(c));
    }
}

/// Retire tout ce que les mods avaient ajoute.
pub fn clear_content() {
    set_content(Content::default());
}

pub fn content() -> std::sync::Arc<Content> {
    CONTENT.read().ok().and_then(|c| c.clone()).unwrap_or_default()
}

/// Lit le bloc de mod de numero `id`, s'il existe.
pub fn with<R>(id: u8, f: impl FnOnce(&BlockDef) -> R) -> Option<R> {
    if id < FIRST_BLOCK {
        return None;
    }
    let all = CONTENT.read().ok()?;
    all.as_ref()?.blocks.iter().find(|d| d.id == id).map(f)
}

/// Lit l'objet de mod de numero `id`, s'il existe.
pub fn with_item<R>(id: u16, f: impl FnOnce(&ItemDef) -> R) -> Option<R> {
    if id < FIRST_ITEM {
        return None;
    }
    let all = CONTENT.read().ok()?;
    all.as_ref()?.items.iter().find(|d| d.id == id).map(f)
}

/// Lit la creature de mod de numero `skin`, si elle existe.
pub fn with_mob<R>(skin: u8, f: impl FnOnce(&MobDef) -> R) -> Option<R> {
    if skin == 0 {
        return None;
    }
    let all = CONTENT.read().ok()?;
    all.as_ref()?.mobs.iter().find(|d| d.id == skin).map(f)
}

/// Numero du bloc de mod de ce nom complet.
pub fn block_by_key(key: &str) -> Option<u8> {
    CONTENT.read().ok()?.as_ref()?.blocks.iter().find(|d| d.key == key).map(|d| d.id)
}

/// Numero de l'objet (ou du bloc) de mod de ce nom complet.
pub fn item_by_key(key: &str) -> Option<u16> {
    let all = CONTENT.read().ok()?;
    let c = all.as_ref()?;
    c.items.iter().find(|d| d.key == key).map(|d| d.id).or_else(|| c.blocks.iter().find(|d| d.key == key).map(|d| d.id as u16))
}

// ---------------------------------------------------------------- bac a sable Lua

/// Memoire qu'un mod peut occuper.
const MEMORY: usize = 64 << 20;

/// Heure limite de l'appel en cours : au-dela, le script est interrompu.
struct Deadline(Cell<Instant>);

/// Un interprete Lua ferme : ni fichiers, ni reseau, ni systeme, memoire et temps bornes.
/// Un mod recu d'un serveur ne peut agir que par les fonctions que le jeu lui donne.
pub fn sandbox() -> mlua::Result<Lua> {
    let lua = Lua::new_with(StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8, LuaOptions::default())?;
    lua.set_memory_limit(MEMORY)?;
    let g = lua.globals();
    for name in ["dofile", "loadfile", "load", "loadstring", "require", "collectgarbage"] {
        g.set(name, Value::Nil)?;
    }
    if let Ok(s) = g.get::<Table>("string") {
        s.set("dump", Value::Nil)?;
    }
    lua.set_app_data(Deadline(Cell::new(Instant::now() + Duration::from_secs(3600))));
    lua.set_hook(HookTriggers::new().every_nth_instruction(1000), |lua, _| {
        match lua.app_data_ref::<Deadline>() {
            Some(d) if Instant::now() > d.0.get() => Err(mlua::Error::runtime("le script a mis trop de temps : interrompu")),
            _ => Ok(VmState::Continue),
        }
    });
    Ok(lua)
}

/// Donne `ms` millisecondes au prochain appel du script.
pub fn allow(lua: &Lua, ms: u64) {
    if let Some(d) = lua.app_data_ref::<Deadline>() {
        d.0.set(Instant::now() + Duration::from_millis(ms));
    }
}

/// Execute le texte d'un script, jamais du code deja compile.
pub fn run(lua: &Lua, name: &str, source: &str) -> mlua::Result<()> {
    allow(lua, 2000);
    lua.load(source).set_name(format!("={name}")).set_mode(mlua::ChunkMode::Text).exec()
}

/// Message d'erreur d'un script, sur une ligne.
pub fn error_line(e: &mlua::Error) -> String {
    let text = e.to_string();
    let line = text.lines().next().unwrap_or("erreur");
    line.strip_prefix("runtime error: ").unwrap_or(line).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zip_of(files: &[(&str, &str)]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, text) in files {
            w.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(text.as_bytes()).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    const JSON: &str = r#"{"id":"essai","name":"Essai","version":"1.0"}"#;

    #[test]
    fn archive_with_or_without_a_folder() {
        let flat = Pack::from_zip(zip_of(&[("mod.json", JSON), ("serveur.lua", "x = 1")])).unwrap();
        assert_eq!(flat.manifest.id, "essai");
        assert_eq!(flat.text("serveur.lua"), Some("x = 1"));
        let nested = Pack::from_zip(zip_of(&[("essai/mod.json", JSON), ("essai/textures/a.png", "p"), ("__MACOSX/essai/._mod.json", "x")])).unwrap();
        assert_eq!(nested.file("textures/a.png"), Some(&b"p"[..]));
        assert!(Pack::from_zip(zip_of(&[("serveur.lua", "x = 1")])).is_err());
        assert!(Pack::from_zip(zip_of(&[("mod.json", r#"{"id":"Mauvais Nom","name":"x","version":"1"}"#)])).is_err());
    }

    #[test]
    fn a_folder_always_gives_the_same_archive() {
        let dir = std::env::temp_dir().join(format!("epicube-mod-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("textures")).unwrap();
        std::fs::write(dir.join("mod.json"), JSON).unwrap();
        std::fs::write(dir.join("textures/a.png"), "p").unwrap();
        let (a, b) = (Pack::from_dir(&dir).unwrap(), Pack::from_dir(&dir).unwrap());
        assert_eq!(a.hash, b.hash);
        assert_eq!(a.file("textures/a.png"), Some(&b"p"[..]));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_sandbox_is_closed_and_bounded() {
        let lua = sandbox().unwrap();
        for name in ["io", "os", "require", "load", "dofile", "debug", "package", "coroutine"] {
            assert!(lua.globals().get::<Value>(name).unwrap().is_nil(), "{name} ne doit pas exister");
        }
        assert!(run(&lua, "ok", "x = string.upper('a') .. math.floor(2.5)").is_ok());
        // Une boucle sans fin est interrompue, une montagne de memoire est refusee.
        allow(&lua, 50);
        assert!(lua.load("while true do end").exec().is_err());
        allow(&lua, 2000);
        assert!(lua.load("local t = {} for i = 1, 1e9 do t[i] = string.rep('x', 1e6) end").exec().is_err());
        // Du code compile n'est pas accepte.
        assert!(run(&lua, "bin", "\x1bLua").is_err());
    }
}
