//! Generation procedurale du monde. Fonction pure de (dimension, graine, chunk).
//!
//! La surface suit les principes de la generation de Minecraft, transposes au Japon :
//! - le relief vient de bruits superposes (continents, collines, massifs), creuse de rivieres ;
//! - le biome se choisit par la temperature et l'humidite, comme sur une carte climatique ;
//! - le sous-sol est perce de trois familles de grottes (cavernes, tunnels, boyaux) et de ravins,
//!   noyes par endroits (lacs souterrains), avec de la lave tout au fond ;
//! - les minerais se repartissent selon la profondeur, chacun avec son etage de predilection ;
//! - les structures (galeries de mine, geodes) se placent sur une grille de regions, pour etre
//!   identiques quel que soit l'ordre dans lequel les chunks sont engendres.
use crate::blocks::*;
use crate::chunk::{idx, Chunk, H, SEA};
use crate::items::{self, Slot, Stack};
use crate::util::{fbm2, hash, noise2, noise3, Rng};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Biome {
    /// Plaine de susuki.
    Plains,
    /// Foret de cerisiers.
    Forest,
    /// Dunes.
    Desert,
    /// Pays de neige.
    Snow,
    /// Alpes : roche nue et neiges eternelles.
    Mountain,
    /// Foret de grands cedres.
    Cedar,
    /// Erabliere rouge (momiji).
    Maple,
    Bamboo,
    /// Marais de lotus.
    Swamp,
    /// Terres volcaniques : basalte, sources chaudes, coulees.
    Volcano,
}

impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Biome::Plains => "Plaine de susuki",
            Biome::Forest => "Forêt de cerisiers",
            Biome::Desert => "Dunes",
            Biome::Snow => "Pays de neige",
            Biome::Mountain => "Alpes",
            Biome::Cedar => "Forêt de cèdres",
            Biome::Maple => "Érablière rouge",
            Biome::Bamboo => "Bambouseraie",
            Biome::Swamp => "Marais de lotus",
            Biome::Volcano => "Terres volcaniques",
        }
    }
}

/// Niveau sous lequel la pierre devient de l'ardoise.
pub const SLATE_Y: i32 = 64;
/// Plafond de Yomi et de ses 128 blocs de haut, quelle que soit la hauteur des chunks.
pub const YOMI_TOP: i32 = 127;
/// Cote d'une region de galeries de mine, en blocs (6 chunks).
const MINE_REGION: i32 = 96;
/// Cote d'une region de geode, en blocs (4 chunks).
const GEODE_REGION: i32 = 64;

fn smoothstep(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

/// Hauteur du sol et biome d'une colonne de la surface.
pub fn column(seed: u32, x: i32, z: i32) -> (i32, Biome) {
    let (fx, fz) = (x as f64, z as f64);
    let cont = fbm2(fx * 0.004, fz * 0.004, seed, 4);
    let hills = fbm2(fx * 0.016, fz * 0.016, seed.wrapping_add(1), 4);
    let mn = fbm2(fx * 0.006, fz * 0.006, seed.wrapping_add(2), 3);
    let m = smoothstep((mn - 0.15) / 0.35);
    // Le niveau de la mer est a 120 ; les massifs montent jusque vers 280.
    let h = 124.0 + cont * 22.0 + hills * 7.0 + m * (62.0 + hills * 26.0);
    let mut h = (h as i32).clamp(72, 300);
    let temp = fbm2(fx * 0.0025, fz * 0.0025, seed.wrapping_add(3), 2);
    let moist = fbm2(fx * 0.003, fz * 0.003, seed.wrapping_add(4), 2);
    let high = m > 0.55 && h > 158;
    // Rivieres : la ou un bruit lent change de signe, le sol s'abaisse sous le niveau de l'eau.
    let river = noise2(fx * 0.0035, fz * 0.0035, seed.wrapping_add(6)).abs();
    if river < 0.045 && h > SEA && h < 150 {
        let k = smoothstep(1.0 - river / 0.045);
        h = (h as f32 + ((SEA - 3) as f32 - h as f32) * k).round() as i32;
    }
    let temperate = (-0.25..=0.22).contains(&temp);
    // Marais : basses terres humides, aplanies au ras de l'eau.
    let swamp = temperate && moist > 0.16 && !high && h > SEA - 2 && h <= SEA + 5;
    if swamp {
        h = SEA + (noise2(fx * 0.09, fz * 0.09, seed.wrapping_add(7)) * 1.7).round() as i32;
    }
    let biome = if high {
        if noise2(fx * 0.005, fz * 0.005, seed.wrapping_add(8)) > 0.25 {
            Biome::Volcano
        } else {
            Biome::Mountain
        }
    } else if swamp {
        Biome::Swamp
    } else if temp > 0.22 {
        if moist > 0.0 {
            Biome::Bamboo
        } else {
            Biome::Desert
        }
    } else if temp < -0.25 {
        Biome::Snow
    } else if moist > 0.24 {
        Biome::Cedar
    } else if moist > 0.03 {
        Biome::Forest
    } else if moist < -0.2 {
        Biome::Maple
    } else {
        Biome::Plains
    };
    (h, biome)
}

/// Zones ou les grottes peuvent percer la surface.
fn entrance(seed: u32, x: i32, z: i32) -> bool {
    noise2(x as f64 * 0.03, z as f64 * 0.03, seed.wrapping_add(9)) > 0.5
}

/// Essences d'arbres.
const SAKURA: u8 = 0;
const SUGI: u8 = 1;
const MOMIJI: u8 = 2;

/// (hauteur du sol, hauteur du tronc, essence) si un arbre pousse sur cette colonne.
fn tree_at(seed: u32, x: i32, z: i32) -> Option<(i32, i32, u8)> {
    let (h, biome) = column(seed, x, z);
    if h <= SEA + 1 {
        return None;
    }
    let (rarity, kind) = match biome {
        Biome::Forest => (28, SAKURA),
        Biome::Cedar => (20, SUGI),
        Biome::Maple => (26, MOMIJI),
        Biome::Plains => (350, SAKURA),
        Biome::Snow => (130, SUGI),
        Biome::Swamp => (110, SAKURA),
        Biome::Bamboo => (260, SAKURA),
        _ => return None,
    };
    let r = hash(x, 7, z, seed);
    if r % rarity != 0 || entrance(seed, x, z) {
        return None;
    }
    let trunk = match kind {
        SUGI => 7 + ((r >> 8) % 4) as i32,
        MOMIJI => 4 + ((r >> 8) % 2) as i32,
        _ => 4 + ((r >> 8) % 3) as i32,
    };
    Some((h, trunk, kind))
}

/// Grotte ? Trois familles, comme autant de bruits :
/// tunnels sinueux (la ou deux bruits s'annulent ensemble), boyaux etroits (les memes, plus fins),
/// et grandes cavernes en profondeur (la ou un bruit lent depasse un seuil, de plus en plus bas).
fn cave(seed: u32, x: i32, y: i32, z: i32) -> bool {
    let (fx, fy, fz) = (x as f64, y as f64, z as f64);
    let n1 = noise3(fx * 0.035, fy * 0.05, fz * 0.035, seed.wrapping_add(20));
    if n1.abs() < 0.12 {
        let n2 = noise3(fx * 0.035, fy * 0.05, fz * 0.035, seed.wrapping_add(21));
        if n2.abs() < 0.12 {
            return true;
        }
    }
    if y < 116 {
        let n3 = noise3(fx * 0.075, fy * 0.09, fz * 0.075, seed.wrapping_add(28));
        if n3.abs() < 0.055 && noise3(fx * 0.075, fy * 0.09, fz * 0.075, seed.wrapping_add(29)).abs() < 0.055 {
            return true;
        }
    }
    if y < 108 {
        let depth = (108 - y) as f32 / 108.0;
        return noise3(fx * 0.018, fy * 0.03, fz * 0.018, seed.wrapping_add(22)) > 0.62 - depth * 0.13;
    }
    false
}

/// Ravin : longue entaille etroite, large en haut, qui se resserre vers le fond.
fn ravine(seed: u32, x: i32, y: i32, z: i32, h: i32) -> bool {
    let top = (h - 7).min(114);
    if y < 40 || y > top {
        return false;
    }
    let (fx, fz) = (x as f64, z as f64);
    if noise2(fx * 0.003, fz * 0.003, seed.wrapping_add(24)) < 0.35 {
        return false;
    }
    let width = 0.02 + 0.035 * (y - 40) as f32 / (top - 40).max(1) as f32;
    noise2(fx * 0.012, fz * 0.012, seed.wrapping_add(23)).abs() < width
}

/// Niveau de la nappe souterraine sous cette colonne (0 = terrain sec).
fn water_table(seed: u32, x: i32, z: i32) -> i32 {
    let (fx, fz) = (x as f64, z as f64);
    if noise2(fx * 0.01, fz * 0.01, seed.wrapping_add(25)) < 0.25 {
        return 0;
    }
    30 + ((noise2(fx * 0.004, fz * 0.004, seed.wrapping_add(26)) * 0.5 + 0.5) * 60.0) as i32
}

/// Paysage souterrain d'une colonne.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cavern {
    Plain,
    /// Grottes moussues, eclairees par la mousse lumineuse.
    Mossy,
    /// Grottes a concretions : stalactites et stalagmites.
    Dripstone,
}

