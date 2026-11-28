//! Kit de création de mods Rust pour Epicube Craft.
//!
//! Un mod Rust se compile en WebAssembly (`cargo build --release --target wasm32-unknown-unknown`).
//! Le fichier obtenu se range dans le dossier du mod sous le nom `serveur.wasm` (ce que le mod fait
//! dans le monde) ou `client.wasm` (ce qu'il fait à l'écran du joueur).
//!
//! ```ignore
//! use epicube_mod::server::*;
//!
//! epicube_mod::start!(init);
//!
//! fn init() {
//!     on_join(|p| p.tell(&format!("Bienvenue {} !", p.name)));
//!     command("soin", "Rend toute sa vie", |p, _args| {
//!         p.set_health(20.0);
//!         Some("Te voilà guéri.".into())
//!     });
//! }
//! ```
//!
//! Le mod tourne dans un bac à sable : il n'a ni fichiers, ni réseau, ni système. Il n'agit sur le jeu
//! que par les fonctions de ce kit.

pub mod wire;

/// Numéros des fonctions du jeu. Le jeu et le kit doivent s'accorder sur cette liste.
pub mod op {
    // Côté serveur.
    pub const LOG: u32 = 1;
    pub const LISTEN: u32 = 2;
    pub const COMMAND: u32 = 3;
    pub const BLOCK: u32 = 4;
    pub const STORE_GET: u32 = 5;
    pub const STORE_SET: u32 = 6;
    pub const SAY: u32 = 7;
    pub const TELL: u32 = 8;
    pub const PLAYERS: u32 = 9;
    pub const ONLINE: u32 = 10;
    pub const POS: u32 = 11;
    pub const TELEPORT: u32 = 12;
    pub const GIVE: u32 = 13;
    pub const HEALTH: u32 = 14;
    pub const SET_HEALTH: u32 = 15;
    pub const MODE: u32 = 16;
    pub const KICK: u32 = 17;
    pub const SEND: u32 = 18;
    pub const GET_BLOCK: u32 = 19;
    pub const SET_BLOCK: u32 = 20;
    pub const TIME: u32 = 21;
    pub const SET_TIME: u32 = 22;
    pub const ITEM: u32 = 23;
    pub const RECIPE: u32 = 24;
    pub const SMELT: u32 = 25;
    pub const ORE: u32 = 26;
    pub const COUNT: u32 = 27;
    pub const TAKE: u32 = 28;
    pub const HELD: u32 = 29;
    pub const SET_MODE: u32 = 30;
    pub const HUNGER: u32 = 31;
    pub const SET_HUNGER: u32 = 32;
    pub const SPAWN: u32 = 33;
    pub const DROP: u32 = 34;
    pub const EXPLODE: u32 = 35;
    pub const DIMENSION: u32 = 36;
    pub const MOB: u32 = 37;
    pub const SOUND: u32 = 38;
    pub const RULE: u32 = 39;
    pub const DAMAGE: u32 = 40;
    pub const SET_HOME: u32 = 41;
    pub const CLEAR: u32 = 42;
    pub const TOP: u32 = 43;
    pub const FILL: u32 = 44;
    pub const INFO: u32 = 45;
    /// Range des objets dans un coffre (pose s'il n'y en a pas) : x, y, z, objet, nombre.
    pub const CHEST: u32 = 46;
    /// Enchante l'objet que tient le joueur : joueur, enchantement, niveau.
    pub const ENCHANT: u32 = 47;
    // Côté client.
    pub const C_LOG: u32 = 100;
    pub const C_SEND: u32 = 102;
    pub const C_CHAT: u32 = 103;
    pub const C_POS: u32 = 104;
    pub const C_TEXT: u32 = 105;
    pub const C_RECT: u32 = 106;
    pub const C_SCREEN: u32 = 107;
    pub const C_TEXT_WIDTH: u32 = 108;
    pub const C_OPEN: u32 = 109;
    pub const C_CLOSE: u32 = 110;
    pub const C_MOUSE: u32 = 111;
    pub const C_IMAGE: u32 = 112;
    pub const C_ICON: u32 = 113;
    pub const C_SOUND: u32 = 114;
}

/// Ce que le jeu demande au mod : traiter un événement, ou exécuter une commande.
pub mod kind {
    pub const EVENT: u32 = 0;
    pub const COMMAND: u32 = 1;
}

pub use wire::Val;

mod guest;
pub use guest::{call, on};

#[doc(hidden)]
pub use guest::__handle;

