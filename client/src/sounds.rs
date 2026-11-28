//! Quel son pour quoi : la matiere des blocs, la voix des creatures, les evenements du serveur.
use crate::audio;
use glam::Vec3;
use shared::blocks::*;
use shared::mobs as mk;

/// Tous les bruitages que le jeu demande (un test verifie qu'ils existent dans le dossier audio).
#[cfg(test)]
pub const SFX: &[&str] = &[
    "pas_herbe", "pas_pierre", "pas_bois", "pas_neige", "pas_sable", "pas_eau",
    "coup_pierre", "casse_pierre", "coup_bois", "casse_bois", "coup_terre", "casse_terre", "coup_verre", "casse_verre",
    "coup_metal", "casse_metal", "coup_tissu", "casse_tissu", "pose",
    "frappe", "blessure", "lame", "parade", "tir", "fleche", "explosion",
    "mange", "ramasse", "pieces", "coffre_ouvre", "coffre_ferme", "levier", "bouton", "piston", "distributeur",
    "plouf", "gong", "grincement", "casse_outil",
    "clic", "ouvre", "ferme", "case", "fabrique",
    "cerf", "sanglier", "tanuki", "grue", "oni", "squelette", "araignee", "lanterne", "yurei", "wanyudo", "ryujin", "villageois",
];

/// Boucles d'ambiance.
#[cfg(test)]
pub const AMBIENCES: &[&str] = &["jour", "nuit", "grotte", "vent", "yomi", "sous_eau", "feu", "eau", "lave"];

use shared::proto::sound as kind;

/// Matiere d'un bloc : (son des pas, son des coups). Les coups donnent aussi "casse_<matiere>".
pub fn material(b: B) -> (&'static str, &'static str) {
    match b {
        GRASS | DIRT | FARMLAND | MOSS | GLOW_MOSS | CRIMSON_MOSS | LEAVES | CEDAR_LEAVES | MAPLE_LEAVES | WISTERIA | SAPLING | WHEAT | CARROT
        | TALLGRASS | FLOWER | CACTUS | SOY | TEA | STEM | KABOCHA | LOTUS => ("herbe", "terre"),
        SAND | GRAVEL | SOUL_SAND => ("sable", "terre"),
        SNOW => ("neige", "terre"),
        PLANKS | LOG | CRAFT | CHEST | BAMBOO | COMPOSTER | NOTE | TARGET | DOOR | LEVER | TORCH | SIGTORCH | SUNDIAL | VERMILION | BOOKSHELF | EXPLOSIVE
        | BONE_BLOCK | HOOK => ("bois", "bois"),
        GLASS | ICE | LAMP | CRYSTAL | ONIBI | ABYSS_PORTAL | ZENITH_PORTAL | GATEWAY => ("pierre", "verre"),
        IRON_BLOCK | VOLT_BLOCK | SATETSU | ANVIL | PISTON | STICKY_PISTON | PISTON_HEAD | DROPPER | HOPPER | OBSERVER | RAIL | POWERED_RAIL
        | DETECTOR_RAIL | SPAWNER | REPEATER | COMPARATOR | BUTTON | PLATE => ("pierre", "metal"),
        WOOL | BED | COBWEB | TRIPWIRE | WIRE => ("sable", "tissu"),
        _ => ("pierre", "pierre"),
    }
}

/// Bloc qui fait du bruit en continu : 0 = feu, 1 = eau, 2 = lave.
pub fn emitter(b: B, m: u8) -> Option<usize> {
    match b {
        TORCH | IRORI | EMBER | ONIBI => Some(0),
        FURNACE | SMOKER if m & 1 == 1 => Some(0),
        WATER => Some(1),
        LAVA => Some(2),
        _ => None,
    }
}

