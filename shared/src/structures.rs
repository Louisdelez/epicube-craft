//! Structures de la surface : villages, sanctuaires, tombes kofun, temples moussus, torii en ruine.
//!
//! Comme dans Minecraft, le monde est decoupe en regions carrees ; chacune tire au sort, d'apres la
//! graine, si elle porte une structure, laquelle, et ou. Le site est choisi assez loin des bords pour
//! que l'ouvrage tienne tout entier dans sa region : un chunk n'a donc qu'une region a consulter,
//! et la structure sort identique quel que soit l'ordre dans lequel ses chunks sont engendres.
//! Chaque batiment est decrit par une fonction (position relative) -> bloc, ce qui le rend
//! insensible aux frontieres de chunks.
use crate::blocks::*;
use crate::chunk::{idx, Chunk, H, SEA};
use crate::gen::{column, Biome};
use crate::util::{hash, Rng};

/// Cote d'une region, en blocs (10 chunks).
pub const REGION: i32 = 160;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Kind {
    Village,
    /// Sanctuaire shinto : torii, allee, pavillon.
    Shrine,
    /// Tombe ancienne sous son tertre, piegee.
    Kofun,
    /// Temple moussu de la bambouseraie, garde par un piege a fleches.
    Temple,
    /// Torii d'obsidienne effondre.
    Ruin,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Village => "Village",
            Kind::Shrine => "Sanctuaire",
            Kind::Kofun => "Tombe kofun",
            Kind::Temple => "Temple moussu",
            Kind::Ruin => "Torii en ruine",
        }
    }
}

/// Butins des coffres : la metadonnee du coffre dit lequel tirer.
pub const LOOT_MINE: u8 = 0;
pub const LOOT_HOUSE: u8 = 1;
pub const LOOT_SHRINE: u8 = 2;
pub const LOOT_TOMB: u8 = 3;
pub const LOOT_TEMPLE: u8 = 4;
pub const LOOT_RUIN: u8 = 5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Site {
    pub kind: Kind,
    pub x: i32,
    pub z: i32,
    /// Niveau du sol au centre.
    pub y: i32,
    /// Graine propre a la structure.
    pub seed: u32,
}

/// Structure de la region (rx, rz), s'il y en a une : le biome du site decide de ce qui peut s'y dresser,
/// et le terrain doit etre assez plat.
pub fn site(seed: u32, rx: i32, rz: i32) -> Option<Site> {
    let own = hash(rx, 19, rz, seed.wrapping_add(96));
    let mut rng = Rng::new(own as u64);
    let (x, z) = (rx * REGION + rng.range(40, 121), rz * REGION + rng.range(40, 121));
    let (y, biome) = column(seed, x, z);
    if y <= SEA + 2 || y > 170 {
        return None;
    }
    let roll = rng.f();
    let pick = |table: &[(f32, Kind)]| table.iter().find(|(limit, _)| roll < *limit).map(|(_, k)| *k);
    let kind = match biome {
        Biome::Plains => pick(&[(0.34, Kind::Village), (0.44, Kind::Shrine), (0.54, Kind::Kofun), (0.64, Kind::Ruin)]),
        Biome::Forest | Biome::Maple => pick(&[(0.26, Kind::Village), (0.42, Kind::Shrine), (0.54, Kind::Ruin)]),
        Biome::Cedar | Biome::Snow => pick(&[(0.28, Kind::Shrine), (0.42, Kind::Ruin)]),
        Biome::Desert => pick(&[(0.32, Kind::Kofun), (0.46, Kind::Ruin)]),
        Biome::Bamboo => pick(&[(0.38, Kind::Temple), (0.5, Kind::Ruin)]),
        _ => pick(&[(0.2, Kind::Ruin)]),
    }?;
    // Un village veut un terrain presque plat sur toute son etendue ; le reste se contente de moins.
    let (reach, slope) = if kind == Kind::Village { (20, 5) } else { (7, 4) };
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)] {
        let (h, _) = column(seed, x + dx * reach, z + dz * reach);
        if (h - y).abs() > slope || h <= SEA + 1 {
            return None;
        }
    }
    Some(Site { kind, x, z, y, seed: own })
}

