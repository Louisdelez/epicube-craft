//! Objets : piles, outils, nourriture, temps de minage et butin des blocs.
//! Les identifiants < 256 sont des blocs, les autres des objets.
use crate::blocks::*;
use crate::util::Rng;
use serde::{Deserialize, Serialize};

pub type Id = u16;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Stack {
    pub id: Id,
    pub n: u8,
    /// Usure restante des outils (0 pour le reste).
    pub dur: u16,
}

pub type Slot = Option<Stack>;

impl Stack {
    pub fn new(id: Id, n: u8) -> Stack {
        Stack { id, n, dur: max_dur(id) }
    }
}

pub const STICK: Id = 256;
pub const COAL: Id = 257;
pub const IRON_INGOT: Id = 258;
pub const GOLD_INGOT: Id = 259;
pub const DIAMOND: Id = 260;
pub const VOLT_DUST: Id = 261;
pub const SEEDS: Id = 262;
pub const WHEAT_ITEM: Id = 263;
pub const BREAD: Id = 264;
pub const CARROT_ITEM: Id = 265;
pub const RAW_MEAT: Id = 266;
pub const COOKED_MEAT: Id = 267;
pub const LEATHER: Id = 268;
pub const FEATHER: Id = 269;
pub const EGG: Id = 270;
pub const STRING: Id = 271;
pub const BONE: Id = 272;
pub const ROTTEN: Id = 273;
pub const SPORE: Id = 274;
pub const PEARL: Id = 275;
pub const EMBER_CRYSTAL: Id = 276;
pub const BUCKET: Id = 277;
pub const WATER_BUCKET: Id = 278;
pub const LAVA_BUCKET: Id = 279;
pub const LIGHTER: Id = 280;
pub const APPLE: Id = 281;
pub const FLINT: Id = 282;
pub const WYRM_HEART: Id = 283;
pub const ARROW: Id = 284;
pub const CART: Id = 285;
/// Acier forge de l'Abime : ameliore un outil de diamant.
pub const TAMAHAGANE: Id = 286;
pub const ITEM_END: Id = 287;

// Cuisine et ferme (les identifiants 300 a 319 sont ceux des outils).
pub const COOKED_RICE: Id = 320;
pub const SOY_BEANS: Id = 321;
pub const EDAMAME: Id = 322;
pub const TOFU: Id = 323;
pub const MISO_SOUP: Id = 324;
pub const NABE: Id = 325;
pub const RAW_BIRD: Id = 326;
pub const YAKITORI: Id = 327;
pub const RAW_FISH: Id = 328;
pub const COOKED_FISH: Id = 329;
pub const FUGU: Id = 330;
pub const SUSHI: Id = 331;
pub const NORI: Id = 332;
pub const TEA_LEAF: Id = 333;
pub const DRIED_TEA: Id = 334;
pub const GREEN_TEA: Id = 335;
pub const KABOCHA_SLICE: Id = 336;
pub const ROAST_KABOCHA: Id = 337;
pub const KABOCHA_SEEDS: Id = 338;
pub const TAMAGOYAKI: Id = 339;
pub const BENTO: Id = 340;
pub const BOWL: Id = 341;
pub const FERTILIZER: Id = 342;
pub const SHEARS: Id = 343;
pub const FISHING_ROD: Id = 344;
pub const MORE_END: Id = 345;
/// Outils : TOOL_BASE + genre * 4 + niveau (bois, pierre, fer, diamant).
pub const TOOL_BASE: Id = 300;
pub const PICK: u8 = 0;
pub const AXE: u8 = 1;
pub const SHOVEL: u8 = 2;
pub const SWORD: u8 = 3;
pub const HOE: u8 = 4;

/// Cinquieme niveau (tamahagane) : TOOL4_BASE + genre.
pub const TOOL4_BASE: Id = 360;
// Armes japonaises.
/// Arc long.
pub const YUMI: Id = 365;
/// Lance : se manie ou se jette.
pub const YARI: Id = 366;
/// Arme d'hast a longue portee, qui fauche tout un groupe.
pub const NAGINATA: Id = 367;
/// Dague tres rapide.
pub const TANTO: Id = 368;
/// Massue cloutee, lente et devastatrice.
pub const TETSUBO: Id = 369;
/// Lame de jet.
pub const KUNAI: Id = 370;
/// Bouclier de bois (tate).
pub const SHIELD: Id = 371;

// Objets des autres mondes.
/// Eclat de tamahagane, tire du sable de fer de Yomi.
pub const TAMA_SHARD: Id = 372;
/// Robe de plumes celeste : dans l'inventaire, elle permet de planer.
pub const HAGOROMO: Id = 373;
/// Peche celeste : la manger transporte a quelques pas de la.
pub const PEACH: Id = 374;
/// Koban : piece d'or ovale, monnaie des villages.
pub const KOBAN: Id = 375;
pub const REALM_END: Id = 376;

/// Armures : ARMOR_BASE + matiere * 4 + emplacement.
pub const ARMOR_BASE: Id = 380;
pub const ARMOR_END: Id = ARMOR_BASE + 24;
/// Emplacements d'equipement.
pub const HEAD: u8 = 0;
pub const BODY: u8 = 1;
pub const LEGS: u8 = 2;
pub const FEET: u8 = 3;
/// Matieres d'armure : cuir, bambou, fer, or, diamant, tamahagane.
pub const ARMOR_TIERS: u8 = 6;

pub fn armor_id(slot: u8, tier: u8) -> Id {
    ARMOR_BASE + tier as Id * 4 + slot as Id
}

/// (emplacement, matiere) d'une piece d'armure.
pub fn armor(id: Id) -> Option<(u8, u8)> {
    (ARMOR_BASE..ARMOR_END).contains(&id).then(|| (((id - ARMOR_BASE) % 4) as u8, ((id - ARMOR_BASE) / 4) as u8))
}

