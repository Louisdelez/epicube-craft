//! Enchantements : ce qu'un objet peut porter, ce que chacun fait, et comment l'autel les propose.
//!
//! Un objet porte au plus quinze enchantements, chacun d'un niveau de 1 a 15 : quatre bits par
//! enchantement, ranges dans un seul nombre. Un objet sans enchantement s'ecrit comme avant dans les
//! sauvegardes et sur le reseau (voir `items::Stack`).
use crate::items::{self, Id};
use serde::{Deserialize, Serialize};

/// Les enchantements d'un objet.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Ench(pub u64);

pub const SHARPNESS: u8 = 0;
pub const KNOCKBACK: u8 = 1;
pub const LOOTING: u8 = 2;
pub const EFFICIENCY: u8 = 3;
pub const UNBREAKING: u8 = 4;
pub const FORTUNE: u8 = 5;
pub const SILK_TOUCH: u8 = 6;
pub const PROTECTION: u8 = 7;
pub const FEATHER_FALLING: u8 = 8;
pub const THORNS: u8 = 9;
pub const POWER: u8 = 10;
pub const SWEEPING: u8 = 11;
pub const INFINITY: u8 = 12;
pub const MENDING: u8 = 13;
pub const RESPIRATION: u8 = 14;
pub const COUNT: u8 = 15;

/// Ce sur quoi un enchantement se pose.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Katana, hache et armes de melee.
    Blade,
    /// Pioche, hache, pelle, houe.
    Tool,
    Armor,
    Helmet,
    Chest,
    Boots,
    Bow,
    /// Tout ce qui s'use.
    Any,
}

pub struct Def {
    pub name: &'static str,
    pub max: u8,
    pub kind: Kind,
    /// Poids au tirage : plus il est grand, plus l'autel le propose souvent.
    pub weight: u32,
    /// Vrai : l'autel ne le propose jamais (on ne l'obtient qu'autrement).
    pub rare: bool,
}

pub const DEFS: [Def; COUNT as usize] = [
    Def { name: "Tranchant", max: 5, kind: Kind::Blade, weight: 10, rare: false },
    Def { name: "Recul", max: 2, kind: Kind::Blade, weight: 5, rare: false },
    Def { name: "Butin", max: 3, kind: Kind::Blade, weight: 2, rare: false },
    Def { name: "Efficacité", max: 5, kind: Kind::Tool, weight: 10, rare: false },
    Def { name: "Solidité", max: 3, kind: Kind::Any, weight: 5, rare: false },
    Def { name: "Fortune", max: 3, kind: Kind::Tool, weight: 2, rare: false },
    Def { name: "Toucher de soie", max: 1, kind: Kind::Tool, weight: 1, rare: false },
    Def { name: "Protection", max: 4, kind: Kind::Armor, weight: 10, rare: false },
    Def { name: "Chute amortie", max: 4, kind: Kind::Boots, weight: 5, rare: false },
    Def { name: "Épines", max: 3, kind: Kind::Chest, weight: 1, rare: false },
    Def { name: "Puissance", max: 5, kind: Kind::Bow, weight: 10, rare: false },
    Def { name: "Fauchage", max: 3, kind: Kind::Blade, weight: 2, rare: false },
    Def { name: "Infinité", max: 1, kind: Kind::Bow, weight: 1, rare: false },
    Def { name: "Raccommodage", max: 1, kind: Kind::Any, weight: 2, rare: true },
    Def { name: "Apnée", max: 3, kind: Kind::Helmet, weight: 2, rare: false },
];

impl Ench {
    pub const NONE: Ench = Ench(0);

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Niveau de l'enchantement `e` (0 : absent).
    pub fn level(self, e: u8) -> u8 {
        ((self.0 >> (4 * e as u32)) & 15) as u8
    }

    pub fn with(self, e: u8, level: u8) -> Ench {
        let shift = 4 * e as u32;
        Ench((self.0 & !(15 << shift)) | ((level.min(15) as u64) << shift))
    }

    /// Les enchantements presents : (enchantement, niveau).
    pub fn list(self) -> Vec<(u8, u8)> {
        (0..COUNT).filter_map(|e| Some((e, self.level(e))).filter(|p| p.1 > 0)).collect()
    }

    /// Reunit deux jeux d'enchantements, comme sur l'enclume : le plus haut niveau l'emporte, et deux
    /// niveaux egaux donnent le suivant. Les enchantements qui s'excluent gardent celui de `self`.
    pub fn merge(self, other: Ench, id: Id) -> Ench {
        let mut out = self;
        for (e, lvl) in other.list() {
            if !fits(e, id) || out.list().iter().any(|(o, _)| *o != e && clash(*o, e)) {
                continue;
            }
            let mine = out.level(e);
            let new = if mine == lvl { (lvl + 1).min(DEFS[e as usize].max) } else { mine.max(lvl) };
            out = out.with(e, new);
        }
        out
    }
}