/// Parcelle d'un village.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lot {
    /// Maison ; le numero choisit le metier de son habitant et le meuble qui va avec.
    House(u8),
    /// Champ ; le numero choisit la culture.
    Farm(u8),
}

/// Parcelles d'un village : (parcelle, x, z, niveau du sol). Huit emplacements autour du puits ;
/// les trois premiers portent toujours une maison.
pub fn lots(seed: u32, s: &Site) -> Vec<(Lot, i32, i32, i32)> {
    let mut rng = Rng::new(s.seed.wrapping_add(1) as u64);
    let mut out = Vec::new();
    let mut k = 0;
    for gz in -1..=1 {
        for gx in -1..=1 {
            if gx == 0 && gz == 0 {
                continue;
            }
            let roll = rng.f();
            let variant = rng.range(0, 5) as u8;
            let (x, z) = (s.x + gx * 14, s.z + gz * 14);
            let (y, _) = column(seed, x, z);
            let lot = if k < 3 || roll < 0.5 {
                Lot::House(variant)
            } else if roll < 0.8 {
                Lot::Farm(variant % 3)
            } else {
                k += 1;
                continue;
            };
            k += 1;
            if y > SEA + 1 {
                out.push((lot, x, z, y));
            }
        }
    }
    out
}

type Cell = Option<(B, u8)>;

fn some(b: B) -> Cell {
    Some((b, 0))
}

/// Maison minka de 7 x 7 : poteaux de bois, cloisons de planches, fenetres de papier, toit de tuiles.
/// Dedans : un futon, un coffre, une lanterne, et le meuble du metier. `corner` : le bois des poteaux.
fn house(dx: i32, dy: i32, dz: i32, variant: u8, corner: B, loot: u8) -> Cell {
    let (ax, az) = (dx.abs(), dz.abs());
    let m = ax.max(az);
    match dy {
        0 if m <= 3 => some(PLANKS),
        1..=3 if m == 3 => {
            if ax == 3 && az == 3 {
                some(corner)
            } else if dx == 0 && dz == -3 && dy <= 2 {
                some(DOOR)
            } else if dy == 2 && ((ax == 3 && dz == 0) || (dz == 3 && dx == 0)) {
                some(GLASS)
            } else {
                some(PLANKS)
            }
        }
        1..=3 if m < 3 => match (dx, dy, dz) {
            (2, 1, 2) => some(BED),
            (-2, 1, 2) => Some((CHEST, loot)),
            (-2, 1, -2) => some(TORCH),
            (2, 1, -2) => some([COMPOSTER, SMOKER, FURNACE, LAMP, CRAFT][variant as usize % 5]),
            _ => some(AIR),
        },
        // L'avant-toit deborde d'un bloc : on degage dessous.
        1..=3 if m == 4 => some(AIR),
        4 if m <= 4 => some(BRICK),
        5 if m <= 2 => some(BRICK),
        6 if m == 0 => some(BRICK),
        _ => None,
    }
}

/// Puits couvert, au centre du village.
fn well(dx: i32, dy: i32, dz: i32) -> Cell {
    let (ax, az) = (dx.abs(), dz.abs());
    let m = ax.max(az);
    match dy {
        -2..=0 if m == 0 => some(WATER),
        -3..=0 if m <= 1 => some(COBBLE),
        1 | 2 if ax == 1 && az == 1 => some(LOG),
        1 | 2 if m <= 1 => some(AIR),
        3 if m <= 2 => some(BRICK),
        _ => None,
    }
}