/// Emplacement ou se porte un objet : les armures, et la robe de plumes qui se met sur le torse.
pub fn wear_slot(id: Id) -> Option<u8> {
    if id == HAGOROMO {
        Some(BODY)
    } else {
        armor(id).map(|a| a.0)
    }
}

/// Points d'armure d'une piece (un demi-plastron de la barre chacun ; 20 au plus pour une tenue complete).
pub fn armor_points(id: Id) -> f64 {
    const POINTS: [[f64; 4]; 6] = [[1.0, 3.0, 2.0, 1.0], [2.0, 4.0, 3.0, 1.0], [2.0, 6.0, 5.0, 2.0], [2.0, 5.0, 3.0, 1.0], [3.0, 8.0, 6.0, 3.0], [3.0, 8.0, 6.0, 3.0]];
    armor(id).map_or(0.0, |(slot, tier)| POINTS[tier as usize][slot as usize])
}

/// Robustesse d'une piece : elle empeche les gros coups de percer l'armure.
pub fn armor_toughness(id: Id) -> f64 {
    armor(id).map_or(0.0, |(_, tier)| [0.0, 0.0, 0.0, 0.0, 2.0, 3.0][tier as usize])
}

/// Part du recul annulee par une piece (le tamahagane seul tient bon).
pub fn armor_steady(id: Id) -> f64 {
    armor(id).map_or(0.0, |(_, tier)| if tier == 5 { 0.1 } else { 0.0 })
}

/// Protection d'une tenue : (points, robustesse, tenue au recul).
pub fn defense(worn: &[Slot]) -> (f64, f64, f64) {
    worn.iter().flatten().fold((0.0, 0.0, 0.0), |a, st| (a.0 + armor_points(st.id), a.1 + armor_toughness(st.id), a.2 + armor_steady(st.id)))
}

/// Degats restants apres l'armure. Chaque point retire 4 %, mais un coup violent en perce une partie,
/// d'autant moins que l'armure est robuste ; elle n'arrete jamais plus de 80 %.
pub fn mitigate(dmg: f64, points: f64, toughness: f64) -> f64 {
    let eff = (points - 4.0 * dmg / (toughness + 8.0)).max(points / 5.0).min(20.0);
    dmg * (1.0 - eff / 25.0)
}

/// Usure que subit chaque piece portee quand un coup de `dmg` est encaisse.
pub fn armor_wear(dmg: f64) -> u16 {
    (dmg / 4.0).floor().max(1.0) as u16
}

pub fn tool_id(kind: u8, tier: u8) -> Id {
    if tier >= 4 {
        TOOL4_BASE + kind as Id
    } else {
        TOOL_BASE + kind as Id * 4 + tier as Id
    }
}

/// (genre, niveau) si l'objet est un outil. Niveaux : bois, pierre, fer, diamant, tamahagane.
pub fn tool(id: Id) -> Option<(u8, u8)> {
    if id >= crate::mods::FIRST_ITEM {
        // Un outil de mod : son genre (decale d'un cran, 0 voulant dire « pas un outil ») et son niveau.
        return crate::mods::with_item(id, |d| (d.tool, d.tier)).filter(|(k, _)| (1..=5).contains(k)).map(|(k, t)| (k - 1, t.min(4)));
    }
    if (TOOL_BASE..TOOL_BASE + 20).contains(&id) {
        let k = id - TOOL_BASE;
        Some(((k / 4) as u8, (k % 4) as u8))
    } else if (TOOL4_BASE..TOOL4_BASE + 5).contains(&id) {
        Some(((id - TOOL4_BASE) as u8, 4))
    } else {
        None
    }
}

pub fn max_dur(id: Id) -> u16 {
    if let Some(d) = crate::mods::with_item(id, |d| d.durability) {
        return d;
    }
    if let Some((slot, tier)) = armor(id) {
        // Le plastron dure plus que le casque ; le diamant, six fois plus que le cuir.
        return [11, 16, 15, 13][slot as usize] * [5, 9, 15, 7, 33, 37][tier as usize];
    }
    match tool(id) {
        Some((_, tier)) => [60, 132, 251, 1562, 2032][tier as usize],
        None => match id {
            LIGHTER | FISHING_ROD => 64,
            SHEARS => 238,
            YUMI => 384,
            YARI | TANTO => 250,
            NAGINATA => 300,
            TETSUBO => 500,
            SHIELD => 336,
            HAGOROMO => 432,
            _ => 0,
        },
    }
}

/// Caracteristiques d'une arme au corps a corps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weapon {
    /// Degats d'un coup porte a pleine charge.
    pub dmg: f64,
    /// Coups a pleine charge par seconde : plus c'est bas, plus il faut attendre entre deux coups.
    pub speed: f64,
    /// Allonge en plus de celle du bras.
    pub reach: f64,
    /// Recul inflige (1 = normal).
    pub knock: f64,
    /// Usure par coup porte (0 : ce n'est pas une arme).
    pub wear: u16,
}