/// Déclare la fonction de démarrage du mod, appelée une fois à son chargement.
#[macro_export]
macro_rules! start {
    ($init:path) => {
        #[no_mangle]
        pub extern "C" fn ec_init() {
            $init();
        }
        #[no_mangle]
        pub extern "C" fn ec_alloc(len: u32) -> u32 {
            $crate::wire::alloc(len)
        }
        #[no_mangle]
        pub extern "C" fn ec_handle(kind: u32, ptr: u32, len: u32) -> u64 {
            $crate::__handle(kind, ptr, len)
        }
    };
}

/// Un joueur connecté.
#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub id: u32,
    pub name: String,
}

fn text(v: Vec<Val>) -> Option<String> {
    match v.into_iter().next() {
        Some(Val::Str(s)) => Some(s),
        _ => None,
    }
}

fn yes(v: Vec<Val>) -> bool {
    matches!(v.first(), Some(Val::Bool(true)))
}

/// Fonctions d'un mod côté serveur (`serveur.wasm`).
pub mod server {
    use super::*;
    pub use super::{Player, Val};

    impl Player {
        fn id(&self) -> Val {
            Val::Num(self.id as f64)
        }
        /// Position du joueur, ou None s'il est parti.
        pub fn pos(&self) -> Option<[f64; 3]> {
            match call(op::POS, &[self.id()])[..] {
                [Val::Num(x), Val::Num(y), Val::Num(z)] => Some([x, y, z]),
                _ => None,
            }
        }
        pub fn teleport(&self, x: f64, y: f64, z: f64) -> bool {
            yes(call(op::TELEPORT, &[self.id(), x.into(), y.into(), z.into()]))
        }
        /// Donne `n` exemplaires d'un objet : son mot dans `/donner`, ou « mod:bloc ».
        pub fn give(&self, item: &str, n: u32) -> bool {
            yes(call(op::GIVE, &[self.id(), item.into(), (n as f64).into()]))
        }
        /// Écrit une ligne dans le chat de ce joueur.
        pub fn tell(&self, text: &str) {
            call(op::TELL, &[self.id(), text.into()]);
        }
        pub fn health(&self) -> Option<f64> {
            match call(op::HEALTH, &[self.id()])[..] {
                [Val::Num(v)] => Some(v),
                _ => None,
            }
        }
        /// Vie de 0 à 20.
        pub fn set_health(&self, v: f64) {
            call(op::SET_HEALTH, &[self.id(), v.into()]);
        }
        /// Mode de jeu : « survie », « creatif », « aventure » ou « spectateur ».
        pub fn mode(&self) -> Option<String> {
            text(call(op::MODE, &[self.id()]))
        }
        pub fn kick(&self, reason: &str) {
            call(op::KICK, &[self.id(), reason.into()]);
        }
        /// Envoie un message au script client de ce mod, chez ce joueur.
        pub fn send(&self, name: &str, data: &str) {
            call(op::SEND, &[self.id(), name.into(), data.into()]);
        }
        pub fn online(&self) -> bool {
            yes(call(op::ONLINE, &[self.id()]))
        }
        /// Nombre d'exemplaires de cet objet dans son inventaire.
        pub fn count(&self, item: &str) -> u32 {
            match call(op::COUNT, &[self.id(), item.into()])[..] {
                [Val::Num(n)] => n as u32,
                _ => 0,
            }
        }
        /// Lui prend `n` exemplaires d'un objet ; `false`, et rien n'est pris, s'il n'en a pas assez.
        pub fn take(&self, item: &str, n: u32) -> bool {
            yes(call(op::TAKE, &[self.id(), item.into(), (n as f64).into()]))
        }
        /// Objet tenu en main.
        pub fn held(&self) -> Option<String> {
            text(call(op::HELD, &[self.id()]))
        }
        /// Change son mode de jeu : « survie », « creatif », « aventure » ou « spectateur ».
        pub fn set_mode(&self, mode: &str) {
            call(op::SET_MODE, &[self.id(), mode.into()]);
        }
        pub fn hunger(&self) -> Option<f64> {
            match call(op::HUNGER, &[self.id()])[..] {
                [Val::Num(v)] => Some(v),
                _ => None,
            }
        }
        /// Faim de 0 à 20.
        pub fn set_hunger(&self, v: f64) {
            call(op::SET_HUNGER, &[self.id(), v.into()]);
        }
        /// Monde où il se trouve : 0 le monde ordinaire, 1 Yomi, 2 Takamagahara.
        pub fn dimension(&self) -> u8 {
            match call(op::DIMENSION, &[self.id()])[..] {
                [Val::Num(d)] => d as u8,
                _ => 0,
            }
        }
        /// Lui retire de la vie, sans que son armure n'y change rien (une zone interdite, un piège).
        pub fn damage(&self, amount: f64) {
            call(op::DAMAGE, &[self.id(), amount.into()]);
        }
        /// Fixe l'endroit où il revient après sa mort.
        pub fn set_home(&self, x: f64, y: f64, z: f64) {
            call(op::SET_HOME, &[self.id(), x.into(), y.into(), z.into()]);
        }
        /// Vide son inventaire et retire son armure.
        pub fn clear(&self) {
            call(op::CLEAR, &[self.id()]);
        }
        /// Affiche un message au centre de son écran.
        pub fn info(&self, text: &str) {
            call(op::INFO, &[self.id(), text.into()]);
        }
        /// Lui fait entendre un bruitage : un son du jeu, ou « mod:fichier ».
        pub fn sound(&self, name: &str, volume: f64) {
            call(op::SOUND, &[self.id(), name.into(), volume.into()]);
        }
    }