/// Voix d'une creature, et la hauteur a laquelle elle la pousse.
pub fn voice(kind: u8, baby: bool) -> Option<(&'static str, f32)> {
    let (name, pitch) = match kind {
        mk::COW => ("cerf", 0.9),
        mk::PIG => ("sanglier", 1.0),
        mk::SHEEP => ("tanuki", 1.1),
        mk::CHICKEN => ("grue", 1.25),
        mk::ZOMBIE => ("oni", 0.8),
        mk::SKELETON => ("squelette", 1.0),
        mk::SPIDER => ("araignee", 1.0),
        mk::BOMBYX => ("lanterne", 1.0),
        mk::SHADE => ("yurei", 0.9),
        mk::BRAISIER => ("wanyudo", 0.85),
        mk::WYRM => ("ryujin", 0.7),
        mk::VILLAGER => ("villageois", 1.0),
        _ => return None,
    };
    Some((name, if baby { pitch * 1.45 } else { pitch }))
}

/// Une creature donne de la voix. `stress` : 0 au repos, 1 blessee, 2 mourante.
pub fn speak(kind: u8, baby: bool, at: Vec3, stress: u8) {
    if let Some((name, pitch)) = voice(kind, baby) {
        let (vol, bend, range) = match stress {
            0 => (0.55, 1.0, 18.0),
            1 => (0.9, 1.18, 24.0),
            _ => (1.0, 0.72, 28.0),
        };
        // Le dragon s'entend de tres loin.
        let range = if kind == mk::WYRM { 90.0 } else { range };
        audio::play_tuned(name, Some(at), vol, pitch * bend, range);
    }
}

/// Un bloc vient de changer en `at` (centre du bloc) : bruit de casse ou de pose.
pub fn block_changed(old: B, new: B, at: Vec3) {
    let quiet = |b: B| matches!(b, AIR | WATER | LAVA | PISTON_HEAD | WIRE | TRIPWIRE);
    if !quiet(old) && quiet(new) {
        audio::play_at(&format!("casse_{}", material(old).1), at, 0.9);
    } else if quiet(old) && !quiet(new) {
        let m = material(new).1;
        if m == "pierre" || m == "bois" {
            audio::play_at("pose", at, 0.8);
        } else {
            audio::play_tuned(&format!("coup_{m}"), Some(at), 0.8, 0.8, audio::RANGE);
        }
    }
}

/// Son demande par le serveur.
pub fn server(kind: u8, pitch: u8, at: Vec3) {
    match kind {
        kind::DRUM => audio::drum(pitch, at),
        kind::EXPLOSION => audio::play_tuned("explosion", Some(at), 1.0, 1.0, 72.0),
        kind::LEVER => audio::play_at("levier", at, 0.8),
        kind::BUTTON => audio::play_at("bouton", at, 0.8),
        kind::PISTON => audio::play_tuned("piston", Some(at), 0.9, if pitch == 0 { 1.0 } else { 0.8 }, audio::RANGE),
        kind::DROPPER => audio::play_at("distributeur", at, 0.8),
        kind::PARRY => audio::play_at("parade", at, 1.0),
        kind::SHIELD_DOWN => audio::play_tuned("parade", Some(at), 1.0, 0.6, audio::RANGE),
        kind::CHEST_OPEN => audio::play_at("coffre_ouvre", at, 0.8),
        kind::CHEST_CLOSE => audio::play_at("coffre_ferme", at, 0.8),
        kind::ENCHANT => audio::play_tuned("pieces", Some(at), 1.0, 0.7, audio::RANGE),
        kind::ANVIL => audio::play_tuned("parade", Some(at), 1.0, 0.8, audio::RANGE),
        kind::XP => audio::play_tuned("pieces", Some(at), 0.35, 1.6, audio::RANGE),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_material_and_voice_has_its_sounds() {
        for b in 1..=ANVIL {
            let (step, hit) = material(b);
            for name in [format!("pas_{step}"), format!("coup_{hit}"), format!("casse_{hit}")] {
                assert!(SFX.contains(&name.as_str()), "{name} n'est pas dans la liste des bruitages");
            }
        }
        for kind in 0..=mk::VILLAGER {
            if let Some((name, _)) = voice(kind, false) {
                assert!(SFX.contains(&name));
            }
        }
        assert_eq!(material(SNOW).0, "neige");
        assert_eq!(material(PLANKS), ("bois", "bois"));
        assert!(voice(mk::CART, false).is_none());
    }
}