/// Arme que represente l'objet tenu. Un outil frappe aussi, mais s'use deux fois plus vite qu'une arme.
pub fn weapon(held: Option<Id>) -> Weapon {
    let w = |dmg: f64, speed: f64, reach: f64, knock: f64, wear: u16| Weapon { dmg, speed, reach, knock, wear };
    let Some(id) = held else { return w(1.0, 4.0, 0.0, 1.0, 0) };
    // Une arme de mod : ses degats, a la cadence d'un katana.
    if let Some(dmg) = crate::mods::with_item(id, |d| d.damage as f64).filter(|d| *d > 0.0) {
        return w(dmg, 1.6, 0.0, 1.0, 1);
    }
    match tool(id) {
        Some((kind, tier)) => {
            let t = tier as usize;
            match kind {
                SWORD => w([4.0, 5.0, 6.0, 7.0, 8.0][t], 1.6, 0.0, 1.0, 1),
                AXE => w([7.0, 9.0, 9.0, 9.0, 10.0][t], [0.8, 0.8, 0.9, 1.0, 1.0][t], 0.0, 1.0, 2),
                PICK => w([2.0, 3.0, 4.0, 5.0, 6.0][t], 1.2, 0.0, 1.0, 2),
                SHOVEL => w([2.5, 3.5, 4.5, 5.5, 6.5][t], 1.0, 0.0, 1.0, 2),
                _ => w(1.0, [1.0, 2.0, 3.0, 4.0, 4.0][t], 0.0, 1.0, 2),
            }
        }
        None => match id {
            TANTO => w(3.5, 2.6, 0.0, 0.6, 1),
            NAGINATA => w(6.0, 1.1, 1.5, 1.0, 1),
            TETSUBO => w(9.0, 0.6, 0.0, 1.7, 1),
            YARI => w(7.0, 1.1, 0.7, 1.0, 1),
            _ => w(1.0, 4.0, 0.0, 1.0, 0),
        },
    }
}

/// Part des degats selon le temps `elapsed` (s) ecoule depuis le coup precedent :
/// 20 % si l'on frappe sans attendre, 100 % une fois l'arme revenue en garde.
pub fn charge(elapsed: f64, speed: f64) -> f64 {
    let t = (elapsed * speed).clamp(0.0, 1.0);
    0.2 + 0.8 * t * t
}

/// Force d'un tir a l'arc pour une corde tenue `held` secondes (0 a 1).
pub fn draw_power(held: f64) -> f64 {
    let t = held.clamp(0.0, 1.0);
    (t * t + 2.0 * t) / 3.0
}

pub fn max_stack(id: Id) -> u8 {
    if let Some(n) = crate::mods::with_item(id, |d| if d.durability > 0 { 1 } else { d.stack.clamp(1, 64) }) {
        return n;
    }
    if id == KUNAI {
        16
    } else if max_dur(id) > 0 || matches!(id, BUCKET | WATER_BUCKET | LAVA_BUCKET | CART | MISO_SOUP | NABE | GREEN_TEA) || id == BED as Id {
        1
    } else {
        64
    }
}

/// (faim, saturation) rendues par un aliment. La saturation est une reserve cachee :
/// elle se vide avant la faim, et c'est elle qui fait la valeur d'un vrai repas.
pub fn meal(id: Id) -> Option<(f64, f64)> {
    if id >= crate::mods::FIRST_ITEM {
        return crate::mods::with_item(id, |d| (d.food as f64, d.saturation as f64)).filter(|m| m.0 > 0.0);
    }
    Some(match id {
        APPLE | PEACH => (4.0, 2.4),
        BREAD => (5.0, 6.0),
        CARROT_ITEM => (3.0, 3.6),
        RAW_MEAT => (3.0, 1.8),
        COOKED_MEAT => (8.0, 12.8),
        ROTTEN => (4.0, 0.8),
        COOKED_RICE => (3.0, 3.6),
        SOY_BEANS => (1.0, 0.6),
        EDAMAME => (5.0, 6.0),
        TOFU => (4.0, 4.8),
        MISO_SOUP => (6.0, 7.2),
        NABE => (10.0, 12.0),
        RAW_BIRD => (2.0, 1.2),
        YAKITORI => (6.0, 7.2),
        RAW_FISH => (2.0, 0.4),
        COOKED_FISH => (6.0, 9.6),
        FUGU => (1.0, 0.2),
        SUSHI => (5.0, 6.0),
        KABOCHA_SLICE => (2.0, 1.2),
        ROAST_KABOCHA => (5.0, 6.0),
        TAMAGOYAKI => (4.0, 4.8),
        BENTO => (12.0, 14.4),
        GREEN_TEA => (1.0, 1.2),
        _ => return None,
    })
}
/// Points de faim rendus par un aliment.
pub fn food(id: Id) -> Option<f64> {
    meal(id).map(|m| m.0)
}
/// Ce qu'on risque en mangeant.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Risk {
    None,
    /// Nausee pendant 30 s, avec cette probabilite : la faim descend plus vite.
    Queasy(f64),
    /// Poison long et nausee, a coup sur.
    Poison,
}
pub fn risk(id: Id) -> Risk {
    match id {
        ROTTEN => Risk::Queasy(0.8),
        RAW_BIRD => Risk::Queasy(0.3),
        FUGU => Risk::Poison,
        _ => Risk::None,
    }
}
/// Les soupes et le the se servent dans un bol, qu'on recupere.
pub fn served_in_bowl(id: Id) -> bool {
    matches!(id, MISO_SOUP | NABE | GREEN_TEA)
}
/// Chance qu'un dechet vegetal fasse monter le compost d'un cran (0 = ne se composte pas).
pub fn compost_chance(id: Id) -> f64 {
    match id {
        SEEDS | KABOCHA_SEEDS | NORI | DRIED_TEA => 0.3,
        TEA_LEAF | KABOCHA_SLICE => 0.5,
        WHEAT_ITEM | CARROT_ITEM | SOY_BEANS | APPLE => 0.65,
        BREAD | EDAMAME | COOKED_RICE | TOFU | ROAST_KABOCHA => 0.85,
        BENTO => 1.0,
        1..=255 => match id as B {
            SAPLING | LEAVES | CEDAR_LEAVES | MAPLE_LEAVES | TALLGRASS | MOSS => 0.3,
            CACTUS | BAMBOO => 0.5,
            FLOWER | KABOCHA | TEA | LOTUS => 0.65,
            _ => 0.0,
        },
        _ => 0.0,
    }
}
/// Tuile de l'atlas qui represente l'objet dans l'inventaire.
pub fn icon(id: Id) -> u16 {
    if let Some(t) = crate::mods::with_item(id, |d| d.tile) {
        return t;
    }
    if id < 256 {
        // Les plaques minces se reconnaissent a leur dessus.
        let face = if shape(id as B) == Shape::Thin { 2 } else { 4 };
        // Lanterne allumee, theier bien fourni.
        let meta = match id as B {
            SIGTORCH => 1,
            TEA => 2,
            _ => 0,
        };
        tile(id as B, meta, face)
    } else if (COOKED_RICE..MORE_END).contains(&id) {
        t::ITEM_MORE + (id - COOKED_RICE)
    } else if let Some((kind, tier)) = tool(id) {
        // Les outils de tamahagane et les armes suivent les vingt outils d'origine.
        if tier >= 4 {
            t::ITEM_TOOLS + 20 + kind as u16
        } else {
            t::ITEM_TOOLS + (id - TOOL_BASE)
        }
    } else if (TAMA_SHARD..REALM_END).contains(&id) {
        t::ITEM_REALM + (id - TAMA_SHARD)
    } else if (ARMOR_BASE..ARMOR_END).contains(&id) {
        let k = id - ARMOR_BASE;
        t::ITEM_ARMOR[(k / 8) as usize] + k % 8
    } else if (YUMI..=SHIELD).contains(&id) {
        t::ITEM_TOOLS + 25 + (id - YUMI)
    } else {
        t::ITEM_MISC + (id - 256)
    }
}