/// Chiffres romains des niveaux.
pub fn roman(level: u8) -> &'static str {
    ["", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"].get(level as usize).copied().unwrap_or("X+")
}

/// « Tranchant III » ; un enchantement a un seul niveau se nomme sans chiffre.
pub fn label(e: u8, level: u8) -> String {
    let d = &DEFS[e as usize];
    if d.max == 1 {
        d.name.to_string()
    } else {
        format!("{} {}", d.name, roman(level))
    }
}

/// Deux enchantements qui ne vont pas ensemble sur un meme objet.
pub fn clash(a: u8, b: u8) -> bool {
    let pair = |x: u8, y: u8| (a == x && b == y) || (a == y && b == x);
    pair(FORTUNE, SILK_TOUCH) || pair(INFINITY, MENDING)
}

/// A quelle piece d'armure correspond l'objet : 0 tete, 1 torse, 2 jambes, 3 pieds.
fn armor_slot(id: Id) -> Option<u8> {
    items::armor(id).map(|a| a.0).or((id == items::HAGOROMO).then_some(1))
}

/// L'enchantement `e` se pose-t-il sur l'objet `id` ?
pub fn fits(e: u8, id: Id) -> bool {
    if e >= COUNT || items::max_dur(id) == 0 {
        return false;
    }
    let tool = items::tool(id).map(|t| t.0);
    let blade = matches!(tool, Some(items::SWORD | items::AXE)) || matches!(id, items::NAGINATA | items::TANTO | items::TETSUBO | items::YARI);
    match DEFS[e as usize].kind {
        Kind::Blade => blade && (e != SWEEPING || matches!(tool, Some(items::SWORD)) || id == items::NAGINATA),
        Kind::Tool => tool.is_some_and(|k| k != items::SWORD) || id == items::SHEARS,
        Kind::Armor => armor_slot(id).is_some(),
        Kind::Helmet => armor_slot(id) == Some(0),
        Kind::Chest => armor_slot(id) == Some(1),
        Kind::Boots => armor_slot(id) == Some(3),
        Kind::Bow => id == items::YUMI,
        Kind::Any => true,
    }
}

/// Un objet peut-il recevoir un enchantement ?
pub fn enchantable(id: Id) -> bool {
    (0..COUNT).any(|e| fits(e, id))
}

/// Points d'experience qu'il faut pour passer du niveau `level` au suivant.
pub fn to_next(level: u32) -> u32 {
    match level {
        0..=15 => 2 * level + 7,
        16..=30 => 5 * level - 38,
        _ => 9 * level - 158,
    }
}

/// Niveau atteint avec `points` d'experience, et part du niveau suivant deja acquise (0 a 1).
pub fn level_of(points: u32) -> (u32, f32) {
    let (mut level, mut left) = (0, points);
    while left >= to_next(level) {
        left -= to_next(level);
        level += 1;
    }
    (level, left as f32 / to_next(level) as f32)
}

/// Points d'experience qu'il faut en tout pour atteindre le niveau `level`.
pub fn points_for(level: u32) -> u32 {
    (0..level).map(to_next).sum()
}

/// Petit generateur de nombres, pour que l'autel propose la meme chose tant qu'on n'a rien enchante.
fn next(seed: &mut u64) -> u32 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*seed >> 33) as u32
}

/// Une offre de l'autel : son prix en niveaux, et les enchantements qu'elle pose (le premier est montre).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    pub cost: u8,
    pub ench: Ench,
    pub shown: (u8, u8),
}

/// Les trois offres de l'autel pour l'objet `id`, entoure de `shelves` bibliotheques (15 au plus comptent).
/// `seed` change a chaque enchantement pose par le joueur.
pub fn offers(id: Id, shelves: u8, seed: u64) -> Vec<Offer> {
    if !enchantable(id) {
        return Vec::new();
    }
    let b = shelves.min(15) as u32;
    let mut rng = seed ^ ((id as u64) << 32) ^ 0x9E37_79B9_7F4A_7C15;
    let base = 1 + next(&mut rng) % 8 + b / 2 + next(&mut rng) % (b + 1);
    let costs = [(base / 3).max(1), base * 2 / 3 + 1, base.max(b * 2)];
    costs
        .into_iter()
        .map(|cost| {
            let cost = cost.min(30);
            let pool: Vec<u8> = (0..COUNT).filter(|e| fits(*e, id) && !DEFS[*e as usize].rare).collect();
            let mut ench = Ench::NONE;
            let mut shown = (0, 0);
            // Un premier enchantement, puis d'autres tant que la chance tient : plus l'offre est chere, plus elle en porte.
            let mut chance = cost + 1;
            loop {
                let free: Vec<u8> = pool.iter().copied().filter(|e| ench.level(*e) == 0 && !ench.list().iter().any(|(o, _)| clash(*o, *e))).collect();
                let total: u32 = free.iter().map(|e| DEFS[*e as usize].weight).sum();
                if total == 0 {
                    break;
                }
                let mut pick = next(&mut rng) % total;
                let e = free.into_iter().find(|e| {
                    let w = DEFS[*e as usize].weight;
                    if pick < w {
                        true
                    } else {
                        pick -= w;
                        false
                    }
                });
                let Some(e) = e else { break };
                // Le niveau suit le prix : a 30 niveaux, le plus haut ; il flotte d'un cran.
                let max = DEFS[e as usize].max as u32;
                let level = ((cost * max + next(&mut rng) % 12).div_ceil(30)).clamp(1, max) as u8;
                ench = ench.with(e, level);
                if shown.1 == 0 {
                    shown = (e, level);
                }
                if next(&mut rng) % 50 >= chance {
                    break;
                }
                chance /= 2;
            }
            Offer { cost: cost as u8, ench, shown }
        })
        .collect()
}

