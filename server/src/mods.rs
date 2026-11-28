//! Mods cote serveur : chaque mod a son script ferme (Lua ou WebAssembly, voir `shared::script`),
//! ses evenements, ses commandes, ses blocs et sa petite memoire enregistree avec le monde.
//!
//! Un script n'agit sur la partie que par les fonctions donnees ici. Pendant qu'il tourne, le serveur
//! lui est prete par un pointeur : rien d'autre ne touche au serveur tant que l'appel n'est pas revenu.
use super::*;
use shared::commands as cmd;
use shared::mods::{self as sm, BlockDef, Pack, TileDef};
use shared::script::{kind, op, Api, Script, Val};
use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;

/// Temps accorde a un script pour traiter un evenement, en millisecondes.
const EVENT_MS: u64 = 100;
/// Memoire d'un mod : nombre de cles, taille d'une cle et d'une valeur.
const STORE_KEYS: usize = 1024;
const STORE_KEY: usize = 64;
const STORE_VALUE: usize = 64 << 10;
/// Fichier du monde qui retient le numero donne a chaque bloc de mod.
const PALETTE: &str = "mods_blocs.txt";

thread_local! {
    /// Le serveur, le temps d'un appel a un script.
    static SERVER: Cell<*mut Server> = const { Cell::new(std::ptr::null_mut()) };
}

/// Donne le serveur a une fonction appelee par un script.
fn srv<R>(f: impl FnOnce(&mut Server) -> Result<R, String>) -> Result<R, String> {
    let p = SERVER.with(|s| s.get());
    if p.is_null() {
        return Err("cette fonction s'utilise dans un événement ou une commande, pas au chargement du mod".into());
    }
    // SAFETY : le pointeur n'est pose que par `Lend`, pendant que l'hote des mods detient le serveur en
    // exclusivite sans s'en servir ; l'hote est sorti du serveur, aucun appel ne s'imbrique.
    f(unsafe { &mut *p })
}

/// Pose le serveur pour la duree d'un appel, et le retire quoi qu'il arrive.
struct Lend;
impl Lend {
    fn new(s: &mut Server) -> Lend {
        SERVER.with(|c| c.set(s as *mut Server));
        Lend
    }
}
impl std::ops::Drop for Lend {
    fn drop(&mut self) {
        SERVER.with(|c| c.set(std::ptr::null_mut()));
    }
}

/// Valeur passee a un evenement.
#[derive(Clone)]
pub(crate) enum Arg {
    N(f64),
    S(String),
}

/// Bloc declare par un script, avant qu'on lui donne un numero.
struct NewBlock {
    id: String,
    name: String,
    /// Fichiers des textures : dessus, cotes, dessous.
    files: [String; 3],
    hardness: f32,
    light: u8,
    tool: u8,
    clear: bool,
    /// Mot de l'objet laisse, ou None pour le bloc lui-meme ; Some("") pour rien.
    drop: Option<String>,
    shape: u8,
}

/// Objet declare par un script.
struct NewItem {
    id: String,
    name: String,
    file: String,
    def: sm::ItemDef,
}

/// Recette declaree par un script : les objets sont encore des mots.
struct NewRecipe {
    w: u8,
    h: u8,
    cells: Vec<String>,
    out: String,
    n: u8,
}

/// Filon declare par un script.
struct NewOre {
    block: String,
    replace: String,
    def: sm::OreDef,
}

/// Creature declaree par un script : son butin est encore fait de mots.
struct NewMob {
    id: String,
    def: sm::MobDef,
    loot: Vec<(String, u8, u8)>,
}

/// Ce qu'un script a declare, et ce qu'il retient.
#[derive(Default)]
struct Reg {
    loading: bool,
    listens: HashSet<String>,
    /// (nom, aide)
    commands: Vec<(String, String)>,
    blocks: Vec<NewBlock>,
    items: Vec<NewItem>,
    recipes: Vec<NewRecipe>,
    /// (objet mis au four, objet obtenu)
    smelts: Vec<(String, String)>,
    ores: Vec<NewOre>,
    mobs: Vec<NewMob>,
    /// Regles fixees au chargement du mod, appliquees une fois la partie prete : (regle, valeur).
    rules: Vec<(String, bool)>,
    store: HashMap<String, String>,
    store_dirty: bool,
    errors: u32,
}

struct Loaded {
    pack: Pack,
    reg: Rc<RefCell<Reg>>,
    script: Option<Script>,
}

pub(crate) struct Host {
    mods: Vec<Loaded>,
    pub(crate) infos: Vec<sm::Info>,
    /// Ce que les mods ajoutent au jeu : blocs, objets, recettes, cuissons, filons.
    pub(crate) content: sm::Content,
    pub(crate) tiles: Vec<TileDef>,
}

/// Mot qui designe un bloc dans un script : celui de `/donner`, ou « mod:bloc ».
pub(crate) fn block_word(b: u8) -> String {
    if b == AIR {
        return "air".into();
    }
    sm::with(b, |d| d.key.clone()).unwrap_or_else(|| cmd::item_word(b as u16))
}

fn block_from(word: &str) -> Option<u8> {
    if word == "air" {
        return Some(AIR);
    }
    if let Some(b) = sm::block_by_key(word) {
        return Some(b);
    }
    let word = cmd::plain(word);
    (1..sm::FIRST_BLOCK).find(|b| cmd::item_word(*b as u16) == word)
}

/// Objet designe par un mot : un bloc de mod, ou ce que `/donner` comprend.
fn item_from(word: &str) -> Option<u16> {
    sm::item_by_key(word).or_else(|| cmd::item_from(word))
}

/// Mot qui designe un objet dans un script.
pub(crate) fn item_word(id: u16) -> String {
    if id < 256 {
        return block_word(id as u8);
    }
    sm::with_item(id, |d| d.key.clone()).unwrap_or_else(|| cmd::item_word(id))
}

fn num(a: &[Val], i: usize) -> Result<f64, String> {
    match a.get(i) {
        Some(Val::Num(n)) => Ok(*n),
        _ => Err(format!("valeur n° {} : un nombre est attendu", i + 1)),
    }
}

fn text(a: &[Val], i: usize) -> Result<&str, String> {
    match a.get(i) {
        Some(Val::Str(s)) => Ok(s),
        _ => Err(format!("valeur n° {} : un texte est attendu", i + 1)),
    }
}

fn pid(a: &[Val]) -> Result<u32, String> {
    num(a, 0).map(|n| n as u32)
}

fn cell(a: &[Val]) -> Result<[i32; 3], String> {
    let (x, y, z) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
    let b = shared::WORLD_BORDER;
    if !(x.is_finite() && y.is_finite() && z.is_finite()) || x.abs() > b || z.abs() > b {
        return Err("coordonnées hors du monde".into());
    }
    Ok([x.floor() as i32, y.floor() as i32, z.floor() as i32])
}

/// Lit la description d'un bloc : id, nom, dessus, cotes, dessous, durete, lumiere, outil, transparent, butin.
fn read_block(a: &[Val]) -> Result<NewBlock, String> {
    let id = text(a, 0).map_err(|_| "blocks.register : « id » manquant")?.to_string();
    if id.is_empty() || !sm::valid_id(&format!("{id}x")) {
        return Err(format!("blocks.register : id invalide « {id} » (minuscules, chiffres, tirets)"));
    }
    let file = |i: usize| match a.get(i) {
        Some(Val::Str(f)) if f.contains('/') => f.clone(),
        Some(Val::Str(f)) if !f.is_empty() => format!("textures/{f}"),
        _ => format!("textures/{id}.png"),
    };
    let tool = match a.get(7) {
        Some(Val::Str(t)) => match cmd::plain(t).as_str() {
            "pioche" | "pick" | "pickaxe" => sm::tool::PICK,
            "hache" | "axe" => sm::tool::AXE,
            "pelle" | "shovel" => sm::tool::SHOVEL,
            _ => sm::tool::NONE,
        },
        _ => sm::tool::NONE,
    };
    let drop = match a.get(9) {
        Some(Val::Bool(false)) => Some(String::new()),
        Some(Val::Str(s)) => Some(s.clone()),
        _ => None,
    };
    let name = match a.get(1) {
        Some(Val::Str(n)) if !n.trim().is_empty() => n.chars().filter(|c| !c.is_control()).take(40).collect(),
        _ => id.clone(),
    };
    Ok(NewBlock {
        name,
        files: [file(2), file(3), file(4)],
        hardness: (num(a, 5).unwrap_or(1.0) as f32).clamp(-1.0, 1000.0),
        light: num(a, 6).unwrap_or(0.0).clamp(0.0, 15.0) as u8,
        tool,
        clear: matches!(a.get(8), Some(Val::Bool(true))),
        drop,
        shape: match a.get(10) {
            Some(Val::Str(w)) if matches!(cmd::plain(w).as_str(), "plante" | "plant" | "croix" | "cross") => 1,
            _ => 0,
        },
        id,
    })
}