/// Bloc pose quand on utilise l'objet, s'il y en a un.
pub fn place_block(id: Id) -> Option<B> {
    match id {
        SEEDS => Some(WHEAT),
        CARROT_ITEM => Some(CARROT),
        VOLT_DUST => Some(WIRE),
        STRING => Some(TRIPWIRE),
        SOY_BEANS => Some(SOY),
        KABOCHA_SEEDS => Some(STEM),
        // Les numeros reserves aux mods n'existent que si un mod de la partie les a pris.
        160..=255 if crate::mods::with(id as B, |_| ()).is_none() => None,
        1..=255 if !matches!(id as B, WATER | LAVA | WHEAT | CARROT | WIRE | PISTON_HEAD | TRIPWIRE | SOY | STEM) => Some(id as B),
        _ => None,
    }
}

/// Blocs qui disparaissent quand leur support est retire.
pub fn needs_support(b: B) -> bool {
    matches!(shape(b), Shape::Cross | Shape::Flat | Shape::Thin) || b == CACTUS
}
/// Direction du bloc qui porte `b` : le sol, sauf pour ce qui s'accroche aux murs.
pub fn support_dir(b: B, m: u8) -> [i32; 3] {
    match b {
        LEVER | BUTTON | SIGTORCH | HOOK => crate::signal::attach_of(m),
        // La stalactite tient au plafond.
        STALACTITE if m & 1 == 1 => [0, 1, 0],
        _ => [0, -1, 0],
    }
}

/// Le bloc `b` peut-il tenir sur `below` ?
pub fn support_ok(b: B, below: B) -> bool {
    match b {
        WHEAT | CARROT | SOY | STEM => below == FARMLAND,
        SAPLING | FLOWER | TALLGRASS | TEA => matches!(below, GRASS | DIRT | MOSS | CRIMSON_MOSS),
        ONIBI => below == SOUL_SAND,
        STALACTITE => opaque(below),
        LOTUS => below == WATER,
        BAMBOO => matches!(below, GRASS | DIRT | BAMBOO),
        CACTUS => matches!(below, SAND | CACTUS),
        TORCH | LEVER | SIGTORCH | WIRE | REPEATER | COMPARATOR | BUTTON | PLATE | HOOK | RAIL | POWERED_RAIL | DETECTOR_RAIL => {
            opaque(below) || below == GLASS
        }
        _ => true,
    }
}

pub fn replaceable(b: B) -> bool {
    matches!(b, AIR | WATER | LAVA | TALLGRASS)
}

/// Durete : < 0 incassable, 0 instantane.
pub fn hardness(b: B) -> f64 {
    if let Some(h) = crate::mods::with(b, |d| d.hardness as f64) {
        return h;
    }
    match b {
        AIR | WATER | LAVA | BEDROCK => -1.0,
        TALLGRASS | FLOWER | SAPLING | WHEAT | CARROT | SOY | TEA | STEM | LOTUS | ONIBI | TORCH | WIRE | LEVER | SIGTORCH | EXPLOSIVE | REPEATER | COMPARATOR | BUTTON | TRIPWIRE | HOOK => 0.0,
        LEAVES | CEDAR_LEAVES | MAPLE_LEAVES | WISTERIA | BED | SUNDIAL => 0.2,
        GLASS | LAMP | EMBER | RAIL | POWERED_RAIL | DETECTOR_RAIL | BAMBOO | STALACTITE => 0.3,
        CACTUS | ABYSS_ROCK => 0.4,
        DIRT | SAND | FARMLAND | ICE | SNOW | PLATE | MOSS | GLOW_MOSS | SOUL_SAND | CRIMSON_MOSS => 0.5,
        GRASS | GRAVEL | COMPOSTER => 0.6,
        WOOL | NOTE | TARGET => 0.8,
        STONE | PISTON | STICKY_PISTON | PISTON_HEAD | KABOCHA | CRYSTAL | BASALT => 1.5,
        LOG | PLANKS | COBBLE | BRICK | IRORI | COBWEB | BONE_BLOCK | VERMILION => 2.0,
        CRAFT | CHEST => 2.5,
        DOOR | ZENITH_STONE | OBSERVER | HOPPER | SLATE => 3.0,
        GOLD_ORE | IRON_ORE | COAL_ORE | DIAMOND_ORE | VOLT_ORE => 3.0,
        FURNACE | DROPPER | SMOKER => 3.5,
        IRON_BLOCK | ABYSS_PORTAL | ZENITH_PORTAL | VOLT_BLOCK | SPAWNER => 5.0,
        OBSIDIAN => 50.0,
        SATETSU => 30.0,
        GATEWAY => -1.0,
        _ => 1.0,
    }
}