/// Champ de 7 x 7 borde de troncs, une rigole au milieu.
fn farm(dx: i32, dy: i32, dz: i32, crop: u8, r: u32) -> Cell {
    let m = dx.abs().max(dz.abs());
    match dy {
        0 if m == 3 => some(LOG),
        0 if dx == 0 && m < 3 => some(WATER),
        0 if m < 3 => some(FARMLAND),
        1 if m < 3 && dx != 0 => Some(([WHEAT, CARROT, SOY][crop as usize % 3], (r % 4) as u8)),
        1..=3 if m <= 3 => some(AIR),
        _ => None,
    }
}

/// Torii de bois laque vermillon, dans l'axe X.
fn torii(dx: i32, dy: i32) -> Cell {
    let ax = dx.abs();
    match dy {
        1..=4 if ax == 2 => some(VERMILION),
        4 if ax < 2 => some(VERMILION),
        5 if ax <= 3 => some(VERMILION),
        1..=3 if ax < 2 => some(AIR),
        _ => None,
    }
}

/// Sanctuaire : torii, allee de pierre, deux gardiens de pierre, pavillon sureleve.
fn shrine(dx: i32, dy: i32, dz: i32) -> Cell {
    if dz == -10 {
        return torii(dx, dy);
    }
    if (-9..=-4).contains(&dz) {
        return match (dx.abs(), dy) {
            (0, 0) => some(COBBLE),
            (2, 1) if dz == -5 => some(COBBLE),
            (2, 2) if dz == -5 => some(MOSS),
            (0..=2, 1..=3) => some(AIR),
            _ => None,
        };
    }
    if dy == 0 && dx.abs().max(dz.abs()) <= 4 {
        return some(COBBLE);
    }
    house(dx, dy, dz, 3, VERMILION, LOOT_SHRINE).map(|(b, m)| match (b, dx, dz) {
        // Ni futon ni lanterne de papier : une lampe et le coffre des offrandes.
        (BED, ..) | (TORCH, ..) => (AIR, 0),
        (PLANKS, _, _) if dy == 0 => (COBBLE, 0),
        _ => (b, m),
    })
}

/// Tertre funeraire : dome de terre, chambre de pierre, quatre coffres, et sous la dalle
/// centrale une charge de feux d'artifice qui n'attend qu'un pas.
fn kofun(dx: i32, dy: i32, dz: i32, sandy: bool) -> Cell {
    let (ax, az) = (dx.abs(), dz.abs());
    let m = ax.max(az);
    let inside = |y: i32| dx * dx + dz * dz + (y * 8 / 5).pow(2) <= 49;
    if !(-2..=5).contains(&dy) || !inside(dy.max(0)) {
        return None;
    }
    // Couloir d'entree, au sud.
    if dx == 0 && (-7..=-3).contains(&dz) && (1..=2).contains(&dy) {
        return some(AIR);
    }
    if m <= 3 && dy <= 4 {
        return match dy {
            -2 => some(COBBLE),
            -1 if m == 0 || (ax + az == 1) => some(EXPLOSIVE),
            -1 => some(COBBLE),
            0 => some(COBBLE),
            1..=3 if m <= 2 => match (ax, dy, az) {
                (0, 1, 0) => some(PLATE),
                (2, 1, 2) => Some((CHEST, LOOT_TOMB)),
                _ => some(AIR),
            },
            _ => some(COBBLE),
        };
    }
    if dy < 0 {
        return None;
    }
    let skin = !inside(dy + 1);
    some(match (sandy, skin) {
        (true, _) => SAND,
        (false, true) => GRASS,
        (false, false) => DIRT,
    })
}