pub fn cavern(seed: u32, x: i32, z: i32) -> Cavern {
    let n = noise2(x as f64 * 0.006, z as f64 * 0.006, seed.wrapping_add(27));
    if n > 0.3 {
        Cavern::Mossy
    } else if n < -0.3 {
        Cavern::Dripstone
    } else {
        Cavern::Plain
    }
}

fn set_m(c: &mut Chunk, x: usize, y: usize, z: usize, b: B, m: u8) {
    let i = idx(x, y, z);
    c.b[i] = b;
    c.m[i] = m;
}

/// Filons d'un minerai. `deep` : le minerai se concentre vers le bas de sa tranche ;
/// sinon il culmine au milieu. `shy` : chance de renoncer a un bloc qui toucherait une grotte.
#[allow(clippy::too_many_arguments)]
fn vein(c: &mut Chunk, rng: &mut Rng, ore: B, count: i32, lo: i32, hi: i32, deep: bool, size: i32, shy: f32) {
    for _ in 0..count {
        let (r1, r2) = (rng.f(), rng.f());
        let k = if deep { (r1 - r2).abs() } else { (r1 + r2) * 0.5 };
        let (mut x, mut y, mut z) = (rng.range(0, 16), lo + (k * (hi - lo) as f32) as i32, rng.range(0, 16));
        for _ in 0..size {
            if (0..16).contains(&x) && (1..H as i32 - 1).contains(&y) && (0..16).contains(&z) {
                let (ux, uy, uz) = (x as usize, y as usize, z as usize);
                let cur = c.get(ux, uy, uz);
                if cur == STONE || cur == SLATE {
                    let open = shy > 0.0
                        && (c.get(ux, uy + 1, uz) == AIR
                            || c.get(ux, uy - 1, uz) == AIR
                            || (ux > 0 && c.get(ux - 1, uy, uz) == AIR)
                            || (ux < 15 && c.get(ux + 1, uy, uz) == AIR)
                            || (uz > 0 && c.get(ux, uy, uz - 1) == AIR)
                            || (uz < 15 && c.get(ux, uy, uz + 1) == AIR));
                    if !(open && rng.chance(shy)) {
                        // Dans l'ardoise, le minerai garde la roche sombre autour de lui.
                        set_m(c, ux, uy, uz, ore, (cur == SLATE) as u8);
                    }
                }
            }
            match rng.range(0, 6) {
                0 => x += 1,
                1 => x -= 1,
                2 => y += 1,
                3 => y -= 1,
                4 => z += 1,
                _ => z -= 1,
            }
        }
    }
}

/// Troncon de galerie : de `from` vers `dir`, sur `len` blocs.
#[derive(Clone, Copy, Debug)]
pub struct Gallery {
    pub from: [i32; 3],
    pub dir: [i32; 2],
    pub len: i32,
}

/// Galeries de mine abandonnees de la region (rx, rz) : un carrefour, quatre bras, des embranchements.
/// Tout tient dans la region, si bien qu'un chunk n'a jamais a regarder que la sienne.
pub fn mineshaft(seed: u32, rx: i32, rz: i32) -> Vec<Gallery> {
    let mut rng = Rng::new(hash(rx, 11, rz, seed.wrapping_add(90)) as u64);
    let mut out = Vec::new();
    if !rng.chance(0.4) {
        return out;
    }
    let hub = [rx * MINE_REGION + rng.range(44, 53), rng.range(40, 100), rz * MINE_REGION + rng.range(44, 53)];
    for dir in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
        let len = rng.range(12, 25);
        out.push(Gallery { from: hub, dir, len });
        for side in [-1, 1] {
            if !rng.chance(0.55) {
                continue;
            }
            let turn = [dir[1] * side, dir[0] * side];
            let at = rng.range(5, len);
            let from = [hub[0] + dir[0] * at, hub[1], hub[2] + dir[1] * at];
            let l2 = rng.range(8, 17);
            out.push(Gallery { from, dir: turn, len: l2 });
            if rng.chance(0.35) {
                let end = [from[0] + turn[0] * l2, hub[1], from[2] + turn[1] * l2];
                out.push(Gallery { from: end, dir, len: rng.range(5, 9) });
            }
        }
    }
    out
}

/// Geode de cristal de la region (rx, rz) : (centre, rayon), s'il y en a une.
pub fn geode(seed: u32, rx: i32, rz: i32) -> Option<([i32; 3], i32)> {
    let mut rng = Rng::new(hash(rx, 13, rz, seed.wrapping_add(92)) as u64);
    if !rng.chance(0.3) {
        return None;
    }
    let r = rng.range(5, 8);
    Some(([rx * GEODE_REGION + rng.range(8, 56), rng.range(12, 96), rz * GEODE_REGION + rng.range(8, 56)], r))
}

/// Contenu d'un coffre pose par la generation : toujours le meme pour un coffre donne.
/// Chaque monde et chaque structure a son butin ; `table` est la metadonnee du coffre.
pub fn chest_loot(dim: u8, seed: u32, p: [i32; 3], table: u8) -> Vec<Slot> {
    use crate::structures as st;
    let mut rng = Rng::new(hash(p[0], p[1], p[2], seed.wrapping_add(93)) as u64);
    let mut slots: Vec<Slot> = vec![None; 27];
    // (objet, minimum, maximum, chance)
    let yomi: [(u16, i32, i32, f32); 10] = [
        (items::GOLD_INGOT, 1, 4, 0.7),
        (items::EMBER_CRYSTAL, 2, 6, 0.6),
        (OBSIDIAN as u16, 2, 5, 0.4),
        (items::IRON_INGOT, 2, 6, 0.4),
        (items::KUNAI, 3, 8, 0.35),
        (items::ARROW, 6, 12, 0.35),
        (items::DIAMOND, 1, 3, 0.25),
        (items::LIGHTER, 1, 1, 0.2),
        (items::TAMA_SHARD, 1, 2, 0.2),
        (items::TETSUBO, 1, 1, 0.06),
    ];
    let sky: [(u16, i32, i32, f32); 6] = [
        (items::PEARL, 2, 5, 0.8),
        (items::DIAMOND, 2, 6, 0.7),
        (items::PEACH, 2, 5, 0.7),
        (items::GOLD_INGOT, 3, 7, 0.6),
        (items::TAMAHAGANE, 1, 2, 0.4),
        (CRYSTAL as u16, 2, 4, 0.4),
    ];
    let house: [(u16, i32, i32, f32); 11] = [
        (items::BREAD, 1, 4, 0.7),
        (items::SEEDS, 2, 6, 0.5),
        (items::WHEAT_ITEM, 2, 6, 0.5),
        (TORCH as u16, 2, 5, 0.4),
        (items::CARROT_ITEM, 1, 4, 0.4),
        (items::KOBAN, 1, 3, 0.4),
        (items::COAL, 2, 5, 0.3),
        (items::SOY_BEANS, 2, 5, 0.3),
        (items::DRIED_TEA, 1, 3, 0.3),
        (items::BOWL, 1, 3, 0.3),
        (items::IRON_INGOT, 1, 2, 0.15),
    ];
    let shrine: [(u16, i32, i32, f32); 6] = [
        (items::KOBAN, 2, 6, 0.9),
        (items::DRIED_TEA, 2, 5, 0.5),
        (items::GOLD_INGOT, 1, 3, 0.5),
        (items::VOLT_DUST, 2, 5, 0.4),
        (TORCH as u16, 2, 6, 0.4),
        (items::PEARL, 1, 1, 0.2),
    ];
    let tomb: [(u16, i32, i32, f32); 8] = [
        (items::GOLD_INGOT, 2, 6, 0.7),
        (items::KOBAN, 3, 8, 0.7),
        (items::BONE, 2, 6, 0.6),
        (items::IRON_INGOT, 2, 6, 0.5),
        (items::DIAMOND, 1, 3, 0.35),
        (items::ROTTEN, 2, 6, 0.3),
        (items::TANTO, 1, 1, 0.1),
        (items::TAMA_SHARD, 1, 1, 0.08),
    ];
    let temple: [(u16, i32, i32, f32); 7] = [
        (items::GOLD_INGOT, 2, 5, 0.6),
        (items::KOBAN, 2, 6, 0.6),
        (items::KUNAI, 4, 8, 0.5),
        (items::ARROW, 6, 12, 0.5),
        (items::DIAMOND, 1, 3, 0.4),
        (items::IRON_INGOT, 2, 5, 0.4),
        (items::YUMI, 1, 1, 0.15),
    ];
    let ruin: [(u16, i32, i32, f32); 6] = [
        (OBSIDIAN as u16, 1, 3, 0.8),
        (items::FLINT, 1, 3, 0.6),
        (items::LIGHTER, 1, 1, 0.5),
        (items::GOLD_INGOT, 1, 3, 0.4),
        (items::EMBER_CRYSTAL, 1, 3, 0.3),
        (items::KOBAN, 1, 3, 0.3),
    ];
    let special: Option<&[(u16, i32, i32, f32)]> = match (dim, table) {
        (ABYSS, _) => Some(&yomi),
        (ZENITH, _) => Some(&sky),
        (_, st::LOOT_HOUSE) => Some(&house),
        (_, st::LOOT_SHRINE) => Some(&shrine),
        (_, st::LOOT_TOMB) => Some(&tomb),
        (_, st::LOOT_TEMPLE) => Some(&temple),
        (_, st::LOOT_RUIN) => Some(&ruin),
        _ => None,
    };
    if let Some(table) = special {
        for (id, lo, hi, chance) in table {
            if rng.chance(*chance) {
                slots[rng.range(0, 27) as usize] = Some(Stack::new(*id, rng.range(*lo, hi + 1).min(items::max_stack(*id) as i32) as u8));
            }
        }
        if dim == ZENITH {
            // Le tresor de la pagode : la robe de plumes, a coup sur.
            slots[13] = Some(Stack::new(items::HAGOROMO, 1));
        } else if slots.iter().all(|s| s.is_none()) {
            slots[13] = Some(Stack::new(table[0].0, table[0].1 as u8));
        }
        return slots;
    }
    let table: [(u16, i32, i32, f32); 18] = [
        (RAIL as u16, 4, 9, 0.6),
        (items::COAL, 2, 7, 0.6),
        (TORCH as u16, 2, 7, 0.5),
        (items::BREAD, 1, 4, 0.5),
        (items::IRON_INGOT, 1, 5, 0.45),
        (items::VOLT_DUST, 2, 7, 0.35),
        (items::ARROW, 4, 9, 0.35),
        (items::SEEDS, 2, 5, 0.3),
        (items::KABOCHA_SEEDS, 1, 3, 0.25),
        (items::GOLD_INGOT, 1, 3, 0.25),
        (items::KUNAI, 2, 5, 0.2),
        (items::FERTILIZER, 2, 5, 0.2),
        (items::DRIED_TEA, 1, 4, 0.2),
        (POWERED_RAIL as u16, 1, 4, 0.15),
        (items::CART, 1, 1, 0.08),
        (items::DIAMOND, 1, 2, 0.08),
        (items::TANTO, 1, 1, 0.05),
        (items::TAMAHAGANE, 1, 1, 0.02),
    ];
    for (id, lo, hi, chance) in table {
        if rng.chance(chance) {
            let slot = rng.range(0, 27) as usize;
            slots[slot] = Some(Stack::new(id, rng.range(lo, hi + 1).min(items::max_stack(id) as i32) as u8));
        }
    }
    if slots.iter().all(|s| s.is_none()) {
        slots[13] = Some(Stack::new(items::COAL, 3));
    }
    slots
}

