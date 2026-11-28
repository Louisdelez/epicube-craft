//! Recettes d'artisanat, cuisson et combustibles.
use crate::blocks::*;
use crate::items::*;
use std::sync::OnceLock;

pub struct Recipe {
    pub w: usize,
    pub h: usize,
    /// w * h cases, 0 = vide.
    pub cells: Vec<Id>,
    pub out: Id,
    pub n: u8,
}

fn add(v: &mut Vec<Recipe>, rows: &[&str], map: &[(char, Id)], out: Id, n: u8) {
    let w = rows[0].len();
    let cells = rows
        .iter()
        .flat_map(|r| r.chars())
        .map(|ch| map.iter().find(|(c, _)| *c == ch).map(|(_, id)| *id).unwrap_or(0))
        .collect();
    v.push(Recipe { w, h: rows.len(), cells, out, n });
}

/// Recettes du jeu suivies de celles des mods de la partie. Remplacees a chaque changement de mods ;
/// l'ancienne liste reste en memoire (quelques centaines d'octets), car on a pu en garder des references.
static ALL: std::sync::RwLock<Option<&'static [Recipe]>> = std::sync::RwLock::new(None);

/// Toutes les recettes : celles du jeu, puis celles des mods. Leur rang sert a les designer sur le reseau.
pub fn recipes() -> &'static [Recipe] {
    ALL.read().ok().and_then(|a| *a).unwrap_or_else(builtin)
}

/// Ajoute les recettes des mods a la suite de celles du jeu.
pub fn set_mod_recipes(defs: &[crate::mods::RecipeDef]) {
    let mut all: Vec<Recipe> = builtin().iter().map(|r| Recipe { w: r.w, h: r.h, cells: r.cells.clone(), out: r.out, n: r.n }).collect();
    for d in defs {
        let (w, h) = (d.w as usize, d.h as usize);
        if (1..=3).contains(&w) && (1..=3).contains(&h) && d.cells.len() == w * h && d.out != 0 && d.n > 0 {
            all.push(Recipe { w, h, cells: d.cells.clone(), out: d.out, n: d.n });
        }
    }
    if let Ok(mut slot) = ALL.write() {
        // Sans recette de mod, on revient simplement a la liste du jeu.
        *slot = if defs.is_empty() { None } else { Some(Box::leak(all.into_boxed_slice())) };
    }
}

