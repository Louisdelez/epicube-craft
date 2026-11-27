//! Registre des blocs : identifiants, formes, proprietes et textures.
#![allow(dead_code)]

pub type B = u8;

pub const AIR: B = 0;
pub const STONE: B = 1;
pub const GRASS: B = 2;
pub const DIRT: B = 3;
pub const COBBLE: B = 4;
pub const PLANKS: B = 5;
pub const SAPLING: B = 6;
pub const BEDROCK: B = 7;
pub const WATER: B = 8;
pub const LAVA: B = 9;
pub const SAND: B = 10;
pub const GRAVEL: B = 11;
pub const GOLD_ORE: B = 12;
pub const IRON_ORE: B = 13;
pub const COAL_ORE: B = 14;
pub const LOG: B = 15;
pub const LEAVES: B = 16;
pub const GLASS: B = 17;
pub const DIAMOND_ORE: B = 18;
pub const VOLT_ORE: B = 19;
pub const SNOW: B = 20;
pub const ICE: B = 21;
pub const CRAFT: B = 22;
pub const FURNACE: B = 23;
pub const CHEST: B = 24;
pub const OBSIDIAN: B = 25;
pub const FARMLAND: B = 26;
pub const WHEAT: B = 27;
pub const CARROT: B = 28;
pub const TORCH: B = 29;
pub const WIRE: B = 30;
pub const LEVER: B = 31;
pub const SIGTORCH: B = 32;
pub const LAMP: B = 33;
pub const EXPLOSIVE: B = 34;
pub const BED: B = 35;
pub const WOOL: B = 36;
pub const ABYSS_ROCK: B = 37;
pub const EMBER: B = 38;
pub const ABYSS_PORTAL: B = 39;
pub const ZENITH_STONE: B = 40;
pub const ZENITH_PORTAL: B = 41;
pub const BRICK: B = 42;
pub const TALLGRASS: B = 43;
pub const FLOWER: B = 44;
pub const CACTUS: B = 45;
pub const IRON_BLOCK: B = 46;
pub const DOOR: B = 47;
/// Relais ofuda : retarde et regenere le signal.
pub const REPEATER: B = 48;
/// Balance onmyo : compare ou soustrait deux signaux, lit les conteneurs.
pub const COMPARATOR: B = 49;
/// Sceau hanko : bouton.
pub const BUTTON: B = 50;
/// Plancher rossignol : dalle sensible au poids.
pub const PLATE: B = 51;
/// Poussoir karakuri.
pub const PISTON: B = 52;
pub const STICKY_PISTON: B = 53;
/// Tete d'un poussoir deploye (jamais en objet).
pub const PISTON_HEAD: B = 54;
/// Daruma guetteur : signale tout changement du bloc qu'il regarde.
pub const OBSERVER: B = 55;
/// Bloc de reiseki : source permanente.
pub const VOLT_BLOCK: B = 56;
/// Cadran solaire.
pub const SUNDIAL: B = 57;
/// Distributeur karakuri.
pub const DROPPER: B = 58;
/// Tremie masu : fait passer les objets d'un conteneur a l'autre.
pub const HOPPER: B = 59;
/// Tambour taiko.
pub const NOTE: B = 60;
pub const RAIL: B = 61;
/// Rail de reiseki : lance le chariot quand il est alimente, le freine sinon.
pub const POWERED_RAIL: B = 62;
/// Rail guetteur : donne un signal quand un chariot passe.
pub const DETECTOR_RAIL: B = 63;
/// Corde shimenawa tendue entre deux piquets.
pub const TRIPWIRE: B = 64;
/// Piquet de shimenawa.
pub const HOOK: B = 65;
/// Cible de kyudo.
pub const TARGET: B = 66;
/// Plant de soja (4 stades).
pub const SOY: B = 67;
/// Theier : on cueille ses jeunes feuilles sans l'arracher (4 stades, le dernier porte la recolte).
pub const TEA: B = 68;
/// Tige de kabocha : mure, elle donne une courge sur une case voisine.
pub const STEM: B = 69;
pub const KABOCHA: B = 70;
pub const BAMBOO: B = 71;
/// Bac a compost : niveau 0 a 7, 8 = engrais pret.
pub const COMPOSTER: B = 72;
/// Gril shichirin : cuit les aliments deux fois plus vite que le fourneau.
pub const SMOKER: B = 73;
/// Foyer irori : cuit quatre aliments a la fois, sans combustible.
pub const IRORI: B = 74;
pub const CEDAR_LEAVES: B = 75;
/// Feuillage rouge des erables.
pub const MAPLE_LEAVES: B = 76;
pub const MOSS: B = 77;
/// Mousse lumineuse des grottes (hikarigoke).
pub const GLOW_MOSS: B = 78;
/// Concretion : stalagmite (0) ou stalactite pendue au plafond (1).
pub const STALACTITE: B = 79;
/// Ardoise : la roche sombre et dure des profondeurs.
pub const SLATE: B = 80;
/// Cristal de roche, au coeur des geodes.
pub const CRYSTAL: B = 81;
pub const BASALT: B = 82;
pub const COBWEB: B = 83;
/// Lotus pose sur l'eau.
pub const LOTUS: B = 84;
/// Stele maudite : tant qu'elle tient debout, des oni sortent de l'ombre autour d'elle.
pub const SPAWNER: B = 85;
/// Mousse pourpre du bois de higanbana (Yomi).
pub const CRIMSON_MOSS: B = 86;
/// Sable des ames : on s'y enfonce et l'on y marche au ralenti.
pub const SOUL_SAND: B = 87;
/// Onibi : flamme bleue des morts.
pub const ONIBI: B = 88;
pub const BONE_BLOCK: B = 89;
/// Sable de fer (satetsu), dont on tire le tamahagane.
pub const SATETSU: B = 90;
/// Feuillage de glycine celeste.
pub const WISTERIA: B = 91;
/// Porte celeste : relie l'ile du Ryujin a l'ile de la pagode.
pub const GATEWAY: B = 92;
/// Bois laque vermillon des torii et des sanctuaires.
pub const VERMILION: B = 93;