pub const SURFACE: u8 = 0;
/// Monde souterrain de roche ardente et de lave.
pub const ABYSS: u8 = 1;
/// Iles flottantes dans le vide, repaire du Wyrm.
pub const ZENITH: u8 = 2;

/// Genere un chunk de la dimension demandee.
pub fn generate(dim: u8, seed: u32, cx: i32, cz: i32) -> Chunk {
    match dim {
        ABYSS => abyss(seed, cx, cz),
        ZENITH => zenith(seed, cx, cz),
        _ => surface(seed, cx, cz),
    }
}

/// Contrees de Yomi, le pays des morts.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Yomi {
    /// Landes de braise : roche nue et lave.
    Wastes,
    /// Bois de higanbana : mousse pourpre, arbres rouges, fleurs des morts.
    Crimson,
    /// Vallee des ames : sable des ames, onibi, ossements.
    Souls,
    /// Coulees de basalte.
    Basalt,
}

impl Yomi {
    pub fn name(self) -> &'static str {
        match self {
            Yomi::Wastes => "Landes de braise",
            Yomi::Crimson => "Bois de higanbana",
            Yomi::Souls => "Vallée des âmes",
            Yomi::Basalt => "Coulées de basalte",
        }
    }
}

impl Cavern {
    pub fn name(self) -> &'static str {
        match self {
            Cavern::Plain => "grottes nues",
            Cavern::Mossy => "grottes moussues",
            Cavern::Dripstone => "grottes à concrétions",
        }
    }
}

pub fn yomi_biome(seed: u32, x: i32, z: i32) -> Yomi {
    let (fx, fz) = (x as f64, z as f64);
    let n = noise2(fx * 0.008, fz * 0.008, seed.wrapping_add(45));
    if n > 0.3 {
        Yomi::Crimson
    } else if n < -0.3 {
        Yomi::Souls
    } else if noise2(fx * 0.008, fz * 0.008, seed.wrapping_add(46)) > 0.35 {
        Yomi::Basalt
    } else {
        Yomi::Wastes
    }
}

/// Cote d'une region de forteresse de Yomi, en blocs (8 chunks).
const FORT_REGION: i32 = 128;

/// Centre de la forteresse de la region (rx, rz), s'il y en a une.
pub fn fortress(seed: u32, rx: i32, rz: i32) -> Option<[i32; 3]> {
    let mut rng = Rng::new(hash(rx, 17, rz, seed.wrapping_add(95)) as u64);
    if !rng.chance(0.5) {
        return None;
    }
    Some([rx * FORT_REGION + rng.range(56, 73), rng.range(50, 62), rz * FORT_REGION + rng.range(56, 73)])
}

/// Bloc de la forteresse a cette position relative a son centre, ou None hors de l'ouvrage :
/// un donjon carre et quatre ponts en croix, portes par des piles.
fn fortress_block(dx: i32, dy: i32, dz: i32) -> Option<B> {
    let (ax, az) = (dx.abs(), dz.abs());
    if ax <= 4 && az <= 4 {
        // Donjon : sol, murs perces de quatre portes, toit ; la stele au centre, deux coffres.
        if !(0..=5).contains(&dy) {
            return None;
        }
        let wall = ax == 4 || az == 4;
        let door = wall && ax.min(az) <= 1 && (1..=3).contains(&dy);
        return Some(if dy == 0 || dy == 5 || (wall && !door) {
            BRICK
        } else if dy == 1 && dx == 0 && dz == 0 {
            SPAWNER
        } else if dy == 1 && ((dx, dz) == (3, 3) || (dx, dz) == (-3, -3)) {
            CHEST
        } else {
            AIR
        });
    }
    // Ponts : tablier de trois de large, parapets, piles tous les douze blocs.
    let (along, across) = if az <= 2 && ax <= 44 { (ax, az) } else if ax <= 2 && az <= 44 { (az, ax) } else { return None };
    match dy {
        0 => Some(BRICK),
        1 if across == 2 => Some(BRICK),
        1..=4 => Some(AIR),
        _ if dy < 0 && dy >= -40 && across == 0 && along % 12 == 8 => Some(BRICK),
        _ => None,
    }
}