/// Les fonctions du jeu offertes au script du mod `mod_id` (numeros : `epicube_mod::op`).
fn api(mod_id: &str, reg: &Rc<RefCell<Reg>>) -> Api {
    let (id, reg) = (mod_id.to_string(), reg.clone());
    let nothing = || Ok(Vec::new());
    Rc::new(move |code: u32, a: &[Val]| -> Result<Vec<Val>, String> {
        match code {
            op::LOG => {
                println!("[mod {id}] {}", text(a, 0)?);
                nothing()
            }
            op::LISTEN => {
                reg.borrow_mut().listens.insert(text(a, 0)?.to_string());
                nothing()
            }
            op::COMMAND => {
                let name = cmd::plain(text(a, 0)?.trim_start_matches('/'));
                let mut r = reg.borrow_mut();
                if !r.loading {
                    return Err("une commande se déclare au chargement du mod".into());
                }
                if name.is_empty() || name.len() > 24 || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    return Err(format!("nom de commande invalide « {name} »"));
                }
                if cmd::find(&name).is_some() {
                    return Err(format!("/{name} est une commande du jeu"));
                }
                r.commands.retain(|c| c.0 != name);
                r.commands.push((name.clone(), text(a, 1).unwrap_or("").chars().take(80).collect()));
                Ok(vec![Val::Str(name)])
            }
            op::BLOCK => {
                let mut r = reg.borrow_mut();
                if !r.loading {
                    return Err("un bloc se déclare au chargement du mod".into());
                }
                let b = read_block(a)?;
                let key = format!("{id}:{}", b.id);
                r.blocks.retain(|o| o.id != b.id);
                r.blocks.push(b);
                Ok(vec![Val::Str(key)])
            }
            op::ITEM => {
                let mut r = reg.borrow_mut();
                if !r.loading {
                    return Err("un objet se déclare au chargement du mod".into());
                }
                let name = text(a, 0).map_err(|_| "items.register : « id » manquant")?.to_string();
                if name.is_empty() || !sm::valid_id(&format!("{name}x")) {
                    return Err(format!("items.register : id invalide « {name} » (minuscules, chiffres, tirets)"));
                }
                if r.blocks.iter().any(|b| b.id == name) {
                    return Err(format!("« {name} » est déjà un bloc de ce mod"));
                }
                let file = match a.get(2) {
                    Some(Val::Str(f)) if f.contains('/') => f.clone(),
                    Some(Val::Str(f)) if !f.is_empty() => format!("textures/{f}"),
                    _ => format!("textures/{name}.png"),
                };
                let tool = match a.get(8) {
                    Some(Val::Str(t)) => match cmd::plain(t).as_str() {
                        "pioche" | "pick" | "pickaxe" => sm::kind::PICK,
                        "hache" | "axe" => sm::kind::AXE,
                        "pelle" | "shovel" => sm::kind::SHOVEL,
                        "epee" | "katana" | "sword" => sm::kind::SWORD,
                        "houe" | "hoe" => sm::kind::HOE,
                        _ => sm::kind::NONE,
                    },
                    _ => sm::kind::NONE,
                };
                let f = |i: usize, default: f64, max: f64| num(a, i).unwrap_or(default).clamp(0.0, max);
                let def = sm::ItemDef {
                    stack: f(3, 64.0, 64.0).max(1.0) as u8,
                    food: f(4, 0.0, 20.0) as f32,
                    saturation: f(5, 0.0, 20.0) as f32,
                    fuel: f(6, 0.0, 100_000.0) as f32,
                    durability: f(7, 0.0, 60_000.0) as u16,
                    tool,
                    tier: f(9, 0.0, 4.0) as u8,
                    damage: f(10, 0.0, 1000.0) as f32,
                    ..sm::ItemDef::default()
                };
                let shown = match a.get(1) {
                    Some(Val::Str(n)) if !n.trim().is_empty() => n.chars().filter(|c| !c.is_control()).take(40).collect(),
                    _ => name.clone(),
                };
                let key = format!("{id}:{name}");
                r.items.retain(|o| o.id != name);
                r.items.push(NewItem { id: name, name: shown, file, def });
                Ok(vec![Val::Str(key)])
            }
            op::RECIPE => {
                let mut r = reg.borrow_mut();
                if !r.loading {
                    return Err("une recette se déclare au chargement du mod".into());
                }
                let (w, h) = (num(a, 2)? as usize, num(a, 3)? as usize);
                if !(1..=3).contains(&w) || !(1..=3).contains(&h) || a.len() != 4 + w * h {
                    return Err("recette : le dessin fait de 1 à 3 lignes de 1 à 3 cases, toutes de la même largeur".into());
                }
                let cells = a[4..].iter().map(|v| if let Val::Str(s) = v { s.clone() } else { String::new() }).collect();
                r.recipes.push(NewRecipe { w: w as u8, h: h as u8, cells, out: text(a, 0)?.to_string(), n: num(a, 1).unwrap_or(1.0).clamp(1.0, 64.0) as u8 });
                nothing()
            }
            op::SMELT => {
                let mut r = reg.borrow_mut();
                if !r.loading {
                    return Err("une cuisson se déclare au chargement du mod".into());
                }
                r.smelts.push((text(a, 0)?.to_string(), text(a, 1)?.to_string()));
                nothing()
            }
            op::ORE => {
                let mut r = reg.borrow_mut();
                if !r.loading {
                    return Err("un filon se déclare au chargement du mod".into());
                }
                let f = |i: usize, default: f64, max: f64| num(a, i).unwrap_or(default).clamp(0.0, max);
                let def = sm::OreDef { veins: f(2, 4.0, 64.0) as u8, size: f(3, 6.0, 64.0) as u8, min_y: f(4, 1.0, 383.0) as u16, max_y: f(5, 64.0, 383.0) as u16, dim: f(6, 0.0, 2.0) as u8, ..sm::OreDef::default() };
                r.ores.push(NewOre { block: text(a, 0)?.to_string(), replace: text(a, 1).unwrap_or("roche").to_string(), def });
                nothing()
            }
            op::MOB => {
                let mut r = reg.borrow_mut();
                if !r.loading {
                    return Err("une créature se déclare au chargement du mod".into());
                }
                let name = text(a, 0).map_err(|_| "mobs.register : « id » manquant")?.to_string();
                if name.is_empty() || !sm::valid_id(&format!("{name}x")) {
                    return Err(format!("mobs.register : id invalide « {name} » (minuscules, chiffres, tirets)"));
                }
                let base_word = text(a, 2).map_err(|_| "mobs.register : « base » manquant (cerf, oni, araignee...)")?;
                let base = cmd::mob_from(base_word).filter(|k| *k <= shared::mobs::BRAISIER).ok_or_else(|| format!("mobs.register : base inconnue « {base_word} »"))?;
                let f = |i: usize, default: f64, lo: f64, hi: f64| num(a, i).unwrap_or(default).clamp(lo, hi);
                let shown = match a.get(1) {
                    Some(Val::Str(n)) if !n.trim().is_empty() => n.chars().filter(|c| !c.is_control()).take(40).collect(),
                    _ => name.clone(),
                };
                let def = sm::MobDef {
                    name: shown,
                    base,
                    hp: f(3, shared::mobs::max_hp(base), 1.0, 2000.0) as f32,
                    damage: f(4, 0.0, 0.0, 100.0) as f32,
                    scale: f(5, 1.0, 0.25, 4.0) as f32,
                    color: [f(6, 1.0, 0.0, 1.0), f(7, 1.0, 0.0, 1.0), f(8, 1.0, 0.0, 1.0)].map(|c| (c * 255.0) as u8),
                    rarity: f(9, 0.0, 0.0, 1.0) as f32,
                    model: match a.get(10) {
                        Some(Val::Str(f)) if f.contains('/') => f.clone(),
                        Some(Val::Str(f)) if !f.is_empty() => format!("modeles/{f}"),
                        _ => String::new(),
                    },
                    ..sm::MobDef::default()
                };
                // Le butin suit : des triplets objet, minimum, maximum.
                let loot = a.get(11..).unwrap_or_default().chunks(3).filter_map(|c| match c {
                    [Val::Str(item), lo, hi] => {
                        let n = |v: &Val| if let Val::Num(n) = v { n.clamp(0.0, 64.0) as u8 } else { 1 };
                        Some((item.clone(), n(lo), n(hi)))
                    }
                    _ => None,
                }).collect();
                let key = format!("{id}:{name}");
                r.mobs.retain(|o| o.id != name);
                r.mobs.push(NewMob { id: name, def, loot });
                Ok(vec![Val::Str(key)])
            }
            op::COUNT => srv(|s| {
                let word = text(a, 1)?;
                let item = item_from(word).ok_or_else(|| format!("objet inconnu : {word}"))?;
                Ok(vec![Val::Num(s.clients.get(&pid(a)?).map_or(0, |c| items::count(&c.inv, item)) as f64)])
            }),
            op::TAKE => srv(|s| {
                let word = text(a, 1)?;
                let item = item_from(word).ok_or_else(|| format!("objet inconnu : {word}"))?;
                let n = num(a, 2).unwrap_or(1.0).max(1.0) as u32;
                let Some(c) = s.clients.get_mut(&pid(a)?).filter(|c| c.ready) else { return Ok(vec![Val::Bool(false)]) };
                // Tout ou rien : on ne prend que si le joueur a le compte.
                if items::count(&c.inv, item) < n {
                    return Ok(vec![Val::Bool(false)]);
                }
                items::remove(&mut c.inv, item, n);
                c.inv_dirty = true;
                Ok(vec![Val::Bool(true)])
            }),
            op::HELD => srv(|s| Ok(s.clients.get(&pid(a)?).and_then(|c| c.held()).map(|i| Val::Str(item_word(i))).into_iter().collect())),
            op::TAKE_HELD => srv(|s| {
                let Some(c) = s.clients.get_mut(&pid(a)?).filter(|c| c.ready && !c.dead) else { return Ok(Vec::new()) };
                let Some(st) = c.inv[c.sel].take() else { return Ok(Vec::new()) };
                c.inv_dirty = true;
                // Le jeton : objet, nombre, usure, enchantements. Le nom de l'objet y est en clair, pour qu'il
                // reste juste si les numeros des objets de mods changent d'une partie a l'autre.
                Ok(vec![Val::Str(item_word(st.id)), Val::Num(st.n as f64), Val::Str(format!("{}*{}*{}*{}", item_word(st.id), st.n, st.dur, st.ench.0))])
            }),
            op::GIVE_STACK => srv(|s| {
                let token = text(a, 1)?;
                let parts: Vec<&str> = token.split('*').collect();
                let stack = match parts.as_slice() {
                    [word, n, dur, ench] => item_from(word).and_then(|id| Some(Stack { id, n: n.parse::<u8>().ok()?.clamp(1, items::max_stack(id)), dur: dur.parse::<u16>().ok()?.min(items::max_dur(id)), ench: shared::ench::Ench(ench.parse().ok()?) })),
                    _ => None,
                };
                let who = pid(a)?;
                match stack {
                    Some(st) if s.clients.get(&who).is_some_and(|c| c.ready) => {
                        s.give(who, st);
                        Ok(vec![Val::Bool(true)])
                    }
                    _ => Ok(vec![Val::Bool(false)]),
                }
            }),
            op::SET_MODE => srv(|s| {
                let word = text(a, 1)?;
                let mode = cmd::index_of(&cmd::MODES, word).ok_or_else(|| format!("mode inconnu : {word}"))?;
                let who = pid(a)?;
                if s.clients.get(&who).is_some_and(|c| c.ready) {
                    s.set_mode(who, mode);
                }
                nothing()
            }),
            op::HUNGER => srv(|s| Ok(s.clients.get(&pid(a)?).map(|c| Val::Num(c.hunger)).into_iter().collect())),
            op::SET_HUNGER => srv(|s| {
                let v = num(a, 1)?;
                if let Some(c) = s.clients.get_mut(&pid(a)?).filter(|c| c.ready && v.is_finite()) {
                    c.hunger = v.clamp(0.0, 20.0);
                    c.stats_dirty = true;
                }
                nothing()
            }),
            op::SPAWN => {
                let word = text(a, 0)?.to_string();
                let (x, y, z) = (num(a, 1)?, num(a, 2)?, num(a, 3)?);
                cell(&a[1..])?;
                srv(|s| {
                    if s.mobs.len() >= 400 {
                        return Ok(vec![Val::Bool(false)]);
                    }
                    let at = Vec3::new(x, y, z);
                    if s.spawn_mod_mob(&word, at) {
                        return Ok(vec![Val::Bool(true)]);
                    }
                    let kind = cmd::mob_from(&word).ok_or_else(|| format!("créature inconnue : {word}"))?;
                    s.spawn_mob(kind, at, 0.0, shared::mobs::max_hp(kind));
                    Ok(vec![Val::Bool(true)])
                })
            }
            op::DROP => {
                let word = text(a, 0)?;
                let item = item_from(word).ok_or_else(|| format!("objet inconnu : {word}"))?;
                let n = (num(a, 1).unwrap_or(1.0).max(1.0) as u32).min(items::max_stack(item) as u32);
                let (x, y, z) = (num(a, 2)?, num(a, 3)?, num(a, 4)?);
                cell(&a[2..])?;
                srv(|s| {
                    if s.drops.len() < 2000 {
                        s.spawn_drop(Vec3::new(x, y, z), Vec3::ZERO, Stack::new(item, n as u8), 0.5);
                    }
                    nothing()
                })
            }
            op::EXPLODE => {
                let (x, y, z) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
                cell(a)?;
                let power = num(a, 3).unwrap_or(3.0).clamp(0.5, 8.0);
                srv(|s| {
                    s.explode(Vec3::new(x, y, z), power);
                    nothing()
                })
            }
            op::SOUND => {
                // Joueur vise (ou personne : tous ceux du monde), nom, volume, puis l'endroit s'il y en a un.
                let name = text(a, 1)?.chars().take(64).collect::<String>();
                let volume = num(a, 2).unwrap_or(1.0).clamp(0.0, 2.0) as f32;
                let pos = match (num(a, 3), num(a, 4), num(a, 5)) {
                    (Ok(x), Ok(y), Ok(z)) if x.is_finite() && y.is_finite() && z.is_finite() => Some([x, y, z]),
                    _ => None,
                };
                srv(|s| {
                    let msg = ServerMsg::Play { name, pos, volume };
                    match a.first() {
                        Some(Val::Num(who)) => {
                            if let Some(c) = s.clients.get(&(*who as u32)).filter(|c| c.ready) {
                                c.send(msg);
                            }
                        }
                        _ => s.broadcast(msg, None),
                    }
                    nothing()
                })
            }
            op::RULE => {
                let on = !matches!(a.get(1), Some(Val::Bool(false)));
                let name = cmd::plain(text(a, 0)?);
                if !matches!(name.as_str(), "pvp" | "combat" | "regen" | "regeneration" | "mobs" | "creatures") {
                    return Err(format!("règle inconnue : {name} (pvp, regen, mobs)"));
                }
                // Au chargement du mod, la partie n'existe pas encore : la regle attend qu'elle soit prete.
                if reg.borrow().loading {
                    reg.borrow_mut().rules.push((name, on));
                    return nothing();
                }
                srv(|s| {
                    s.set_rule(&name, on);
                    nothing()
                })
            }
            op::DAMAGE => srv(|s| {
                let dmg = num(a, 1)?;
                let who = pid(a)?;
                // Un coup que l'armure ne pare pas : la bordure d'un UHC, un piege.
                if let Some(c) = s.clients.get_mut(&who).filter(|c| c.ready && !c.dead && !shared::rules::invulnerable(c.mode) && dmg.is_finite() && dmg > 0.0) {
                    c.hp -= dmg.min(1000.0);
                    c.stats_dirty = true;
                }
                nothing()
            }),
            op::SET_HOME => srv(|s| {
                let (x, y, z) = (num(a, 1)?, num(a, 2)?, num(a, 3)?);
                cell(&a[1..])?;
                if let Some(c) = s.clients.get_mut(&pid(a)?).filter(|c| c.ready) {
                    c.home = Some([x, y, z]);
                }
                nothing()
            }),
            op::CLEAR => srv(|s| {
                if let Some(c) = s.clients.get_mut(&pid(a)?).filter(|c| c.ready) {
                    c.inv = [None; 36];
                    c.armor = [None; 4];
                    (c.cursor, c.craft) = (None, [None; 9]);
                    c.inv_dirty = true;
                }
                nothing()
            }),
            op::TOP => {
                let (x, z) = (num(a, 0)?, num(a, 1)?);
                let p = cell(&[Val::Num(x), Val::Num(0.0), Val::Num(z)])?;
                srv(|s| {
                    s.world.ensure((p[0] >> 4, p[2] >> 4));
                    let top = (1..shared::chunk::H as i32).rev().find(|&y| s.world.get([p[0], y, p[2]]).is_some_and(|(b, m)| blocks::solid(b, m)));
                    Ok(top.map(|y| Val::Num(y as f64)).into_iter().collect())
                })
            }
            op::FILL => {
                let (p, q) = (cell(a)?, cell(a.get(3..).unwrap_or_default())?);
                let word = text(a, 6)?;
                let b = block_from(word).ok_or_else(|| format!("bloc inconnu : {word}"))?;
                let (lo, hi) = ([p[0].min(q[0]), p[1].min(q[1]), p[2].min(q[2])], [p[0].max(q[0]), p[1].max(q[1]), p[2].max(q[2])]);
                let volume = (0..3).map(|k| (hi[k] - lo[k] + 1) as i64).product::<i64>();
                if volume > 200_000 {
                    return Err("world.fill : 200 000 blocs au plus d'un coup".into());
                }
                srv(|s| {
                    let mut n = 0;
                    for cx in (lo[0] >> 4)..=(hi[0] >> 4) {
                        for cz in (lo[2] >> 4)..=(hi[2] >> 4) {
                            s.world.ensure((cx, cz));
                        }
                    }
                    for y in lo[1]..=hi[1] {
                        for z in lo[2]..=hi[2] {
                            for x in lo[0]..=hi[0] {
                                n += s.world.set([x, y, z], b, 0) as u32;
                            }
                        }
                    }
                    Ok(vec![Val::Num(n as f64)])
                })
            }
            op::INFO => {
                let line: String = text(a, 1)?.chars().filter(|c| !c.is_control()).take(100).collect();
                srv(|s| {
                    match a.first() {
                        Some(Val::Num(who)) => s.info(*who as u32, &line),
                        _ => s.broadcast_all(ServerMsg::Info { text: line }, None),
                    }
                    nothing()
                })
            }
            op::DIMENSION => srv(|s| Ok(s.clients.get(&pid(a)?).map(|c| Val::Num(c.dim as f64)).into_iter().collect())),
            op::STORE_GET => Ok(reg.borrow().store.get(text(a, 0)?).map(|v| Val::Str(v.clone())).into_iter().collect()),
            op::STORE_SET => {
                let key = text(a, 0)?.to_string();
                let mut r = reg.borrow_mut();
                match a.get(1) {
                    Some(Val::Str(v)) => {
                        if key.len() > STORE_KEY || v.len() > STORE_VALUE || (r.store.len() >= STORE_KEYS && !r.store.contains_key(&key)) {
                            return Err("store.set : clé ou valeur trop grande, ou mémoire du mod pleine".into());
                        }
                        r.store.insert(key, v.clone());
                    }
                    Some(Val::Num(n)) => {
                        r.store.insert(key, n.to_string());
                    }
                    _ => {
                        r.store.remove(&key);
                    }
                }
                r.store_dirty = true;
                nothing()
            }
            op::SAY => srv(|s| {
                s.announce(text(a, 0)?, shared::proto::chat::INFO);
                nothing()
            }),
            op::TELL => srv(|s| {
                s.tell(pid(a)?, text(a, 1)?, shared::proto::chat::INFO);
                nothing()
            }),
            op::PLAYERS => srv(|s| {
                let mut list: Vec<(u32, String)> = s.clients.iter().filter(|(_, c)| c.ready).map(|(id, c)| (*id, c.name.clone())).collect();
                list.sort();
                Ok(list.into_iter().flat_map(|(id, name)| [Val::Num(id as f64), Val::Str(name)]).collect())
            }),
            op::ONLINE => srv(|s| Ok(vec![Val::Bool(s.clients.get(&pid(a)?).is_some_and(|c| c.ready))])),
            op::POS => srv(|s| Ok(s.clients.get(&pid(a)?).filter(|c| c.ready).map(|c| c.pos.map(Val::Num).to_vec()).unwrap_or_default())),
            op::TELEPORT => srv(|s| {
                let (x, y, z) = (num(a, 1)?, num(a, 2)?, num(a, 3)?);
                let b = shared::WORLD_BORDER - 1.0;
                if !(x.is_finite() && y.is_finite() && z.is_finite()) || x.abs() > b || z.abs() > b || !(-64.0..=1024.0).contains(&y) {
                    return Err("teleport : coordonnées hors du monde".into());
                }
                let Some(c) = s.clients.get_mut(&pid(a)?).filter(|c| c.ready) else { return Ok(vec![Val::Bool(false)]) };
                let to = [x, y, z];
                (c.pos, c.prev, c.fall_peak, c.moved) = (to, to, y, true);
                c.send(ServerMsg::Teleport { pos: to });
                Ok(vec![Val::Bool(true)])
            }),
            op::GIVE => srv(|s| {
                let word = text(a, 1)?;
                let item = item_from(word).ok_or_else(|| format!("objet inconnu : {word}"))?;
                let who = pid(a)?;
                if !s.clients.get(&who).is_some_and(|c| c.ready) {
                    return Ok(vec![Val::Bool(false)]);
                }
                let per = items::max_stack(item) as u32;
                let mut left = (num(a, 2).unwrap_or(1.0).max(1.0) as u32).min(2304);
                // Un objet donne deja enchante : player:give(objet, 1, "recul", 2).
                let mut ench = shared::ench::Ench::NONE;
                if let Ok(word) = text(a, 3) {
                    let which = cmd::ench_from(word).ok_or_else(|| format!("enchantement inconnu : {word}"))?;
                    if shared::ench::fits(which, item) {
                        ench = ench.with(which, (num(a, 4).unwrap_or(1.0).max(1.0) as u8).min(15));
                    }
                }
                while left > 0 {
                    let take = left.min(per);
                    s.give(who, Stack { ench, ..Stack::new(item, take as u8) });
                    left -= take;
                }
                Ok(vec![Val::Bool(true)])
            }),
            op::CHEST => {
                let p = cell(a)?;
                let word = text(a, 3)?;
                let item = item_from(word).ok_or_else(|| format!("objet inconnu : {word}"))?;
                let n = (num(a, 4).unwrap_or(1.0).max(1.0) as u32).min(items::max_stack(item) as u32) as u8;
                srv(|s| {
                    s.world.ensure((p[0] >> 4, p[2] >> 4));
                    if s.world.get(p).map(|c| c.0) != Some(blocks::CHEST) {
                        s.world.set(p, blocks::CHEST, 0);
                        s.world.chests.insert(p, vec![None; 27]);
                    }
                    // Dans une case libre prise au hasard, comme un butin qu'on decouvre.
                    let free: Vec<usize> = s.world.chests.entry(p).or_insert_with(|| vec![None; 27]).iter().enumerate().filter(|(_, c)| c.is_none()).map(|(i, _)| i).collect();
                    if free.is_empty() {
                        return Ok(vec![Val::Bool(false)]);
                    }
                    let slot = free[s.rng.range(0, free.len() as i32) as usize];
                    if let Some(chest) = s.world.chests.get_mut(&p) {
                        chest[slot] = Some(Stack::new(item, n));
                    }
                    s.cont_dirty.insert(p);
                    Ok(vec![Val::Bool(true)])
                })
            }
            op::ENCHANT => srv(|s| {
                let word = text(a, 1)?;
                let which = cmd::ench_from(word).ok_or_else(|| format!("enchantement inconnu : {word}"))?;
                let level = (num(a, 2).unwrap_or(1.0).max(1.0) as u8).min(shared::ench::DEFS[which as usize].max);
                let Some(c) = s.clients.get_mut(&pid(a)?) else { return Ok(vec![Val::Bool(false)]) };
                let done = match c.inv[c.sel].as_mut() {
                    Some(st) if shared::ench::fits(which, st.id) => {
                        st.ench = st.ench.with(which, level);
                        true
                    }
                    _ => false,
                };
                c.inv_dirty |= done;
                Ok(vec![Val::Bool(done)])
            }),
            op::HEALTH => srv(|s| Ok(s.clients.get(&pid(a)?).map(|c| Val::Num(c.hp)).into_iter().collect())),
            op::SET_HEALTH => srv(|s| {
                let v = num(a, 1)?;
                if let Some(c) = s.clients.get_mut(&pid(a)?).filter(|c| c.ready && !c.dead && v.is_finite()) {
                    c.hp = v.clamp(0.0, 20.0);
                    c.stats_dirty = true;
                }
                nothing()
            }),
            op::MODE => srv(|s| Ok(s.clients.get(&pid(a)?).map(|c| Val::Str(cmd::MODES[(c.mode as usize).min(3)].to_string())).into_iter().collect())),
            op::KICK => srv(|s| {
                // Le joueur sort une fois le script revenu : le serveur ne change pas sous ses pieds.
                let reason = text(a, 1).unwrap_or("");
                s.mod_kicks.push((pid(a)?, if reason.is_empty() { "exclu par un mod du serveur".into() } else { reason.to_string() }));
                nothing()
            }),
            op::SEND => {
                let (name, data) = (text(a, 1)?, text(a, 2).unwrap_or(""));
                if name.len() > sm::MSG_NAME || data.len() > sm::MSG_DATA {
                    return Err("send : message trop long".into());
                }
                srv(|s| {
                    if let Some(c) = s.clients.get(&pid(a)?).filter(|c| c.ready) {
                        c.send(ServerMsg::ModMsg { mod_id: id.clone(), name: name.to_string(), data: data.to_string() });
                    }
                    nothing()
                })
            }
            op::GET_BLOCK => {
                let p = cell(a)?;
                srv(|s| {
                    s.world.ensure((p[0] >> 4, p[2] >> 4));
                    Ok(s.world.get(p).map(|(b, _)| Val::Str(block_word(b))).into_iter().collect())
                })
            }
            op::SET_BLOCK => {
                let p = cell(a)?;
                let word = text(a, 3)?;
                let b = block_from(word).ok_or_else(|| format!("bloc inconnu : {word}"))?;
                srv(|s| {
                    // Le monde se charge la ou le script ecrit : il peut batir une arene loin de tout joueur.
                    s.world.ensure((p[0] >> 4, p[2] >> 4));
                    let Some((old, _)) = s.world.get(p) else { return Ok(vec![Val::Bool(false)]) };
                    if old != b && old != AIR {
                        // Un coffre rend son contenu, un poussoir part avec sa tete.
                        s.break_block(p, false);
                    }
                    Ok(vec![Val::Bool(old == b || b == AIR || s.world.set(p, b, 0))])
                })
            }
            op::TIME => srv(|s| Ok(vec![Val::Num((s.time * 24.0 + 6.0).rem_euclid(24.0))])),
            op::SET_TIME => srv(|s| {
                let hour = num(a, 0)?;
                if hour.is_finite() {
                    s.time = ((hour - 6.0) / 24.0).rem_euclid(1.0);
                    s.world.time = s.time;
                    s.broadcast_all(ServerMsg::Time { time: s.time }, None);
                }
                nothing()
            }),
            _ => Err(format!("fonction n° {code} inconnue de cette version du jeu")),
        }
    })
}