    /// Écrit dans le journal du serveur.
    pub fn log(text: &str) {
        call(op::LOG, &[text.into()]);
    }

    /// Affiche un message au centre de l'écran de tous les joueurs.
    pub fn announce(text: &str) {
        call(op::INFO, &[Val::Nil, text.into()]);
    }

    /// Règles de la partie, qu'un mode de jeu peut changer.
    pub mod rules {
        use super::*;
        /// Combat entre joueurs.
        pub fn pvp(on: bool) {
            call(op::RULE, &["pvp".into(), Val::Bool(on)]);
        }
        /// La vie remonte toute seule quand on a bien mangé.
        pub fn regen(on: bool) {
            call(op::RULE, &["regen".into(), Val::Bool(on)]);
        }
        /// Les créatures naissent d'elles-mêmes dans le monde.
        pub fn mobs(on: bool) {
            call(op::RULE, &["mobs".into(), Val::Bool(on)]);
        }
    }

    /// Écrit une ligne dans le chat de tous les joueurs.
    pub fn say(text: &str) {
        call(op::SAY, &[text.into()]);
    }

    pub fn players() -> Vec<Player> {
        call(op::PLAYERS, &[])
            .chunks(2)
            .filter_map(|c| match c {
                [Val::Num(id), Val::Str(name)] => Some(Player { id: *id as u32, name: name.clone() }),
                _ => None,
            })
            .collect()
    }

    fn player_of(vals: &[Val]) -> Option<Player> {
        match vals {
            [Val::Num(id), Val::Str(name), ..] => Some(Player { id: *id as u32, name: name.clone() }),
            _ => None,
        }
    }

    fn word(v: Option<&Val>) -> &str {
        match v {
            Some(Val::Str(s)) => s,
            _ => "",
        }
    }

    fn cell(a: &[Val]) -> [i32; 3] {
        let n = |i: usize| match a.get(i) {
            Some(Val::Num(v)) => *v as i32,
            _ => 0,
        };
        [n(0), n(1), n(2)]
    }

    pub fn on_join(f: impl Fn(&Player) + 'static) {
        on("join", move |a| {
            if let Some(p) = player_of(a) {
                f(&p);
            }
            true
        });
    }

    pub fn on_leave(f: impl Fn(&Player) + 'static) {
        on("leave", move |a| {
            if let Some(p) = player_of(a) {
                f(&p);
            }
            true
        });
    }

    /// Un joueur écrit dans le chat. Renvoyer `false` : le message n'est pas diffusé.
    pub fn on_chat(f: impl Fn(&Player, &str) -> bool + 'static) {
        on("chat", move |a| player_of(a).is_none_or(|p| f(&p, word(a.get(2)))));
    }

    /// Un joueur casse un bloc. Renvoyer `false` : le bloc reste en place.
    pub fn on_break(f: impl Fn(&Player, [i32; 3], &str) -> bool + 'static) {
        on("break", move |a| player_of(a).is_none_or(|p| f(&p, cell(&a[2..]), word(a.get(5)))));
    }

    /// Un joueur pose un bloc. Renvoyer `false` : le bloc n'est pas posé.
    pub fn on_place(f: impl Fn(&Player, [i32; 3], &str) -> bool + 'static) {
        on("place", move |a| player_of(a).is_none_or(|p| f(&p, cell(&a[2..]), word(a.get(5)))));
    }

    /// Vingt fois par seconde.
    pub fn on_tick(f: impl Fn() + 'static) {
        on("tick", move |_| {
            f();
            true
        });
    }

    /// Un joueur fait un clic droit en tenant un objet. Renvoyer `false` : le jeu n'en fait rien.
    pub fn on_use(f: impl Fn(&Player, &str) -> bool + 'static) {
        on("use", move |a| player_of(a).is_none_or(|p| f(&p, word(a.get(2)))));
    }