fn abyss(seed: u32, cx: i32, cz: i32) -> Chunk {
    let mut c = Chunk::new();
    let top = YOMI_TOP;
    let rockish = |b: B| b == ABYSS_ROCK || b == BASALT;
    for lz in 0..16usize {
        for lx in 0..16usize {
            let (x, z) = (cx * 16 + lx as i32, cz * 16 + lz as i32);
            let (fx, fz) = (x as f64, z as f64);
            let biome = yomi_biome(seed, x, z);
            let rock = if biome == Yomi::Basalt { BASALT } else { ABYSS_ROCK };
            for y in 0..=top {
                let b = if y == 0 || y == top {
                    BEDROCK
                } else {
                    let fy = y as f64;
                    let n = noise3(fx * 0.022, fy * 0.035, fz * 0.022, seed.wrapping_add(40))
                        + 0.35 * noise3(fx * 0.07, fy * 0.07, fz * 0.07, seed.wrapping_add(41));
                    // Plancher et plafond pleins : la roche gagne pres des bords.
                    let edge = (12 - y).max(y - (top - 14)).max(0) as f32 / 12.0;
                    if n + edge * 1.3 > 0.05 {
                        rock
                    } else if y <= 30 {
                        LAVA
                    } else {
                        AIR
                    }
                };
                c.set(lx, y as usize, lz, b);
            }
            for y in 32..top as usize {
                let r = hash(x, y as i32, z, seed);
                // Blocs de braise accroches aux plafonds.
                if rockish(c.get(lx, y, lz)) && c.get(lx, y - 1, lz) == AIR && r % 30 == 0 {
                    c.set(lx, y, lz, EMBER);
                }
                // Sols : chaque contree a le sien.
                if c.get(lx, y, lz) != AIR || !rockish(c.get(lx, y - 1, lz)) || c.get(lx, y + 1, lz) != AIR {
                    continue;
                }
                match biome {
                    Yomi::Crimson => {
                        c.set(lx, y - 1, lz, CRIMSON_MOSS);
                        let inside = (2..14).contains(&lx) && (2..14).contains(&lz);
                        if r % 37 == 1 && inside && y + 9 < top as usize {
                            // Arbre rouge : tronc sombre, boule de feuillage ecarlate.
                            let trunk = 4 + (r >> 8) as usize % 3;
                            for dy in -2i32..=1 {
                                let rad: i32 = if dy >= 0 { 1 } else { 2 };
                                for dz in -rad..=rad {
                                    for dx in -rad..=rad {
                                        let (ux, uy, uz) = ((lx as i32 + dx) as usize, (y + trunk) as i32 + dy, (lz as i32 + dz) as usize);
                                        if c.get(ux, uy as usize, uz) == AIR && !(dx.abs() == rad && dz.abs() == rad && rad == 2) {
                                            c.set(ux, uy as usize, uz, MAPLE_LEAVES);
                                        }
                                    }
                                }
                            }
                            for dy in 0..trunk {
                                c.set(lx, y + dy, lz, LOG);
                            }
                        } else if r % 6 == 0 {
                            c.set(lx, y, lz, FLOWER);
                        }
                    }
                    Yomi::Souls => {
                        c.set(lx, y - 1, lz, SOUL_SAND);
                        if rockish(c.get(lx, y - 2, lz)) {
                            c.set(lx, y - 2, lz, SOUL_SAND);
                        }
                        if r % 23 == 0 {
                            c.set(lx, y, lz, ONIBI);
                        } else if r % 97 == 1 {
                            // Ossements a demi enfouis.
                            for dy in 0..1 + (r >> 8) as usize % 3 {
                                if c.get(lx, y + dy, lz) == AIR {
                                    c.set(lx, y + dy, lz, BONE_BLOCK);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    // Forteresse : donjon et ponts de tuiles sombres jetes au-dessus de la lave.
    let (x0, z0) = (cx * 16, cz * 16);
    if let Some(f) = fortress(seed, x0.div_euclid(FORT_REGION), z0.div_euclid(FORT_REGION)) {
        for lz in 0..16usize {
            for lx in 0..16usize {
                let (dx, dz) = (x0 + lx as i32 - f[0], z0 + lz as i32 - f[2]);
                if dx.abs() > 44 || dz.abs() > 44 {
                    continue;
                }
                for y in (f[1] - 40).max(1)..=f[1] + 5 {
                    let Some(b) = fortress_block(dx, y - f[1], dz) else { continue };
                    // Les piles s'arretent a la roche.
                    if y < f[1] && c.get(lx, y as usize, lz) != AIR && c.get(lx, y as usize, lz) != LAVA {
                        continue;
                    }
                    c.set(lx, y as usize, lz, b);
                }
            }
        }
    }
    // Minerais : l'or affleure partout, le sable de fer se cache tout en bas, jamais a l'air libre.
    let mut rng = Rng::new(hash(cx, 1, cz, seed.wrapping_add(78)) as u64);
    let mut lode = |c: &mut Chunk, ore: B, meta: u8, count: i32, lo: i32, hi: i32, size: i32, hidden: bool| {
        for _ in 0..count {
            let (mut x, mut y, mut z) = (rng.range(1, 15), rng.range(lo, hi), rng.range(1, 15));
            for _ in 0..size {
                let (ux, uy, uz) = (x.clamp(1, 14) as usize, y.clamp(2, H as i32 - 3) as usize, z.clamp(1, 14) as usize);
                let open = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)]
                    .iter()
                    .any(|(dx, dy, dz)| matches!(c.get((ux as i32 + dx) as usize, (uy as i32 + dy) as usize, (uz as i32 + dz) as usize), AIR | LAVA));
                if rockish(c.get(ux, uy, uz)) && !(hidden && open) {
                    set_m(c, ux, uy, uz, ore, meta);
                }
                match rng.range(0, 3) {
                    0 => x += rng.range(-1, 2),
                    1 => y += rng.range(-1, 2),
                    _ => z += rng.range(-1, 2),
                }
            }
        }
    };
    lode(&mut c, GOLD_ORE, 2, 9, 10, 116, 6, false);
    lode(&mut c, SATETSU, 0, 2, 8, 25, 3, true);
    c
}

/// Centre de l'ile de la pagode, loin au nord de l'ile du Ryujin.
pub const PAGODA: [i32; 2] = [0, 380];
/// Rayon du cercle des piliers d'obsidienne.
const PILLAR_RING: f32 = 42.0;

/// Hauteur du sol d'une ile de Takamagahara et son epaisseur, ou None dans le vide.
fn zenith_column(seed: u32, x: i32, z: i32) -> Option<(i32, i32)> {
    let (fx, fz) = (x as f32, z as f32);
    let d = (fx * fx + fz * fz).sqrt();
    let dp = ((fx - PAGODA[0] as f32).powi(2) + (fz - PAGODA[1] as f32).powi(2)).sqrt();
    if dp < 30.0 {
        // Ile de la pagode : toujours la, quel que soit le hasard des ilots.
        let n = fbm2(fx * 0.02, fz * 0.02, seed.wrapping_add(52), 3);
        let thick = (1.0 - (dp / 30.0).powi(2)) * 14.0 + n * 3.0;
        (thick >= 1.0).then_some((64, thick as i32))
    } else if d < 72.0 {
        let n = fbm2(fx * 0.02, fz * 0.02, seed.wrapping_add(50), 3);
        let thick = (1.0 - (d / 72.0).powi(2)) * 26.0 + n * 5.0;
        (thick >= 1.0).then(|| (64 + (n * 3.0) as i32, thick as i32))
    } else if d > 150.0 {
        // Ilots lointains.
        let n = fbm2(fx * 0.012, fz * 0.012, seed.wrapping_add(51), 3);
        (n > 0.3).then(|| (62 + (n * 8.0) as i32, ((n - 0.3) * 90.0) as i32 + 1))
    } else {
        None
    }
}

/// Centre du pilier `k` (0 a 7).
fn pillar(k: i32) -> (i32, i32) {
    let a = k as f32 * std::f32::consts::FRAC_PI_4;
    ((a.cos() * PILLAR_RING) as i32, (a.sin() * PILLAR_RING) as i32)
}

/// Positions des huit cristaux poses au sommet des piliers : tant qu'il en reste, le Ryujin se soigne.
pub fn zenith_crystals(seed: u32) -> Vec<[i32; 3]> {
    (0..8)
        .filter_map(|k| {
            let (px, pz) = pillar(k);
            zenith_column(seed, px, pz).map(|(top, _)| [px, top + 16 + (k % 3) * 6, pz])
        })
        .collect()
}

/// Bloc de la pagode a cette position relative a son pied : trois etages de bois,
/// avant-toits de tuiles a chaque niveau, deux volees de marches, le tresor tout en haut.
fn pagoda_block(dx: i32, dy: i32, dz: i32) -> Option<B> {
    const HALF: i32 = 3;
    let (ax, az) = (dx.abs(), dz.abs());
    let m = ax.max(az);
    if !(1..=21).contains(&dy) || m > HALF + 2 {
        return None;
    }
    if dy > 15 {
        // Toit en pyramide, puis fleche.
        return if dy <= 19 && m <= HALF + 1 - (dy - 16) {
            Some(BRICK)
        } else if m == 0 {
            Some(LOG)
        } else {
            None
        };
    }
    let (storey, level) = ((dy - 1) / 5, (dy - 1) % 5);
    if level == 4 {
        // Plancher de l'etage suivant, borde d'un avant-toit ; une tremie laisse passer l'escalier.
        if m > HALF {
            return Some(BRICK);
        }
        let hole = match storey {
            0 => dz == 2 && (dx == 0 || dx == 1),
            1 => dz == -2 && (dx == 0 || dx == -1),
            _ => false,
        };
        return Some(if hole { AIR } else { PLANKS });
    }
    if m > HALF {
        return None;
    }
    if m == HALF {
        if ax == HALF && az == HALF {
            return Some(LOG);
        }
        // Portes au rez-de-chaussee, fenetres au-dessus, au milieu de chaque face.
        let middle = ax.min(az) == 0;
        return Some(if middle && ((storey == 0 && level < 2) || (storey > 0 && level == 1)) { AIR } else { PLANKS });
    }
    // Marches : une volee vers +X au rez-de-chaussee, une vers -X au premier.
    let step = match storey {
        0 if dz == 2 => Some(dx + 2),
        1 if dz == -2 => Some(2 - dx),
        _ => None,
    };
    if step == Some(level) {
        return Some(PLANKS);
    }
    Some(match (storey, level, m) {
        (0, 0, 0) => SPAWNER,
        (2, 0, 0) => CHEST,
        _ => AIR,
    })
}

fn zenith(seed: u32, cx: i32, cz: i32) -> Chunk {
    let mut c = Chunk::new();
    let crystals = zenith_crystals(seed);
    for lz in 0..16usize {
        for lx in 0..16usize {
            let (x, z) = (cx * 16 + lx as i32, cz * 16 + lz as i32);
            let Some((top, thick)) = zenith_column(seed, x, z) else { continue };
            for y in (top - thick).max(1)..=top {
                c.set(lx, y as usize, lz, ZENITH_STONE);
            }
            // Huit piliers d'obsidienne en cercle, un cristal au sommet de chacun.
            for k in 0..8 {
                let (px, pz) = pillar(k);
                if (x - px).pow(2) + (z - pz).pow(2) <= 6 {
                    for y in top + 1..top + 16 + (k % 3) * 6 {
                        c.set(lx, y as usize, lz, OBSIDIAN);
                    }
                }
            }
            if let Some(p) = crystals.iter().find(|p| p[0] == x && p[2] == z) {
                c.set(lx, p[1] as usize, lz, CRYSTAL);
            }
            // Portail de retour au centre.
            if x == 0 && z == 0 {
                c.set(lx, top as usize, lz, BEDROCK);
                c.set(lx, top as usize + 1, lz, ZENITH_PORTAL);
            }
            let (dx, dz) = (x - PAGODA[0], z - PAGODA[1]);
            let far = (x as f32).hypot(z as f32) > 150.0;
            if dx.abs() <= 5 && dz.abs() <= 5 {
                // Pagode, sur un socle a niveau.
                for dy in 1..=21 {
                    if let Some(b) = pagoda_block(dx, dy, dz) {
                        c.set(lx, (top + dy) as usize, lz, b);
                    }
                }
            } else if (dx, dz) == (0, -9) {
                // Porte celeste du retour, devant la pagode.
                c.set(lx, top as usize + 1, lz, GATEWAY);
            } else if far && thick >= 4 && (2..14).contains(&lx) && (2..14).contains(&lz) && hash(x, 9, z, seed) % 55 == 0 {
                // Glycine celeste : tronc court, grappes mauves qui retombent.
                let h = top as usize + 3 + hash(x, 10, z, seed) as usize % 2;
                for y in top as usize + 1..=h {
                    c.set(lx, y, lz, LOG);
                }
                for dz2 in -2i32..=2 {
                    for dx2 in -2i32..=2 {
                        let (ux, uz) = ((lx as i32 + dx2) as usize, (lz as i32 + dz2) as usize);
                        let edge = dx2.abs().max(dz2.abs());
                        let hang = if edge == 2 { hash(x + dx2, 11, z + dz2, seed) as usize % 3 } else { 0 };
                        for y in h + 1 - hang..=h + 1 + (edge < 2) as usize {
                            if c.get(ux, y, uz) == AIR && !(dx2.abs() == 2 && dz2.abs() == 2) {
                                c.set(ux, y, uz, WISTERIA);
                            }
                        }
                    }
                }
            }
        }
    }
    c
}

fn surface(seed: u32, cx: i32, cz: i32) -> Chunk {
    let mut c = Chunk::new();
    let (x0, z0) = (cx * 16, cz * 16);
    let mut hs = [[0i32; 16]; 16];
    let mut biomes = [[Biome::Plains; 16]; 16];
    // Terrain, eau et grottes.
    for lz in 0..16usize {
        for lx in 0..16usize {
            let (x, z) = (x0 + lx as i32, z0 + lz as i32);
            let (h, biome) = column(seed, x, z);
            hs[lz][lx] = h;
            biomes[lz][lx] = biome;
            let ent = entrance(seed, x, z);
            let shore = h <= SEA + 1;
            let top = match biome {
                Biome::Swamp => {
                    if h >= SEA {
                        GRASS
                    } else {
                        DIRT
                    }
                }
                _ if shore => {
                    if h > SEA - 4 {
                        SAND
                    } else {
                        GRAVEL
                    }
                }
                Biome::Desert => SAND,
                Biome::Snow => SNOW,
                Biome::Volcano => BASALT,
                Biome::Mountain => {
                    if h > 200 {
                        SNOW
                    } else {
                        STONE
                    }
                }
                _ => GRASS,
            };
            let filler = match biome {
                Biome::Swamp => DIRT,
                _ if shore => SAND,
                Biome::Desert => SAND,
                Biome::Mountain => STONE,
                Biome::Volcano => BASALT,
                _ => DIRT,
            };
            let table = water_table(seed, x, z);
            for y in 0..=h.max(SEA) {
                // Roche-mere : un plancher plein, puis de plus en plus trouee sur trois blocs.
                let bedrock = y == 0 || (y <= 3 && hash(x, y, z, seed.wrapping_add(30)) % (y as u32 + 1) == 0);
                let mut b = if bedrock {
                    BEDROCK
                } else if y > h {
                    if y == SEA && biome == Biome::Snow {
                        ICE
                    } else {
                        WATER
                    }
                } else if y == h {
                    top
                } else if y > h - 4 {
                    filler
                } else if y < SLATE_Y + (hash(x, y, z, seed.wrapping_add(31)) % 8) as i32 {
                    SLATE
                } else {
                    STONE
                };
                if !bedrock && y <= h {
                    let limit = if shore {
                        h - 8
                    } else if ent {
                        h + 1
                    } else {
                        h - 5
                    };
                    if y < limit && (cave(seed, x, y, z) || ravine(seed, x, y, z, h)) {
                        // Lave au fond, nappe d'eau la ou le terrain est noye, air ailleurs.
                        b = if y < 12 {
                            LAVA
                        } else if y <= table {
                            WATER
                        } else {
                            AIR
                        };
                    }
                }
                c.set(lx, y as usize, lz, b);
            }
            // Terres volcaniques : sources chaudes et flaques de lave dans les creux.
            if biome == Biome::Volcano && h > SEA + 2 && c.get(lx, h as usize, lz) == BASALT {
                let pool = noise2(x as f64 * 0.11, z as f64 * 0.11, seed.wrapping_add(32));
                if pool > 0.5 && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|(dx, dz)| column(seed, x + dx, z + dz).0 >= h) {
                    c.set(lx, h as usize, lz, if pool > 0.78 { LAVA } else { WATER });
                }
            }
        }
    }

    // Decor des grottes : mousse et mousse lumineuse, ou stalactites et stalagmites.
    for lz in 0..16usize {
        for lx in 0..16usize {
            let (x, z) = (x0 + lx as i32, z0 + lz as i32);
            let kind = cavern(seed, x, z);
            for y in 5..(hs[lz][lx] - 6).clamp(5, 126) as usize {
                if c.get(lx, y, lz) != AIR {
                    continue;
                }
                let rock = |b: B| b == STONE || b == SLATE;
                let (below, above) = (c.get(lx, y - 1, lz), c.get(lx, y + 1, lz));
                let r = hash(x, y as i32, z, seed.wrapping_add(33));
                match kind {
                    Cavern::Mossy => {
                        if rock(below) && r % 10 < 7 {
                            c.set(lx, y - 1, lz, MOSS);
                            if r % 9 == 0 {
                                c.set(lx, y, lz, TALLGRASS);
                            }
                        }
                        if rock(above) && (r >> 8) % 9 == 0 {
                            c.set(lx, y + 1, lz, GLOW_MOSS);
                        }
                    }
                    Cavern::Dripstone => {
                        if rock(above) && (r >> 8) % 5 == 0 {
                            set_m(&mut c, lx, y, lz, STALACTITE, 1);
                        } else if rock(below) && r % 7 == 0 {
                            c.set(lx, y, lz, STALACTITE);
                        }
                    }
                    // Partout ailleurs, un peu de mousse lumineuse de loin en loin.
                    Cavern::Plain => {
                        if rock(above) && r % 70 == 0 {
                            c.set(lx, y + 1, lz, GLOW_MOSS);
                        }
                    }
                }
            }
        }
    }

    // Geode : coquille de basalte, paroi de cristal, coeur vide.
    if let Some((g, radius)) = geode(seed, x0.div_euclid(GEODE_REGION), z0.div_euclid(GEODE_REGION)) {
        for lz in 0..16usize {
            for lx in 0..16usize {
                let (dx, dz) = (x0 + lx as i32 - g[0], z0 + lz as i32 - g[2]);
                if dx.abs() > radius || dz.abs() > radius || g[1] + radius + 6 > hs[lz][lx] {
                    continue;
                }
                for y in (g[1] - radius).max(4)..=g[1] + radius {
                    let d = (((dx * dx + dz * dz + (y - g[1]).pow(2)) as f32).sqrt() + 0.5) as i32;
                    let b = match radius - d {
                        i32::MIN..=-1 => continue,
                        0 => BASALT,
                        1 => CRYSTAL,
                        _ => AIR,
                    };
                    c.set(lx, y as usize, lz, b);
                }
            }
        }
    }

    // Galeries de mine abandonnees : couloirs de 3 x 3, etais de bois, rails, toiles et coffres.
    for g in mineshaft(seed, x0.div_euclid(MINE_REGION), z0.div_euclid(MINE_REGION)) {
        let y = g.from[1] as usize;
        let side = [g.dir[1], g.dir[0]];
        for t in 0..=g.len {
            for w in -1..=1 {
                let (x, z) = (g.from[0] + g.dir[0] * t + side[0] * w, g.from[2] + g.dir[1] * t + side[1] * w);
                let (lx, lz) = (x - x0, z - z0);
                if !(0..16).contains(&lx) || !(0..16).contains(&lz) {
                    continue;
                }
                let (lx, lz) = (lx as usize, lz as usize);
                // Trop pres de la surface, ou sous la mer : la galerie s'interrompt.
                if g.from[1] + 8 > hs[lz][lx] || hs[lz][lx] <= SEA + 1 {
                    continue;
                }
                let r = hash(x, g.from[1], z, seed.wrapping_add(94));
                let post = t % 5 == 0;
                for dy in 0..3usize {
                    let cur = c.get(lx, y + dy, lz);
                    if cur == WATER || cur == LAVA {
                        continue;
                    }
                    let b = if post && dy == 2 {
                        PLANKS
                    } else if post && w != 0 {
                        LOG
                    } else if dy == 2 && w != 0 && r % 13 == 0 {
                        COBWEB
                    } else {
                        AIR
                    };
                    c.set(lx, y + dy, lz, b);
                }
                // Passerelle de planches au-dessus du vide.
                let floor = c.get(lx, y - 1, lz);
                if floor == AIR || floor == STALACTITE || floor == TALLGRASS {
                    c.set(lx, y - 1, lz, PLANKS);
                }
                if c.get(lx, y, lz) == AIR && !matches!(c.get(lx, y - 1, lz), WATER | LAVA) {
                    if w == 0 && t == g.len && r % 2 == 0 {
                        // Coffre au bout de la galerie.
                        c.set(lx, y, lz, CHEST);
                    } else if w == 0 && t > 0 && r % 10 < 6 {
                        // Rails par troncons, dans l'axe du couloir.
                        set_m(&mut c, lx, y, lz, RAIL, (g.dir[0] == 0) as u8);
                    } else if w == 1 && t % 10 == 7 && r % 3 == 0 {
                        c.set(lx, y, lz, TORCH);
                    }
                }
            }
        }
    }

    // Taniere : salle de pierre moussue, une stele maudite au centre, deux coffres.
    let den = hash(cx, 5, cz, seed.wrapping_add(91));
    let dy0 = 16 + (den >> 8) as i32 % 84;
    if den % 14 == 0 && hs[7][7] > SEA + 2 && (4..11).all(|i| hs[i][7] > dy0 + 11 && hs[7][i] > dy0 + 11) {
        for lz in 4..11usize {
            for lx in 4..11usize {
                for dy in 0..5usize {
                    let wall = lx == 4 || lx == 10 || lz == 4 || lz == 10 || dy == 0 || dy == 4;
                    let r = hash(lx as i32, dy as i32, lz as i32, den);
                    let b = if !wall {
                        AIR
                    } else if r % 3 == 0 {
                        MOSS
                    } else {
                        COBBLE
                    };
                    c.set(lx, dy0 as usize + dy, lz, b);
                }
            }
        }
        let floor = dy0 as usize + 1;
        c.set(7, floor, 7, SPAWNER);
        c.set(5, floor, 7, CHEST);
        c.set(9, floor, 7 - (den >> 4) as usize % 2 * 2 + 1, CHEST);
    }

    // Minerais : chacun a son etage. Le charbon partout, le fer au milieu, l'or et le reiseki en bas,
    // le diamant tout au fond, et plus rare encore s'il affleure dans une grotte.
    let mut rng = Rng::new(hash(cx, 0, cz, seed.wrapping_add(77)) as u64);
    // Etages de Minecraft, decales de 64 : notre niveau 0 est son niveau -64.
    vein(&mut c, &mut rng, GRAVEL, 12, 8, 150, false, 14, 0.0);
    vein(&mut c, &mut rng, DIRT, 6, 90, 160, false, 14, 0.0);
    vein(&mut c, &mut rng, COAL_ORE, 60, 64, 256, false, 12, 0.0);
    vein(&mut c, &mut rng, IRON_ORE, 24, 40, 120, false, 6, 0.0);
    vein(&mut c, &mut rng, IRON_ORE, 14, 144, 300, false, 5, 0.0);
    vein(&mut c, &mut rng, GOLD_ORE, 8, 2, 96, false, 5, 0.0);
    vein(&mut c, &mut rng, VOLT_ORE, 14, 2, 80, true, 5, 0.0);
    vein(&mut c, &mut rng, DIAMOND_ORE, 2, 2, 80, true, 4, 0.5);

    // Plantes, selon le biome.
    for lz in 0..16usize {
        for lx in 0..16usize {
            let (x, z) = (x0 + lx as i32, z0 + lz as i32);
            let (h, biome) = (hs[lz][lx], biomes[lz][lx]);
            let r = hash(x, 3, z, seed);
            // Lotus poses sur l'eau calme des marais.
            if biome == Biome::Swamp && h < SEA && r % 6 == 0 && c.get(lx, SEA as usize, lz) == WATER {
                c.set(lx, SEA as usize + 1, lz, LOTUS);
            }
            if h <= SEA || h as usize + 10 >= H {
                continue;
            }
            let hu = h as usize;
            let bamboo = |c: &mut Chunk, n: usize| {
                for dy in 1..=n {
                    if c.get(lx, hu + dy, lz) == AIR {
                        c.set(lx, hu + dy, lz, BAMBOO);
                    }
                }
            };
            match c.get(lx, hu, lz) {
                GRASS if c.get(lx, hu + 1, lz) == AIR => match biome {
                    Biome::Bamboo => {
                        if r % 5 == 0 {
                            bamboo(&mut c, 3 + (r / 5) as usize % 6);
                        } else if r % 9 == 1 {
                            c.set(lx, hu + 1, lz, TALLGRASS);
                        }
                    }
                    Biome::Swamp => {
                        if r % 5 == 0 {
                            c.set(lx, hu + 1, lz, TALLGRASS);
                        }
                    }
                    _ => {
                        let grass = if biome == Biome::Plains { 5 } else { 8 };
                        let flowers = if biome == Biome::Maple { 19 } else { 31 };
                        if r % grass == 0 {
                            c.set(lx, hu + 1, lz, TALLGRASS);
                        } else if r % flowers == 0 {
                            c.set(lx, hu + 1, lz, FLOWER);
                        } else if r % 233 == 0 && biome != Biome::Cedar {
                            // Theier sauvage.
                            c.set(lx, hu + 1, lz, TEA);
                        } else if r % 307 == 0 {
                            c.set(lx, hu + 1, lz, KABOCHA);
                        } else if r % 149 == 0 && matches!(biome, Biome::Forest | Biome::Plains) {
                            bamboo(&mut c, 3 + (r / 149) as usize % 5);
                        }
                    }
                },
                SAND if biome == Biome::Desert && r % 160 == 0 => {
                    for k in 1..=(2 + (r >> 9) % 2) as usize {
                        c.set(lx, hu + k, lz, CACTUS);
                    }
                }
                _ => {}
            }
        }
    }

    // Arbres : on parcourt aussi les colonnes voisines pour les feuillages qui debordent.
    for z in z0 - 3..z0 + 19 {
        for x in x0 - 3..x0 + 19 {
            let Some((h, trunk, kind)) = tree_at(seed, x, z) else { continue };
            let mut put = |px: i32, py: i32, pz: i32, b: B| {
                let (lx, lz) = (px - x0, pz - z0);
                if (0..16).contains(&lx) && (0..16).contains(&lz) && (1..H as i32).contains(&py) {
                    let (ux, uy, uz) = (lx as usize, py as usize, lz as usize);
                    let cur = c.get(ux, uy, uz);
                    if matches!(cur, AIR | TALLGRASS | FLOWER | BAMBOO) || (b == LOG && matches!(cur, LEAVES | CEDAR_LEAVES | MAPLE_LEAVES)) {
                        c.set(ux, uy, uz, b);
                    }
                }
            };
            let top = h + trunk;
            match kind {
                // Cedre : fut elance, branches en etages qui s'elargissent vers le bas.
                SUGI => {
                    put(x, top + 1, z, CEDAR_LEAVES);
                    for i in 0..trunk - 2 {
                        let r: i32 = if i % 2 == 0 { 1 } else if i > 4 { 3 } else { 2 };
                        for dz in -r..=r {
                            for dx in -r..=r {
                                if dx.abs() + dz.abs() <= r + (r > 1) as i32 {
                                    put(x + dx, top - i, z + dz, CEDAR_LEAVES);
                                }
                            }
                        }
                    }
                    for y in h + 1..=top {
                        put(x, y, z, LOG);
                    }
                }
                // Erable : tronc court, large couronne aplatie.
                MOMIJI => {
                    for (dy, r) in [(-2, 2), (-1, 3), (0, 3), (1, 2)] {
                        for dz in -r..=r {
                            for dx in -r..=r {
                                let far = dx * dx + dz * dz;
                                if far <= r * r + 1 && !(far > r * r - 2 && hash(x + dx, top + dy, z + dz, seed) % 3 == 0) {
                                    put(x + dx, top + dy, z + dz, MAPLE_LEAVES);
                                }
                            }
                        }
                    }
                    for y in h + 1..=top - 1 {
                        put(x, y, z, LOG);
                    }
                }
                // Cerisier : boule de feuillage.
                _ => {
                    for dy in -2..=1 {
                        let r: i32 = if dy >= 0 { 1 } else { 2 };
                        for dz in -r..=r {
                            for dx in -r..=r {
                                let corner = dx.abs() == r && dz.abs() == r;
                                if corner && (dy == 1 || hash(x + dx, top + dy, z + dz, seed) % 2 == 0) {
                                    continue;
                                }
                                put(x + dx, top + dy, z + dz, LEAVES);
                            }
                        }
                    }
                    for y in h + 1..=top - 1 {
                        put(x, y, z, LOG);
                    }
                }
            }
        }
    }
    // Villages, sanctuaires, tombes, temples, ruines : par-dessus le paysage.
    crate::structures::build(&mut c, seed, cx, cz, &hs);
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const SEED: u32 = 20251;

    fn count(c: &Chunk, b: B) -> usize {
        c.b.iter().filter(|x| **x == b).count()
    }

    #[test]
    fn deterministic_with_a_sealed_floor() {
        let (a, b) = (surface(SEED, 3, -5), surface(SEED, 3, -5));
        assert_eq!(a.b, b.b);
        assert_eq!(a.m, b.m);
        for z in 0..16 {
            for x in 0..16 {
                assert_eq!(a.get(x, 0, z), BEDROCK, "pas de trou dans la roche-mere");
            }
        }
        assert_ne!(surface(SEED + 1, 3, -5).b, a.b);
    }

    #[test]
    fn terrain_stays_smooth_at_the_world_border() {
        // A 30 millions de blocs, un flottant simple avance par pas de 2 blocs : le bruit lu en simple
        // precision donnerait la meme valeur a deux colonnes voisines. Lu en double, chaque colonne a la sienne.
        let far = crate::WORLD_BORDER as i32 - 1000;
        let fine: Vec<f32> = (0..64).map(|i| fbm2((far + i) as f64 * 0.016, 0.5, SEED, 4)).collect();
        assert!(fine.windows(2).all(|w| w[0] != w[1]), "le relief ne se fige pas en marches");
        let coarse: Vec<f32> = (0..64).map(|i| fbm2(((far + i) as f32 * 0.016) as f64, 0.5, SEED, 4)).collect();
        assert!(coarse.windows(2).filter(|w| w[0] == w[1]).count() > 10, "c'est bien ce que la simple precision aurait donne");
        // Le terrain y est aussi doux qu'ailleurs, et les chunks s'y engendrent normalement.
        for i in 0..200 {
            let (a, b) = (column(SEED, far + i, -far).0, column(SEED, far + i + 1, -far).0);
            // (Un bord de marais ou de riviere peut faire une marche de quelques blocs, la comme ailleurs.)
            assert!((a - b).abs() <= 12, "falaise de {} blocs en x = {}", (a - b).abs(), far + i);
        }
        let c = surface(SEED, far >> 4, -far >> 4);
        assert_eq!(c.get(0, 0, 0), BEDROCK);
        assert!(c.b.iter().any(|b| *b == STONE) && c.b.iter().any(|b| *b == AIR));
        assert_eq!(c.b, surface(SEED, far >> 4, -far >> 4).b);
        // Les structures se placent aussi loin que pres.
        let found = (0..400).filter(|k| crate::structures::site(SEED, far / crate::structures::REGION - k, 7).is_some()).count();
        assert!(found > 10, "structures pres de la bordure : {found}");
    }

    #[test]
    fn many_biomes_and_rivers() {
        let mut seen = HashSet::new();
        let mut rivers = 0;
        for z in (-3000..3000).step_by(40) {
            for x in (-3000..3000).step_by(40) {
                let (h, biome) = column(SEED, x, z);
                seen.insert(biome);
                assert!((1..H as i32 - 8).contains(&h));
                rivers += (noise2(x as f64 * 0.0035, z as f64 * 0.0035, SEED + 6).abs() < 0.02 && h < SEA) as u32;
            }
        }
        assert!(seen.len() >= 9, "biomes rencontres : {seen:?}");
        assert!(rivers > 20, "les rivieres creusent leur lit : {rivers}");
    }

    #[test]
    fn ores_keep_to_their_depths() {
        let (mut coal, mut diamond, mut deep_ore) = (0, 0, 0);
        for cz in 0..6 {
            for cx in 0..6 {
                let c = surface(SEED, cx, cz);
                coal += count(&c, COAL_ORE);
                diamond += count(&c, DIAMOND_ORE);
                for y in 0..H {
                    for z in 0..16 {
                        for x in 0..16 {
                            let b = c.get(x, y, z);
                            assert!(b != DIAMOND_ORE || y < 90, "diamant a y = {y}");
                            assert!(b != VOLT_ORE || y < 90, "reiseki a y = {y}");
                            assert!(b != SLATE || y < (SLATE_Y + 8) as usize, "ardoise a y = {y}");
                            assert!(b != STONE || y >= SLATE_Y as usize, "pierre claire a y = {y}");
                            if matches!(b, DIAMOND_ORE | VOLT_ORE | GOLD_ORE | IRON_ORE | COAL_ORE) && c.m[idx(x, y, z)] == 1 {
                                deep_ore += 1;
                                assert!(y < (SLATE_Y + 8) as usize);
                            }
                        }
                    }
                }
            }
        }
        assert!(coal > diamond * 8, "charbon {coal}, diamant {diamond}");
        assert!(diamond > 10 && deep_ore > 10, "diamant {diamond}, minerais dans l'ardoise {deep_ore}");
    }

    #[test]
    fn caves_water_and_decor() {
        let (mut air, mut rock, mut lakes, mut moss, mut drip, mut glow) = (0usize, 0usize, 0, 0, 0, 0);
        for cz in -8..8 {
            for cx in -8..8 {
                let c = surface(SEED, cx, cz);
                for y in 30..100 {
                    for z in 0..16 {
                        for x in 0..16 {
                            match c.get(x, y, z) {
                                AIR => air += 1,
                                WATER => lakes += 1,
                                _ => rock += 1,
                            }
                        }
                    }
                }
                moss += count(&c, MOSS);
                glow += count(&c, GLOW_MOSS);
                drip += count(&c, STALACTITE);
            }
        }
        let open = air as f32 / (air + rock) as f32;
        assert!((0.04..0.3).contains(&open), "part de vide sous terre : {open}");
        assert!(lakes > 200, "lacs souterrains : {lakes}");
        assert!(moss > 100 && glow > 20 && drip > 100, "mousse {moss}, mousse lumineuse {glow}, concretions {drip}");
    }

    #[test]
    fn mineshafts_geodes_and_dens() {
        // Une region de galeries : les troncons restent dans la region, et le chunk du carrefour en porte la trace.
        let (rx, rz) = (-20..20).flat_map(|rx| (-20..20).map(move |rz| (rx, rz))).find(|(rx, rz)| !mineshaft(SEED, *rx, *rz).is_empty()).unwrap();
        let shafts = mineshaft(SEED, rx, rz);
        for g in &shafts {
            for end in [g.from, [g.from[0] + g.dir[0] * g.len, g.from[1], g.from[2] + g.dir[1] * g.len]] {
                assert_eq!((end[0].div_euclid(MINE_REGION), end[2].div_euclid(MINE_REGION)), (rx, rz), "galerie hors de sa region");
            }
        }
        let mut wood = 0;
        let mut rails = 0;
        let mut chests = Vec::new();
        for cz in rz * 6..rz * 6 + 6 {
            for cx in rx * 6..rx * 6 + 6 {
                let c = surface(SEED, cx, cz);
                for y in 30..110 {
                    for z in 0..16 {
                        for x in 0..16 {
                            match c.get(x, y, z) {
                                PLANKS | LOG => wood += 1,
                                RAIL => rails += 1,
                                CHEST => chests.push([cx * 16 + x as i32, y as i32, cz * 16 + z as i32]),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        // Certaines regions tombent sous la mer ou sous une vallee : on cherche plus loin si besoin.
        if hs_ok(rx, rz) {
            assert!(wood > 40 && rails > 10, "etais {wood}, rails {rails}");
        }
        for p in chests {
            let loot = chest_loot(SURFACE, SEED, p, 0);
            assert!(loot.iter().flatten().count() >= 1);
            assert_eq!(loot, chest_loot(SURFACE, SEED, p, 0), "le contenu ne depend que du coffre");
        }

        // Geodes : du cristal a l'interieur, du basalte autour.
        let (gx, gz) = (-20..20).flat_map(|rx| (-20..20).map(move |rz| (rx, rz))).find(|(rx, rz)| {
            geode(SEED, *rx, *rz).is_some_and(|(g, r)| column(SEED, g[0], g[2]).0 > g[1] + r + 14)
        }).unwrap();
        let (g, _) = geode(SEED, gx, gz).unwrap();
        let c = surface(SEED, g[0].div_euclid(16), g[2].div_euclid(16));
        assert!(count(&c, CRYSTAL) > 10, "cristal : {}", count(&c, CRYSTAL));
        assert_eq!(c.get(g[0].rem_euclid(16) as usize, g[1] as usize, g[2].rem_euclid(16) as usize), AIR, "coeur creux");

        // Tanieres : une stele et deux coffres.
        let den = (0..40).flat_map(|cx| (0..40).map(move |cz| surface(SEED, cx, cz))).find(|c| count(c, SPAWNER) > 0).expect("au moins une taniere");
        assert_eq!(count(&den, SPAWNER), 1);
        assert!(count(&den, CHEST) >= 2);
    }

    /// Le carrefour de la region est-il sous une terre assez haute pour que les galeries existent ?
    fn hs_ok(rx: i32, rz: i32) -> bool {
        mineshaft(SEED, rx, rz).first().is_some_and(|g| column(SEED, g.from[0], g.from[2]).0 > g.from[1] + 14)
    }

    #[test]
    fn yomi_has_four_lands_a_fortress_and_iron_sand() {
        let mut lands = HashSet::new();
        for z in (-1500..1500).step_by(25) {
            for x in (-1500..1500).step_by(25) {
                lands.insert(yomi_biome(SEED, x, z));
            }
        }
        assert_eq!(lands.len(), 4, "contrees rencontrees : {lands:?}");
        let (mut moss, mut sand, mut flames, mut satetsu, mut gold, mut basalt) = (0, 0, 0, 0, 0, 0);
        for cz in -10..10 {
            for cx in -10..10 {
                let c = abyss(SEED, cx, cz);
                moss += count(&c, CRIMSON_MOSS);
                sand += count(&c, SOUL_SAND);
                flames += count(&c, ONIBI);
                gold += count(&c, GOLD_ORE);
                basalt += count(&c, BASALT);
                for y in 0..H {
                    for z in 0..16 {
                        for x in 0..16 {
                            if c.get(x, y, z) == SATETSU {
                                satetsu += 1;
                                assert!(y < 30, "sable de fer a y = {y}");
                                assert!(c.get(x, y + 1, z) != AIR && c.get(x, y - 1, z) != AIR, "sable de fer a l'air libre");
                            }
                        }
                    }
                }
                assert_eq!(c.get(0, 0, 0), BEDROCK);
                assert_eq!(c.get(0, YOMI_TOP as usize, 0), BEDROCK);
            }
        }
        assert!(moss > 200 && sand > 200 && flames > 5 && basalt > 1000, "mousse {moss}, sable {sand}, onibi {flames}, basalte {basalt}");
        assert!(gold > 400 && (20..gold / 6).contains(&satetsu), "or {gold}, sable de fer {satetsu}");

        // Forteresse : un donjon avec sa stele et ses deux coffres, des ponts de tuiles.
        let (rx, rz) = (-9..9).flat_map(|rx| (-9..9).map(move |rz| (rx, rz))).find(|(rx, rz)| fortress(SEED, *rx, *rz).is_some()).unwrap();
        let f = fortress(SEED, rx, rz).unwrap();
        let keep = abyss(SEED, f[0].div_euclid(16), f[2].div_euclid(16));
        assert_eq!(keep.get(f[0].rem_euclid(16) as usize, f[1] as usize + 1, f[2].rem_euclid(16) as usize), SPAWNER);
        let mut bricks = 0;
        let mut chests = 0;
        for cz in rz * 8..rz * 8 + 8 {
            for cx in rx * 8..rx * 8 + 8 {
                let c = abyss(SEED, cx, cz);
                bricks += count(&c, BRICK);
                chests += count(&c, CHEST);
            }
        }
        assert!(bricks > 1500 && chests == 2, "tuiles {bricks}, coffres {chests}");
        assert!(chest_loot(ABYSS, SEED, f, 0).iter().flatten().count() >= 1);
    }

    #[test]
    fn takamagahara_has_crystals_and_a_pagoda() {
        // Huit cristaux, chacun au sommet de son pilier d'obsidienne.
        let crystals = zenith_crystals(SEED);
        assert_eq!(crystals.len(), 8);
        for p in &crystals {
            let c = zenith(SEED, p[0].div_euclid(16), p[2].div_euclid(16));
            let (lx, lz) = (p[0].rem_euclid(16) as usize, p[2].rem_euclid(16) as usize);
            assert_eq!(c.get(lx, p[1] as usize, lz), CRYSTAL);
            assert_eq!(c.get(lx, p[1] as usize - 1, lz), OBSIDIAN);
        }
        // L'ile de la pagode : la tour, sa stele, son coffre au dernier etage, la porte du retour.
        let (mut wood, mut tiles, mut chest, mut stele, mut gate) = (0, 0, 0, 0, 0);
        for cz in (PAGODA[1] - 16).div_euclid(16)..=(PAGODA[1] + 16).div_euclid(16) {
            for cx in -1..=0 {
                let c = zenith(SEED, cx, cz);
                wood += count(&c, PLANKS) + count(&c, LOG);
                tiles += count(&c, BRICK);
                chest += count(&c, CHEST);
                stele += count(&c, SPAWNER);
                gate += count(&c, GATEWAY);
            }
        }
        assert!(wood > 250 && tiles > 150, "bois {wood}, tuiles {tiles}");
        assert_eq!((chest, stele, gate), (1, 1, 1));
        // On peut monter : chaque marche est a un bloc au-dessus de la precedente, avec la place de passer.
        for (storey, dz, dir) in [(0, 2, 1), (1, -2, -1)] {
            for i in 0..4 {
                let (dx, dy) = ((i - 2) * dir, 1 + storey * 5 + i);
                assert_eq!(pagoda_block(dx, dy, dz), Some(PLANKS), "marche {i} de l'etage {storey}");
                assert_eq!(pagoda_block(dx, dy + 1, dz), Some(AIR));
                assert_eq!(pagoda_block(dx, dy + 2, dz), Some(AIR));
            }
        }
        assert_eq!(chest_loot(ZENITH, SEED, [0, 80, 380], 0)[13].map(|s| s.id), Some(items::HAGOROMO));
        // Des glycines sur les ilots lointains.
        let mut wisteria = 0;
        for cz in 10..40 {
            for cx in -15..15 {
                wisteria += count(&zenith(SEED, cx, cz), WISTERIA);
            }
        }
        assert!(wisteria > 50, "glycines : {wisteria}");
    }

    #[test]
    fn each_forest_has_its_trees() {
        let mut leaves = HashSet::new();
        for (biome, leaf) in [(Biome::Forest, LEAVES), (Biome::Cedar, CEDAR_LEAVES), (Biome::Maple, MAPLE_LEAVES)] {
            let spot = (-200..200).flat_map(|i| (-200..200).map(move |j| (i * 16 + 8, j * 16 + 8))).find(|(x, z)| {
                let (h, b) = column(SEED, *x, *z);
                b == biome && h > SEA + 3 && [(8, 8), (-8, -8), (8, -8), (-8, 8)].iter().all(|(dx, dz)| column(SEED, x + dx, z + dz).1 == biome)
            });
            let (x, z) = spot.unwrap_or_else(|| panic!("aucun {biome:?} trouve"));
            let mut n = 0;
            for dz in -1..=1 {
                for dx in -1..=1 {
                    n += count(&surface(SEED, x.div_euclid(16) + dx, z.div_euclid(16) + dz), leaf);
                }
            }
            assert!(n > 30, "{biome:?} : {n} blocs de feuillage");
            leaves.insert(leaf);
        }
        assert_eq!(leaves.len(), 3);
    }
}