/// Temple moussu : une longue salle de pierre. A mi-chemin, une corde tendue entre deux piquets
/// declenche deux distributeurs charges de fleches ; le coffre est au fond.
fn temple(dx: i32, dy: i32, dz: i32, r: u32) -> Cell {
    let (ax, az) = (dx.abs(), dz.abs());
    if ax > 3 || az > 8 || !(0..=5).contains(&dy) {
        return None;
    }
    let stone = if r % 3 == 0 { MOSS } else { COBBLE };
    let wall = ax == 3 || az == 8;
    if dy == 0 || dy == 5 {
        return some(stone);
    }
    if wall {
        return match (dx, dy, dz) {
            (0, 1 | 2, -8) => some(AIR),
            // Distributeurs dans les murs, au-dessus des blocs que chargent les piquets.
            (-3, 2, -2) => Some((DROPPER, 0)),
            (3, 2, -2) => Some((DROPPER, 1)),
            // Le bloc qui porte un piquet doit conduire : pierre nue, pas de mousse.
            (_, 1, -2) => some(COBBLE),
            _ => some(stone),
        };
    }
    match (dx, dy, dz) {
        (-2, 1, -2) => Some((HOOK, 2 << 1)),
        (2, 1, -2) => Some((HOOK, 1 << 1)),
        (-1..=1, 1, -2) => some(TRIPWIRE),
        (0, 1, 7) => Some((CHEST, LOOT_TEMPLE)),
        (-2 | 2, 1, 7) => some(TORCH),
        _ => some(AIR),
    }
}

/// Torii d'obsidienne effondre : un cadre incomplet, des eclats au sol, un coffre.
fn ruin(dx: i32, dy: i32, dz: i32, r: u32) -> Cell {
    if dz == 0 && (-1..=2).contains(&dx) && (0..=4).contains(&dy) {
        let frame = dx == -1 || dx == 2 || dy == 0 || dy == 4;
        // Il manque toujours des blocs : jamais un cadre complet.
        let missing = r % 4 == 0 || (dx == 2 && dy == 3);
        return if frame && !missing { some(OBSIDIAN) } else if dy > 0 { some(AIR) } else { None };
    }
    match (dx, dy, dz) {
        (3, 1, 1) => Some((CHEST, LOOT_RUIN)),
        (-2, 1, 1) | (1, 1, -1) if r % 2 == 0 => some(OBSIDIAN),
        (-3..=4, 0, -2..=2) if r % 3 != 0 => some(if r % 2 == 0 { COBBLE } else { MOSS }),
        _ => None,
    }
}