/// Indices des tuiles dans l'atlas (32 tuiles de 16 px par ligne).
pub mod t {
    pub const GRASS_TOP: u16 = 0;
    pub const GRASS_SIDE: u16 = 1;
    pub const DIRT: u16 = 2;
    pub const STONE: u16 = 3;
    pub const COBBLE: u16 = 4;
    pub const SAND: u16 = 5;
    pub const GRAVEL: u16 = 6;
    pub const LOG_SIDE: u16 = 7;
    pub const LOG_TOP: u16 = 8;
    pub const LEAVES: u16 = 9;
    pub const PLANKS: u16 = 10;
    pub const BEDROCK: u16 = 11;
    pub const WATER: u16 = 12;
    pub const LAVA: u16 = 13;
    pub const COAL_ORE: u16 = 14;
    pub const IRON_ORE: u16 = 15;
    pub const GOLD_ORE: u16 = 16;
    pub const DIAMOND_ORE: u16 = 17;
    pub const VOLT_ORE: u16 = 18;
    pub const GLASS: u16 = 19;
    pub const SNOW: u16 = 20;
    pub const ICE: u16 = 21;
    pub const CRAFT_TOP: u16 = 22;
    pub const CRAFT_SIDE: u16 = 23;
    pub const FURNACE_FRONT: u16 = 24;
    pub const FURNACE_SIDE: u16 = 25;
    pub const FURNACE_ON: u16 = 26;
    pub const CHEST: u16 = 27;
    pub const OBSIDIAN: u16 = 28;
    pub const FARMLAND: u16 = 29;
    pub const WHEAT0: u16 = 30; // 30..=33
    pub const CARROT0: u16 = 34; // 34..=37
    pub const SAPLING: u16 = 38;
    pub const TORCH: u16 = 39;
    pub const WIRE: u16 = 40;
    pub const LEVER: u16 = 41;
    pub const SIGTORCH_ON: u16 = 42;
    pub const SIGTORCH_OFF: u16 = 43;
    pub const LAMP_OFF: u16 = 44;
    pub const LAMP_ON: u16 = 45;
    pub const EXPLOSIVE: u16 = 46;
    pub const BED: u16 = 47;
    pub const WOOL: u16 = 48;
    pub const ABYSS_ROCK: u16 = 49;
    pub const EMBER: u16 = 50;
    pub const ABYSS_PORTAL: u16 = 51;
    pub const ZENITH_STONE: u16 = 52;
    pub const ZENITH_PORTAL: u16 = 53;
    pub const BRICK: u16 = 54;
    pub const TALLGRASS: u16 = 55;
    pub const FLOWER: u16 = 56;
    pub const CACTUS: u16 = 57;
    pub const IRON_BLOCK: u16 = 58;
    pub const DOOR: u16 = 59;
    pub const WHITE: u16 = 60;
    pub const SNOW_SIDE: u16 = 61;
    /// 20 outils : genre * 4 + niveau.
    pub const ITEM_TOOLS: u16 = 64;
    /// Autres objets : ITEM_MISC + (id - 256).
    pub const ITEM_MISC: u16 = 96;
    pub const HEART: u16 = 160;
    pub const HEART_EMPTY: u16 = 161;
    pub const FOOD: u16 = 162;
    pub const FOOD_EMPTY: u16 = 163;
    pub const BUBBLE: u16 = 164;
    pub const SUN: u16 = 165;
    pub const MOON: u16 = 166;
    /// Quatre stades de fissure pendant le minage.
    pub const CRACK0: u16 = 168;
    /// Disque blanc aux bords doux : coins arrondis de l'interface.
    pub const CIRCLE: u16 = 172;
    /// Boule de nuage, ombree par en dessous.
    pub const PUFF: u16 = 173;
    /// Motif de vagues seigaiha, fond des menus.
    pub const SEIGAIHA: u16 = 174;
    pub const WIRE_DOT: u16 = 128;
    /// Bras de fil, du centre vers la droite.
    pub const WIRE_ARM: u16 = 129;
    /// Fil qui grimpe le long d'un bloc.
    pub const WIRE_V: u16 = 130;
    /// Dessus du relais : 4 retards eteints, puis 4 allumes.
    pub const REPEATER0: u16 = 131;
    /// Dessus de la balance : eteinte, allumee, puis les memes en mode soustraction.
    pub const COMPARATOR0: u16 = 139;
    pub const BUTTON: u16 = 143;
    pub const PLATE: u16 = 144;
    pub const PISTON_FACE: u16 = 145;
    pub const STICKY_FACE: u16 = 146;
    pub const PISTON_SIDE: u16 = 147;
    pub const PISTON_BACK: u16 = 148;
    /// Avant du corps quand la tete est sortie.
    pub const PISTON_OPEN: u16 = 149;
    pub const PISTON_ARM: u16 = 150;
    pub const OBSERVER_FRONT: u16 = 151;
    pub const OBSERVER_SIDE: u16 = 152;
    pub const OBSERVER_BACK: u16 = 153;
    pub const OBSERVER_BACK_ON: u16 = 154;
    pub const VOLT_BLOCK: u16 = 155;
    pub const SUNDIAL_TOP: u16 = 156;
    pub const SUNDIAL_SIDE: u16 = 157;
    pub const DROPPER_FRONT: u16 = 158;
    pub const LEVER_ON: u16 = 159;
    pub const SUNDIAL_NIGHT: u16 = 175;
    pub const DROPPER_SIDE: u16 = 176;
    /// Tranche des plaques minces (relais, balance).
    pub const SLAB_SIDE: u16 = 177;
    pub const HOPPER_TOP: u16 = 178;
    pub const HOPPER_SIDE: u16 = 179;
    pub const NOTE: u16 = 180;
    /// Rail droit, dessine le long de l'axe X.
    pub const RAIL: u16 = 181;
    /// Virage qui relie +X et +Z.
    pub const RAIL_CURVE: u16 = 182;
    pub const POWERED_RAIL: u16 = 183;
    pub const POWERED_RAIL_ON: u16 = 184;
    pub const DETECTOR_RAIL: u16 = 185;
    pub const DETECTOR_RAIL_ON: u16 = 186;
    pub const ROPE: u16 = 187;
    pub const HOOK: u16 = 188;
    pub const TARGET: u16 = 189;
    pub const NOTE_TOP: u16 = 191;
    /// Objets de la cuisine et de la ferme : ITEM_MORE + (id - 320).
    pub const ITEM_MORE: u16 = 192;
    pub const SOY0: u16 = 224; // 224..=227
    pub const TEA0: u16 = 228; // 228..=231
    pub const STEM0: u16 = 232; // 232..=235
    pub const KABOCHA_SIDE: u16 = 236;
    pub const KABOCHA_TOP: u16 = 237;
    pub const BAMBOO: u16 = 238;
    pub const COMPOST_SIDE: u16 = 240;
    pub const COMPOST_TOP: u16 = 241;
    pub const COMPOST_FULL: u16 = 242;
    pub const SMOKER_FRONT: u16 = 243;
    pub const SMOKER_ON: u16 = 244;
    pub const SMOKER_SIDE: u16 = 245;
    pub const IRORI_TOP: u16 = 246;
    pub const IRORI_ON: u16 = 247;
    pub const IRORI_SIDE: u16 = 248;
    pub const SLATE: u16 = 62;
    pub const BASALT: u16 = 63;
    pub const COBWEB: u16 = 190;
    pub const CEDAR_LEAVES: u16 = 217;
    pub const MAPLE_LEAVES: u16 = 218;
    pub const MOSS: u16 = 219;
    pub const GLOW_MOSS: u16 = 220;
    pub const STALACTITE: u16 = 221;
    pub const STALAGMITE: u16 = 222;
    pub const CRYSTAL: u16 = 223;
    pub const LOTUS: u16 = 249;
    pub const SPAWNER: u16 = 250;
    /// Minerais pris dans l'ardoise : charbon, fer, or, diamant, reiseki.
    pub const SLATE_ORE0: u16 = 251;
    // Tuiles logees a droite des modeles (lignes 12 et 13 de l'atlas).
    pub const CRIMSON_MOSS: u16 = 1011;
    pub const SOUL_SAND: u16 = 1012;
    pub const ONIBI: u16 = 1013;
    pub const BONE_BLOCK: u16 = 1014;
    pub const SATETSU: u16 = 408;
    pub const ABYSS_GOLD: u16 = 409;
    pub const WISTERIA: u16 = 410;
    /// Voile du torii allume.
    pub const VEIL: u16 = 411;
    pub const GATEWAY: u16 = 412;
    pub const VERMILION: u16 = 413;
    /// Objets des autres mondes : ITEM_REALM + (id - 372).
    pub const ITEM_REALM: u16 = 436;
    /// Pieces d'armure : trois rangees de huit tuiles, a droite des modeles de creatures.
    pub const ITEM_ARMOR: [u16; 3] = [440, 472, 504];
    /// Barre d'armure de l'interface : plastron plein, plastron vide.
    pub const ARMOR: u16 = 1009;
    pub const ARMOR_EMPTY: u16 = 1010;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    None,
    Cube,
    Cross,
    Flat,
    Fluid,
    /// Petit volume pose ou accroche, qu'on traverse : relais, balance, sceau, dalle.
    Thin,
}