    /// Un joueur fait un clic droit sur un bloc. Renvoyer `false` : le jeu n'en fait rien.
    pub fn on_interact(f: impl Fn(&Player, [i32; 3], &str) -> bool + 'static) {
        on("interact", move |a| player_of(a).is_none_or(|p| f(&p, cell(&a[2..]), word(a.get(5)))));
    }

    /// Un joueur meurt ; `killer` : le pseudo du joueur qui l'a tué, ou vide.
    pub fn on_death(f: impl Fn(&Player, &str) + 'static) {
        on("death", move |a| {
            if let Some(p) = player_of(a) {
                f(&p, word(a.get(2)));
            }
            true
        });
    }

    pub fn on_respawn(f: impl Fn(&Player) + 'static) {
        on("respawn", move |a| {
            if let Some(p) = player_of(a) {
                f(&p);
            }
            true
        });
    }

    /// Une créature meurt : son nom (« cerf », « oni », « mod:golem »...) et l'endroit.
    pub fn on_mob_death(f: impl Fn(&str, [f64; 3]) + 'static) {
        on("mob_death", move |a| {
            let n = |i: usize| match a.get(i) {
                Some(Val::Num(v)) => *v,
                _ => 0.0,
            };
            f(word(a.first()), [n(1), n(2), n(3)]);
            true
        });
    }

    /// Message envoyé par le script client de ce mod.
    pub fn on_message(f: impl Fn(&Player, &str, &str) + 'static) {
        on("message", move |a| {
            if let Some(p) = player_of(a) {
                f(&p, word(a.get(2)), word(a.get(3)));
            }
            true
        });
    }

    /// Déclare la commande `/name`. La fonction reçoit le joueur et le reste de la ligne ;
    /// ce qu'elle renvoie lui est affiché.
    pub fn command(name: &str, help: &str, f: impl Fn(&Player, &str) -> Option<String> + 'static) {
        let name = text(call(op::COMMAND, &[name.into(), help.into()])).unwrap_or_else(|| name.to_string());
        guest::add_command(name, Box::new(move |a| player_of(a).and_then(|p| f(&p, word(a.get(2))))));
    }