fn builtin() -> &'static [Recipe] {
    static R: OnceLock<Vec<Recipe>> = OnceLock::new();
    R.get_or_init(|| {
        let mut v = Vec::new();
        let (p, c, s) = (PLANKS as Id, COBBLE as Id, STICK);
        add(&mut v, &["L"], &[('L', LOG as Id)], p, 4);
        add(&mut v, &["P", "P"], &[('P', p)], STICK, 4);
        add(&mut v, &["PP", "PP"], &[('P', p)], CRAFT as Id, 1);
        add(&mut v, &["C", "S"], &[('C', COAL), ('S', s)], TORCH as Id, 4);
        add(&mut v, &["CCC", "C C", "CCC"], &[('C', c)], FURNACE as Id, 1);
        add(&mut v, &["PPP", "P P", "PPP"], &[('P', p)], CHEST as Id, 1);
        for (tier, mat) in [p, c, IRON_INGOT, DIAMOND].into_iter().enumerate() {
            let m = [('X', mat), ('S', s)];
            let tier = tier as u8;
            add(&mut v, &["XXX", " S ", " S "], &m, tool_id(PICK, tier), 1);
            add(&mut v, &["XX", "XS", " S"], &m, tool_id(AXE, tier), 1);
            add(&mut v, &["X", "S", "S"], &m, tool_id(SHOVEL, tier), 1);
            add(&mut v, &["X", "X", "S"], &m, tool_id(SWORD, tier), 1);
            add(&mut v, &["XX", " S", " S"], &m, tool_id(HOE, tier), 1);
        }
        // Armures : casque, cuirasse, cuissards et jambieres, dans la matiere choisie.
        for (tier, mat) in [LEATHER, BAMBOO as Id, IRON_INGOT, GOLD_INGOT, DIAMOND].into_iter().enumerate() {
            let m = [('X', mat)];
            let tier = tier as u8;
            add(&mut v, &["XXX", "X X"], &m, armor_id(HEAD, tier), 1);
            add(&mut v, &["X X", "XXX", "XXX"], &m, armor_id(BODY, tier), 1);
            add(&mut v, &["XXX", "X X", "X X"], &m, armor_id(LEGS, tier), 1);
            add(&mut v, &["X X", "X X"], &m, armor_id(FEET, tier), 1);
        }
        // Le tamahagane se forge sur une piece de diamant, comme pour les outils.
        for slot in 0..4 {
            add(&mut v, &["T", "X"], &[('T', TAMAHAGANE), ('X', armor_id(slot, 4))], armor_id(slot, 5), 1);
        }
        // Cuisine : le riz se cuit d'abord, puis se faconne.
        add(&mut v, &["RRR"], &[('R', COOKED_RICE)], BREAD, 1);
        add(&mut v, &["F", "R"], &[('F', RAW_FISH), ('R', COOKED_RICE)], SUSHI, 2);
        add(&mut v, &["SSS"], &[('S', SOY_BEANS)], TOFU, 1);
        add(&mut v, &["P P", " P "], &[('P', p)], BOWL, 4);
        add(&mut v, &["TN", "SB"], &[('T', TOFU), ('N', NORI), ('S', SOY_BEANS), ('B', BOWL)], MISO_SOUP, 1);
        add(&mut v, &["MD", "TB"], &[('M', COOKED_MEAT), ('D', CARROT_ITEM), ('T', TOFU), ('B', BOWL)], NABE, 1);
        add(&mut v, &["D", "B"], &[('D', DRIED_TEA), ('B', BOWL)], GREEN_TEA, 1);
        add(&mut v, &["OY", "FD"], &[('O', BREAD), ('Y', TAMAGOYAKI), ('F', COOKED_FISH), ('D', CARROT_ITEM)], BENTO, 1);
        add(&mut v, &["K"], &[('K', KABOCHA as Id)], KABOCHA_SLICE, 4);
        add(&mut v, &["L"], &[('L', KABOCHA_SLICE)], KABOCHA_SEEDS, 1);
        // Ferme.
        add(&mut v, &["B"], &[('B', BONE)], FERTILIZER, 3);
        add(&mut v, &["P P", "P P", "PPP"], &[('P', p)], COMPOSTER as Id, 1);
        add(&mut v, &[" L ", "LFL", " L "], &[('L', LOG as Id), ('F', FURNACE as Id)], SMOKER as Id, 1);
        add(&mut v, &[" S ", "SCS", "LLL"], &[('S', s), ('C', COAL), ('L', LOG as Id)], IRORI as Id, 1);
        add(&mut v, &[" I", "I "], &[('I', IRON_INGOT)], SHEARS, 1);
        add(&mut v, &["  S", " ST", "S T"], &[('S', s), ('T', STRING)], FISHING_ROD, 1);
        add(&mut v, &["B", "B"], &[('B', BAMBOO as Id)], STICK, 1);
        add(&mut v, &["SS", "SS"], &[('S', STRING)], WOOL as Id, 1);
        add(&mut v, &["WWW", "PPP"], &[('W', WOOL as Id), ('P', p)], BED as Id, 1);
        // Armes.
        let iron = IRON_INGOT;
        add(&mut v, &["I", "S"], &[('I', iron), ('S', s)], TANTO, 1);
        add(&mut v, &["  I", " S ", "S  "], &[('I', iron), ('S', s)], YARI, 1);
        add(&mut v, &[" II", " S ", "S  "], &[('I', iron), ('S', s)], NAGINATA, 1);
        add(&mut v, &[" I ", "III", " S "], &[('I', iron), ('S', s)], TETSUBO, 1);
        add(&mut v, &[" BT", "B T", " BT"], &[('B', BAMBOO as Id), ('T', STRING)], YUMI, 1);
        add(&mut v, &["F", "S", "E"], &[('F', FLINT), ('S', s), ('E', FEATHER)], ARROW, 4);
        add(&mut v, &["I", "T"], &[('I', iron), ('T', STRING)], KUNAI, 4);
        add(&mut v, &["PIP", "PPP", " P "], &[('P', p), ('I', iron)], SHIELD, 1);
        // Tamahagane : acier de l'Abime, pose sur un outil de diamant pour le porter au dernier niveau.
        add(&mut v, &["SGS", "G G", "SGS"], &[('S', TAMA_SHARD), ('G', GOLD_INGOT)], TAMAHAGANE, 1);
        add(&mut v, &["B"], &[('B', BONE_BLOCK as Id)], FERTILIZER, 9);
        // Bois laque : des planches teintes a la fleur de higanbana.
        add(&mut v, &["PPP", "PFP", "PPP"], &[('P', p), ('F', FLOWER as Id)], VERMILION as Id, 8);
        for kind in [PICK, AXE, SHOVEL, SWORD, HOE] {
            add(&mut v, &["T", "X"], &[('T', TAMAHAGANE), ('X', tool_id(kind, 3))], tool_id(kind, 4), 1);
        }
        add(&mut v, &["PP", "PP", "PP"], &[('P', p)], DOOR as Id, 3);
        add(&mut v, &["I I", " I "], &[('I', IRON_INGOT)], BUCKET, 1);
        add(&mut v, &["I ", " F"], &[('I', IRON_INGOT), ('F', FLINT)], LIGHTER, 1);
        add(&mut v, &["III", "III", "III"], &[('I', IRON_INGOT)], IRON_BLOCK as Id, 1);
        add(&mut v, &["B"], &[('B', IRON_BLOCK as Id)], IRON_INGOT, 9);
        add(&mut v, &["S", "C"], &[('S', s), ('C', c)], LEVER as Id, 1);
        add(&mut v, &["V", "S"], &[('V', VOLT_DUST), ('S', s)], SIGTORCH as Id, 1);
        add(&mut v, &[" G ", "GVG", " G "], &[('G', GLASS as Id), ('V', VOLT_DUST)], LAMP as Id, 1);
        add(&mut v, &["XSX", "SXS", "XSX"], &[('X', SPORE), ('S', SAND as Id)], EXPLOSIVE as Id, 1);
        // Circuits de reiseki.
        let (st, d, torch) = (STONE as Id, VOLT_DUST, SIGTORCH as Id);
        add(&mut v, &["TVT", "SSS"], &[('T', torch), ('V', d), ('S', st)], REPEATER as Id, 1);
        add(&mut v, &[" T ", "TET", "SSS"], &[('T', torch), ('E', EMBER_CRYSTAL), ('S', st)], COMPARATOR as Id, 1);
        add(&mut v, &["S"], &[('S', st)], BUTTON as Id, 1);
        add(&mut v, &["PP"], &[('P', p)], PLATE as Id, 1);
        add(&mut v, &["PPP", "CIC", "CVC"], &[('P', p), ('C', c), ('I', IRON_INGOT), ('V', d)], PISTON as Id, 1);
        // Le riz colle : un onigiri rend le poussoir gluant.
        add(&mut v, &["O", "K"], &[('O', BREAD), ('K', PISTON as Id)], STICKY_PISTON as Id, 1);
        add(&mut v, &["CCC", "VVE", "CCC"], &[('C', c), ('V', d), ('E', EMBER_CRYSTAL)], OBSERVER as Id, 1);
        add(&mut v, &["VVV", "VVV", "VVV"], &[('V', d)], VOLT_BLOCK as Id, 1);
        add(&mut v, &["B"], &[('B', VOLT_BLOCK as Id)], d, 9);
        add(&mut v, &["GGG", "EEE", "PPP"], &[('G', GLASS as Id), ('E', EMBER_CRYSTAL), ('P', p)], SUNDIAL as Id, 1);
        add(&mut v, &["CCC", "C C", "CVC"], &[('C', c), ('V', d)], DROPPER as Id, 1);
        let (iron, gold) = (IRON_INGOT, GOLD_INGOT);
        add(&mut v, &["I I", "IKI", " I "], &[('I', iron), ('K', CHEST as Id)], HOPPER as Id, 1);
        add(&mut v, &["LLL", "PVP", "PPP"], &[('L', LEATHER), ('P', p), ('V', d)], NOTE as Id, 1);
        add(&mut v, &["I I", "ISI", "I I"], &[('I', iron), ('S', s)], RAIL as Id, 16);
        add(&mut v, &["G G", "GSG", "GVG"], &[('G', gold), ('S', s), ('V', d)], POWERED_RAIL as Id, 6);
        add(&mut v, &["I I", "IDI", "IVI"], &[('I', iron), ('D', PLATE as Id), ('V', d)], DETECTOR_RAIL as Id, 6);
        add(&mut v, &["I", "S", "P"], &[('I', iron), ('S', s), ('P', p)], HOOK as Id, 2);
        add(&mut v, &[" V ", "VWV", " V "], &[('V', d), ('W', WHEAT_ITEM)], TARGET as Id, 1);
        add(&mut v, &["I I", "III"], &[('I', iron)], CART, 1);
        add(
            &mut v,
            &["PEP", "EDE", "PEP"],
            &[('P', PEARL), ('E', EMBER_CRYSTAL), ('D', DIAMOND)],
            ZENITH_PORTAL as Id,
            1,
        );
        v
    })
}