/// Ce qui, en Lua, habille les fonctions numerotees du jeu. Rend la fonction qui recoit evenements et commandes.
fn prelude() -> String {
    let ops = [
        ("LOG", op::LOG), ("LISTEN", op::LISTEN), ("COMMAND", op::COMMAND), ("BLOCK", op::BLOCK), ("STORE_GET", op::STORE_GET),
        ("STORE_SET", op::STORE_SET), ("SAY", op::SAY), ("TELL", op::TELL), ("PLAYERS", op::PLAYERS), ("ONLINE", op::ONLINE),
        ("POS", op::POS), ("TELEPORT", op::TELEPORT), ("GIVE", op::GIVE), ("HEALTH", op::HEALTH), ("SET_HEALTH", op::SET_HEALTH),
        ("MODE", op::MODE), ("KICK", op::KICK), ("SEND", op::SEND), ("GET_BLOCK", op::GET_BLOCK), ("SET_BLOCK", op::SET_BLOCK),
        ("TIME", op::TIME), ("SET_TIME", op::SET_TIME), ("ITEM", op::ITEM), ("RECIPE", op::RECIPE), ("SMELT", op::SMELT), ("ORE", op::ORE),
        ("COUNT", op::COUNT), ("TAKE", op::TAKE), ("HELD", op::HELD), ("SET_MODE", op::SET_MODE), ("HUNGER", op::HUNGER),
        ("SET_HUNGER", op::SET_HUNGER), ("SPAWN", op::SPAWN), ("DROP", op::DROP), ("EXPLODE", op::EXPLODE), ("DIMENSION", op::DIMENSION),
        ("MOB", op::MOB), ("SOUND", op::SOUND),
        ("RULE", op::RULE), ("DAMAGE", op::DAMAGE), ("SET_HOME", op::SET_HOME), ("CLEAR", op::CLEAR), ("TOP", op::TOP), ("FILL", op::FILL), ("INFO", op::INFO),
        ("CHEST", op::CHEST), ("ENCHANT", op::ENCHANT), ("TAKE_HELD", op::TAKE_HELD), ("GIVE_STACK", op::GIVE_STACK),
    ];
    let table: Vec<String> = ops.iter().map(|(n, v)| format!("{n}={v}")).collect();
    format!("local OP = {{{}}}\n{PRELUDE}", table.join(","))
}

/// Nombre d'erreurs de script rencontrees depuis le lancement : les essais s'assurent qu'un jeu n'en fait pas.
pub static SCRIPT_ERRORS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

const PRELUDE: &str = r##"
local call = _call
_call = nil
local handlers, commands = {}, {}