pub fn shape(b: B) -> Shape {
    match b {
        AIR => Shape::None,
        WATER | LAVA => Shape::Fluid,
        SAPLING | WHEAT | CARROT | TORCH | LEVER | SIGTORCH | TALLGRASS | FLOWER | SOY | TEA | STEM | BAMBOO | STALACTITE | COBWEB | ONIBI => Shape::Cross,
        WIRE | RAIL | POWERED_RAIL | DETECTOR_RAIL | TRIPWIRE | LOTUS => Shape::Flat,
        REPEATER | COMPARATOR | BUTTON | PLATE | HOOK => Shape::Thin,
        _ => Shape::Cube,
    }
}

/// Blocs qui participent aux circuits.
pub fn is_signal(b: B) -> bool {
    matches!(
        b,
        WIRE | LEVER | SIGTORCH | LAMP | DOOR | EXPLOSIVE | REPEATER | COMPARATOR | BUTTON | PLATE | PISTON | STICKY_PISTON | OBSERVER | VOLT_BLOCK | SUNDIAL | DROPPER | HOPPER | NOTE | POWERED_RAIL | DETECTOR_RAIL | TRIPWIRE | HOOK | TARGET | SPAWNER
    )
}

/// Le bloc arrete-t-il les entites ? (`m` = metadonnee, ex. porte ouverte)
pub fn solid(b: B, m: u8) -> bool {
    // Le voile d'un torii allume (bit 1) se traverse.
    shape(b) == Shape::Cube && !(b == DOOR && m & 1 == 1) && !(b == ABYSS_PORTAL && m & 2 != 0)
}