/// Cherche la recette posee dans une grille 3x3 (9 cases), a n'importe quelle position, miroir compris.
pub fn match_grid(grid: &[Slot]) -> Option<&'static Recipe> {
    let id = |x: usize, y: usize| grid[y * 3 + x].map(|s| s.id).unwrap_or(0);
    let (mut x0, mut y0, mut x1, mut y1) = (3, 3, 0, 0);
    for y in 0..3 {
        for x in 0..3 {
            if id(x, y) != 0 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if x0 > x1 {
        return None;
    }
    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
    recipes().iter().find(|r| {
        r.w == w
            && r.h == h
            && [false, true].into_iter().any(|mirror| {
                (0..h).all(|y| {
                    (0..w).all(|x| {
                        let rx = if mirror { w - 1 - x } else { x };
                        r.cells[y * w + rx] == id(x0 + x, y0 + y)
                    })
                })
            })
    })
}

/// Ce que donne la grille : une recette, ou la reparation de deux outils identiques uses,
/// dont les durabilites s'additionnent avec un bonus de 5 %.
pub fn result(grid: &[Slot]) -> Option<Stack> {
    let used: Vec<Stack> = grid.iter().flatten().copied().collect();
    if let [a, b] = used[..] {
        let max = max_dur(a.id);
        if a.id == b.id && max > 0 && a.n == 1 && b.n == 1 {
            let dur = (a.dur as u32 + b.dur as u32 + max as u32 / 20).min(max as u32) as u16;
            return Some(Stack { id: a.id, n: 1, dur });
        }
    }
    match_grid(grid).map(|r| Stack::new(r.out, r.n))
}

/// Ingredients regroupes par objet.
pub fn ingredients(r: &Recipe) -> Vec<(Id, u32)> {
    let mut out: Vec<(Id, u32)> = Vec::new();
    for &c in r.cells.iter().filter(|&&c| c != 0) {
        match out.iter_mut().find(|(id, _)| *id == c) {
            Some(e) => e.1 += 1,
            None => out.push((c, 1)),
        }
    }
    out
}

pub fn can_craft(r: &Recipe, inv: &[Slot]) -> bool {
    ingredients(r).iter().all(|&(id, n)| count(inv, id) >= n)
}

/// Resultat de la cuisson d'un objet au four.
pub fn smelt(id: Id) -> Option<Id> {
    if let Some(out) = crate::mods::content().smelts.iter().find(|d| d.input == id).map(|d| d.out) {
        return Some(out);
    }
    Some(match id {
        x if x == IRON_ORE as Id => IRON_INGOT,
        x if x == GOLD_ORE as Id => GOLD_INGOT,
        x if x == SAND as Id => GLASS as Id,
        x if x == COBBLE as Id => STONE as Id,
        x if x == SLATE as Id => STONE as Id,
        x if x == SATETSU as Id => TAMA_SHARD,
        x if x == LOG as Id => COAL,
        RAW_MEAT => COOKED_MEAT,
        WHEAT_ITEM => COOKED_RICE,
        SOY_BEANS => EDAMAME,
        RAW_BIRD => YAKITORI,
        RAW_FISH => COOKED_FISH,
        TEA_LEAF => DRIED_TEA,
        KABOCHA_SLICE => ROAST_KABOCHA,
        EGG => TAMAGOYAKI,
        _ => return None,
    })
}

/// Ce qui se cuit sur un gril shichirin ou un foyer irori : la cuisine, pas les minerais.
pub fn grills(id: Id) -> bool {
    smelt(id).is_some_and(|r| meal(r).is_some() || r == DRIED_TEA)
}
/// Duree de combustion en secondes (0 = pas un combustible).
pub fn fuel(id: Id) -> f64 {
    if let Some(f) = crate::mods::with_item(id, |d| d.fuel as f64) {
        return f;
    }
    match id {
        COAL => 80.0,
        LAVA_BUCKET => 1000.0,
        STICK | BOWL => 5.0,
        x if x == BAMBOO as Id => 2.5,
        x if x == PLANKS as Id || x == LOG as Id || x == CRAFT as Id || x == CHEST as Id => 15.0,
        x if x == SAPLING as Id => 5.0,
        _ => 0.0,
    }
}

pub const COOK_TIME: f64 = 8.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(cells: [Id; 9]) -> Vec<Slot> {
        cells.iter().map(|&id| if id == 0 { None } else { Some(Stack::new(id, 1)) }).collect()
    }

    #[test]
    fn matches_anywhere_and_mirrored() {
        let (p, s) = (PLANKS as Id, STICK);
        // Planches -> batons, decale dans la grille.
        let g = grid([0, 0, 0, 0, 0, p, 0, 0, p]);
        assert_eq!(match_grid(&g).unwrap().out, STICK);
        // Hache en miroir.
        let g = grid([p, p, 0, s, p, 0, s, 0, 0]);
        assert_eq!(match_grid(&g).unwrap().out, tool_id(AXE, 0));
        // Deux pioches usees se fondent en une, avec 5 % de bonus ; jamais au-dela du neuf.
        let pick = tool_id(PICK, 2);
        let mut g: Vec<Slot> = vec![None; 9];
        g[1] = Some(Stack { id: pick, n: 1, dur: 100 });
        g[7] = Some(Stack { id: pick, n: 1, dur: 60 });
        assert_eq!(result(&g), Some(Stack { id: pick, n: 1, dur: 100 + 60 + 251 / 20 }));
        g[7] = Some(Stack { id: pick, n: 1, dur: 240 });
        assert_eq!(result(&g).map(|s| s.dur), Some(251));
        // Tamahagane sur une pioche de diamant.
        let g = grid([0, TAMAHAGANE, 0, 0, tool_id(PICK, 3), 0, 0, 0, 0]);
        assert_eq!(result(&g).map(|s| s.id), Some(tool_id(PICK, 4)));
        // Rien de valide.
        let g = grid([p, 0, 0, 0, 0, 0, 0, 0, s]);
        assert!(match_grid(&g).is_none());
    }
}