/// Genre d'outil qui accelere le minage du bloc.
fn best_tool(b: B) -> Option<u8> {
    if b >= crate::mods::FIRST_BLOCK {
        return match crate::mods::with(b, |d| d.tool)? {
            crate::mods::tool::PICK => Some(PICK),
            crate::mods::tool::AXE => Some(AXE),
            crate::mods::tool::SHOVEL => Some(SHOVEL),
            _ => None,
        };
    }
    match b {
        STONE | COBBLE | BRICK | FURNACE | GOLD_ORE | IRON_ORE | COAL_ORE | DIAMOND_ORE | VOLT_ORE | IRON_BLOCK
        | OBSIDIAN | ABYSS_ROCK | ZENITH_STONE | ICE | LAMP | PISTON | STICKY_PISTON | PISTON_HEAD | OBSERVER | VOLT_BLOCK
        | DROPPER | HOPPER | RAIL | POWERED_RAIL | DETECTOR_RAIL | SMOKER | SLATE | CRYSTAL | BASALT | STALACTITE | SPAWNER | SATETSU | BONE_BLOCK | CRIMSON_MOSS => Some(PICK),
        LOG | PLANKS | CRAFT | CHEST | DOOR | BED | PLATE | SUNDIAL | NOTE | KABOCHA | BAMBOO | COMPOSTER | IRORI | VERMILION => Some(AXE),
        DIRT | GRASS | SAND | GRAVEL | FARMLAND | SNOW | MOSS | GLOW_MOSS | SOUL_SAND => Some(SHOVEL),
        _ => None,
    }
}

/// Niveau de pioche minimal pour que le bloc donne quelque chose.
fn min_pick(b: B) -> Option<u8> {
    match b {
        STONE | COBBLE | BRICK | FURNACE | SMOKER | COAL_ORE | ABYSS_ROCK | ZENITH_STONE | SLATE | BASALT | CRYSTAL => Some(0),
        IRON_ORE | IRON_BLOCK => Some(1),
        GOLD_ORE | DIAMOND_ORE | VOLT_ORE => Some(2),
        OBSIDIAN | SATETSU => Some(3),
        _ => None,
    }
}

pub fn can_harvest(b: B, held: Option<Id>) -> bool {
    match min_pick(b) {
        None => true,
        Some(need) => matches!(held.and_then(tool), Some((PICK, tier)) if tier >= need),
    }
}

/// Secondes necessaires pour casser le bloc, ou None s'il est incassable.
pub fn break_time(b: B, held: Option<Id>) -> Option<f64> {
    let h = hardness(b);
    if h < 0.0 {
        return None;
    }
    let mult = match (held.and_then(tool), best_tool(b)) {
        (Some((kind, tier)), Some(best)) if kind == best => [2.0, 4.0, 6.0, 8.0, 9.0][tier as usize],
        // Les ciseaux coupent net feuillage et fourrure ; une lame tranche le bambou.
        _ if held == Some(SHEARS) && matches!(b, LEAVES | CEDAR_LEAVES | MAPLE_LEAVES | WISTERIA | WOOL | TALLGRASS | COBWEB) => 15.0,
        (Some((SWORD, _)), _) if matches!(b, BAMBOO | COBWEB) => 15.0,
        _ => 1.0,
    };
    Some(if can_harvest(b, held) { h * 1.5 / mult } else { h * 5.0 })
}

/// Objets laisses par un bloc casse.
pub fn drops(b: B, m: u8, rng: &mut Rng) -> Vec<Stack> {
    let one = |id: Id, n: i32| vec![Stack::new(id, n as u8)];
    // Bloc de mod : lui-meme, l'objet que le mod a choisi, ou rien.
    match crate::mods::with(b, |d| d.drop) {
        Some(crate::mods::DROP_NOTHING) => return vec![],
        Some(0) | None => {}
        Some(id) => return one(id, 1),
    }
    match b {
        AIR | WATER | LAVA | BEDROCK | GLASS | ICE | PISTON_HEAD | SPAWNER | ONIBI | GATEWAY => vec![],
        ABYSS_PORTAL if m & 2 != 0 => vec![],
        CRIMSON_MOSS => one(ABYSS_ROCK as Id, 1),
        // La glycine celeste donne parfois une peche.
        WISTERIA => {
            if rng.chance(0.1) {
                one(PEACH, 1)
            } else {
                vec![]
            }
        }
        COBWEB => one(STRING, 1),
        // Les autres feuillages laissent parfois une pousse ou une branche.
        CEDAR_LEAVES | MAPLE_LEAVES => {
            if rng.chance(0.06) {
                one(SAPLING as Id, 1)
            } else if rng.chance(0.08) {
                one(STICK, 1)
            } else {
                vec![]
            }
        }
        STONE => one(COBBLE as Id, 1),
        GRASS | FARMLAND | SNOW => one(DIRT as Id, 1),
        COAL_ORE => one(COAL, 1),
        DIAMOND_ORE => one(DIAMOND, 1),
        VOLT_ORE => one(VOLT_DUST, rng.range(3, 6)),
        EMBER => one(EMBER_CRYSTAL, 2),
        WIRE => one(VOLT_DUST, 1),
        TRIPWIRE => one(STRING, 1),
        LEAVES => {
            if rng.chance(0.08) {
                one(SAPLING as Id, 1)
            } else if rng.chance(0.06) {
                one(APPLE, 1)
            } else {
                vec![]
            }
        }
        TALLGRASS => {
            // Les herbes folles cachent des semences de toutes sortes.
            let r = rng.f();
            if r < 0.12 {
                one(SEEDS, 1)
            } else if r < 0.17 {
                one(SOY_BEANS, 1)
            } else if r < 0.21 {
                one(CARROT_ITEM, 1)
            } else if r < 0.24 {
                one(KABOCHA_SEEDS, 1)
            } else {
                vec![]
            }
        }
        SOY => one(SOY_BEANS, if m >= 3 { rng.range(2, 5) } else { 1 }),
        TEA => {
            let mut v = one(TEA as Id, 1);
            if m >= 3 {
                v.push(Stack::new(TEA_LEAF, rng.range(2, 4) as u8));
            }
            v
        }
        STEM => one(KABOCHA_SEEDS, if m >= 3 { rng.range(1, 4) } else { 1 }),
        GRAVEL => {
            if rng.chance(0.125) {
                one(FLINT, 1)
            } else {
                one(GRAVEL as Id, 1)
            }
        }
        WHEAT => {
            if m >= 3 {
                vec![Stack::new(WHEAT_ITEM, 1), Stack::new(SEEDS, rng.range(1, 4) as u8)]
            } else {
                one(SEEDS, 1)
            }
        }
        CARROT => one(CARROT_ITEM, if m >= 3 { rng.range(2, 5) } else { 1 }),
        _ => one(b as Id, 1),
    }
}