function on(event, fn)
  if type(event) ~= "string" or type(fn) ~= "function" then error("on(evenement, fonction)", 2) end
  local list = handlers[event]
  if not list then
    list = {}
    handlers[event] = list
    call(OP.LISTEN, event)
  end
  list[#list + 1] = fn
end

function command(name, help, fn)
  if type(help) == "function" then fn, help = help, "" end
  if type(name) ~= "string" or type(fn) ~= "function" then error("command(nom, aide, fonction)", 2) end
  commands[call(OP.COMMAND, name, tostring(help))] = fn
end

function print(...)
  local t = {}
  for i = 1, select("#", ...) do t[i] = tostring((select(i, ...))) end
  call(OP.LOG, table.concat(t, " "))
end

local Player = {}
Player.__index = Player
local function wrap(id, name) return setmetatable({ id = id, name = name }, Player) end
function Player:pos() return call(OP.POS, self.id) end
function Player:teleport(x, y, z) return call(OP.TELEPORT, self.id, x, y, z) end
function Player:give(item, n) return call(OP.GIVE, self.id, item, n or 1) end
function Player:tell(text) call(OP.TELL, self.id, tostring(text)) end
function Player:health() return call(OP.HEALTH, self.id) end
function Player:set_health(v) call(OP.SET_HEALTH, self.id, v) end
function Player:mode() return call(OP.MODE, self.id) end
function Player:kick(reason) call(OP.KICK, self.id, tostring(reason or "")) end
function Player:send(name, data) call(OP.SEND, self.id, tostring(name), tostring(data or "")) end
function Player:online() return call(OP.ONLINE, self.id) end
function Player:count(item) return call(OP.COUNT, self.id, item) end
function Player:give_enchanted(item, enchant, level) return call(OP.GIVE, self.id, item, 1, enchant, level or 1) end
function Player:take(item, n) return call(OP.TAKE, self.id, item, n or 1) end
function Player:held() return call(OP.HELD, self.id) end
function Player:set_mode(mode) call(OP.SET_MODE, self.id, mode) end
function Player:hunger() return call(OP.HUNGER, self.id) end
function Player:set_hunger(v) call(OP.SET_HUNGER, self.id, v) end
function Player:dimension() return call(OP.DIMENSION, self.id) end
function Player:sound(name, volume) call(OP.SOUND, self.id, name, volume or 1) end
function Player:damage(amount) call(OP.DAMAGE, self.id, amount) end
function Player:set_home(x, y, z) call(OP.SET_HOME, self.id, x, y, z) end
function Player:clear() call(OP.CLEAR, self.id) end
function Player:info(text) call(OP.INFO, self.id, tostring(text)) end
function Player:enchant(name, level) return call(OP.ENCHANT, self.id, name, level or 1) end
-- Prend la pile tenue : rend son nom, son nombre et un jeton a rendre a give_stack (usure et enchantements gardes).
function Player:take_held() return call(OP.TAKE_HELD, self.id) end
function Player:give_stack(token) return call(OP.GIVE_STACK, self.id, tostring(token)) end

function players()
  local flat, out = { call(OP.PLAYERS) }, {}
  for i = 1, #flat, 2 do out[#out + 1] = wrap(flat[i], flat[i + 1]) end
  return out
end
function say(text) call(OP.SAY, tostring(text)) end
-- Message au centre de l'ecran de tous les joueurs.
function announce(text) call(OP.INFO, nil, tostring(text)) end
-- Regles de la partie : rules.set("pvp", false), rules.set("regen", false), rules.set("mobs", false).
rules = { set = function(name, on) call(OP.RULE, name, on ~= false) end }

world = {
  get = function(x, y, z) return call(OP.GET_BLOCK, x, y, z) end,
  set = function(x, y, z, block) return call(OP.SET_BLOCK, x, y, z, block) end,
  time = function() return call(OP.TIME) end,
  set_time = function(hour) call(OP.SET_TIME, hour) end,
  spawn = function(mob, x, y, z) return call(OP.SPAWN, mob, x, y, z) end,
  drop = function(item, n, x, y, z) call(OP.DROP, item, n, x, y, z) end,
  explode = function(x, y, z, power) call(OP.EXPLODE, x, y, z, power or 3) end,
  sound = function(name, x, y, z, volume) call(OP.SOUND, nil, name, volume or 1, x, y, z) end,
  top = function(x, z) return call(OP.TOP, x, z) end,
  fill = function(x1, y1, z1, x2, y2, z2, block) return call(OP.FILL, x1, y1, z1, x2, y2, z2, block) end,
  chest = function(x, y, z, item, n) return call(OP.CHEST, x, y, z, item, n or 1) end,
}
items = {
  register = function(i)
    if type(i) ~= "table" then error("items.register{ id = ..., ... }", 2) end
    return call(OP.ITEM, i.id, i.name, i.texture, i.stack, i.food, i.saturation, i.fuel, i.durability, i.tool, i.tier, i.damage)
  end,
}
recipes = {
  -- recipes.shaped{ output = "mod:objet", count = 1, pattern = { "RRR", " B ", " B " }, key = { R = "mod:rubis", B = "baton" } }
  shaped = function(r)
    if type(r) ~= "table" or type(r.pattern) ~= "table" or type(r.key) ~= "table" then error("recipes.shaped{ output, pattern, key }", 2) end
    local w, cells = #r.pattern[1], {}
    for _, line in ipairs(r.pattern) do
      if #line ~= w then error("recipes.shaped : toutes les lignes du dessin ont la même largeur", 2) end
      for i = 1, w do
        local c = line:sub(i, i)
        if c == " " then cells[#cells + 1] = "" else
          cells[#cells + 1] = r.key[c] or error("recipes.shaped : la lettre « " .. c .. " » n'est pas dans key", 2)
        end
      end
    end
    call(OP.RECIPE, r.output, r.count or 1, w, #r.pattern, table.unpack(cells))
  end,
  smelt = function(r) call(OP.SMELT, r.input, r.output) end,
}
mobs = {
  -- mobs.register{ id = "golem", name = "Golem", base = "oni", health = 60, damage = 8, scale = 1.5,
  --                color = { 0.6, 0.6, 0.7 }, rarity = 0.05, loot = { { item = "fer", min = 1, max = 3 } } }
  register = function(m)
    if type(m) ~= "table" then error("mobs.register{ id = ..., base = ..., ... }", 2) end
    local c, flat = m.color or {}, {}
    for _, l in ipairs(m.loot or {}) do
      flat[#flat + 1] = l.item
      flat[#flat + 1] = l.min or 1
      flat[#flat + 1] = l.max or l.min or 1
    end
    return call(OP.MOB, m.id, m.name, m.base, m.health, m.damage, m.scale, c[1], c[2], c[3], m.rarity, m.model, table.unpack(flat))
  end,
}
worldgen = {
  ore = function(o) call(OP.ORE, o.block, o.replace or "roche", o.veins, o.size, o.min_y, o.max_y, o.dimension) end,
}
blocks = {
  register = function(b)
    if type(b) ~= "table" then error("blocks.register{ id = ..., ... }", 2) end
    local t = b.texture
    local top, side, bottom
    if type(t) == "table" then
      side = t.side
      top = t.top or side
      bottom = t.bottom or top
    else
      top, side, bottom = t, t, t
    end
    return call(OP.BLOCK, b.id, b.name, top, side, bottom, b.hardness, b.light, b.tool, b.transparent == true, b.drop, b.shape)
  end,
}
store = {
  get = function(key) return call(OP.STORE_GET, tostring(key)) end,
  set = function(key, value) call(OP.STORE_SET, tostring(key), value ~= nil and tostring(value) or nil) end,
}

local function emit(event, pid, pname, ...)
  local list = handlers[event]
  if not list then return true end
  local ok = true
  for i = 1, #list do
    local r
    if pid then r = list[i](wrap(pid, pname), ...) else r = list[i](...) end
    if r == false then ok = false end
  end
  return ok
end
-- ---------------------------------------------------------------------------------------------
-- minigame{...} : la base commune d'un mini-jeu. Elle tient l'attente des joueurs, le compte a
-- rebours, les equipes, qui est encore en jeu, l'etat affiche a l'ecran et la victoire ; le jeu
-- n'ecrit que ce qui lui est propre.
--
--   local g = minigame{ id = "pitchout", name = "PitchOut", min = 2, countdown = 15,
--     teams = { "Rouge", "Bleue" },          -- facultatif : sans equipes, chacun pour soi
--     height = 250,                          -- altitude de l'arene (g.oy)
--     build = function(g) ... end,           -- batit la carte autour de g.ox, g.oy, g.oz
--     spawn = function(g, p, i, team) ... end, -- place le joueur numero i au depart
--     start = function(g) ... end,           -- une fois tout le monde en place
--     second = function(g, clock) ... end,   -- chaque seconde de la partie
--     death = function(g, p, killer) ... end,  -- rend vrai si le joueur reste en jeu (il reviendra)
--     respawn = function(g, p) ... end,      -- retour en jeu d'un joueur reste en lice
--     status = function(g, p) return "..." end,
--     limit = 600, timeout = function(g) ... end }  -- fin de partie au temps
--
-- g.phase ("attente", "jeu", "fin"), g.clock, g.count, g:alive(p), g:team(p), g:out(p, pourquoi),
-- g:win(texte), g:players() (ceux en jeu), g:mates(n) (nombre de joueurs de l'equipe n).
function minigame(def)
  local g = { def = def, phase = "attente", clock = 0, ticks = 0, wait = def.countdown or 15,
    ox = 0, oy = def.height or 250, oz = 0, count = 0, started = 0, kills = {}, data = {} }
  local alive, team_of = {}, {}
  local min = def.min or 2
  function g:alive(p) return alive[p.name] == true end
  function g:team(p) return team_of[p.name] end
  function g:team_name(n) return def.teams and def.teams[n] or "" end
  function g:players()
    local out = {}
    for _, p in ipairs(players()) do if alive[p.name] then out[#out + 1] = p end end
    return out
  end
  function g:mates(n)
    local c = 0
    for name, t in pairs(team_of) do if t == n and alive[name] then c = c + 1 end end
    return c
  end
  -- Place numero i sur un cercle de rayon r autour du centre : x, z.
  function g:ring(i, r)
    local a = (i - 1) / math.max(1, g.total or 1) * 2 * math.pi
    return g.ox + math.floor(math.cos(a) * r + 0.5), g.oz + math.floor(math.sin(a) * r + 0.5)
  end
  -- Un tirage de 1 a n, reproductible : les scripts n'ont pas d'horloge a eux.
  function g:roll(n)
    g.seed = ((g.seed or 7) * 1103515245 + 12345 + g.ticks) % 2147483648
    return g.seed % n + 1
  end
  function g:win(text)
    if g.phase == "fin" then return end
    g.phase = "fin"
    rules.set("pvp", false)
    announce(text)
    say(text)
  end
  -- Reste-t-il de quoi jouer ? La derniere equipe, ou le dernier joueur, l'emporte.
  function g:check()
    if g.phase ~= "jeu" or def.endless then return end
    if def.teams then
      local standing, last = 0, nil
      for n = 1, #def.teams do if g:mates(n) > 0 then standing, last = standing + 1, n end end
      if standing <= 1 then g:win(last and ("L'equipe " .. def.teams[last] .. " remporte la partie de " .. def.name .. " !") or (def.name .. " : partie terminee.")) end
    elseif g.count <= 1 and g.started > 1 then
      local winner
      for name in pairs(alive) do winner = name end
      g:win(winner and (winner .. " remporte la partie de " .. def.name .. " !") or (def.name .. " : personne n'a gagne."))
    end
  end
  function g:out(p, why)
    if g.phase ~= "jeu" or not alive[p.name] then return end
    alive[p.name] = nil
    g.count = g.count - 1
    say(p.name .. " est elimine" .. (why or "") .. ". Il reste " .. g.count .. " joueur" .. (g.count > 1 and "s" or "") .. ".")
    g:check()
  end
  function g:begin()
    local list = players()
    if #list == 0 then return end
    local x, _, z = list[1]:pos()
    g.ox, g.oz = math.floor(x or 0), math.floor(z or 0)
    alive, team_of, g.count, g.kills, g.data = {}, {}, 0, {}, {}
    g.total = math.min(#list, def.places or 64)
    if def.build then def.build(g) end
    local places = def.places or 64
    for i, p in ipairs(list) do
      if i <= places then
        local team = def.teams and ((i - 1) % #def.teams + 1) or nil
        team_of[p.name], alive[p.name] = team, true
        g.count = g.count + 1
        p:clear()
        p:set_mode(def.mode or "aventure")
        p:set_health(20)
        p:set_hunger(20)
        if def.spawn then def.spawn(g, p, i, team) end
        if team then p:tell("Vous etes dans l'equipe " .. def.teams[team] .. ".") end
      else
        p:set_mode("spectateur")
      end
    end
    g.started, g.phase, g.clock = g.count, "jeu", 0
    rules.set("pvp", def.pvp ~= false)
    announce(def.name .. " : c'est parti !")
    if def.start then def.start(g) end
  end
  local function status(p)
    if g.phase == "attente" then
      local n = #players()
      if n < min then return def.name .. " - en attente de joueurs (" .. n .. "/" .. min .. ")" end
      return def.name .. " - depart dans " .. g.wait .. " s"
    elseif g.phase == "jeu" then
      return def.name .. " - " .. (def.status and def.status(g, p) or (g.count .. " en jeu"))
    end
    return def.name .. " - partie terminee"
  end
  rules.set("pvp", false)
  rules.set("mobs", false)
  on("join", function(p)
    if g.phase == "attente" then
      p:set_mode("aventure")
      p:tell("Bienvenue dans " .. def.name .. " ! La partie commence des que " .. min .. " joueurs sont la.")
    else
      p:set_mode("spectateur")
      p:tell("La partie est en cours : vous la regardez en spectateur.")
    end
  end)
  on("leave", function(p) g:out(p, " (parti)") end)
  -- Pas de coup entre coequipiers (sauf si le jeu le veut : friendly_fire = true).
  on("hit", function(p, victim)
    if def.teams and not def.friendly_fire and team_of[p.name] and team_of[p.name] == team_of[victim] then return false end
  end)
  on("death", function(p, killer)
    if g.phase ~= "jeu" or not alive[p.name] then return end
    local by = killer ~= "" and killer or nil
    if by then g.kills[by] = (g.kills[by] or 0) + 1 end
    if def.death and def.death(g, p, by) then return end
    g:out(p, by and (" par " .. by) or "")
  end)
  on("respawn", function(p)
    if g.phase == "attente" then return end
    if alive[p.name] and def.respawn then
      p:set_mode(def.mode or "aventure")
      def.respawn(g, p)
    else
      p:set_mode("spectateur")
      p:teleport(g.ox + 0.5, g.oy + 12, g.oz + 0.5)
    end
  end)
  on("tick", function()
    g.ticks = g.ticks + 1
    if g.phase == "jeu" and def.tick then def.tick(g) end
    if g.ticks % 20 ~= 0 then return end
    local list = players()
    if g.phase == "attente" then
      if #list >= min then
        g.wait = g.wait - 1
        if g.wait <= 0 then g:begin() elseif g.wait <= 5 or g.wait % 5 == 0 then announce("Depart dans " .. g.wait .. " s") end
      else
        g.wait = def.countdown or 15
      end
    elseif g.phase == "jeu" then
      g.clock = g.clock + 1
      -- Tomber dans le vide, c'est mourir.
      if def.void then
        for _, p in ipairs(list) do
          local _, y = p:pos()
          if alive[p.name] and y and y < g.oy - def.void then p:damage(1000) end
        end
      end
      if def.second then def.second(g, g.clock) end
      if def.limit and g.clock >= def.limit and g.phase == "jeu" then
        if def.timeout then def.timeout(g) else g:win(def.name .. " : temps ecoule.") end
      end
    end
    for _, p in ipairs(list) do p:send("etat", status(p)) end
  end)
  command(def.id, "Lance la partie sans attendre (avec « demarrer »), ou dit ou elle en est", function(p, args)
    if args == "demarrer" and g.phase == "attente" then
      g:begin()
      return def.name .. " lance."
    end
    return status(p)
  end)
  return g
end

-- Une ile ronde dans le ciel : un plateau et un socle qui s'effile vers le bas.
function world.island(x, y, z, r, block)
  for d = 0, math.min(3, r) do
    world.fill(x - r + d, y - d, z - r + d, x + r - d, y - d, z + r - d, d == 0 and (block or "pierre") or "pierre")
  end
end

return function(kind, name, pid, pname, ...)
  if kind == 0 then return emit(name, pid, pname, ...) end
  local fn = commands[name]
  if not fn then return nil end
  local reply = fn(wrap(pid, pname), ...)
  if reply ~= nil then return tostring(reply) end
end
"##;

impl Loaded {
    fn new(pack: Pack, save_dir: Option<&Path>) -> Loaded {
        let id = pack.manifest.id.clone();
        let reg = Rc::new(RefCell::new(Reg { loading: true, ..Reg::default() }));
        if let Some(text) = save_dir.and_then(|d| std::fs::read_to_string(d.join("mods").join(format!("{id}.json"))).ok()) {
            reg.borrow_mut().store = serde_json::from_str(&text).unwrap_or_default();
        }
        // Un mod Rust (WebAssembly) l'emporte sur un script Lua du meme mod.
        let (file, built) = if let Some(bytes) = pack.file("serveur.wasm") {
            ("serveur.wasm", Some(Script::wasm(bytes, api(&id, &reg))))
        } else if let Some(source) = pack.text("serveur.lua") {
            ("serveur.lua", Some(Script::lua(&format!("{id}/serveur.lua"), &prelude(), source, api(&id, &reg))))
        } else {
            ("", None)
        };
        let script = match built {
            Some(Ok(s)) => Some(s),
            Some(Err(e)) => {
                eprintln!("[mod {id}] {file} n'a pas pu être chargé : {e}");
                let mut r = reg.borrow_mut();
                r.blocks.clear();
                r.items.clear();
                r.recipes.clear();
                r.smelts.clear();
                r.ores.clear();
                r.mobs.clear();
                r.commands.clear();
                r.listens.clear();
                None
            }
            None => None,
        };
        reg.borrow_mut().loading = false;
        Loaded { pack, reg, script }
    }

    fn failed(&self, what: &str, e: &str) {
        let mut r = self.reg.borrow_mut();
        r.errors += 1;
        // Un script qui echoue a chaque tick ne noie pas le journal.
        if r.errors <= 20 {
            SCRIPT_ERRORS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            eprintln!("[mod {}] {what} : {e}", self.pack.manifest.id);
        }
    }
}

impl Host {
    /// Charge les mods, donne un numero a leurs blocs et a leurs objets ; `save_dir` : le dossier du monde.
    pub(crate) fn load(packs: Vec<Pack>, save_dir: Option<&Path>) -> Host {
        let mods: Vec<Loaded> = packs.into_iter().map(|p| Loaded::new(p, save_dir)).collect();
        // Les numeros deja donnes dans ce monde sont repris : un bloc pose, un objet range gardent leur sens.
        let mut palette: Vec<(String, u16)> = save_dir
            .and_then(|d| std::fs::read_to_string(d.join(PALETTE)).ok())
            .map(|text| text.lines().filter_map(|l| l.split_once('=')).filter_map(|(k, v)| Some((k.trim().to_string(), v.trim().parse().ok()?))).collect())
            .unwrap_or_default();
        let known = palette.len();
        let mut content = sm::Content::default();
        let mut tiles: Vec<TileDef> = Vec::new();
        // Numero d'un nom complet : celui du monde, ou le premier libre de la plage donnee.
        let number = |palette: &mut Vec<(String, u16)>, key: &str, range: std::ops::RangeInclusive<u16>| -> Option<u16> {
            if let Some((_, id)) = palette.iter().find(|(k, id)| k == key && range.contains(id)) {
                return Some(*id);
            }
            let id = range.clone().find(|id| !palette.iter().any(|(_, used)| used == id))?;
            palette.push((key.to_string(), id));
            Some(id)
        };
        let tile_of = |tiles: &mut Vec<TileDef>, m: &Loaded, file: &String| -> u16 {
            let mod_id = &m.pack.manifest.id;
            if let Some(t) = tiles.iter().find(|t| t.mod_id == *mod_id && t.file == *file) {
                return t.tile;
            }
            if m.pack.file(file).is_none() {
                eprintln!("[mod {mod_id}] texture absente : {file}");
                return blocks::t::WHITE;
            }
            if tiles.len() >= sm::TILES as usize {
                eprintln!("[mod {mod_id}] trop de textures de mods ({} au plus) : {file} ignorée", sm::TILES);
                return blocks::t::WHITE;
            }
            let tile = sm::FIRST_TILE + tiles.len() as u16;
            tiles.push(TileDef { tile, mod_id: mod_id.clone(), file: file.clone() });
            tile
        };
        // Les mots des objets laisses se resolvent une fois tous les blocs et objets connus.
        let mut drops: Vec<(usize, String, String)> = Vec::new();
        for m in &mods {
            let mod_id = &m.pack.manifest.id;
            let r = m.reg.borrow();
            for nb in &r.blocks {
                let key = format!("{mod_id}:{}", nb.id);
                let Some(id) = number(&mut palette, &key, sm::FIRST_BLOCK as u16..=255) else {
                    eprintln!("[mod {mod_id}] bloc « {} » ignoré : ce monde a déjà {} blocs de mods", nb.id, 256 - sm::FIRST_BLOCK as usize);
                    continue;
                };
                let t = [tile_of(&mut tiles, m, &nb.files[0]), tile_of(&mut tiles, m, &nb.files[1]), tile_of(&mut tiles, m, &nb.files[2])];
                if let Some(word) = &nb.drop {
                    drops.push((content.blocks.len(), word.clone(), mod_id.clone()));
                }
                content.blocks.push(BlockDef { id: id as u8, key, name: nb.name.clone(), tiles: t, hardness: nb.hardness, light: nb.light, tool: nb.tool, clear: nb.clear, drop: 0, shape: nb.shape });
            }
            for ni in &r.items {
                let key = format!("{mod_id}:{}", ni.id);
                let Some(id) = number(&mut palette, &key, sm::FIRST_ITEM..=sm::FIRST_ITEM + sm::ITEMS - 1) else {
                    eprintln!("[mod {mod_id}] objet « {} » ignoré : ce monde a déjà {} objets de mods", ni.id, sm::ITEMS);
                    continue;
                };
                content.items.push(sm::ItemDef { id, key, name: ni.name.clone(), tile: tile_of(&mut tiles, m, &ni.file), ..ni.def.clone() });
            }
        }
        // Un mot designe un bloc ou un objet : « mod:nom », un nom du meme mod, ou un mot de /donner.
        let find = |content: &sm::Content, mod_id: &str, word: &str| -> Option<u16> {
            let full = if word.contains(':') { word.to_string() } else { format!("{mod_id}:{word}") };
            content.items.iter().find(|d| d.key == full).map(|d| d.id).or_else(|| content.blocks.iter().find(|d| d.key == full).map(|d| d.id as u16)).or_else(|| {
                let plain = cmd::plain(word);
                cmd::item_from(word).filter(|id| *id < sm::FIRST_ITEM).or_else(|| (1..sm::FIRST_BLOCK as u16).find(|b| cmd::item_word(*b) == plain))
            })
        };
        for (i, word, mod_id) in drops {
            content.blocks[i].drop = if word.is_empty() {
                sm::DROP_NOTHING
            } else {
                find(&content, &mod_id, &word).unwrap_or_else(|| {
                    eprintln!("[mod {mod_id}] {} : objet inconnu « {word} », le bloc se laissera lui-même", content.blocks[i].key);
                    0
                })
            };
        }
        for m in &mods {
            let mod_id = &m.pack.manifest.id;
            let r = m.reg.borrow();
            'recipes: for nr in &r.recipes {
                let Some(out) = find(&content, mod_id, &nr.out) else {
                    eprintln!("[mod {mod_id}] recette ignorée : objet inconnu « {} »", nr.out);
                    continue;
                };
                let mut cells = Vec::new();
                for word in &nr.cells {
                    match (word.is_empty(), find(&content, mod_id, word)) {
                        (true, _) => cells.push(0),
                        (false, Some(id)) => cells.push(id),
                        (false, None) => {
                            eprintln!("[mod {mod_id}] recette de « {} » ignorée : objet inconnu « {word} »", nr.out);
                            continue 'recipes;
                        }
                    }
                }
                content.recipes.push(sm::RecipeDef { w: nr.w, h: nr.h, cells, out, n: nr.n });
            }
            for (input, out) in &r.smelts {
                match (find(&content, mod_id, input), find(&content, mod_id, out)) {
                    (Some(input), Some(out)) => content.smelts.push(sm::SmeltDef { input, out }),
                    _ => eprintln!("[mod {mod_id}] cuisson ignorée : objet inconnu dans « {input} » -> « {out} »"),
                }
            }
            for nm in &r.mobs {
                let id = content.mobs.len() as u8 + 1;
                if content.mobs.len() >= 250 {
                    eprintln!("[mod {mod_id}] créature « {} » ignorée : trop de créatures de mods", nm.id);
                    continue;
                }
                let loot = nm.loot.iter().filter_map(|(word, lo, hi)| {
                    let item = find(&content, mod_id, word);
                    if item.is_none() {
                        eprintln!("[mod {mod_id}] créature « {} » : objet de butin inconnu « {word} »", nm.id);
                    }
                    Some((item?, *lo, *hi))
                }).collect();
                let mut def = sm::MobDef { id, key: format!("{mod_id}:{}", nm.id), loot, ..nm.def.clone() };
                // Son modele : un fichier du mod, pour les premieres creatures qui en demandent un.
                if !def.model.is_empty() && (m.pack.file(&def.model).is_none() || content.mobs.iter().filter(|d| !d.model.is_empty()).count() >= sm::MODELS) {
                    eprintln!("[mod {mod_id}] créature « {} » : modèle {} absent, ou trop de modèles ({} au plus) ; elle garde la silhouette de sa base", nm.id, def.model, sm::MODELS);
                    def.model.clear();
                }
                content.mobs.push(def);
            }
            for no in &r.ores {
                let block = |word: &str| if word == "air" || word == "roche" { Some(0) } else { find(&content, mod_id, word).filter(|id| *id < 256).map(|id| id as u8) };
                match (block(&no.block), block(&no.replace)) {
                    (Some(b), Some(rep)) if b != 0 => content.ores.push(sm::OreDef { block: b, replace: rep, ..no.def.clone() }),
                    _ => eprintln!("[mod {mod_id}] filon ignoré : bloc inconnu dans « {} » ou « {} »", no.block, no.replace),
                }
            }
        }
        if let Some(dir) = save_dir.filter(|_| palette.len() != known) {
            let text: String = palette.iter().map(|(k, id)| format!("{k}={id}\n")).collect();
            let _ = std::fs::create_dir_all(dir);
            if let Err(e) = std::fs::write(dir.join(PALETTE), text) {
                eprintln!("Numéros des blocs et objets de mods non enregistrés : {e}");
            }
        }
        for m in &mods {
            let mf = &m.pack.manifest;
            println!("Mod chargé : {} {} ({})", mf.name, mf.version, mf.id);
        }
        content.commands = mods.iter().filter(|m| m.script.is_some()).flat_map(|m| m.reg.borrow().commands.clone()).collect();
        let infos = mods.iter().map(|m| m.pack.info()).collect();
        Host { mods, infos, content, tiles }
    }

    pub(crate) fn pack(&self, hash: &sm::Hash) -> Option<&Pack> {
        self.mods.iter().map(|m| &m.pack).find(|p| p.hash == *hash)
    }

    fn listens(&self, event: &str) -> bool {
        self.mods.iter().any(|m| m.script.is_some() && m.reg.borrow().listens.contains(event))
    }

    /// Previent les mods d'un evenement (un seul si `only` est donne). Faux si l'un d'eux le refuse.
    fn emit(&mut self, s: &mut Server, only: Option<&str>, event: &str, who: Option<(u32, String)>, args: &[Arg]) -> bool {
        let mut vals: Vec<Val> = vec![Val::Str(event.to_string())];
        match who {
            Some((id, name)) => vals.extend([Val::Num(id as f64), Val::Str(name)]),
            None => vals.extend([Val::Nil, Val::Nil]),
        }
        vals.extend(args.iter().map(|a| match a {
            Arg::N(n) => Val::Num(*n),
            Arg::S(text) => Val::Str(text.clone()),
        }));
        let mut ok = true;
        for m in &mut self.mods {
            if only.is_some_and(|id| id != m.pack.manifest.id) || !m.reg.borrow().listens.contains(event) {
                continue;
            }
            let Some(script) = m.script.as_mut() else { continue };
            let lend = Lend::new(s);
            let result = script.handle(kind::EVENT, &vals, EVENT_MS);
            drop(lend);
            match result {
                Ok(out) => ok &= !matches!(out.first(), Some(Val::Bool(false))),
                Err(e) => m.failed(&format!("événement « {event} »"), &e),
            }
        }
        ok
    }

    /// Execute la commande `name` si un mod l'a declaree. None : aucun mod ne la connait.
    fn command(&mut self, s: &mut Server, name: &str, who: (u32, String), args: &str) -> Option<Result<String, String>> {
        let m = self.mods.iter_mut().find(|m| m.script.is_some() && m.reg.borrow().commands.iter().any(|c| c.0 == name))?;
        let vals = [Val::Str(name.to_string()), Val::Num(who.0 as f64), Val::Str(who.1), Val::Str(args.to_string())];
        let lend = Lend::new(s);
        let result = m.script.as_mut()?.handle(kind::COMMAND, &vals, EVENT_MS);
        drop(lend);
        Some(match result {
            Ok(out) => Ok(match out.into_iter().next() {
                Some(Val::Str(reply)) => reply,
                _ => String::new(),
            }),
            Err(e) => {
                m.failed(&format!("commande /{name}"), &e);
                Err(format!("/{name} a échoué : {e}"))
            }
        })
    }

    /// Regles que les mods ont fixees a leur chargement.
    pub(crate) fn initial_rules(&self) -> Vec<(String, bool)> {
        self.mods.iter().filter(|m| m.script.is_some()).flat_map(|m| m.reg.borrow().rules.clone()).collect()
    }

    /// Commandes des mods : (nom, aide).
    pub(crate) fn commands(&self) -> Vec<(String, String)> {
        self.mods.iter().filter(|m| m.script.is_some()).flat_map(|m| m.reg.borrow().commands.clone()).collect()
    }

    /// Ecrit la memoire des mods dans le dossier du monde.
    pub(crate) fn save(&self, dir: &Path) {
        for m in &self.mods {
            let mut r = m.reg.borrow_mut();
            if !std::mem::take(&mut r.store_dirty) {
                continue;
            }
            let folder = dir.join("mods");
            let _ = std::fs::create_dir_all(&folder);
            match serde_json::to_string(&r.store) {
                Ok(text) => {
                    if let Err(e) = std::fs::write(folder.join(format!("{}.json", m.pack.manifest.id)), text) {
                        eprintln!("[mod {}] mémoire non enregistrée : {e}", m.pack.manifest.id);
                    }
                }
                Err(e) => eprintln!("[mod {}] mémoire non enregistrée : {e}", m.pack.manifest.id),
            }
        }
    }
}

impl Server {
    /// Regle qu'un mode de jeu peut changer : combat entre joueurs, vie qui remonte, naissance des creatures.
    pub(crate) fn set_rule(&mut self, name: &str, on: bool) {
        match name {
            "pvp" | "combat" => self.pvp = on,
            "regen" | "regeneration" => self.regen = on,
            _ => self.spawning = on,
        }
    }

    /// Previent les mods ; `player` : le joueur concerne. Faux si un mod refuse l'action.
    pub(crate) fn mod_event(&mut self, event: &str, player: Option<u32>, args: &[Arg]) -> bool {
        self.mod_event_to(None, event, player, args)
    }

    pub(crate) fn mod_event_to(&mut self, only: Option<&str>, event: &str, player: Option<u32>, args: &[Arg]) -> bool {
        if !self.mods.as_ref().is_some_and(|h| h.listens(event)) {
            return true;
        }
        let who = match player {
            Some(id) => {
                let Some((dim, name)) = self.clients.get(&id).map(|c| (c.dim, c.name.clone())) else { return true };
                // Le monde vu par le script est celui du joueur.
                self.enter(dim);
                Some((id, name))
            }
            None => None,
        };
        let Some(mut host) = self.mods.take() else { return true };
        let ok = host.emit(self, only, event, who, args);
        self.mods = Some(host);
        self.mod_kicks_out();
        ok
    }

    /// Commande declaree par un mod. None : ce n'en est pas une.
    pub(crate) fn mod_command(&mut self, id: u32, name: &str, args: &str) -> Option<Result<String, String>> {
        let who = self.clients.get(&id).map(|c| (id, c.name.clone()))?;
        let mut host = self.mods.take()?;
        let out = host.command(self, &cmd::plain(name), who, args);
        self.mods = Some(host);
        self.mod_kicks_out();
        out
    }

    /// Fait sortir les joueurs qu'un script a exclus.
    fn mod_kicks_out(&mut self) {
        for (id, reason) in std::mem::take(&mut self.mod_kicks) {
            self.kick(id, &reason);
        }
    }

    /// Annonce les mods a un joueur qui arrive. Faux s'il n'y en a pas : il entre directement.
    pub(crate) fn offer_mods(&self, id: u32) -> bool {
        let Some(h) = self.mods.as_ref().filter(|h| !h.infos.is_empty()) else { return false };
        if let Some(c) = self.clients.get(&id) {
            c.send(ServerMsg::Mods { mods: h.infos.clone(), content: h.content.clone(), tiles: h.tiles.clone() });
        }
        true
    }

    /// Envoie un morceau de l'archive d'un mod.
    pub(crate) fn mod_part(&self, id: u32, hash: sm::Hash, offset: u32) {
        let (Some(c), Some(pack)) = (self.clients.get(&id), self.mods.as_ref().and_then(|h| h.pack(&hash))) else { return };
        let from = (offset as usize).min(pack.bytes.len());
        let to = (from + sm::PART).min(pack.bytes.len());
        c.send(ServerMsg::ModPart { hash, offset, total: pack.bytes.len() as u32, data: pack.bytes[from..to].to_vec() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn pack(id: &str, server: &str, extra: &[(&str, &[u8])]) -> Pack {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        w.start_file("mod.json", opts).unwrap();
        w.write_all(format!(r#"{{"id":"{id}","name":"Essai","version":"1.0"}}"#).as_bytes()).unwrap();
        w.start_file("serveur.lua", opts).unwrap();
        w.write_all(server.as_bytes()).unwrap();
        for (name, data) in extra {
            w.start_file(*name, opts).unwrap();
            w.write_all(data).unwrap();
        }
        Pack::from_zip(w.finish().unwrap().into_inner()).unwrap()
    }

    fn server(packs: Vec<Pack>) -> (Server, Receiver<ServerMsg>) {
        let mut srv = Server::build(7, None, accounts::Accounts::default(), Some(packs));
        srv.rules.cheats = true;
        let (tx, rx) = mpsc::channel();
        let mut c = Client::new(tx, [8.5, 200.0, 8.5]);
        c.name = "Aki".into();
        c.ready = true;
        srv.clients.insert(1, c);
        srv.world.ensure((0, 0));
        (srv, rx)
    }

    /// Les blocs de mods sont connus de tout le programme : ces essais passent l'un apres l'autre.
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn chat_lines(rx: &Receiver<ServerMsg>) -> Vec<String> {
        rx.try_iter().filter_map(|m| if let ServerMsg::Chat { text, .. } = m { Some(text) } else { None }).collect()
    }

    #[test]
    fn scripts_see_events_and_act_on_the_game() {
        let script = r#"
            local rubis = blocks.register{ id = "rubis", name = "Bloc de rubis", texture = "rubis.png", hardness = 2, light = 9, tool = "pioche" }
            on("join", function(p) p:tell("Bienvenue " .. p.name) end)
            on("chat", function(p, text) if text == "secret" then return false end end)
            on("break", function(p, x, y, z, block) if block == "pierre" then return false end end)
            on("message", function(p, name, data) p:send("echo", name .. "=" .. data) end)
            command("rubis", "Donne un bloc de rubis", function(p, args)
                p:give(rubis, 3)
                world.set(8, 100, 8, rubis)
                store.set("dernier", p.name)
                return "ok " .. args .. " " .. world.get(8, 100, 8)
            end)
        "#;
        let _alone = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let (mut srv, rx) = server(vec![pack("essai", script, &[("textures/rubis.png", b"png")])]);
        // Le bloc a pris le premier numero libre, et le jeu le connait.
        let def = srv.mods.as_ref().unwrap().content.blocks[0].clone();
        assert_eq!((def.id, def.key.as_str(), def.tiles[0], def.light), (sm::FIRST_BLOCK, "essai:rubis", sm::FIRST_TILE, 9));
        assert_eq!(items::name(def.id as u16), "Bloc de rubis");
        assert_eq!(blocks::emit(def.id, 0), 9);

        srv.mod_event("join", Some(1), &[]);
        assert_eq!(chat_lines(&rx), ["Bienvenue Aki"]);
        // Un message refuse par un mod n'est pas diffuse.
        srv.chat(1, "secret".into());
        assert!(chat_lines(&rx).is_empty());
        srv.chat(1, "bonjour".into());
        assert_eq!(chat_lines(&rx), ["<Aki> bonjour"]);
        // La commande du mod donne, pose, retient et repond.
        srv.chat(1, "/rubis vite".into());
        assert_eq!(chat_lines(&rx), ["ok vite essai:rubis"]);
        assert_eq!(srv.world.get([8, 100, 8]).map(|b| b.0), Some(def.id));
        assert_eq!(items::count(&srv.clients[&1].inv, def.id as u16), 3);
        assert!(!srv.mod_event("break", Some(1), &[Arg::N(0.0), Arg::N(0.0), Arg::N(0.0), Arg::S("pierre".into())]));
        assert!(srv.mod_event("break", Some(1), &[Arg::N(0.0), Arg::N(0.0), Arg::N(0.0), Arg::S("terre".into())]));
        // Un message du script client revient par le script serveur.
        srv.on_msg(1, ClientMsg::ModMsg { mod_id: "essai".into(), name: "a".into(), data: "b".into() });
        assert!(rx.try_iter().any(|m| matches!(m, ServerMsg::ModMsg { name, data, .. } if name == "echo" && data == "a=b")));
        sm::clear_content();
    }

    #[test]
    fn a_broken_or_endless_script_does_not_stop_the_server() {
        let _alone = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let (mut srv, rx) = server(vec![
            pack("casse", "ceci n'est pas du lua", &[]),
            pack("lent", r#"on("tick", function() while true do end end) command("boum", function() error("aie") end)"#, &[]),
        ]);
        let began = Instant::now();
        srv.mod_event("tick", None, &[]);
        assert!(began.elapsed() < Duration::from_secs(2));
        srv.chat(1, "/boum".into());
        assert!(chat_lines(&rx)[0].contains("aie"));
        // Un script ne peut pas sortir de son bac a sable.
        let (srv, _rx) = server(vec![pack("curieux", r#"assert(io == nil and os == nil and require == nil and load == nil)"#, &[])]);
        assert!(srv.mods.as_ref().unwrap().mods[0].script.is_some());
        sm::clear_content();
    }

    /// Le gros mod d'exemple ecrit en Rust, compile en WebAssembly par exemples/mods/rubis/construire.sh.
    #[test]
    fn a_rust_mod_runs_like_a_lua_one() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../exemples/mods/rubis");
        if !dir.join("serveur.wasm").exists() {
            eprintln!("mod Rubis non compile : essai saute");
            return;
        }
        let _alone = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let (mut srv, rx) = server(vec![Pack::from_dir(&dir).unwrap()]);
        let c = srv.mods.as_ref().unwrap().content.clone();
        let block = |name: &str| c.blocks.iter().find(|d| d.key == format!("rubis:{name}")).unwrap().clone();
        let item = |name: &str| c.items.iter().find(|d| d.key == format!("rubis:{name}")).unwrap().id;
        // Tout ce que le mod declare est la : blocs, objets, recettes, cuisson, filon, creature.
        assert_eq!((c.blocks.len(), c.items.len(), c.recipes.len(), c.smelts.len(), c.ores.len(), c.mobs.len()), (3, 4, 5, 1, 1, 1));
        assert_eq!((block("minerai").drop, block("rubis").light, block("fleur").shape), (item("gemme"), 9, 1));
        assert_eq!(blocks::shape(block("fleur").id), blocks::Shape::Cross);
        assert_eq!(items::tool(item("pioche")), Some((items::PICK, 3)));
        assert_eq!((c.mobs[0].key.as_str(), c.mobs[0].base, c.mobs[0].hp), ("rubis:golem", shared::mobs::ZOMBIE, 60.0));
        // Chaque image du mod a sa tuile.
        assert!(srv.mods.as_ref().unwrap().tiles.len() >= 7);
        srv.mod_event("join", Some(1), &[]);
        assert!(chat_lines(&rx)[0].contains("touche G"));
        // Casser un minerai : compte, retenu, et annonce au script client.
        assert!(srv.mod_event("break", Some(1), &[Arg::N(0.0), Arg::N(0.0), Arg::N(0.0), Arg::S("rubis:minerai".into())]));
        assert!(rx.try_iter().any(|m| matches!(m, ServerMsg::ModMsg { mod_id, name, data } if mod_id == "rubis" && name == "mines" && data == "1")));
        // La boutique : sans rubis, rien ; avec, l'echange se fait et un son est joue.
        let buy = |srv: &mut Server| srv.on_msg(1, ClientMsg::ModMsg { mod_id: "rubis".into(), name: "acheter".into(), data: "2".into() });
        buy(&mut srv);
        assert!(rx.try_iter().any(|m| matches!(m, ServerMsg::ModMsg { name, data, .. } if name == "achat" && data == "non")));
        srv.clients.get_mut(&1).unwrap().inv[0] = Some(Stack::new(item("gemme"), 10));
        buy(&mut srv);
        let got: Vec<ServerMsg> = rx.try_iter().collect();
        assert!(got.iter().any(|m| matches!(m, ServerMsg::Play { name, .. } if name == "rubis:achat")));
        assert!(got.iter().any(|m| matches!(m, ServerMsg::ModMsg { name, data, .. } if name == "bourse" && data == "4")));
        assert_eq!((items::count(&srv.clients[&1].inv, item("pioche")), items::count(&srv.clients[&1].inv, item("gemme"))), (1, 4));
        srv.chat(1, "/rubis".into());
        assert_eq!(chat_lines(&rx), ["Aki : 1 minerais cassés, 4 rubis en poche"]);
        // Les sources du mod ne partent pas dans son archive ; ses sons, si.
        let pack = srv.mods.as_ref().unwrap().pack(&srv.mods.as_ref().unwrap().infos[0].hash).unwrap();
        assert!(pack.file("serveur/src/lib.rs").is_none() && pack.file("client.wasm").is_some() && pack.file("sons/achat.wav").is_some());
        sm::clear_content();
    }

    #[test]
    fn mods_add_items_recipes_smelting_and_ores() {
        let script = r#"
            blocks.register{ id = "minerai", name = "Minerai de rubis", texture = "a.png", hardness = 3, tool = "pioche", drop = "gemme" }
            local gemme = items.register{ id = "gemme", name = "Rubis", texture = "a.png", stack = 16 }
            local pioche = items.register{ id = "pioche", name = "Pioche de rubis", texture = "a.png", durability = 900, tool = "pioche", tier = 3 }
            items.register{ id = "tarte", name = "Tarte", texture = "a.png", food = 6, saturation = 4 }
            items.register{ id = "epee", name = "Lame de rubis", texture = "a.png", durability = 500, damage = 11 }
            recipes.shaped{ output = "pioche", pattern = { "GGG", " B ", " B " }, key = { G = gemme, B = "baton" } }
            recipes.shaped{ output = "essai:gemme", count = 4, pattern = { "M" }, key = { M = "minerai" } }
            recipes.smelt{ input = "minerai", output = "gemme" }
            worldgen.ore{ block = "minerai", veins = 20, size = 8, min_y = 5, max_y = 60 }
            on("use", function(p, item) if item == "essai:tarte" then p:tell("miam") return false end end)
            on("interact", function(p, x, y, z, block) if block == "essai:minerai" then p:tell("joli") return false end end)
            on("mob_death", function(mob, x, y, z) say("fin de " .. mob) end)
            command("sac", function(p) return p:count(gemme) .. " " .. tostring(p:take(gemme, 2)) .. " " .. p:count(gemme) .. " " .. tostring(p:held()) end)
        "#;
        let _alone = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let (mut srv, rx) = server(vec![pack("essai", script, &[("textures/a.png", b"png")])]);
        let c = srv.mods.as_ref().unwrap().content.clone();
        let item = |name: &str| c.items.iter().find(|d| d.key == format!("essai:{name}")).unwrap().id;
        let (ore, gem, pick) = (c.blocks[0].id, item("gemme"), item("pioche"));
        // Les objets ont les proprietes demandees, et le jeu les traite comme les siens.
        assert_eq!((items::name(gem), items::max_stack(gem), items::max_stack(pick)), ("Rubis".to_string(), 16, 1));
        assert_eq!((items::tool(pick), items::max_dur(pick)), (Some((items::PICK, 3)), 900));
        assert_eq!(items::meal(item("tarte")), Some((6.0, 4.0)));
        assert_eq!(items::weapon(Some(item("epee"))).dmg, 11.0);
        assert!(items::break_time(ore, Some(pick)).unwrap() < items::break_time(ore, None).unwrap());
        assert_eq!(items::drops(ore, 0, &mut srv.rng), vec![Stack::new(gem, 1)]);
        assert_eq!(cmd::item_from("rubis"), Some(gem));
        // Recettes et cuisson.
        let grid = |cells: [u16; 9]| -> Vec<Slot> { cells.iter().map(|&id| if id == 0 { None } else { Some(Stack::new(id, 1)) }).collect() };
        let s = items::STICK;
        assert_eq!(craft::result(&grid([gem, gem, gem, 0, s, 0, 0, s, 0])).map(|st| st.id), Some(pick));
        assert_eq!(craft::result(&grid([0, 0, 0, 0, ore as u16, 0, 0, 0, 0])), Some(Stack::new(gem, 4)));
        assert_eq!(craft::smelt(ore as u16), Some(gem));
        // La generation seme le minerai, toujours au meme endroit pour la meme graine.
        let count = |ch: &shared::chunk::Chunk| ch.b.iter().filter(|b| **b == ore).count();
        let (a, b) = (gen::generate(0, 7, 3, 3), gen::generate(0, 7, 3, 3));
        assert!(count(&a) > 20, "{} blocs de minerai", count(&a));
        assert_eq!(a.b, b.b);
        assert_eq!(count(&gen::generate(1, 7, 3, 3)), 0);
        // Evenements : objet utilise, bloc touche, creature morte.
        srv.clients.get_mut(&1).unwrap().inv[0] = Some(Stack::new(item("tarte"), 1));
        srv.use_item(1, None);
        assert_eq!(chat_lines(&rx), ["miam"]);
        srv.clients.get_mut(&1).unwrap().pos = [8.5, 100.0, 8.5];
        srv.world.set([8, 100, 10], ore, 0);
        srv.use_item(1, Some(([8, 100, 10], [8, 101, 10])));
        assert_eq!(chat_lines(&rx), ["joli"]);
        srv.mod_deaths.push((0, shared::mobs::COW, 0, Vec3::ZERO));
        srv.tick();
        assert!(chat_lines(&rx).contains(&"fin de cerf".to_string()));
        // Inventaire : compter, prendre, objet tenu.
        srv.clients.get_mut(&1).unwrap().inv[0] = Some(Stack::new(gem, 5));
        srv.chat(1, "/sac".into());
        assert_eq!(chat_lines(&rx), ["5 true 3 essai:gemme"]);
        sm::clear_content();
        assert_eq!(craft::smelt(ore as u16), None);
    }

    #[test]
    fn mods_add_creatures() {
        let script = r#"
            items.register{ id = "coeur", name = "Coeur de golem", texture = "a.png" }
            local golem = mobs.register{ id = "golem", name = "Golem", base = "oni", health = 60, damage = 8, scale = 1.5,
                color = { 0.5, 0.5, 1 }, loot = { { item = "coeur", min = 2, max = 2 } } }
            mobs.register{ id = "cerf-blanc", base = "cerf", rarity = 1 }
            on("mob_death", function(mob) say("mort : " .. mob) end)
            command("golem", function(p) local x, y, z = p:pos() return tostring(world.spawn(golem, x, y, z)) end)
        "#;
        let _alone = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let (mut srv, rx) = server(vec![pack("essai", script, &[("textures/a.png", b"png")])]);
        let c = srv.mods.as_ref().unwrap().content.clone();
        let heart = c.items[0].id;
        assert_eq!((c.mobs[0].key.as_str(), c.mobs[0].base, c.mobs[0].hp, c.mobs[0].color), ("essai:golem", shared::mobs::ZOMBIE, 60.0, [127, 127, 255]));
        srv.rules.difficulty = shared::rules::NORMAL;
        srv.chat(1, "/golem".into());
        let got: Vec<ServerMsg> = rx.try_iter().collect();
        assert!(got.iter().any(|m| matches!(m, ServerMsg::Chat { text, .. } if text == "true")));
        // Les joueurs la voient arriver sous sa forme de mod.
        assert!(got.iter().any(|m| matches!(m, ServerMsg::MobSpawn { skin, .. } if *skin == c.mobs[0].id)));
        let golem = srv.mobs.last().unwrap();
        assert_eq!((golem.kind, golem.skin, golem.hp), (shared::mobs::ZOMBIE, c.mobs[0].id, 60.0));
        // Une creature du jeu peut naitre sous une forme de mod : ici, tous les cerfs.
        srv.spawn_mob(shared::mobs::COW, Vec3::new(8.5, 200.0, 8.5), 0.0, 10.0);
        assert_eq!(srv.mobs.last().unwrap().skin, c.mobs[1].id);
        // A sa mort, elle laisse son butin et les mods sont prevenus sous son nom.
        srv.mobs.retain(|m| m.skin == c.mobs[0].id);
        srv.mobs[0].hp = 0.0;
        for _ in 0..3 {
            srv.tick();
        }
        assert!(srv.mobs.is_empty());
        assert_eq!(srv.drops.iter().filter(|d| d.stack.id == heart).map(|d| d.stack.n as u32).sum::<u32>(), 2);
        assert!(chat_lines(&rx).contains(&"mort : essai:golem".to_string()));
        sm::clear_content();
    }

    /// Ajoute un second joueur a la partie d'essai.
    fn second(srv: &mut Server, name: &str) -> Receiver<ServerMsg> {
        let (tx, rx) = mpsc::channel();
        let mut c = Client::new(tx, [10.5, 200.0, 10.5]);
        c.name = name.into();
        c.ready = true;
        srv.clients.insert(2, c);
        rx
    }

    fn infos(rx: &Receiver<ServerMsg>) -> Vec<String> {
        rx.try_iter().filter_map(|m| if let ServerMsg::Info { text } = m { Some(text) } else { None }).collect()
    }

    fn mode(name: &str) -> Pack {
        Pack::from_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../modes").join(name)).unwrap()
    }

    #[test]
    fn a_game_of_uhc() {
        let _alone = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let pack = mode("uhc");
        assert_eq!(pack.manifest.mode.as_ref().map(|m| m.name.as_str()), Some("UHC"));
        let (mut srv, rx) = server(vec![pack]);
        let rx2 = second(&mut srv, "Ren");
        // Le mode coupe la vie qui remonte et le combat tant que la partie n'a pas commence.
        assert!(!srv.regen && !srv.pvp);
        srv.mod_event("join", Some(1), &[]);
        srv.mod_event("join", Some(2), &[]);
        assert_eq!(srv.clients[&1].mode, shared::rules::ADVENTURE);
        srv.chat(1, "/uhc demarrer".into());
        // Les joueurs sont en survie, loin l'un de l'autre, sur la terre ferme.
        let (a, b) = (Vec3::from(srv.clients[&1].pos), Vec3::from(srv.clients[&2].pos));
        assert!(a.distance(b) > 100.0, "{a} et {b}");
        assert_eq!((srv.clients[&1].mode, srv.clients[&2].mode), (shared::rules::SURVIVAL, shared::rules::SURVIVAL));
        assert!(infos(&rx).iter().any(|t| t.contains("c'est parti")));
        // Hors de la zone, on perd de la vie.
        srv.clients.get_mut(&1).unwrap().pos = [5000.0, 200.0, 5000.0];
        for _ in 0..20 {
            srv.mod_event("tick", None, &[]);
        }
        assert!(srv.clients[&1].hp < 20.0);
        // Un joueur meurt de la main de l'autre : il ne reste que le vainqueur.
        srv.clients.get_mut(&1).unwrap().hit_by = Some((2, Instant::now()));
        srv.kill(1);
        let got: Vec<ServerMsg> = rx2.try_iter().collect();
        assert!(got.iter().any(|m| matches!(m, ServerMsg::Chat { text, .. } if text.contains("Aki est elimine par Ren"))));
        assert!(got.iter().any(|m| matches!(m, ServerMsg::Info { text } if text.contains("Ren remporte l'UHC"))));
        // Le vaincu laisse sa tete ; elle rend deux coeurs au vainqueur, comme la pomme doree en rend quatre.
        let item = |name: &str| srv.mods.as_ref().unwrap().content.items.iter().find(|d| d.key == format!("uhc:{name}")).unwrap().id;
        let (head, apple) = (item("tete"), item("pomme_doree"));
        assert!(srv.drops.iter().any(|d| d.stack.id == head), "la tete tombe");
        {
            let c = srv.clients.get_mut(&2).unwrap();
            (c.hp, c.sel, c.inv[0]) = (10.0, 0, Some(Stack::new(apple, 2)));
        }
        assert!(!srv.mod_event("use", Some(2), &[Arg::S("uhc:pomme_doree".into())]));
        assert_eq!((srv.clients[&2].hp, srv.clients[&2].inv[0].map(|s| s.n)), (18.0, Some(1)));
        // La pomme doree se fabrique : une pomme entouree d'or.
        assert!(srv.mods.as_ref().unwrap().content.recipes.iter().any(|r| r.out == apple));
        // Pas de seconde vie : le vaincu revient en spectateur.
        srv.on_msg(1, ClientMsg::Respawn);
        assert_eq!(srv.clients[&1].mode, shared::rules::SPECTATOR);
        sm::clear_content();
    }

    #[test]
    fn a_game_of_sky_wars() {
        let _alone = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let pack = mode("skywars");
        assert_eq!(pack.manifest.mode.as_ref().map(|m| m.name.as_str()), Some("Sky Wars"));
        let (mut srv, rx) = server(vec![pack]);
        let rx2 = second(&mut srv, "Ren");
        srv.mod_event("join", Some(1), &[]);
        srv.mod_event("join", Some(2), &[]);
        // La carte se batit pendant l'attente, autour du premier joueur.
        for _ in 0..80 {
            srv.mod_event("tick", None, &[]);
        }
        srv.chat(1, "/skywars demarrer".into());
        // Chacun attend dans sa cage de papier, au-dessus de son ile, loin de l'autre ; on ne se bat pas encore.
        let (a, b) = (srv.clients[&1].pos, srv.clients[&2].pos);
        assert!(a[1] > 255.0 && b[1] > 255.0 && Vec3::from(a).distance(Vec3::from(b)) > 50.0, "{a:?} {b:?}");
        assert!(!srv.pvp && srv.clients[&1].mode == shared::rules::ADVENTURE);
        let (ax, az) = (a[0].floor() as i32, a[2].floor() as i32);
        assert_eq!(srv.world.get([ax + 1, 258, az]).map(|c| c.0), Some(blocks::GLASS), "la cage");
        assert_eq!(srv.world.get([ax, 250, az]).map(|c| c.0), Some(blocks::GRASS), "l'ile");
        // Les coffres des iles et du centre sont garnis : dix-huit coffres pour huit iles et le centre.
        let arena: Vec<_> = srv.world.chests.iter().filter(|(p, _)| (251..=252).contains(&p[1])).collect();
        let full = arena.iter().filter(|(_, c)| c.iter().flatten().count() >= 2).count();
        assert_eq!((arena.len(), full), (20, 20));
        let center = srv.world.chests.iter().filter(|(p, _)| p[1] == 252).count();
        assert_eq!(center, 4);
        // Les cages s'ouvrent : on tombe sur son ile, le combat est ouvert, on peut batir.
        for _ in 0..(20 * 9) {
            srv.mod_event("tick", None, &[]);
        }
        assert!(srv.pvp && srv.clients[&1].mode == shared::rules::SURVIVAL);
        assert_eq!(srv.world.get([ax + 1, 258, az]).map(|c| c.0), Some(blocks::AIR));
        assert!(infos(&rx).iter().any(|t| t.contains("C'est parti")));
        let at = |p: [i32; 3], word: &str| [Arg::N(p[0] as f64), Arg::N(p[1] as f64), Arg::N(p[2] as f64), Arg::S(word.into())];
        assert!(srv.mod_event("place", Some(1), &at([ax + 6, 250, az], "planches_de_hinoki")));
        assert!(!srv.mod_event("place", Some(1), &at([ax + 600, 250, az], "planches_de_hinoki")), "pas hors de l'arene");
        // Tomber dans le vide tue ; pas de seconde vie, et le dernier debout gagne.
        srv.clients.get_mut(&2).unwrap().pos[1] = 100.0;
        srv.clients.get_mut(&2).unwrap().hit_by = Some((1, Instant::now()));
        for _ in 0..20 {
            srv.mod_event("tick", None, &[]);
        }
        assert!(srv.clients[&2].hp <= 0.0, "la chute dans le vide est mortelle");
        srv.kill(2);
        let got: Vec<ServerMsg> = rx2.try_iter().collect();
        assert!(got.iter().any(|m| matches!(m, ServerMsg::Chat { text, .. } if text.contains("Ren est elimine par Aki"))));
        assert!(got.iter().any(|m| matches!(m, ServerMsg::Info { text } if text.contains("Aki remporte la partie de Sky Wars"))));
        srv.on_msg(2, ClientMsg::Respawn);
        assert_eq!(srv.clients[&2].mode, shared::rules::SPECTATOR);
        sm::clear_content();
    }

    #[test]
    fn a_game_of_hunger_games() {
        let _alone = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let pack = mode("hungergames");
        assert_eq!(pack.manifest.mode.as_ref().map(|m| m.name.as_str()), Some("Hunger Games"));
        let (mut srv, rx) = server(vec![pack]);
        let rx2 = second(&mut srv, "Ren");
        srv.mod_event("join", Some(1), &[]);
        srv.mod_event("join", Some(2), &[]);
        // L'arene se batit pendant l'attente : la corne d'abondance, puis les coffres disperses.
        for _ in 0..(5 * 26) {
            srv.mod_event("tick", None, &[]);
        }
        let corn = srv.world.chests.values().filter(|c| c.iter().flatten().count() >= 3).count();
        assert!(srv.world.chests.len() >= 8 + 20 && corn >= 8, "{} coffres, {corn} bien garnis", srv.world.chests.len());
        assert!(srv.world.chests.values().all(|c| c.iter().flatten().count() >= 1), "aucun coffre vide");
        srv.chat(1, "/hungergames demarrer".into());
        // Chaque tribut est sur son socle d'obsidienne, en cercle autour de la corne ; on ne se bat pas encore.
        let (a, b) = (srv.clients[&1].pos, srv.clients[&2].pos);
        let under = |p: [f64; 3]| srv.world.get([p[0].floor() as i32, p[1].floor() as i32 - 1, p[2].floor() as i32]).map(|c| c.0);
        assert_eq!((under(a), under(b)), (Some(blocks::OBSIDIAN), Some(blocks::OBSIDIAN)));
        assert!(!srv.pvp && srv.clients[&1].mode == shared::rules::ADVENTURE);
        // Qui quitte son socle avant le signal y est remis.
        srv.clients.get_mut(&1).unwrap().pos[0] += 4.0;
        srv.mod_event("tick", None, &[]);
        assert_eq!(srv.clients[&1].pos, a);
        // Le signal, puis le temps de courir : le combat s'ouvre.
        srv.chat(1, "/hungergames signal".into());
        for _ in 0..(20 * 17) {
            srv.mod_event("tick", None, &[]);
        }
        assert!(srv.pvp);
        assert!(infos(&rx).iter().any(|t| t.contains("Que les Hunger Games commencent")));
        // On ne casse ni ne pose rien dans l'arene.
        let at = |p: [i32; 3], word: &str| [Arg::N(p[0] as f64), Arg::N(p[1] as f64), Arg::N(p[2] as f64), Arg::S(word.into())];
        assert!(!srv.mod_event("break", Some(1), &at([0, 100, 0], "pierre")));
        assert!(!srv.mod_event("place", Some(1), &at([0, 100, 0], "pierre")));
        // Hors de l'arene, on perd de la vie.
        srv.clients.get_mut(&2).unwrap().pos[0] += 500.0;
        for _ in 0..20 {
            srv.mod_event("tick", None, &[]);
        }
        assert!(srv.clients[&2].hp < 20.0);
        // Un tribut tombe : le dernier en vie gagne, le vaincu regarde.
        srv.clients.get_mut(&2).unwrap().hit_by = Some((1, Instant::now()));
        srv.kill(2);
        let got: Vec<ServerMsg> = rx2.try_iter().collect();
        assert!(got.iter().any(|m| matches!(m, ServerMsg::Chat { text, .. } if text.contains("Ren est tombe, vaincu par Aki"))));
        assert!(got.iter().any(|m| matches!(m, ServerMsg::Info { text } if text.contains("Aki remporte les Hunger Games"))));
        srv.on_msg(2, ClientMsg::Respawn);
        assert_eq!(srv.clients[&2].mode, shared::rules::SPECTATOR);
        sm::clear_content();
    }

    /// Chaque mode de jeu du dossier `modes` se charge, se lance a deux joueurs et tourne une minute
    /// sans que son script fasse la moindre erreur ; les objets qu'il donne existent tous.
    #[test]
    fn every_bundled_mode_plays() {
        let _alone = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../modes");
        let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().filter(|e| e.path().join("mod.json").exists()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
        names.sort();
        // Faction n'est pas une partie qui se lance : c'est un monde garde, qui a son propre essai.
        names.retain(|n| n != "faction" && n != "smp");
        assert!(names.len() >= 15, "{names:?}");
        for name in names {
            let before = SCRIPT_ERRORS.load(std::sync::atomic::Ordering::Relaxed);
            let pack = mode(&name);
            let shown = pack.manifest.mode.as_ref().unwrap_or_else(|| panic!("{name} : pas de rubrique mode")).name.clone();
            let (mut srv, rx) = server(vec![pack]);
            let _rx2 = second(&mut srv, "Ren");
            srv.mod_event("join", Some(1), &[]);
            srv.mod_event("join", Some(2), &[]);
            for _ in 0..(20 * 8) {
                srv.mod_event("tick", None, &[]);
            }
            srv.chat(1, format!("/{name} demarrer"));
            let moved = srv.clients[&1].pos != [8.5, 200.0, 8.5];
            for second in 0..70 {
                for _ in 0..20 {
                    srv.mod_event("tick", None, &[]);
                }
                // Une mort et un retour en jeu en cours de route, pour passer par ces chemins-la aussi.
                if second == 20 {
                    srv.clients.get_mut(&2).unwrap().hit_by = Some((1, Instant::now()));
                    srv.kill(2);
                    srv.on_msg(2, ClientMsg::Respawn);
                }
            }
            let errors = SCRIPT_ERRORS.load(std::sync::atomic::Ordering::Relaxed) - before;
            assert_eq!(errors, 0, "{name} ({shown}) : erreurs de script");
            let status = rx.try_iter().filter_map(|m| if let ServerMsg::ModMsg { name: n, data, .. } = m { (n == "etat").then_some(data) } else { None }).last();
            assert!(status.is_some_and(|t| !t.contains("attente")), "{name} : la partie n'a pas commence");
            assert!(moved || srv.clients[&1].pos != [8.5, 200.0, 8.5], "{name} : les joueurs n'ont pas ete places");
            sm::clear_content();
        }
    }

    /// Releve, dans les scripts des modes, les mots qui ressemblent a un objet ou un bloc et n'en sont pas.
    /// cargo test -p server names_in_modes -- --ignored --nocapture
    #[test]
    #[ignore]
    fn names_in_modes() {
        let _alone = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../modes");
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(lua) = std::fs::read_to_string(entry.path().join("serveur.lua")) else { continue };
            let (_srv, _rx) = server(vec![mode(&name)]);
            let mut unknown = Vec::new();
            for (k, piece) in lua.split('"').enumerate() {
                let word = piece.strip_prefix(&format!("{name}:")).unwrap_or(piece);
                let looks = k % 2 == 1 && word.contains('_') && word.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
                if looks && item_from(piece).is_none() && block_from(piece).is_none() && cmd::ench_from(piece).is_none() {
                    unknown.push(piece.to_string());
                }
            }
            unknown.sort();
            unknown.dedup();
            println!("{name} : {unknown:?}");
            sm::clear_content();
        }
    }

    #[test]
    fn a_world_of_factions() {
        let _alone = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let before = SCRIPT_ERRORS.load(std::sync::atomic::Ordering::Relaxed);
        let pack = mode("faction");
        assert!(pack.manifest.mode.as_ref().is_some_and(|m| m.keep_world), "le monde des factions est garde");
        let (mut srv, rx) = server(vec![pack]);
        let rx2 = second(&mut srv, "Ren");
        srv.rules.difficulty = shared::rules::NORMAL;
        srv.mod_event("join", Some(1), &[]);
        srv.mod_event("join", Some(2), &[]);
        let said = |rx: &Receiver<ServerMsg>| -> Vec<String> { rx.try_iter().filter_map(|m| if let ServerMsg::Chat { text, .. } = m { Some(text) } else { None }).collect() };
        let at = |p: [i32; 3], word: &str| [Arg::N(p[0] as f64), Arg::N(p[1] as f64), Arg::N(p[2] as f64), Arg::S(word.into())];
        // La zone sure, autour du point de depart : on n'y batit pas, on ne s'y bat pas, on n'y revendique rien.
        assert!(!srv.mod_event("place", Some(1), &at([8, 200, 8], "pierre")));
        srv.chat(1, "/f creer Dojo".into());
        srv.chat(1, "/f claim".into());
        assert!(said(&rx).iter().any(|t| t.contains("zone sure")));
        // Loin de la : Aki revendique un chunk, y fixe sa base, et lui seul y touche.
        srv.clients.get_mut(&1).unwrap().pos = [200.5, 150.0, 200.5];
        srv.clients.get_mut(&2).unwrap().pos = [202.5, 150.0, 202.5];
        srv.chat(1, "/f claim".into());
        srv.chat(1, "/f sethome".into());
        assert!(srv.mod_event("place", Some(1), &at([200, 150, 200], "pierre")));
        assert!(!srv.mod_event("place", Some(2), &at([200, 150, 200], "pierre")), "un etranger ne batit pas chez une faction");
        assert!(!srv.mod_event("break", Some(2), &at([201, 150, 205], "pierre")));
        assert!(!srv.mod_event("interact", Some(2), &at([201, 150, 205], "coffre_tansu")));
        assert!(srv.mod_event("place", Some(2), &at([300, 150, 300], "pierre")), "la zone libre est a tout le monde");
        // Ren est invite, rejoint : il est chez lui, et l'on ne se frappe plus entre membres.
        assert!(srv.mod_event("hit", Some(1), &[Arg::S("Ren".into())]), "deux inconnus peuvent se battre");
        srv.chat(1, "/f inviter Ren".into());
        srv.chat(2, "/f rejoindre Dojo".into());
        assert!(srv.mod_event("place", Some(2), &at([200, 150, 200], "pierre")));
        assert!(!srv.mod_event("hit", Some(1), &[Arg::S("Ren".into())]));
        srv.on_msg(1, ClientMsg::Hit { id: 2, sprint: false });
        assert_eq!(srv.clients[&2].hp, 20.0, "le coup entre membres n'est pas porte");
        // La puissance : 10 par membre, 4 de moins a chaque mort. Deux terres pour 20 de puissance, c'est tenu.
        srv.clients.get_mut(&2).unwrap().pos = [220.5, 150.0, 200.5];
        srv.chat(2, "/f claim".into());
        srv.chat(2, "/f quitter".into());
        // Reste Aki seul, 10 de puissance pour 2 terres. Il meurt trois fois : plus de puissance, il ne tient plus ses terres.
        for _ in 0..3 {
            srv.kill(1);
            srv.on_msg(1, ClientMsg::Respawn);
        }
        srv.chat(1, "/f power".into());
        assert!(said(&rx).iter().any(|t| t.contains("Votre puissance : 0 / 10")));
        // Ren fonde sa faction et lui prend le chunk qu'il ne peut plus tenir.
        srv.chat(2, "/f creer Ronin".into());
        srv.clients.get_mut(&2).unwrap().pos = [200.5, 150.0, 200.5];
        srv.chat(2, "/f claim".into());
        assert!(said(&rx2).iter().any(|t| t.contains("revendique le chunk")));
        assert!(!srv.mod_event("place", Some(1), &at([200, 150, 200], "pierre")), "les terres ont change de mains");
        assert!(srv.mod_event("place", Some(2), &at([200, 150, 200], "pierre")));
        // Les relations : ennemis d'un mot, allies seulement si les deux le veulent.
        // (Aki est revenu au point de depart, dans la zone sure, ou l'on ne se bat pas : il en ressort.)
        assert!(!srv.mod_event("hit", Some(2), &[Arg::S("Aki".into())]), "pas de combat en zone sure");
        srv.clients.get_mut(&1).unwrap().pos = [205.5, 150.0, 205.5];
        srv.chat(2, "/f ennemi Dojo".into());
        srv.chat(2, "/f allie Dojo".into());
        assert!(srv.mod_event("hit", Some(2), &[Arg::S("Aki".into())]), "une alliance proposee n'est pas encore une alliance");
        srv.chat(1, "/f allie Ronin".into());
        assert!(!srv.mod_event("hit", Some(2), &[Arg::S("Aki".into())]));
        // Tout cela est enregistre avec le monde.
        let store = &srv.mods.as_ref().unwrap();
        let _ = store;
        srv.chat(1, "/f carte".into());
        srv.chat(1, "/f info".into());
        srv.chat(1, "/f liste".into());
        for _ in 0..(20 * 3) {
            srv.mod_event("tick", None, &[]);
        }
        assert!(rx.try_iter().any(|m| matches!(m, ServerMsg::ModMsg { name, data, .. } if name == "etat" && data.contains("Dojo"))));
        assert_eq!(SCRIPT_ERRORS.load(std::sync::atomic::Ordering::Relaxed) - before, 0, "erreurs de script");
        sm::clear_content();
    }

    #[test]
    fn a_world_of_smp() {
        let _alone = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let before = SCRIPT_ERRORS.load(std::sync::atomic::Ordering::Relaxed);
        let pack = mode("smp");
        assert!(pack.manifest.mode.as_ref().is_some_and(|m| m.keep_world));
        let (mut srv, rx) = server(vec![pack]);
        let rx2 = second(&mut srv, "Ren");
        srv.rules.difficulty = shared::rules::NORMAL;
        srv.mod_event("join", Some(1), &[]);
        srv.mod_event("join", Some(2), &[]);
        let said = |rx: &Receiver<ServerMsg>| -> Vec<String> { rx.try_iter().filter_map(|m| if let ServerMsg::Chat { text, .. } = m { Some(text) } else { None }).collect() };
        let at = |p: [i32; 3], word: &str| [Arg::N(p[0] as f64), Arg::N(p[1] as f64), Arg::N(p[2] as f64), Arg::S(word.into())];
        let second_of = |srv: &mut Server, n: usize| {
            for _ in 0..(20 * n) {
                srv.mod_event("tick", None, &[]);
            }
        };
        // Un nouveau venu recoit de quoi commencer ; le point de depart est protege.
        assert!(items::count(&srv.clients[&1].inv, items::tool_id(items::PICK, 1)) == 1);
        assert!(!srv.mod_event("break", Some(1), &at([8, 199, 8], "pierre")));
        assert!(!srv.mod_event("hit", Some(1), &[Arg::S("Ren".into())]), "pas de combat au point de depart");
        assert!(srv.mod_event("break", Some(1), &at([300, 80, 300], "pierre")));
        // Une maison : on la fixe, on s'eloigne, /home y ramene apres quelques secondes sans bouger.
        srv.clients.get_mut(&1).unwrap().pos = [120.5, 90.0, 120.5];
        srv.chat(1, "/sethome base".into());
        srv.clients.get_mut(&1).unwrap().pos = [400.5, 90.0, 400.5];
        srv.chat(1, "/home base".into());
        second_of(&mut srv, 5);
        assert_eq!(srv.clients[&1].pos[0], 120.5);
        // Bouger annule le depart.
        srv.chat(1, "/spawn".into());
        srv.clients.get_mut(&1).unwrap().pos[0] += 3.0;
        second_of(&mut srv, 5);
        assert_eq!(srv.clients[&1].pos[0], 123.5);
        // Ren demande a rejoindre Aki, qui accepte.
        srv.chat(2, "/tpa Aki".into());
        srv.chat(1, "/tpaccept".into());
        second_of(&mut srv, 5);
        assert_eq!(srv.clients[&2].pos[0], 123.5);
        // Loin du depart on peut se battre, sauf si l'un des deux y a renonce.
        assert!(srv.mod_event("hit", Some(1), &[Arg::S("Ren".into())]));
        srv.chat(2, "/pvp non".into());
        assert!(!srv.mod_event("hit", Some(1), &[Arg::S("Ren".into())]));
        assert!(!srv.mod_event("hit", Some(2), &[Arg::S("Aki".into())]));
        // L'argent : cent kobans a l'arrivee, que l'on se donne ; la banque change les pieces.
        srv.chat(1, "/payer Ren 30".into());
        srv.clients.get_mut(&1).unwrap().inv[20] = Some(Stack::new(items::KOBAN, 12));
        srv.chat(1, "/banque deposer 12".into());
        srv.chat(1, "/argent".into());
        assert!(said(&rx).iter().any(|t| t.contains("Vous avez 82 kobans")));
        // L'hotel des ventes : Aki vend sa pioche enchantee et usee ; Ren l'achete telle quelle.
        let pick = Stack { dur: 77, ench: shared::ench::Ench::NONE.with(shared::ench::EFFICIENCY, 3), ..Stack::new(items::tool_id(items::PICK, 2), 1) };
        {
            let c = srv.clients.get_mut(&1).unwrap();
            (c.inv, c.sel, c.inv_dirty) = ([None; 36], 0, true);
            c.inv[0] = Some(pick);
        }
        srv.chat(1, "/ah vendre 50".into());
        assert!(srv.clients[&1].inv[0].is_none(), "la pile en vente quitte l'inventaire");
        srv.chat(2, "/ah".into());
        let listing = rx2.try_iter().filter_map(|m| if let ServerMsg::ModMsg { name, data, .. } = m { (name == "ah").then_some(data) } else { None }).last().expect("la fenetre de l'hotel des ventes");
        assert!(listing.contains("pioche_en_fer|1|50|Aki|0"), "{listing}");
        srv.clients.get_mut(&2).unwrap().inv = [None; 36];
        srv.on_msg(2, ClientMsg::ModMsg { mod_id: "smp".into(), name: "ah_acheter".into(), data: "1/1".into() });
        assert!(srv.clients[&2].inv.iter().flatten().any(|st| *st == pick), "l'objet arrive avec son usure et ses enchantements");
        srv.chat(1, "/argent".into());
        srv.chat(2, "/argent".into());
        assert!(said(&rx).iter().any(|t| t.contains("Vous avez 132 kobans")));
        assert!(said(&rx2).iter().any(|t| t.contains("Vous avez 80 kobans")));
        // On ne peut pas acheter sans argent ; retirer sa propre vente rend l'objet.
        srv.clients.get_mut(&1).unwrap().inv[0] = Some(Stack::new(items::DIAMOND, 5));
        srv.chat(1, "/ah vendre 5000".into());
        srv.chat(2, "/ah acheter 2".into());
        assert!(said(&rx2).iter().any(|t| t.contains("Il vous manque")));
        srv.chat(1, "/ah acheter 2".into());
        assert_eq!(items::count(&srv.clients[&1].inv, items::DIAMOND), 5);
        // La nuit : la moitie des joueurs suffit a la passer.
        srv.chat(1, "/heure nuit".into());
        srv.chat(1, "/dormir".into());
        assert!((srv.time * 24.0 + 6.0).rem_euclid(24.0) < 12.0, "le jour est revenu");
        // Mort : /back ramene sur les lieux.
        srv.clients.get_mut(&1).unwrap().pos = [500.5, 90.0, 500.5];
        srv.kill(1);
        srv.on_msg(1, ClientMsg::Respawn);
        srv.chat(1, "/back".into());
        second_of(&mut srv, 5);
        assert_eq!(srv.clients[&1].pos[0], 500.5);
        srv.chat(1, "/regles".into());
        srv.chat(1, "/homes".into());
        assert_eq!(SCRIPT_ERRORS.load(std::sync::atomic::Ordering::Relaxed) - before, 0, "erreurs de script");
        sm::clear_content();
    }

    #[test]
    fn a_game_of_bed_wars() {
        let _alone = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let (mut srv, rx) = server(vec![mode("bedwars")]);
        let rx2 = second(&mut srv, "Ren");
        assert!(!srv.spawning);
        srv.mod_event("join", Some(1), &[]);
        srv.mod_event("join", Some(2), &[]);
        srv.chat(1, "/bedwars demarrer".into());
        // Chacun est sur son ile, dans le ciel, avec de quoi commencer ; le combat est ouvert.
        let (a, b) = (srv.clients[&1].pos, srv.clients[&2].pos);
        assert!(a[1] > 240.0 && b[1] > 240.0 && (a[0] - b[0]).abs() > 40.0, "{a:?} {b:?}");
        assert!(srv.pvp && srv.clients[&1].home.is_some());
        let block = |name: &str| srv.mods.as_ref().unwrap().content.blocks.iter().find(|d| d.key == format!("bedwars:{name}")).unwrap().id;
        let (red_bed, blue_bed, generator) = (block("lit_rouge"), block("lit_bleu"), block("generateur"));
        let (ax, az) = (a[0].floor() as i32, a[2].floor() as i32);
        assert_eq!(srv.world.get([ax, 250, az]).map(|c| c.0), Some(generator));
        assert_eq!(srv.world.get([ax + 3, 251, az]).map(|c| c.0), Some(red_bed));
        // On ne casse ni l'arene ni son propre lit ; le lit adverse, si.
        let at = |p: [i32; 3], word: &str| [Arg::N(p[0] as f64), Arg::N(p[1] as f64), Arg::N(p[2] as f64), Arg::S(word.into())];
        assert!(!srv.mod_event("break", Some(1), &at([ax, 250, az + 2], "pierre")));
        assert!(!srv.mod_event("break", Some(1), &at([ax + 3, 251, az], "bedwars:lit_rouge")));
        assert!(srv.mod_event("break", Some(1), &at([0, 251, 0], "bedwars:lit_bleu")));
        assert!(infos(&rx2).iter().any(|t| t.contains("Le lit de l'equipe Bleue est detruit par Aki")));
        let _ = blue_bed;
        // Un bloc pose par un joueur se casse.
        assert!(srv.mod_event("place", Some(1), &at([ax, 252, az], "fourrure")));
        assert!(srv.mod_event("break", Some(2), &at([ax, 252, az], "fourrure")));
        // La boutique : le fer achete de la fourrure.
        srv.clients.get_mut(&1).unwrap().inv[5] = Some(Stack::new(items::IRON_INGOT, 4));
        let before = items::count(&srv.clients[&1].inv, blocks::WOOL as u16);
        srv.on_msg(1, ClientMsg::ModMsg { mod_id: "bedwars".into(), name: "acheter".into(), data: "1".into() });
        assert_eq!(items::count(&srv.clients[&1].inv, blocks::WOOL as u16), before + 16);
        assert_eq!(items::count(&srv.clients[&1].inv, items::IRON_INGOT), 0);
        // Les generateurs donnent du fer.
        let drops = srv.drops.len();
        for _ in 0..40 {
            srv.mod_event("tick", None, &[]);
        }
        assert!(srv.drops.len() > drops);
        // Toute la boutique se vend : chaque article existe, et se paie dans sa monnaie.
        for index in 1..=18 {
            let c = srv.clients.get_mut(&1).unwrap();
            (c.inv, c.inv_dirty) = ([None; 36], true);
            for (slot, coin) in [items::IRON_INGOT, items::GOLD_INGOT, items::DIAMOND, items::EMBER_CRYSTAL].into_iter().enumerate() {
                c.inv[30 + slot] = Some(Stack::new(coin, 32));
            }
            srv.on_msg(1, ClientMsg::ModMsg { mod_id: "bedwars".into(), name: "acheter".into(), data: index.to_string() });
            let c = &srv.clients[&1];
            let paid = (30..34).any(|k| c.inv[k].is_none_or(|st| st.n < 32));
            assert!(paid && c.inv[..30].iter().flatten().count() >= 1, "article {index}");
        }
        // La forge d'equipe s'ameliore contre des diamants : l'ile produit alors plus vite.
        srv.on_msg(1, ClientMsg::ModMsg { mod_id: "bedwars".into(), name: "forge".into(), data: String::new() });
        assert!(infos(&rx).iter().any(|t| t.contains("ameliore sa forge")) || rx2.try_iter().any(|m| matches!(m, ServerMsg::Chat { text, .. } if text.contains("ameliore sa forge"))));
        // Les ilots de diamant sont batis, entre les iles.
        assert_eq!(srv.world.get([ax - 8, 250, az + 18]).map(|c| c.0), Some(generator));
        // Sans lit, la mort est definitive : l'equipe rouge gagne.
        srv.kill(2);
        assert!(infos(&rx).iter().any(|t| t.contains("L'equipe Rouge remporte la partie")));
        srv.on_msg(2, ClientMsg::Respawn);
        assert_eq!(srv.clients[&2].mode, shared::rules::SPECTATOR);
        sm::clear_content();
    }

    #[test]
    fn players_can_fight_each_other_when_the_game_allows_it() {
        let _alone = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let (mut srv, _rx) = server(vec![]);
        let _rx2 = second(&mut srv, "Ren");
        srv.rules.difficulty = shared::rules::NORMAL;
        srv.clients.get_mut(&1).unwrap().last_hit = Instant::now() - Duration::from_secs(5);
        srv.on_msg(1, ClientMsg::Hit { id: 2, sprint: false });
        assert!(srv.clients[&2].hp < 20.0);
        // Combat coupe : plus rien ne passe.
        srv.pvp = false;
        let hp = srv.clients[&2].hp;
        srv.clients.get_mut(&1).unwrap().last_hit = Instant::now() - Duration::from_secs(5);
        srv.clients.get_mut(&2).unwrap().inv_t = 0.0;
        srv.on_msg(1, ClientMsg::Hit { id: 2, sprint: false });
        assert_eq!(srv.clients[&2].hp, hp);
        sm::clear_content();
    }
}
