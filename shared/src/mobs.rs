//! Creatures : genres et caracteristiques communes au client et au serveur.
//! Les identifiants decrivent un comportement ; leur apparence est japonaise :
//! COW = cerf sika, PIG = sanglier, SHEEP = tanuki, CHICKEN = grue, ZOMBIE = oni,
//! SKELETON = samourai squelette, SPIDER = tsuchigumo, BOMBYX = lanterne hantee,
//! SHADE = yurei, BRAISIER = wanyudo, WYRM = Ryujin.
use crate::items::{self, Id};

pub const COW: u8 = 0;
pub const PIG: u8 = 1;
pub const SHEEP: u8 = 2;
pub const CHICKEN: u8 = 3;
pub const ZOMBIE: u8 = 4;
pub const SKELETON: u8 = 5;
pub const SPIDER: u8 = 6;
/// Creature a spores qui explose pres du joueur.
pub const BOMBYX: u8 = 7;
/// Ombre qui se teleporte.
pub const SHADE: u8 = 8;
/// Esprit de feu volant de l'Abime.
pub const BRAISIER: u8 = 9;
/// Dragon du Zenith.
pub const WYRM: u8 = 10;
/// Chariot daihachi : roule sur les rails, se monte et se casse comme un objet. (11 est le modele du joueur.)
pub const CART: u8 = 12;
/// Villageois : paisible, il commerce. Son metier tient dans son age (0 a 4).
pub const VILLAGER: u8 = 13;
pub const WYRM_HP: f64 = 200.0;

pub fn hostile(kind: u8) -> bool {
    (ZOMBIE..=WYRM).contains(&kind)
}

/// (demi-largeur, hauteur) de la boite de collision d'un adulte.
pub fn size(kind: u8) -> (f64, f64) {
    match kind {
        COW => (0.4, 1.5),
        PIG => (0.42, 0.95),
        SHEEP => (0.32, 0.95),
        CHICKEN => (0.22, 1.3),
        ZOMBIE => (0.35, 2.0),
        SPIDER => (0.6, 0.8),
        BOMBYX => (0.35, 1.3),
        SHADE => (0.3, 2.5),
        BRAISIER => (0.55, 1.4),
        WYRM => (1.5, 2.0),
        CART => (0.5, 0.75),
        _ => (0.3, 1.8),
    }
}

pub fn max_hp(kind: u8) -> f64 {
    match kind {
        COW => 10.0,
        PIG => 10.0,
        SHEEP => 8.0,
        CHICKEN => 4.0,
        SPIDER => 16.0,
        SHADE => 30.0,
        BRAISIER => 14.0,
        WYRM => WYRM_HP,
        _ => 20.0,
    }
}

/// Vitesse de marche en blocs par seconde.
pub fn speed(kind: u8) -> f64 {
    match kind {
        CHICKEN | VILLAGER => 1.0,
        COW | SHEEP => 1.2,
        PIG => 1.3,
        ZOMBIE => 2.4,
        SKELETON => 2.0,
        SPIDER => 3.2,
        BOMBYX => 2.2,
        _ => 4.0,
    }
}

/// Aliment qui attire l'animal et le met en mode reproduction.
pub fn lure(kind: u8) -> Option<Id> {
    match kind {
        COW | SHEEP => Some(items::WHEAT_ITEM),
        PIG => Some(items::CARROT_ITEM),
        CHICKEN => Some(items::SEEDS),
        _ => None,
    }
}

/// Degats infliges au joueur au contact.
pub fn melee(kind: u8) -> f64 {
    match kind {
        ZOMBIE => 3.0,
        SPIDER => 2.0,
        SHADE => 5.0,
        _ => 0.0,
    }
}