pub fn name(id: Id) -> String {
    if let Some(n) = crate::mods::with_item(id, |d| d.name.clone()) {
        return n;
    }
    if id < 256 {
        if let Some(n) = crate::mods::with(id as B, |d| d.name.clone()) {
            return n;
        }
    }
    if let Some((kind, tier)) = tool(id) {
        // Un katana de bois est un sabre d'entrainement.
        if (kind, tier) == (SWORD, 0) {
            return "Bokken".to_string();
        }
        let k = ["Pioche", "Hache ono", "Pelle", "Katana", "Houe kuwa"][kind as usize];
        let m = ["en bois", "en pierre", "en fer", "en diamant", "de tamahagane"][tier as usize];
        return format!("{k} {m}");
    }
    if let Some((slot, tier)) = armor(id) {
        // La tenue de cuir est celle du voyageur ; les autres sont de vraies armures.
        if tier == 0 {
            return ["Chapeau kasa", "Veste de cuir", "Hakama de cuir", "Sandales waraji"][slot as usize].to_string();
        }
        let k = ["Casque kabuto", "Cuirasse do", "Cuissards haidate", "Jambieres suneate"][slot as usize];
        let m = ["", "de bambou", "de fer", "d'or", "de diamant", "de tamahagane"][tier as usize];
        return format!("{k} {m}");
    }
    let s = match id {
        STICK => "Baton",
        COAL => "Charbon",
        IRON_INGOT => "Lingot de fer",
        GOLD_INGOT => "Lingot d'or",
        DIAMOND => "Diamant",
        VOLT_DUST => "Poudre de reiseki",
        SEEDS => "Grains de riz",
        WHEAT_ITEM => "Gerbe de riz",
        BREAD => "Onigiri",
        CARROT_ITEM => "Daikon",
        RAW_MEAT => "Viande crue",
        COOKED_MEAT => "Viande cuite",
        LEATHER => "Cuir",
        FEATHER => "Plume",
        EGG => "Oeuf",
        STRING => "Corde shimenawa",
        BONE => "Os",
        ROTTEN => "Chair d'oni",
        SPORE => "Poudre de lanterne",
        PEARL => "Perle d'esprit",
        EMBER_CRYSTAL => "Cristal de braise",
        BUCKET => "Seau en bois",
        WATER_BUCKET => "Seau d'eau",
        LAVA_BUCKET => "Seau de lave",
        LIGHTER => "Briquet",
        APPLE => "Pomme",
        FLINT => "Silex",
        WYRM_HEART => "Perle du Ryujin",
        ARROW => "Fleche",
        CART => "Chariot daihachi",
        TAMAHAGANE => "Lingot de tamahagane",
        TAMA_SHARD => "Eclat de tamahagane",
        HAGOROMO => "Robe de plumes hagoromo",
        PEACH => "Peche celeste",
        KOBAN => "Koban",
        YUMI => "Arc yumi",
        YARI => "Lance yari",
        NAGINATA => "Naginata",
        TANTO => "Dague tanto",
        TETSUBO => "Massue tetsubo",
        KUNAI => "Kunai",
        SHIELD => "Bouclier tate",
        COOKED_RICE => "Riz cuit",
        SOY_BEANS => "Graines de soja",
        EDAMAME => "Edamame",
        TOFU => "Tofu",
        MISO_SOUP => "Soupe miso",
        NABE => "Marmite nabe",
        RAW_BIRD => "Volaille crue",
        YAKITORI => "Yakitori",
        RAW_FISH => "Poisson cru",
        COOKED_FISH => "Poisson grille",
        FUGU => "Fugu",
        SUSHI => "Sushi",
        NORI => "Algue nori",
        TEA_LEAF => "Feuilles de the",
        DRIED_TEA => "The seche",
        GREEN_TEA => "The vert",
        KABOCHA_SLICE => "Tranche de kabocha",
        ROAST_KABOCHA => "Kabocha roti",
        KABOCHA_SEEDS => "Graines de kabocha",
        TAMAGOYAKI => "Tamagoyaki",
        BENTO => "Bento",
        BOWL => "Bol",
        FERTILIZER => "Engrais",
        SHEARS => "Ciseaux",
        FISHING_ROD => "Canne a peche",
        _ if id < 256 => match id as B {
            STONE => "Pierre",
            GRASS => "Herbe",
            DIRT => "Terre",
            COBBLE => "Pierre de muraille",
            PLANKS => "Planches de hinoki",
            SAPLING => "Jeune cerisier",
            BEDROCK => "Roche-mere",
            SAND => "Sable",
            GRAVEL => "Gravier",
            GOLD_ORE => "Minerai d'or",
            IRON_ORE => "Minerai de fer",
            COAL_ORE => "Minerai de charbon",
            LOG => "Tronc de cedre",
            LEAVES => "Feuilles",
            GLASS => "Paroi shoji",
            DIAMOND_ORE => "Minerai de diamant",
            VOLT_ORE => "Minerai de reiseki",
            SNOW => "Neige",
            ICE => "Glace",
            CRAFT => "Etabli",
            FURNACE => "Fourneau kamado",
            CHEST => "Coffre tansu",
            OBSIDIAN => "Obsidienne",
            FARMLAND => "Terre labouree",
            TORCH => "Lanterne de papier",
            LEVER => "Levier de bambou",
            SIGTORCH => "Lanterne hitodama",
            LAMP => "Lanterne andon",
            EXPLOSIVE => "Feux d'artifice",
            BED => "Futon",
            WOOL => "Fourrure",
            ABYSS_ROCK => "Roche de Yomi",
            EMBER => "Bloc de braise",
            ABYSS_PORTAL => "Voile de torii",
            ZENITH_STONE => "Pierre celeste",
            ZENITH_PORTAL => "Porte de lune",
            BRICK => "Tuiles kawara",
            TALLGRASS => "Herbes susuki",
            FLOWER => "Lycoris rouge",
            CACTUS => "Cactus",
            IRON_BLOCK => "Bloc de fer",
            DOOR => "Porte fusuma",
            REPEATER => "Relais ofuda",
            COMPARATOR => "Balance onmyo",
            BUTTON => "Sceau hanko",
            PLATE => "Plancher rossignol",
            PISTON => "Poussoir karakuri",
            STICKY_PISTON => "Poussoir karakuri gluant",
            OBSERVER => "Daruma guetteur",
            VOLT_BLOCK => "Bloc de reiseki",
            SUNDIAL => "Cadran solaire",
            DROPPER => "Distributeur karakuri",
            HOPPER => "Tremie masu",
            NOTE => "Tambour taiko",
            RAIL => "Rail",
            POWERED_RAIL => "Rail de reiseki",
            DETECTOR_RAIL => "Rail guetteur",
            HOOK => "Piquet de shimenawa",
            TARGET => "Cible de kyudo",
            TEA => "Theier",
            CEDAR_LEAVES => "Feuillage de cedre",
            MAPLE_LEAVES => "Feuillage d'erable",
            MOSS => "Mousse",
            GLOW_MOSS => "Mousse lumineuse",
            STALACTITE => "Concretion",
            SLATE => "Ardoise",
            CRYSTAL => "Cristal de roche",
            BASALT => "Basalte",
            COBWEB => "Toile de tsuchigumo",
            LOTUS => "Lotus",
            SPAWNER => "Stele maudite",
            CRIMSON_MOSS => "Mousse pourpre",
            SOUL_SAND => "Sable des ames",
            ONIBI => "Onibi",
            BONE_BLOCK => "Bloc d'ossements",
            SATETSU => "Sable de fer satetsu",
            WISTERIA => "Glycine celeste",
            GATEWAY => "Porte celeste",
            VERMILION => "Bois laque vermillon",
            KABOCHA => "Kabocha",
            BAMBOO => "Bambou",
            COMPOSTER => "Bac a compost",
            SMOKER => "Gril shichirin",
            IRORI => "Foyer irori",
            _ => "Bloc",
        },
        _ => "Objet",
    };
    s.to_string()
}

