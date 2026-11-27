//! Protocole reseau : messages binaires prefixes par leur longueur (u32 LE).
use crate::items::Slot;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::{self, Read, Write};

pub const PROTOCOL_VERSION: u32 = 18;

/// Nature d'une ligne de chat.
pub mod chat {
    /// Message d'un joueur.
    pub const SAY: u8 = 0;
    /// Annonce du jeu ou reponse a une commande.
    pub const INFO: u8 = 1;
    /// Commande refusee ou mal ecrite.
    pub const ERROR: u8 = 2;
    /// Message prive.
    pub const WHISPER: u8 = 3;
}

/// Sons que le serveur fait jouer (`ServerMsg::Sound`, champ `kind`).
pub mod sound {
    /// Tambour taiko ; `pitch` de 0 a 24 demi-tons.
    pub const DRUM: u8 = 0;
    pub const EXPLOSION: u8 = 1;
    pub const LEVER: u8 = 2;
    pub const BUTTON: u8 = 3;
    /// `pitch` 0 = le piston sort, 1 = il rentre.
    pub const PISTON: u8 = 4;
    pub const DROPPER: u8 = 5;
    /// Un coup pare au bouclier.
    pub const PARRY: u8 = 6;
    pub const CHEST_OPEN: u8 = 7;
    pub const CHEST_CLOSE: u8 = 8;
}
const MAX_FRAME: usize = 4 << 20;