/// Cube plein qui cache les faces voisines et bloque la lumiere.
pub fn opaque(b: B) -> bool {
    shape(b) == Shape::Cube && !matches!(b, LEAVES | CEDAR_LEAVES | MAPLE_LEAVES | WISTERIA | GLASS | BED | DOOR | PISTON_HEAD | SUNDIAL | ABYSS_PORTAL)
}

/// Attenuation supplementaire de la lumiere (15 = bloque tout).
pub fn opacity(b: B) -> u8 {
    match b {
        WATER => 2,
        LEAVES | CEDAR_LEAVES | MAPLE_LEAVES | WISTERIA => 1,
        LAVA => 15,
        _ if opaque(b) => 15,
        _ => 0,
    }
}

/// Lumiere emise par le bloc (0..=15).
pub fn emit(b: B, m: u8) -> u8 {
    match b {
        LAVA | EMBER => 15,
        TORCH => 14,
        ABYSS_PORTAL | ZENITH_PORTAL => 11,
        FURNACE | SMOKER if m & 1 == 1 => 13,
        IRORI => 13,
        CRYSTAL | ONIBI | GATEWAY => 10,
        GLOW_MOSS => 9,
        LAMP if m & 1 == 1 => 15,
        SIGTORCH if m & 1 == 1 => 7,
        REPEATER if m & 16 != 0 => 5,
        COMPARATOR if m >> 4 != 0 => 5,
        VOLT_BLOCK => 4,
        // Explosif amorce.
        EXPLOSIVE if m & 1 == 1 => 12,
        _ => 0,
    }
}