    /// Outil qui accélère le minage d'un bloc.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Tool {
        None,
        Pick,
        Axe,
        Shovel,
    }

    /// Ce que laisse un bloc cassé.
    #[derive(Clone, Debug, PartialEq)]
    pub enum Drop {
        /// Le bloc lui-même.
        Itself,
        Nothing,
        /// Un autre objet : son mot dans `/donner`, ou « mod:bloc ».
        Item(String),
    }

    /// Un bloc ajouté par le mod.
    #[derive(Clone, Debug)]
    pub struct Block {
        id: String,
        name: String,
        top: String,
        side: String,
        bottom: String,
        hardness: f64,
        light: u8,
        tool: Tool,
        transparent: bool,
        drop: Drop,
        plant: bool,
    }

    impl Block {
        /// `id` : minuscules, chiffres et tirets. La texture par défaut est `textures/<id>.png`.
        pub fn new(id: &str) -> Block {
            let file = format!("{id}.png");
            Block { id: id.into(), name: id.into(), top: file.clone(), side: file.clone(), bottom: file, hardness: 1.0, light: 0, tool: Tool::None, transparent: false, drop: Drop::Itself, plant: false }
        }
        pub fn name(mut self, name: &str) -> Block {
            self.name = name.into();
            self
        }
        /// Une image pour toutes les faces, prise dans le dossier `textures` du mod.
        pub fn texture(mut self, file: &str) -> Block {
            (self.top, self.side, self.bottom) = (file.into(), file.into(), file.into());
            self
        }
        /// Une image pour le dessus, une pour les côtés, une pour le dessous.
        pub fn textures(mut self, top: &str, side: &str, bottom: &str) -> Block {
            (self.top, self.side, self.bottom) = (top.into(), side.into(), bottom.into());
            self
        }
        /// Dureté : 1 = terre battue, 2 = bois, 3 = minerai ; négative = incassable.
        pub fn hardness(mut self, h: f64) -> Block {
            self.hardness = h;
            self
        }
        /// Lumière émise, de 0 à 15.
        pub fn light(mut self, l: u8) -> Block {
            self.light = l;
            self
        }
        pub fn tool(mut self, t: Tool) -> Block {
            self.tool = t;
            self
        }
        /// Laisse passer la lumière, comme le verre.
        pub fn transparent(mut self, yes: bool) -> Block {
            self.transparent = yes;
            self
        }
        pub fn drop(mut self, d: Drop) -> Block {
            self.drop = d;
            self
        }
        /// Une plante : deux plans croisés, que l'on traverse, au lieu d'un cube.
        pub fn plant(mut self, yes: bool) -> Block {
            self.plant = yes;
            self
        }
    }

    pub mod blocks {
        use super::*;
        /// Ajoute un bloc au jeu, au chargement du mod. Renvoie son nom complet, « mod:bloc ».
        pub fn register(b: Block) -> String {
            let tool = match b.tool {
                Tool::None => "",
                Tool::Pick => "pick",
                Tool::Axe => "axe",
                Tool::Shovel => "shovel",
            };
            let drop = match &b.drop {
                Drop::Itself => Val::Nil,
                Drop::Nothing => Val::Bool(false),
                Drop::Item(word) => Val::Str(word.clone()),
            };
            let args = [
                b.id.as_str().into(),
                b.name.as_str().into(),
                b.top.as_str().into(),
                b.side.as_str().into(),
                b.bottom.as_str().into(),
                b.hardness.into(),
                (b.light as f64).into(),
                tool.into(),
                Val::Bool(b.transparent),
                drop,
                if b.plant { "plante".into() } else { Val::Nil },
            ];
            text(call(op::BLOCK, &args)).unwrap_or_default()
        }
    }

    /// Genre d'outil d'un objet.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ToolKind {
        Pick,
        Axe,
        Shovel,
        Sword,
        Hoe,
    }

    /// Un objet ajouté par le mod.
    #[derive(Clone, Debug)]
    pub struct Item {
        id: String,
        name: String,
        texture: String,
        stack: u8,
        food: f64,
        saturation: f64,
        fuel: f64,
        durability: u32,
        tool: Option<(ToolKind, u8)>,
        damage: f64,
    }

    impl Item {
        /// `id` : minuscules, chiffres et tirets. L'icône par défaut est `textures/<id>.png`.
        pub fn new(id: &str) -> Item {
            Item { id: id.into(), name: id.into(), texture: format!("{id}.png"), stack: 64, food: 0.0, saturation: 0.0, fuel: 0.0, durability: 0, tool: None, damage: 0.0 }
        }
        pub fn name(mut self, name: &str) -> Item {
            self.name = name.into();
            self
        }
        pub fn texture(mut self, file: &str) -> Item {
            self.texture = file.into();
            self
        }
        /// Taille d'une pile, de 1 à 64.
        pub fn stack(mut self, n: u8) -> Item {
            self.stack = n;
            self
        }
        /// Un aliment : la faim qu'il rend, et la réserve qui retarde la faim suivante.
        pub fn food(mut self, hunger: f64, saturation: f64) -> Item {
            (self.food, self.saturation) = (hunger, saturation);
            self
        }
        /// Un combustible : secondes de feu dans un four.
        pub fn fuel(mut self, seconds: f64) -> Item {
            self.fuel = seconds;
            self
        }
        /// Un outil : son genre, son niveau (0 bois, 1 pierre, 2 fer, 3 diamant, 4 tamahagane) et son nombre d'usages.
        pub fn tool(mut self, kind: ToolKind, tier: u8, durability: u32) -> Item {
            (self.tool, self.durability) = (Some((kind, tier)), durability);
            self
        }
        /// Une arme : ses dégâts et son nombre d'usages.
        pub fn weapon(mut self, damage: f64, durability: u32) -> Item {
            (self.damage, self.durability) = (damage, durability);
            self
        }
    }

    pub mod items {
        use super::*;
        /// Ajoute un objet au jeu, au chargement du mod. Renvoie son nom complet, « mod:objet ».
        pub fn register(i: Item) -> String {
            let (tool, tier) = match i.tool {
                Some((k, tier)) => {
                    let word = match k {
                        ToolKind::Pick => "pick",
                        ToolKind::Axe => "axe",
                        ToolKind::Shovel => "shovel",
                        ToolKind::Sword => "sword",
                        ToolKind::Hoe => "hoe",
                    };
                    (Val::from(word), tier)
                }
                None => (Val::Nil, 0),
            };
            let args = [
                i.id.as_str().into(),
                i.name.as_str().into(),
                i.texture.as_str().into(),
                (i.stack as f64).into(),
                i.food.into(),
                i.saturation.into(),
                i.fuel.into(),
                (i.durability as f64).into(),
                tool,
                (tier as f64).into(),
                i.damage.into(),
            ];
            text(call(op::ITEM, &args)).unwrap_or_default()
        }
    }

    pub mod recipes {
        use super::*;
        /// Recette d'artisanat : un dessin de 1 à 3 lignes, et l'objet que désigne chaque lettre.
        ///
        /// ```ignore
        /// recipes::shaped("pioche", 1, &["RRR", " B ", " B "], &[('R', "rubis"), ('B', "baton")]);
        /// ```
        pub fn shaped(output: &str, count: u8, pattern: &[&str], key: &[(char, &str)]) {
            let w = pattern.first().map_or(0, |l| l.chars().count());
            let mut args: Vec<Val> = vec![output.into(), (count as f64).into(), (w as f64).into(), (pattern.len() as f64).into()];
            for line in pattern {
                for c in line.chars() {
                    args.push(key.iter().find(|(k, _)| *k == c).map_or("", |(_, word)| word).into());
                }
            }
            call(op::RECIPE, &args);
        }
        /// Cuisson au four : ce qu'on y met, ce qu'on en tire.
        pub fn smelt(input: &str, output: &str) {
            call(op::SMELT, &[input.into(), output.into()]);
        }
    }

    /// Un filon que la génération du monde sème dans la roche.
    #[derive(Clone, Debug)]
    pub struct Ore {
        block: String,
        replace: Option<String>,
        veins: u8,
        size: u8,
        min_y: u16,
        max_y: u16,
        dimension: u8,
    }

    impl Ore {
        pub fn new(block: &str) -> Ore {
            Ore { block: block.into(), replace: None, veins: 4, size: 6, min_y: 1, max_y: 64, dimension: 0 }
        }
        /// Bloc que le filon remplace ; par défaut, la roche du lieu.
        pub fn replace(mut self, block: &str) -> Ore {
            self.replace = Some(block.into());
            self
        }
        /// Nombre d'amas par chunk, et de blocs par amas.
        pub fn veins(mut self, veins: u8, size: u8) -> Ore {
            (self.veins, self.size) = (veins, size);
            self
        }
        pub fn height(mut self, min_y: u16, max_y: u16) -> Ore {
            (self.min_y, self.max_y) = (min_y, max_y);
            self
        }
        /// 0 le monde ordinaire, 1 Yomi, 2 Takamagahara.
        pub fn dimension(mut self, d: u8) -> Ore {
            self.dimension = d;
            self
        }
    }

    pub mod worldgen {
        use super::*;
        pub fn ore(o: Ore) {
            let replace = o.replace.as_deref().map_or(Val::Nil, Val::from);
            let n = |v: f64| Val::Num(v);
            call(op::ORE, &[o.block.as_str().into(), replace, n(o.veins as f64), n(o.size as f64), n(o.min_y as f64), n(o.max_y as f64), n(o.dimension as f64)]);
        }
    }

    /// Une créature ajoutée par le mod : le comportement et la silhouette d'une créature du jeu,
    /// avec ses propres caractéristiques.
    #[derive(Clone, Debug)]
    pub struct Mob {
        id: String,
        name: String,
        base: String,
        health: Option<f64>,
        damage: f64,
        scale: f64,
        color: [f64; 3],
        rarity: f64,
        model: String,
        loot: Vec<(String, u8, u8)>,
    }

    impl Mob {
        /// `base` : « cerf », « sanglier », « tanuki », « grue », « oni », « squelette », « araignee »,
        /// « lanterne », « yurei » ou « wanyudo ».
        pub fn new(id: &str, base: &str) -> Mob {
            Mob { id: id.into(), name: id.into(), base: base.into(), health: None, damage: 0.0, scale: 1.0, color: [1.0; 3], rarity: 0.0, model: String::new(), loot: Vec::new() }
        }
        pub fn name(mut self, name: &str) -> Mob {
            self.name = name.into();
            self
        }
        pub fn health(mut self, hp: f64) -> Mob {
            self.health = Some(hp);
            self
        }
        /// Dégâts au contact.
        pub fn damage(mut self, d: f64) -> Mob {
            self.damage = d;
            self
        }
        pub fn scale(mut self, k: f64) -> Mob {
            self.scale = k;
            self
        }
        /// Teinte : rouge, vert, bleu, de 0 à 1.
        pub fn color(mut self, r: f64, g: f64, b: f64) -> Mob {
            self.color = [r, g, b];
            self
        }
        /// Part des créatures de base qui naissent sous cette forme, de 0 à 1.
        pub fn rarity(mut self, p: f64) -> Mob {
            self.rarity = p;
            self
        }
        /// Lui donne son propre modèle : un fichier Blockbench du dossier `modeles` du mod.
        pub fn model(mut self, file: &str) -> Mob {
            self.model = file.into();
            self
        }
        /// Ajoute un objet à son butin.
        pub fn loot(mut self, item: &str, min: u8, max: u8) -> Mob {
            self.loot.push((item.into(), min, max));
            self
        }
    }

    pub mod mobs {
        use super::*;
        /// Ajoute une créature au jeu, au chargement du mod. Renvoie son nom complet, « mod:creature ».
        pub fn register(m: Mob) -> String {
            let mut args: Vec<Val> = vec![
                m.id.as_str().into(),
                m.name.as_str().into(),
                m.base.as_str().into(),
                m.health.map_or(Val::Nil, Val::Num),
                m.damage.into(),
                m.scale.into(),
                m.color[0].into(),
                m.color[1].into(),
                m.color[2].into(),
                m.rarity.into(),
                m.model.as_str().into(),
            ];
            for (item, lo, hi) in &m.loot {
                args.extend([item.as_str().into(), Val::Num(*lo as f64), Val::Num(*hi as f64)]);
            }
            text(call(op::MOB, &args)).unwrap_or_default()
        }
    }

    /// Le monde où se trouve le joueur de l'événement en cours (le monde ordinaire, dans `on_tick`).
    pub mod world {
        use super::*;
        /// Nom du bloc à cet endroit, ou None si cette partie du monde n'est pas chargée.
        pub fn get(x: i32, y: i32, z: i32) -> Option<String> {
            text(call(op::GET_BLOCK, &[(x as f64).into(), (y as f64).into(), (z as f64).into()]))
        }
        pub fn set(x: i32, y: i32, z: i32, block: &str) -> bool {
            yes(call(op::SET_BLOCK, &[(x as f64).into(), (y as f64).into(), (z as f64).into(), block.into()]))
        }
        /// Heure du jour, de 0 à 24.
        pub fn time() -> f64 {
            match call(op::TIME, &[])[..] {
                [Val::Num(h)] => h,
                _ => 0.0,
            }
        }
        pub fn set_time(hour: f64) {
            call(op::SET_TIME, &[hour.into()]);
        }
        /// Fait naître une créature : « cerf », « oni »... ou « mod:creature ».
        pub fn spawn(mob: &str, x: f64, y: f64, z: f64) -> bool {
            yes(call(op::SPAWN, &[mob.into(), x.into(), y.into(), z.into()]))
        }
        /// Laisse tomber des objets à cet endroit.
        pub fn drop(item: &str, n: u32, x: f64, y: f64, z: f64) {
            call(op::DROP, &[item.into(), (n as f64).into(), x.into(), y.into(), z.into()]);
        }
        /// Explosion, d'une puissance de 0,5 à 8.
        pub fn explode(x: f64, y: f64, z: f64, power: f64) {
            call(op::EXPLODE, &[x.into(), y.into(), z.into(), power.into()]);
        }
        /// Hauteur du bloc plein le plus haut de cette colonne (le monde s'y charge au besoin).
        pub fn top(x: i32, z: i32) -> Option<i32> {
            match call(op::TOP, &[(x as f64).into(), (z as f64).into()])[..] {
                [Val::Num(y)] => Some(y as i32),
                _ => None,
            }
        }
        /// Remplit un pavé du même bloc (200 000 blocs au plus) ; renvoie le nombre de blocs posés.
        pub fn fill(from: [i32; 3], to: [i32; 3], block: &str) -> u32 {
            let n = |v: i32| Val::Num(v as f64);
            match call(op::FILL, &[n(from[0]), n(from[1]), n(from[2]), n(to[0]), n(to[1]), n(to[2]), block.into()])[..] {
                [Val::Num(count)] => count as u32,
                _ => 0,
            }
        }
        /// Bruitage entendu autour de cet endroit : un son du jeu, ou « mod:fichier ».
        pub fn sound(name: &str, x: f64, y: f64, z: f64, volume: f64) {
            call(op::SOUND, &[Val::Nil, name.into(), volume.into(), x.into(), y.into(), z.into()]);
        }
    }

    /// Mémoire du mod, enregistrée avec le monde.
    pub mod store {
        use super::*;
        pub fn get(key: &str) -> Option<String> {
            text(call(op::STORE_GET, &[key.into()]))
        }
        pub fn set(key: &str, value: &str) {
            call(op::STORE_SET, &[key.into(), value.into()]);
        }
        pub fn remove(key: &str) {
            call(op::STORE_SET, &[key.into(), Val::Nil]);
        }
    }
}