/// Prix, en niveaux, de la fusion de deux objets sur l'enclume : un par niveau d'enchantement obtenu, plus un.
pub fn anvil_cost(result: Ench) -> u32 {
    1 + result.list().iter().map(|(_, l)| *l as u32).sum::<u32>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_are_packed_and_listed() {
        let e = Ench::NONE.with(SHARPNESS, 3).with(UNBREAKING, 2);
        assert_eq!((e.level(SHARPNESS), e.level(UNBREAKING), e.level(LOOTING)), (3, 2, 0));
        assert_eq!(e.list(), vec![(SHARPNESS, 3), (UNBREAKING, 2)]);
        assert_eq!(e.with(SHARPNESS, 0).list(), vec![(UNBREAKING, 2)]);
        assert_eq!(label(SHARPNESS, 3), "Tranchant III");
        assert_eq!(label(SILK_TOUCH, 1), "Toucher de soie");
    }

    #[test]
    fn enchantments_fit_their_kind_of_item() {
        let (katana, pick, bow) = (items::tool_id(items::SWORD, 2), items::tool_id(items::PICK, 2), items::YUMI);
        assert!(fits(SHARPNESS, katana) && !fits(SHARPNESS, pick) && !fits(EFFICIENCY, katana));
        assert!(fits(FORTUNE, pick) && fits(POWER, bow) && !fits(POWER, katana));
        assert!(fits(FEATHER_FALLING, items::armor_id(3, 2)) && !fits(FEATHER_FALLING, items::armor_id(0, 2)));
        assert!(fits(UNBREAKING, katana) && fits(UNBREAKING, bow));
        assert!(!enchantable(items::STICK) && enchantable(katana));
    }

    #[test]
    fn experience_levels_follow_the_curve() {
        assert_eq!((to_next(0), to_next(15), to_next(16), to_next(31)), (7, 37, 42, 121));
        assert_eq!(level_of(0).0, 0);
        assert_eq!(level_of(7), (1, 0.0));
        assert_eq!(points_for(30), 1395);
        assert_eq!(level_of(points_for(30)).0, 30);
    }

    #[test]
    fn the_altar_offers_more_with_shelves_and_stays_put() {
        let katana = items::tool_id(items::SWORD, 2);
        let poor = offers(katana, 0, 42);
        let rich = offers(katana, 15, 42);
        assert_eq!(poor.len(), 3);
        assert_eq!(poor, offers(katana, 0, 42), "memes offres tant que rien ne change");
        assert!(poor[2].cost <= 8 && rich[2].cost == 30, "{} / {}", poor[2].cost, rich[2].cost);
        assert!(poor.iter().chain(&rich).all(|o| !o.ench.is_empty() && o.ench.list().iter().all(|(e, l)| fits(*e, katana) && *l <= DEFS[*e as usize].max)));
        assert!(offers(items::STICK, 15, 1).is_empty());
        // Jamais Fortune et Toucher de soie ensemble.
        let pick = items::tool_id(items::PICK, 3);
        for seed in 0..300 {
            for o in offers(pick, 15, seed) {
                assert!(!(o.ench.level(FORTUNE) > 0 && o.ench.level(SILK_TOUCH) > 0));
            }
        }
    }

    #[test]
    fn merging_raises_equal_levels() {
        let katana = items::tool_id(items::SWORD, 2);
        let a = Ench::NONE.with(SHARPNESS, 2).with(UNBREAKING, 1);
        let b = Ench::NONE.with(SHARPNESS, 2).with(KNOCKBACK, 2).with(EFFICIENCY, 3);
        let m = a.merge(b, katana);
        assert_eq!(m.list(), vec![(SHARPNESS, 3), (KNOCKBACK, 2), (UNBREAKING, 1)], "l'efficacite ne va pas sur un katana");
        assert_eq!(Ench::NONE.with(SHARPNESS, 5).merge(Ench::NONE.with(SHARPNESS, 5), katana).level(SHARPNESS), 5);
        assert_eq!(anvil_cost(m), 7);
    }
}