/// Case visee par un clic dans une fenetre.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotRef {
    /// Inventaire du joueur : 0..9 barre rapide, 9..36 sac.
    Inv(u8),
    /// Grille d'artisanat 3x3 (l'inventaire n'utilise que 0, 1, 3, 4).
    Craft(u8),
    CraftOut,
    /// Equipement porte : 0 tete, 1 torse, 2 jambes, 3 pieds.
    Armor(u8),
    /// Conteneur ouvert. Four : 0 entree, 1 combustible, 2 sortie. Coffre : 0..27.
    Cont(u8),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    None,
    Inventory,
    Table,
    Furnace,
    Chest,
    /// Echange avec un villageois.
    Trade,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ClientMsg {
    /// `account` : jeton remis par le launcher, pour un joueur connecte a son compte.
    Hello { version: u32, name: String, view: u8, account: Option<String> },
    /// Reponse au defi du serveur : la signature du nombre recu et de l'adresse composee par le joueur.
    Proof { address: String, sig: Vec<u8> },
    Pos { pos: [f64; 3], yaw: f64, pitch: f64, on_ground: bool },
    SetFly(bool),
    /// Nouvelle distance de vue, en chunks.
    SetView(u8),
    /// Le joueur a fini de casser ce bloc.
    Dig { pos: [i32; 3] },
    /// Clic droit : (bloc vise, case voisine ou poser), ou None dans le vide.
    Use { target: Option<([i32; 3], [i32; 3])> },
    Select(u8),
    OpenInventory,
    CloseWindow,
    Click { slot: SlotRef, right: bool, shift: bool },
    /// Fabrique directement une recette a partir de l'inventaire.
    Craft { recipe: u16 },
    DropItem { all: bool },
    /// Coup porte a une creature.
    Attack { id: u32 },
    /// Clic droit sur une creature (la nourrir).
    Interact { id: u32 },
    Respawn,
    /// Voix captee par le micro : des tranches compressees (voir `shared::voice`).
    Voice { data: Vec<u8> },
    /// Ligne tapee dans le chat : un message, ou une commande si elle commence par « / ».
    Chat { text: String },
    /// Change de mode de jeu (si le monde l'autorise).
    SetMode(u8),
    /// Change la difficulte du monde (sauf en mode extreme).
    SetDifficulty(u8),
    /// Mode creatif : prend une pile de cet objet sur le curseur ; 0 jette ce que tient le curseur.
    Pick { id: u16 },
    /// Tire a l'arc (`charge` : secondes de tension, 0 a 1) ou jette l'arme de jet tenue.
    Shoot { charge: f64 },
    /// Leve ou baisse le bouclier.
    Block(bool),
    /// Deploie ou replie la robe de plumes hagoromo.
    Glide(bool),
    /// Accepte l'offre numero `index` du villageois avec qui l'on commerce.
    Trade { index: u8 },
    /// Le client s'est trop eloigne de son origine d'affichage : il demande a tout recevoir de nouveau.
    Rebase,
    /// Mesure du delai reseau : le serveur renvoie `t` tel quel, aussitot recu.
    Ping { t: u64 },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerMsg {
    Welcome { id: u32, seed: u32, spawn: [f64; 3], time: f64, dim: u8 },
    Refused { reason: String },
    /// Defi lance a un joueur qui presente un compte : il doit le signer avec sa cle de session.
    Challenge { nonce: [u8; 32] },
    Chunk { cx: i32, cz: i32, data: Vec<u8> },
    Unload { cx: i32, cz: i32 },
    /// `account` : identifiant de compte du joueur (voir `shared::account`).
    PlayerJoin { id: u32, name: String, account: [u8; 16] },
    PlayerPos { id: u32, pos: [f64; 3], yaw: f64, pitch: f64 },
    PlayerLeave { id: u32 },
    Time { time: f64 },
    BlockChange { pos: [i32; 3], b: u8, m: u8 },
    /// `armor` : les quatre pieces portees (tete, torse, jambes, pieds).
    Inventory { inv: Vec<Slot>, armor: Vec<Slot>, cursor: Slot, craft: Vec<Slot>, craft_out: Slot },
    /// Voix d'un joueur proche.
    Voice { id: u32, data: Vec<u8> },
    /// Ligne a afficher dans le chat ; `kind` est l'une des constantes du module `chat`.
    Chat { text: String, kind: u8 },
    /// Arrivee (true) ou depart d'un joueur, pour tenir la liste des noms que le chat propose.
    Roster { name: String, here: bool },
    /// Mode de jeu du joueur et regles du monde ; envoye a l'arrivee et a chaque changement.
    Rules { mode: u8, rules: crate::rules::Rules },
    /// Tenue d'un joueur, pour que les autres le voient en armure (0 = rien).
    Gear { id: u32, armor: [u16; 4] },
    Window { kind: WindowKind },
    /// Contenu du conteneur ouvert ; `progress` = (flamme, cuisson) du four, de 0 a 1.
    Container { slots: Vec<Slot>, progress: [f64; 2] },
    /// `air` : secondes de souffle restantes sous l'eau (10 = plein).
    /// `effects` : bit 0 = empoisonne, bit 1 = nausee (la faim descend plus vite).
    Stats { hp: f64, hunger: f64, air: f64, effects: u8 },
    Teleport { pos: [f64; 3] },
    DropSpawn { id: u32, pos: [f64; 3], vel: [f64; 3], item: u16 },
    DropRemove { id: u32 },
    Info { text: String },
    MobSpawn { id: u32, kind: u8, pos: [f64; 3], yaw: f64, baby: bool },
    MobPos { id: u32, pos: [f64; 3], yaw: f64 },
    MobHurt { id: u32 },
    MobRemove { id: u32 },
    /// Projectile simule aussi par le client (gravite simple).
    /// `kind` : 0 = fleche (gravite), 1 = boule de feu (ligne droite).
    ProjSpawn { id: u32, pos: [f64; 3], vel: [f64; 3], kind: u8 },
    ProjRemove { id: u32 },
    /// Recul applique au joueur.
    Knock { vel: [f64; 3] },
    /// Changement de dimension : le client oublie tout le monde charge.
    Dimension { dim: u8, pos: [f64; 3] },
    /// Ce joueur n'est plus dans la meme dimension.
    PlayerHide { id: u32 },
    /// Vie du boss de 0 a 1, ou None s'il n'y en a pas.
    Boss { hp: Option<f64> },
    /// Le joueur monte dans ce chariot, ou en descend (None).
    Ride { id: Option<u32> },
    /// Reponse a un Ping, avec la cadence reelle du serveur (ticks par seconde) et le temps moyen d'un tick (ms).
    Pong { t: u64, tps: f64, mspt: f64 },
    /// Offres du villageois dont la fenetre vient de s'ouvrir : (prix, marchandise).
    Trades { title: String, offers: Vec<(crate::items::Stack, crate::items::Stack)> },
    /// Son joue a cet endroit ; `kind` est l'une des constantes du module `sound`.
    Sound { pos: [i32; 3], kind: u8, pitch: u8 },
}

pub fn write_msg<W: Write, T: Serialize>(w: &mut W, msg: &T) -> io::Result<()> {
    let data = bincode::serialize(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    w.write_all(&(data.len() as u32).to_le_bytes())?;
    w.write_all(&data)?;
    w.flush()
}

pub fn read_msg<R: Read, T: DeserializeOwned>(r: &mut R) -> io::Result<T> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "message trop grand"));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    bincode::deserialize(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