/// Fonctions d'un mod côté client (`client.wasm`).
pub mod client {
    use super::*;
    pub use super::Val;

    pub fn log(text: &str) {
        call(op::C_LOG, &[text.into()]);
    }

    /// Envoie un message au script serveur de ce mod.
    pub fn send(name: &str, data: &str) {
        call(op::C_SEND, &[name.into(), data.into()]);
    }

    /// Écrit une ligne dans le chat du joueur, sans l'envoyer.
    pub fn chat(text: &str) {
        call(op::C_CHAT, &[text.into()]);
    }

    /// Position du joueur.
    pub fn pos() -> [f64; 3] {
        match call(op::C_POS, &[])[..] {
            [Val::Num(x), Val::Num(y), Val::Num(z)] => [x, y, z],
            _ => [0.0; 3],
        }
    }

    fn word(v: Option<&Val>) -> &str {
        match v {
            Some(Val::Str(s)) => s,
            _ => "",
        }
    }

    /// Message envoyé par le script serveur de ce mod.
    pub fn on_message(f: impl Fn(&str, &str) + 'static) {
        on("message", move |a| {
            f(word(a.first()), word(a.get(1)));
            true
        });
    }

    /// À chaque image, pour dessiner par-dessus le jeu avec le module `draw`.
    pub fn on_hud(f: impl Fn() + 'static) {
        on("hud", move |_| {
            f();
            true
        });
    }