/// Inscrit dans le chunk (cx, cz) la part qui lui revient de la structure de sa region.
/// `hs` : hauteur du sol de chaque colonne du chunk.
pub fn build(c: &mut Chunk, seed: u32, cx: i32, cz: i32, hs: &[[i32; 16]; 16]) {
    let (x0, z0) = (cx * 16, cz * 16);
    let Some(s) = site(seed, x0.div_euclid(REGION), z0.div_euclid(REGION)) else { return };
    // Rien a faire si la structure est trop loin de ce chunk.
    if (s.x - x0 - 8).abs() > 48 || (s.z - z0 - 8).abs() > 48 {
        return;
    }
    let lots = if s.kind == Kind::Village { lots(seed, &s) } else { Vec::new() };
    let sandy = column(seed, s.x, s.z).1 == Biome::Desert;
    for lz in 0..16usize {
        for lx in 0..16usize {
            let (x, z, h) = (x0 + lx as i32, z0 + lz as i32, hs[lz][lx]);
            let (dx, dz) = (x - s.x, z - s.z);
            let r = hash(x, 23, z, s.seed);
            // Pose une piece : `base` est son niveau de sol, `f` la decrit. Sous la piece, on comble
            // jusqu'au terrain pour qu'elle ne flotte pas ; au-dessus, ce que la piece laisse intact reste.
            let put = |c: &mut Chunk, base: i32, lo: i32, hi: i32, solid: bool, f: &dyn Fn(i32) -> Cell| {
                if solid {
                    for y in (h + 1).max(1)..base {
                        c.set(lx, y as usize, lz, COBBLE);
                    }
                }
                for dy in lo..=hi {
                    let y = base + dy;
                    if y < 2 || y >= H as i32 - 1 {
                        continue;
                    }
                    if let Some((b, m)) = f(dy) {
                        let i = idx(lx, y as usize, lz);
                        c.b[i] = b;
                        c.m[i] = m;
                    }
                }
            };
            match s.kind {
                Kind::Village => {
                    if dx.abs().max(dz.abs()) <= 2 {
                        put(c, s.y, -3, 3, dx.abs().max(dz.abs()) <= 1, &|dy| well(dx, dy, dz));
                        continue;
                    }
                    let mut built = false;
                    for (lot, bx, bz, by) in &lots {
                        let (ex, ez) = (x - bx, z - bz);
                        if ex.abs().max(ez.abs()) > 4 {
                            continue;
                        }
                        let foot = ex.abs().max(ez.abs()) <= 3;
                        match lot {
                            Lot::House(v) => put(c, *by, 0, 6, foot, &|dy| house(ex, dy, ez, *v, LOG, LOOT_HOUSE)),
                            Lot::Farm(crop) => put(c, *by, 0, 3, foot, &|dy| farm(ex, dy, ez, *crop, r)),
                        }
                        built = true;
                    }
                    if built || h <= SEA {
                        continue;
                    }
                    // Chemins de gravier : une allee devant chaque rangee de maisons, une traverse, des lanternes.
                    let row = [-18, -4, 10].iter().any(|rz| (dz == *rz || dz == rz - 1) && dx.abs() <= 21);
                    let cross = (dx == 7 || dx == 8) && (-21..=11).contains(&dz);
                    let lamp = dx == 10 && [-17, -3, 11].contains(&dz);
                    let top = c.get(lx, h as usize, lz);
                    if lamp {
                        put(c, h, 1, 3, false, &|dy| some(if dy == 3 { TORCH } else { LOG }));
                    } else if dz == -22 && (4..=11).contains(&dx) {
                        // Torii a l'entree du village.
                        put(c, h, 1, 5, false, &|dy| torii(dx - 7, dy));
                    } else if (row || cross) && matches!(top, GRASS | DIRT | SNOW | SAND) {
                        c.set(lx, h as usize, lz, GRAVEL);
                        for dy in 1..=2 {
                            if matches!(c.get(lx, (h + dy) as usize, lz), TALLGRASS | FLOWER | BAMBOO | TEA) {
                                c.set(lx, (h + dy) as usize, lz, AIR);
                            }
                        }
                    }
                }
                Kind::Shrine => {
                    let foot = dx.abs().max(dz.abs()) <= 4;
                    if dx.abs() <= 4 && (-10..=4).contains(&dz) {
                        put(c, if foot { s.y } else { h.min(s.y + 1).max(s.y - 1) }, 0, 6, foot, &|dy| shrine(dx, dy, dz));
                    }
                }
                Kind::Kofun => {
                    if dx.abs().max(dz.abs()) <= 7 {
                        put(c, s.y, -2, 5, dx * dx + dz * dz <= 49, &|dy| kofun(dx, dy, dz, sandy));
                    }
                }
                Kind::Temple => {
                    if dx.abs() <= 3 && dz.abs() <= 8 {
                        put(c, s.y, 0, 5, true, &|dy| temple(dx, dy, dz, hash(x, dy, z, s.seed)));
                    }
                }
                Kind::Ruin => {
                    if (-3..=4).contains(&dx) && dz.abs() <= 2 {
                        put(c, h, 0, 4, false, &|dy| ruin(dx, dy, dz, hash(x, dy, z, s.seed)));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::generate;
    use std::collections::HashMap;

    const SEED: u32 = 20251;

    /// Premiere region portant une structure du genre demande, en spirale autour de l'origine.
    fn find(kind: Kind) -> Site {
        (0..60)
            .flat_map(|r: i32| (-r..=r).flat_map(move |a| [(a, -r), (a, r), (-r, a), (r, a)]))
            .find_map(|(rx, rz)| site(SEED, rx, rz).filter(|s| s.kind == kind))
            .unwrap_or_else(|| panic!("aucun {kind:?} trouve"))
    }

    /// Compte les blocs dans les chunks qui entourent un site.
    fn census(s: &Site) -> HashMap<B, usize> {
        let mut n = HashMap::new();
        for cz in (s.z - 40).div_euclid(16)..=(s.z + 40).div_euclid(16) {
            for cx in (s.x - 40).div_euclid(16)..=(s.x + 40).div_euclid(16) {
                for b in generate(0, SEED, cx, cz).b {
                    *n.entry(b).or_insert(0) += 1;
                }
            }
        }
        n
    }

    #[test]
    fn every_kind_exists_and_stays_in_its_region() {
        let mut kinds = HashMap::new();
        for rz in -25..25 {
            for rx in -25..25 {
                if let Some(s) = site(SEED, rx, rz) {
                    *kinds.entry(s.kind).or_insert(0) += 1;
                    assert_eq!((s.x.div_euclid(REGION), s.z.div_euclid(REGION)), (rx, rz));
                    // Marge : le plus grand ouvrage (le village) deborde de 36 blocs au plus.
                    assert!((40..=120).contains(&s.x.rem_euclid(REGION)) && (40..=120).contains(&s.z.rem_euclid(REGION)));
                    assert_eq!(site(SEED, rx, rz), Some(s), "le tirage ne depend que de la region");
                }
            }
        }
        assert_eq!(kinds.len(), 5, "genres rencontres : {kinds:?}");
        assert!(kinds[&Kind::Village] >= 8, "{kinds:?}");
    }

    #[test]
    fn village_has_homes_beds_fields_and_a_well() {
        let s = find(Kind::Village);
        let plots = lots(SEED, &s);
        let houses = plots.iter().filter(|p| matches!(p.0, Lot::House(_))).count();
        assert!(houses >= 3, "au moins trois maisons : {houses}");
        let n = census(&s);
        let count = |b: B| n.get(&b).copied().unwrap_or(0);
        assert_eq!(count(BED), houses, "un futon par maison");
        assert_eq!(count(DOOR), houses * 2, "une porte de deux blocs par maison");
        assert!(count(CHEST) >= houses && count(BRICK) > houses * 80 && count(GRAVEL) > 100);
        assert!(count(VERMILION) >= 10, "le torii de l'entree");
        // Le puits : de l'eau au centre, sous son toit.
        let c = generate(0, SEED, s.x.div_euclid(16), s.z.div_euclid(16));
        let (lx, lz) = (s.x.rem_euclid(16) as usize, s.z.rem_euclid(16) as usize);
        assert_eq!(c.get(lx, s.y as usize, lz), WATER);
        assert_eq!(c.get(lx, s.y as usize + 3, lz), BRICK);
        // Chaque maison est entiere, meme a cheval sur plusieurs chunks : sol, futon, coffre, toit.
        for (lot, bx, bz, by) in plots {
            let at = |dx: i32, dy: i32, dz: i32| {
                let (x, z) = (bx + dx, bz + dz);
                let c = generate(0, SEED, x.div_euclid(16), z.div_euclid(16));
                let i = idx(x.rem_euclid(16) as usize, (by + dy) as usize, z.rem_euclid(16) as usize);
                (c.b[i], c.m[i])
            };
            match lot {
                Lot::House(_) => {
                    assert_eq!(at(2, 1, 2).0, BED);
                    assert_eq!(at(-2, 1, 2), (CHEST, LOOT_HOUSE));
                    assert_eq!(at(0, 1, -3).0, DOOR);
                    assert_eq!(at(0, 2, -3).0, DOOR);
                    assert_eq!(at(-3, 1, -3).0, LOG);
                    assert_eq!(at(3, 3, 3).0, LOG);
                    assert_eq!(at(4, 4, -4).0, BRICK);
                    assert_eq!(at(0, 6, 0).0, BRICK);
                    assert_eq!(at(0, 1, 0).0, AIR);
                }
                Lot::Farm(_) => {
                    assert_eq!(at(0, 0, 0).0, WATER);
                    assert_eq!(at(1, 0, 1).0, FARMLAND);
                    assert!(matches!(at(1, 1, 1).0, WHEAT | CARROT | SOY));
                }
            }
        }
    }

    #[test]
    fn shrine_tomb_temple_and_ruin() {
        let s = find(Kind::Shrine);
        let n = census(&s);
        assert!(n.get(&VERMILION).copied().unwrap_or(0) >= 20, "torii et poteaux du pavillon");
        assert!(n.get(&CHEST).copied().unwrap_or(0) >= 1);

        // Tombe : quatre coffres autour d'une dalle, la charge juste dessous.
        let s = find(Kind::Kofun);
        let c = generate(0, SEED, s.x.div_euclid(16), s.z.div_euclid(16));
        let (lx, lz, y) = (s.x.rem_euclid(16) as usize, s.z.rem_euclid(16) as usize, s.y as usize);
        assert_eq!(c.get(lx, y + 1, lz), PLATE);
        assert_eq!(c.get(lx, y, lz), COBBLE, "la dalle repose sur un bloc qui transmet la charge");
        assert_eq!(c.get(lx, y - 1, lz), EXPLOSIVE);
        let tomb = |dx: i32, dy: i32, dz: i32| {
            let (x, z) = (s.x + dx, s.z + dz);
            let c = generate(0, SEED, x.div_euclid(16), z.div_euclid(16));
            let i = idx(x.rem_euclid(16) as usize, (s.y + dy) as usize, z.rem_euclid(16) as usize);
            (c.b[i], c.m[i])
        };
        for (dx, dz) in [(2, 2), (-2, 2), (2, -2), (-2, -2)] {
            assert_eq!(tomb(dx, 1, dz), (CHEST, LOOT_TOMB));
        }
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            assert_eq!(tomb(dx, -1, dz).0, EXPLOSIVE);
        }
        assert_eq!(tomb(0, 2, -6).0, AIR, "le couloir d'entree");

        // Temple : la corde entre ses deux piquets, un distributeur au-dessus de chaque mur porteur.
        let s = find(Kind::Temple);
        let at = |dx: i32, dy: i32, dz: i32| {
            let (x, z) = (s.x + dx, s.z + dz);
            let c = generate(0, SEED, x.div_euclid(16), z.div_euclid(16));
            let i = idx(x.rem_euclid(16) as usize, (s.y + dy) as usize, z.rem_euclid(16) as usize);
            (c.b[i], c.m[i])
        };
        assert_eq!(at(-2, 1, -2), (HOOK, 4));
        assert_eq!(at(2, 1, -2), (HOOK, 2));
        for dx in -1..=1 {
            assert_eq!(at(dx, 1, -2).0, TRIPWIRE);
        }
        assert_eq!(at(-3, 1, -2).0, COBBLE);
        assert_eq!(at(-3, 2, -2), (DROPPER, 0));
        assert_eq!(at(3, 2, -2), (DROPPER, 1));
        assert_eq!(at(0, 1, 7), (CHEST, LOOT_TEMPLE));
        assert_eq!(at(0, 1, -8).0, AIR, "la porte");

        // Torii en ruine : de l'obsidienne, mais jamais un cadre complet.
        let s = find(Kind::Ruin);
        let n = census(&s);
        let obsidian = n.get(&OBSIDIAN).copied().unwrap_or(0);
        assert!((4..14).contains(&obsidian), "obsidienne : {obsidian}");
    }
}