/// Tuile de l'atlas pour une face. Faces : 0 +X, 1 -X, 2 +Y, 3 -Y, 4 +Z, 5 -Z.
pub fn tile(b: B, m: u8, f: usize) -> u16 {
    match b {
        STONE => t::STONE,
        GRASS => match f {
            2 => t::GRASS_TOP,
            3 => t::DIRT,
            _ => t::GRASS_SIDE,
        },
        DIRT => t::DIRT,
        COBBLE => t::COBBLE,
        PLANKS => t::PLANKS,
        SAPLING => t::SAPLING,
        BEDROCK => t::BEDROCK,
        WATER => t::WATER,
        LAVA => t::LAVA,
        SAND => t::SAND,
        GRAVEL => t::GRAVEL,
        GOLD_ORE => {
            if m & 2 != 0 {
                t::ABYSS_GOLD
            } else if m & 1 == 1 {
                t::SLATE_ORE0 + 2
            } else {
                t::GOLD_ORE
            }
        }
        IRON_ORE => {
            if m & 1 == 1 {
                t::SLATE_ORE0 + 1
            } else {
                t::IRON_ORE
            }
        }
        COAL_ORE => {
            if m & 1 == 1 {
                t::SLATE_ORE0 + 0
            } else {
                t::COAL_ORE
            }
        }
        LOG => {
            if f == 2 || f == 3 {
                t::LOG_TOP
            } else {
                t::LOG_SIDE
            }
        }
        LEAVES => t::LEAVES,
        GLASS => t::GLASS,
        DIAMOND_ORE => {
            if m & 1 == 1 {
                t::SLATE_ORE0 + 3
            } else {
                t::DIAMOND_ORE
            }
        }
        VOLT_ORE => {
            if m & 1 == 1 {
                t::SLATE_ORE0 + 4
            } else {
                t::VOLT_ORE
            }
        }
        SNOW => match f {
            2 => t::SNOW,
            3 => t::DIRT,
            _ => t::SNOW_SIDE,
        },
        ICE => t::ICE,
        CRAFT => match f {
            2 => t::CRAFT_TOP,
            3 => t::PLANKS,
            _ => t::CRAFT_SIDE,
        },
        FURNACE => {
            if f == 4 {
                if m & 1 == 1 {
                    t::FURNACE_ON
                } else {
                    t::FURNACE_FRONT
                }
            } else {
                t::FURNACE_SIDE
            }
        }
        CHEST => t::CHEST,
        OBSIDIAN => t::OBSIDIAN,
        FARMLAND => {
            if f == 2 {
                t::FARMLAND
            } else {
                t::DIRT
            }
        }
        WHEAT => t::WHEAT0 + m.min(3) as u16,
        CARROT => t::CARROT0 + m.min(3) as u16,
        SOY => t::SOY0 + m.min(3) as u16,
        TEA => t::TEA0 + m.min(3) as u16,
        STEM => t::STEM0 + m.min(3) as u16,
        KABOCHA => {
            if f == 2 || f == 3 {
                t::KABOCHA_TOP
            } else {
                t::KABOCHA_SIDE
            }
        }
        BAMBOO => t::BAMBOO,
        CEDAR_LEAVES => t::CEDAR_LEAVES,
        MAPLE_LEAVES => t::MAPLE_LEAVES,
        MOSS => t::MOSS,
        GLOW_MOSS => t::GLOW_MOSS,
        STALACTITE => {
            if m & 1 == 1 {
                t::STALACTITE
            } else {
                t::STALAGMITE
            }
        }
        SLATE => t::SLATE,
        CRYSTAL => t::CRYSTAL,
        BASALT => t::BASALT,
        COBWEB => t::COBWEB,
        LOTUS => t::LOTUS,
        SPAWNER => t::SPAWNER,
        CRIMSON_MOSS => {
            if f == 2 {
                t::CRIMSON_MOSS
            } else {
                t::ABYSS_ROCK
            }
        }
        SOUL_SAND => t::SOUL_SAND,
        ONIBI => t::ONIBI,
        BONE_BLOCK => t::BONE_BLOCK,
        SATETSU => t::SATETSU,
        WISTERIA => t::WISTERIA,
        GATEWAY => t::GATEWAY,
        VERMILION => t::VERMILION,
        COMPOSTER => {
            if f != 2 {
                t::COMPOST_SIDE
            } else if m >= 8 {
                t::COMPOST_FULL
            } else if m > 0 {
                t::COMPOST_TOP
            } else {
                t::HOPPER_TOP
            }
        }
        SMOKER => {
            if f != 4 {
                t::SMOKER_SIDE
            } else if m & 1 == 1 {
                t::SMOKER_ON
            } else {
                t::SMOKER_FRONT
            }
        }
        IRORI => {
            if f != 2 {
                t::IRORI_SIDE
            } else if m & 1 == 1 {
                t::IRORI_ON
            } else {
                t::IRORI_TOP
            }
        }
        TORCH => t::TORCH,
        WIRE => t::WIRE,
        LEVER => {
            if m & 1 == 1 {
                t::LEVER_ON
            } else {
                t::LEVER
            }
        }
        SIGTORCH => {
            if m & 1 == 1 {
                t::SIGTORCH_ON
            } else {
                t::SIGTORCH_OFF
            }
        }
        LAMP => {
            if m & 1 == 1 {
                t::LAMP_ON
            } else {
                t::LAMP_OFF
            }
        }
        EXPLOSIVE => t::EXPLOSIVE,
        BED => t::BED,
        WOOL => t::WOOL,
        ABYSS_ROCK => t::ABYSS_ROCK,
        EMBER => t::EMBER,
        ABYSS_PORTAL => {
            if m & 2 != 0 {
                t::VEIL
            } else {
                t::ABYSS_PORTAL
            }
        }
        ZENITH_STONE => t::ZENITH_STONE,
        ZENITH_PORTAL => t::ZENITH_PORTAL,
        BRICK => t::BRICK,
        TALLGRASS => t::TALLGRASS,
        FLOWER => t::FLOWER,
        CACTUS => t::CACTUS,
        IRON_BLOCK => t::IRON_BLOCK,
        DOOR => t::DOOR,
        REPEATER => {
            if f == 2 {
                t::REPEATER0 + ((m >> 2) & 3) as u16 + if m & 16 != 0 { 4 } else { 0 }
            } else {
                t::SLAB_SIDE
            }
        }
        COMPARATOR => {
            if f == 2 {
                t::COMPARATOR0 + (m >> 4 != 0) as u16 + if m & 4 != 0 { 2 } else { 0 }
            } else {
                t::SLAB_SIDE
            }
        }
        BUTTON => t::BUTTON,
        HOPPER => {
            if f == 2 {
                t::HOPPER_TOP
            } else {
                t::HOPPER_SIDE
            }
        }
        NOTE => {
            if f == 2 || f == 3 {
                t::NOTE_TOP
            } else {
                t::NOTE
            }
        }
        RAIL => {
            if m & 7 >= 2 {
                t::RAIL_CURVE
            } else {
                t::RAIL
            }
        }
        POWERED_RAIL => {
            if m & 8 != 0 {
                t::POWERED_RAIL_ON
            } else {
                t::POWERED_RAIL
            }
        }
        DETECTOR_RAIL => {
            if m & 8 != 0 {
                t::DETECTOR_RAIL_ON
            } else {
                t::DETECTOR_RAIL
            }
        }
        TRIPWIRE => t::ROPE,
        HOOK => t::HOOK,
        TARGET => t::TARGET,
        PLATE => t::PLATE,
        PISTON | STICKY_PISTON => {
            let front = (m & 7) as usize;
            if f == front {
                if m & 8 != 0 {
                    t::PISTON_OPEN
                } else if b == STICKY_PISTON {
                    t::STICKY_FACE
                } else {
                    t::PISTON_FACE
                }
            } else if f == front ^ 1 {
                t::PISTON_BACK
            } else {
                t::PISTON_SIDE
            }
        }
        PISTON_HEAD => {
            if m & 8 != 0 {
                t::STICKY_FACE
            } else {
                t::PISTON_FACE
            }
        }
        OBSERVER => {
            let front = (m & 7) as usize;
            if f == front {
                t::OBSERVER_FRONT
            } else if f == front ^ 1 {
                if m & 8 != 0 {
                    t::OBSERVER_BACK_ON
                } else {
                    t::OBSERVER_BACK
                }
            } else {
                t::OBSERVER_SIDE
            }
        }
        VOLT_BLOCK => t::VOLT_BLOCK,
        SUNDIAL => {
            if f == 2 {
                if m & 16 != 0 {
                    t::SUNDIAL_NIGHT
                } else {
                    t::SUNDIAL_TOP
                }
            } else {
                t::SUNDIAL_SIDE
            }
        }
        DROPPER => {
            if f == (m & 7) as usize {
                t::DROPPER_FRONT
            } else {
                t::DROPPER_SIDE
            }
        }
        _ => t::WHITE,
    }
}