    /// Un clic dans la fenêtre du mod : position en points d'interface, bouton (0 gauche, 1 droit).
    pub fn on_click(f: impl Fn(f64, f64, u8) + 'static) {
        on("click", move |a| {
            if let [Val::Num(x), Val::Num(y), Val::Num(b), ..] = a {
                f(*x, *y, *b as u8);
            }
            true
        });
    }

    /// Texte tapé pendant que la fenêtre du mod est ouverte.
    pub fn on_text(f: impl Fn(&str) + 'static) {
        on("text", move |a| {
            f(word(a.first()));
            true
        });
    }

    /// La fenêtre du mod vient d'être fermée par la touche Échap.
    pub fn on_close(f: impl Fn() + 'static) {
        on("close", move |_| {
            f();
            true
        });
    }

    /// Joue un bruitage : un son du jeu, ou « mod:fichier ».
    pub fn sound(name: &str, volume: f64) {
        call(op::C_SOUND, &[name.into(), volume.into()]);
    }

    /// Une fenêtre de mod : la souris est libérée, les clics arrivent par `on_click`.
    pub mod screen {
        use super::*;
        pub fn open() {
            call(op::C_OPEN, &[]);
        }
        pub fn close() {
            call(op::C_CLOSE, &[]);
        }
        /// Position de la souris, en points d'interface.
        pub fn mouse() -> (f64, f64) {
            match call(op::C_MOUSE, &[])[..] {
                [Val::Num(x), Val::Num(y)] => (x, y),
                _ => (0.0, 0.0),
            }
        }
    }