/// Ajoute une pile dans une rangee de cases ; renvoie ce qui n'a pas pu entrer.
pub fn add_to(slots: &mut [Slot], mut st: Stack) -> Option<Stack> {
    let max = max_stack(st.id);
    if max > 1 {
        for s in slots.iter_mut().flatten() {
            if s.id == st.id && s.n < max {
                let mv = (max - s.n).min(st.n);
                s.n += mv;
                st.n -= mv;
                if st.n == 0 {
                    return None;
                }
            }
        }
    }
    for s in slots.iter_mut() {
        if s.is_none() {
            let mv = st.n.min(max);
            *s = Some(Stack { n: mv, ..st });
            st.n -= mv;
            if st.n == 0 {
                return None;
            }
        }
    }
    Some(st)
}

pub fn count(slots: &[Slot], id: Id) -> u32 {
    slots.iter().flatten().filter(|s| s.id == id).map(|s| s.n as u32).sum()
}

/// Retire `n` objets ; suppose que `count` a ete verifie.
pub fn remove(slots: &mut [Slot], id: Id, mut n: u32) {
    for s in slots.iter_mut() {
        if n == 0 {
            break;
        }
        if let Some(st) = s {
            if st.id == id {
                let mv = (st.n as u32).min(n);
                st.n -= mv as u8;
                n -= mv;
                if st.n == 0 {
                    *s = None;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stacking() {
        let mut inv: Vec<Slot> = vec![None; 2];
        assert!(add_to(&mut inv, Stack::new(DIRT as Id, 60)).is_none());
        assert!(add_to(&mut inv, Stack::new(DIRT as Id, 10)).is_none());
        assert_eq!(count(&inv, DIRT as Id), 70);
        let left = add_to(&mut inv, Stack::new(DIRT as Id, 64)).unwrap();
        assert_eq!(left.n, 6);
        remove(&mut inv, DIRT as Id, 100);
        assert_eq!(count(&inv, DIRT as Id), 28);
    }

    #[test]
    fn mining_rules() {
        assert!(break_time(BEDROCK, None).is_none());
        assert!(!can_harvest(DIAMOND_ORE, Some(tool_id(PICK, 1))));
        assert!(can_harvest(DIAMOND_ORE, Some(tool_id(PICK, 2))));
        let hand = break_time(STONE, None).unwrap();
        let pick = break_time(STONE, Some(tool_id(PICK, 0))).unwrap();
        assert!(pick < hand);
    }
    #[test]
    fn armor_protects_like_it_should() {
        // Chaque piece connait sa place et sa matiere.
        for tier in 0..ARMOR_TIERS {
            for slot in 0..4 {
                let id = armor_id(slot, tier);
                assert_eq!(armor(id), Some((slot, tier)));
                assert_eq!(wear_slot(id), Some(slot));
                assert_eq!(max_stack(id), 1);
                assert!(max_dur(id) > 0 && !name(id).is_empty());
            }
        }
        assert_eq!(wear_slot(HAGOROMO), Some(BODY));
        assert_eq!(wear_slot(STICK), None);
        let suit = |tier: u8| -> Vec<Slot> { (0..4).map(|s| Some(Stack::new(armor_id(s, tier), 1))).collect() };
        // Totaux : cuir 7, bambou 10, fer 15, or 11, diamant et tamahagane 20.
        let totals: Vec<f64> = (0..ARMOR_TIERS).map(|t| defense(&suit(t)).0).collect();
        assert_eq!(totals, [7.0, 10.0, 15.0, 11.0, 20.0, 20.0]);
        assert_eq!(defense(&suit(4)).1, 8.0);
        assert!((defense(&suit(5)).2 - 0.4).abs() < 1e-9);
        // Sans armure, rien n'est retire ; le fer arrete un peu plus de la moitie d'un coup faible.
        assert_eq!(mitigate(6.0, 0.0, 0.0), 6.0);
        assert!((mitigate(2.0, 15.0, 0.0) - 2.0 * (1.0 - 14.0 / 25.0)).abs() < 1e-9);
        // Un coup violent perce : l'armure de fer n'en retire plus que le plancher (3 points sur 15).
        assert!((mitigate(40.0, 15.0, 0.0) - 40.0 * (1.0 - 3.0 / 25.0)).abs() < 1e-9);
        // A points egaux, la robustesse du tamahagane protege mieux d'un gros coup que le diamant.
        assert!(mitigate(12.0, 20.0, 12.0) < mitigate(12.0, 20.0, 8.0));
        assert!(mitigate(12.0, 20.0, 8.0) < mitigate(12.0, 15.0, 0.0));
        // Jamais plus de 80 % d'arrete.
        assert!((mitigate(1.0, 30.0, 20.0) - 0.2).abs() < 1e-9);
        // Le plastron de diamant dure bien plus que le casque de cuir.
        assert_eq!(max_dur(armor_id(HEAD, 0)), 55);
        assert_eq!(max_dur(armor_id(BODY, 4)), 528);
        assert_eq!(armor_wear(3.0), 1);
        assert_eq!(armor_wear(9.0), 2);
    }

    #[test]
    fn tiers_and_weapons() {
        // Le cinquieme niveau prolonge les quatre premiers.
        let best = tool_id(PICK, 4);
        assert_eq!(tool(best), Some((PICK, 4)));
        assert!(max_dur(best) > max_dur(tool_id(PICK, 3)));
        assert!(break_time(STONE, Some(best)).unwrap() < break_time(STONE, Some(tool_id(PICK, 3))).unwrap());
        assert!(can_harvest(OBSIDIAN, Some(best)));
        // Chaque objet a sa propre icone.
        let mut seen = std::collections::HashSet::new();
        for id in (256..ITEM_END).chain(TOOL_BASE..TOOL_BASE + 20).chain(TOOL4_BASE..=SHIELD).chain(COOKED_RICE..MORE_END).chain(TAMA_SHARD..REALM_END).chain(ARMOR_BASE..ARMOR_END) {
            assert!(seen.insert(icon(id)), "icone partagee par l'objet {id}");
            // Les icones d'objets ont leurs plages reservees, a l'ecart des textures de blocs.
            let tile = icon(id);
            assert!((t::ITEM_TOOLS..128).contains(&tile) || (t::ITEM_MORE..t::CEDAR_LEAVES).contains(&tile) || (t::ITEM_REALM..t::ITEM_REALM + 12).contains(&tile) || t::ITEM_ARMOR.iter().any(|r| (*r..*r + 8).contains(&tile)), "l'objet {id} deborde sur la tuile {tile}");
        }
        // La hache frappe plus fort que le katana, mais bien plus lentement.
        let (axe, katana) = (weapon(Some(tool_id(AXE, 2))), weapon(Some(tool_id(SWORD, 2))));
        assert!(axe.dmg > katana.dmg && axe.speed < katana.speed);
        assert!(axe.dmg * axe.speed < katana.dmg * katana.speed, "a la longue, le katana fait plus de degats");
        // Frapper sans attendre ne fait qu'un cinquieme des degats.
        assert!((charge(0.0, 1.6) - 0.2).abs() < 1e-6);
        assert!((charge(1.0, 1.6) - 1.0).abs() < 1e-6);
        assert!((draw_power(1.0) - 1.0).abs() < 1e-6 && draw_power(0.2) < 0.2);
        assert_eq!(max_stack(KUNAI), 16);
        assert_eq!(max_stack(YUMI), 1);
    }
}