    /// Une touche vient d'être enfoncée (« KeyG », « F6 »...).
    pub fn on_key(f: impl Fn(&str) + 'static) {
        on("key", move |a| {
            f(word(a.first()));
            true
        });
    }

    /// Dessin à l'écran, à utiliser dans `on_hud`. Les mesures sont en points d'interface.
    pub mod draw {
        use super::*;
        /// Largeur et hauteur de l'écran.
        pub fn screen() -> (f64, f64) {
            match call(op::C_SCREEN, &[])[..] {
                [Val::Num(w), Val::Num(h)] => (w, h),
                _ => (0.0, 0.0),
            }
        }
        /// Couleur : rouge, vert, bleu, opacité, de 0 à 1.
        pub fn text(x: f64, y: f64, text: &str, color: [f64; 4]) {
            call(op::C_TEXT, &[x.into(), y.into(), text.into(), color[0].into(), color[1].into(), color[2].into(), color[3].into()]);
        }
        pub fn text_width(text: &str) -> f64 {
            match call(op::C_TEXT_WIDTH, &[text.into()])[..] {
                [Val::Num(w)] => w,
                _ => 0.0,
            }
        }
        /// Image du dossier `textures` du mod, étirée dans un rectangle.
        pub fn image(x: f64, y: f64, w: f64, h: f64, file: &str, color: [f64; 4]) {
            call(op::C_IMAGE, &[x.into(), y.into(), w.into(), h.into(), file.into(), color[0].into(), color[1].into(), color[2].into(), color[3].into()]);
        }
        /// Icône d'un objet ou d'un bloc, comme dans l'inventaire.
        pub fn icon(x: f64, y: f64, size: f64, item: &str) {
            call(op::C_ICON, &[x.into(), y.into(), size.into(), item.into()]);
        }
        pub fn rect(x: f64, y: f64, w: f64, h: f64, color: [f64; 4]) {
            call(op::C_RECT, &[x.into(), y.into(), w.into(), h.into(), color[0].into(), color[1].into(), color[2].into(), color[3].into()]);
        }
    }
}
