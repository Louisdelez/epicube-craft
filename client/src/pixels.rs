//! Pixel art du jeu, dessine a la main, d'inspiration japonaise : blocs, objets, icones.
//! Chaque caractere est un pixel ; la palette donne sa couleur ; '.' est transparent.
//! Les blocs sont dessines en 16 x 16 puis affines en 32 x 32 : les diagonales sont adoucies
//! et de petits motifs (brins, cailloux, fibres) sont poses au pixel pres.
//! Les motifs se continuent d'un bloc a l'autre, sans raccord visible.
use crate::atlas::{Img, TILE};
use shared::blocks::t;
use shared::items;
use shared::util::hash;

type Map = [&'static str; 16];
type Pal = [(char, u32)];
/// Cote d'une texture de bloc, en pixels.
const N: usize = 32;

fn color(pal: &Pal, ch: char) -> u32 {
    match pal.iter().find(|(c, _)| *c == ch) {
        Some((_, rgb)) => 0xFF00_0000 | rgb,
        None => panic!("couleur {ch:?} absente de la palette"),
    }
}

/// Image de 32 x 32 pixels en cours de dessin ; 0 = transparent.
#[derive(Clone)]
struct Canvas {
    px: Vec<u32>,
    /// La texture se repete : ce qui sort d'un bord rentre par l'autre.
    wrap: bool,
    /// Couleurs que les motifs ajoutes ne recouvrent jamais (joints, cadres).
    locked: Vec<u32>,
}

impl Canvas {
    /// Lit un dessin de 8, 16 ou 32 pixels de cote. Avec `smooth`, un dessin 16 x 16 est agrandi
    /// en adoucissant ses diagonales (un coin n'est arrondi que si deux voisins sont identiques).
    fn from_map<S: AsRef<str>>(map: &[S], pal: &Pal, wrap: bool, smooth: bool) -> Canvas {
        let n = map.len();
        assert!(n == 8 || n == 16 || n == 32, "dessin de {n} lignes : 8, 16 ou 32 attendues");
        let mut src = vec![0u32; n * n];
        for (y, row) in map.iter().enumerate() {
            let row = row.as_ref();
            assert_eq!(row.chars().count(), n, "ligne {y} : {n} pixels attendus dans {row:?}");
            for (x, ch) in row.chars().enumerate() {
                if ch != '.' {
                    src[y * n + x] = color(pal, ch);
                }
            }
        }
        let at = |x: i32, y: i32| -> u32 {
            let m = n as i32;
            if wrap {
                src[(y.rem_euclid(m) * m + x.rem_euclid(m)) as usize]
            } else if x < 0 || y < 0 || x >= m || y >= m {
                0
            } else {
                src[(y * m + x) as usize]
            }
        };
        let f = N / n;
        let mut px = vec![0u32; N * N];
        for y in 0..n as i32 {
            for x in 0..n as i32 {
                let c = at(x, y);
                let mut quad = [c; 4];
                if smooth && f == 2 {
                    let (up, right, left, down) = (at(x, y - 1), at(x + 1, y), at(x - 1, y), at(x, y + 1));
                    if left == up && left != down && up != right {
                        quad[0] = up;
                    }
                    if up == right && up != left && right != down {
                        quad[1] = right;
                    }
                    if down == left && down != right && left != up {
                        quad[2] = left;
                    }
                    if right == down && right != up && down != left {
                        quad[3] = down;
                    }
                }
                for dy in 0..f {
                    for dx in 0..f {
                        let q = if f == 2 { quad[dy * 2 + dx] } else { c };
                        px[(y as usize * f + dy) * N + x as usize * f + dx] = q;
                    }
                }
            }
        }
        Canvas { px, wrap, locked: Vec::new() }
    }

    /// Texture de bloc qui se repete.
    fn tiled(map: &Map, pal: &Pal) -> Canvas {
        Canvas::from_map(map, pal, true, true)
    }

    /// Dessin sur fond transparent (plante, objet pose, incrustation).
    fn cutout<S: AsRef<str>>(map: &[S], pal: &Pal) -> Canvas {
        Canvas::from_map(map, pal, false, true)
    }

    /// Objet : dessine en 16 x 16 puis affine en 32 x 32, les diagonales adoucies. C'est ce dessin
    /// qui sert d'icone, et auquel on donne de l'epaisseur pour le tenir en main.
    fn item<S: AsRef<str>>(map: &[S], pal: &Pal) -> Canvas {
        Canvas::from_map(map, pal, false, true)
    }

    /// Icone : agrandie telle quelle, chaque pixel reste carre.
    fn icon<S: AsRef<str>>(map: &[S], pal: &Pal) -> Canvas {
        Canvas::from_map(map, pal, false, false)
    }

    fn set(&mut self, x: i32, y: i32, c: u32) {
        let m = N as i32;
        let (x, y) = if self.wrap {
            (x.rem_euclid(m), y.rem_euclid(m))
        } else if x < 0 || y < 0 || x >= m || y >= m {
            return;
        } else {
            (x, y)
        };
        let i = (y * m + x) as usize;
        if !self.locked.contains(&self.px[i]) {
            self.px[i] = c;
        }
    }

    /// Protege certaines couleurs de la palette contre les motifs ajoutes ensuite.
    fn lock(mut self, pal: &Pal, chars: &str) -> Canvas {
        self.locked = chars.chars().map(|ch| color(pal, ch)).collect();
        self
    }

    /// Pose un autre dessin par-dessus (ses pixels transparents laissent voir celui-ci).
    fn over(mut self, top: &Canvas) -> Canvas {
        for (dst, src) in self.px.iter_mut().zip(&top.px) {
            if *src != 0 {
                *dst = *src;
            }
        }
        self
    }

    /// Seme `count` exemplaires d'un petit motif.
    fn sprinkle(mut self, stamp: &[&str], pal: &Pal, count: usize, seed: u32) -> Canvas {
        for i in 0..count {
            let (x, y) = ((hash(i as i32, 0, 0, seed) % N as u32) as i32, (hash(i as i32, 1, 0, seed) % N as u32) as i32);
            for (dy, row) in stamp.iter().enumerate() {
                for (dx, ch) in row.chars().enumerate() {
                    if ch != '.' {
                        self.set(x + dx as i32, y + dy as i32, color(pal, ch));
                    }
                }
            }
        }
        self
    }

    /// Seme des traits d'un pixel de large (fibres du bois, veines), verticaux ou horizontaux.
    fn fibers(mut self, c: u32, count: usize, min: u32, max: u32, vertical: bool, seed: u32) -> Canvas {
        for i in 0..count {
            let h = |k: i32| hash(i as i32, k, 0, seed);
            let (x, y) = ((h(0) % N as u32) as i32, (h(1) % N as u32) as i32);
            let len = (min + h(2) % (max - min + 1)) as i32;
            for k in 0..len {
                if vertical {
                    self.set(x, y + k, c);
                } else {
                    self.set(x + k, y, c);
                }
            }
        }
        self
    }

    /// Copie le dessin dans une tuile de l'atlas.
    fn blit(&self, img: &mut Img, tile: u16, alpha: u8) {
        let k = TILE / N;
        let (ox, oy) = ((tile as usize % 32) * TILE, (tile as usize / 32) * TILE);
        for y in 0..N {
            for x in 0..N {
                let c = self.px[y * N + x];
                let rgba = if c == 0 { [0; 4] } else { [(c >> 16) as u8, (c >> 8) as u8, c as u8, alpha] };
                for dy in 0..k {
                    for dx in 0..k {
                        img.set_px(ox + x * k + dx, oy + y * k + dy, rgba);
                    }
                }
            }
        }
    }
}

/// Dessin decale vers le bas de `dy` lignes : sert aux plantes qui poussent.
fn lowered(map: &Map, dy: usize) -> Vec<String> {
    (0..16).map(|y| if y < dy { ".".repeat(16) } else { map[y - dy].to_string() }).collect()
}

/// Vagues seigaiha : eventails de cercles concentriques qui se chevauchent. Fond des menus.
fn seigaiha() -> Canvas {
    let mut px = vec![0u32; N * N];
    for y in 0..N as i32 {
        for x in 0..N as i32 {
            let mut c = 0xFF1C_2647u32;
            // Les eventails du bas recouvrent ceux du haut.
            'fans: for cy in [48, 32, 16, 0] {
                let xs: &[i32] = if (cy / 16) % 2 == 1 { &[0, 32] } else { &[16, -16, 48] };
                for &cx in xs {
                    let d2 = (x - cx).pow(2) + (y - cy).pow(2);
                    if d2 <= 256 && y <= cy {
                        let d = (d2 as f32).sqrt() as i32;
                        c = if matches!(d, 14..=16 | 9..=10 | 4..=5) { 0xFF3D_4E86 } else { 0xFF23_2F55 };
                        break 'fans;
                    }
                }
            }
            px[(y * N as i32 + x) as usize] = c;
        }
    }
    Canvas { px, wrap: true, locked: Vec::new() }
}

// ---------------------------------------------------------------- sols et roches

const GRASS: Map = [
    "bbbcbbbbabbbbcbb",
    "bbbabbcbbbbbbabb",
    "bcbbbbabbbcbbbbb",
    "babbbbbbbbabbbcb",
    "bbbbcbbbbbbbbbab",
    "bbbbabbbcbbbdbbb",
    "cbbbbbbbabbbbbbb",
    "abbbdbbbbbbcbbbb",
    "bbbbbbbcbbbabbbb",
    "bbcbbbbabbbbbbcb",
    "bbabbbbbbbdbbbab",
    "bbbbbcbbbbbbbbbb",
    "bbbbbabbbcbbbbbb",
    "dbbbbbbbbabbcbbb",
    "bbbcbbbbbbbbabbb",
    "bbbabbbdbbbbbbbb",
];
const GRASS_PAL: &Pal = &[('a', 0x4A8238), ('b', 0x5F9C45), ('c', 0x79B655), ('d', 0x95CB6A)];

const DIRT: Map = [
    "bbbbcbbbbbabbbbb",
    "bcbbbbbbbbbbbcbb",
    "bbbbbabbbdbbbbbb",
    "bbbbbbbbbbbbbbab",
    "abbcbbbbbbbbbbbb",
    "bbbbbbbbabbcbbbb",
    "bbbbbddbbbbbbbbb",
    "bbbbbdabbbbbbcbb",
    "bcbbbbbbbbbbbbbb",
    "bbbbbbbbbcbbabbb",
    "bbabbbbbbbbbbbbb",
    "bbbbbcbbbbbbbbdb",
    "bbbbbbbbbabbbbab",
    "cbbbbbbbbbbbbbbb",
    "bbbabbbbcbbbbbbb",
    "bbbbbbbbbbbbabbc",
];
const DIRT_PAL: &Pal = &[('a', 0x5E4030), ('b', 0x7A5540), ('c', 0x8E6A4E), ('d', 0xA88462)];

/// Bord d'herbe (ou de neige) qui retombe sur le cote d'un bloc de terre.
const CAP: Map = [
    "gggggggggggggggg",
    "hghgggghgghggghg",
    "gggdggggdgggggdg",
    "gdg.gdgg.gdgdg.g",
    ".g...g.d...g...d",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
];
const CAP_GRASS: &Pal = &[('g', 0x5F9C45), ('h', 0x79B655), ('d', 0x4A8238)];
const CAP_SNOW: &Pal = &[('g', 0xEEF3FA), ('h', 0xFFFFFF), ('d', 0xD3DEEE)];

/// Pierre de jardin : fissures et taches de mousse. Sert aussi, recoloree, aux autres roches.
const STONE: Map = [
    "bbbbbbcccbbbbbbb",
    "bbccbbbbbbbbabbb",
    "bbbbbbbbbbbaabbb",
    "ddbbbbbbbccbbbbd",
    "bbddbbbbbbbbbbdb",
    "bbbbbbabbbbbbbbb",
    "bcccbbaabbbbcccb",
    "bbbbbbbbbbbbbbbb",
    "bbbbbbbbbddbbbbb",
    "abbbbbbbdbbdbbba",
    "aabbcccbbbbbbbba",
    "bbbbbbbbbbbbmbbb",
    "bbbbbbbbbbbmmbbb",
    "bbbddbbbbbbbbbbb",
    "bbdbbbbbccbbbbbb",
    "bbbbbbbbbbbbbbbb",
];
const STONE_PAL: &Pal = &[('a', 0x6B727E), ('b', 0x858C98), ('c', 0x9BA2AD), ('d', 0x565C67), ('m', 0x6E8F4A)];
const BEDROCK_PAL: &Pal = &[('a', 0x2A2C33), ('b', 0x3A3D46), ('c', 0x4C505B), ('d', 0x1C1D22), ('m', 0x33363E)];
const OBSIDIAN_PAL: &Pal = &[('a', 0x1E1830), ('b', 0x2C2444), ('c', 0x40355F), ('d', 0x130F20), ('m', 0x6A58A0)];
const ABYSS_PAL: &Pal = &[('a', 0x6A2A2C), ('b', 0x873A3A), ('c', 0xA24E48), ('d', 0x4C1C20), ('m', 0xD9763C)];
const ZENITH_PAL: &Pal = &[('a', 0xBFB79A), ('b', 0xD8D1B4), ('c', 0xECE7CF), ('d', 0xA39B80), ('m', 0xB9A7DA)];

/// Muraille de chateau (ishigaki) : grosses pierres ajustees, joints sombres.
const WALL: Map = [
    "ccccccjcccccccbj",
    "cbbbbbjcbbbbbbbj",
    "bbbbbbjbbbbbbbbj",
    "bbbbbajbbbbbbbaj",
    "baaaaajbaaaaaaaj",
    "jjjjjjjjjjjjjjjj",
    "cjcccccccjcccccc",
    "bjcbbbbbbjcbbbbb",
    "bjbbbbbbajbbbbbb",
    "ajbaaaaaajbaaaaa",
    "jjjjjjjjjjjjjjjj",
    "cccbjccccccbjccc",
    "bbbbjcbbbbbbjcbb",
    "bbbajbbbbbbajbbb",
    "aaaajbaaaaaajbaa",
    "jjjjjjjjjjjjjjjj",
];
const WALL_PAL: &Pal = &[('j', 0x4A4E58), ('a', 0x6F7682), ('b', 0x8A919C), ('c', 0xA3A9B3)];
/// Torchis du fourneau kamado.
const CLAY_PAL: &Pal = &[('j', 0x5E4E40), ('a', 0x7D6A58), ('b', 0x978370), ('c', 0xAE9A86)];

/// Sable ratisse d'un jardin sec : lignes ondulees.
const SAND: Map = [
    "bbbbbbbbbbbbbbbb",
    "ccccbbbbbbbbcccc",
    "bbbbccccccccbbbb",
    "aaaabbbbbbbbaaaa",
    "bbbbaaaaaaaabbbb",
    "bbbbbbbbbbbbbbbb",
    "bbcbbbbbbbbbbbbb",
    "bbbbbbbbbbcbbbbb",
    "bbbbbbbbbbbbbbbb",
    "ccccbbbbbbbbcccc",
    "bbbbccccccccbbbb",
    "aaaabbbbbbbbaaaa",
    "bbbbaaaaaaaabbbb",
    "bbbbbbbbbbbbbbbb",
    "bbbbbbbbcbbbbbbb",
    "bbbbcbbbbbbbbbbb",
];
const SAND_PAL: &Pal = &[('a', 0xCDBD90), ('b', 0xE3D5A6), ('c', 0xF0E6C0)];

const GRAVEL: Map = [
    "jabjjbbjjacjjabj",
    "abbjbccjabbjabbj",
    "jbjjbbjjjbjjjbjj",
    "jjacjjjabjjacjja",
    "bjbbjabjbbjbbbjb",
    "jjjjjbbjjjjjbjjj",
    "acjjabjjacjjjjab",
    "bbjabbjjbbjacjbb",
    "jjjjbjjabjjbbjjj",
    "jabjjjabbjjjjabj",
    "abbjacjbjjabjbbj",
    "jbjjbbjjjabbjjjj",
    "jjabjjjacjbjjacj",
    "cjbbjabjbbjjabbj",
    "bjjjjbbjjjjjjbjj",
    "jjacjjjjabjacjja",
];
const GRAVEL_PAL: &Pal = &[('j', 0x55585F), ('a', 0x74787F), ('b', 0x8F939A), ('c', 0xAEB1B6)];

const SNOW: Map = [
    "bbbcbbbbbbbbcbbb",
    "bbccbbbbabbbbbbb",
    "bbbbbbbbbbbbbbbb",
    "bbbbbbbbbbbccbbb",
    "abbbbcbbbbbbcbbb",
    "bbbbbbbbbbbbbbba",
    "bbbbbbbbabbbbbbb",
    "bccbbbbbbbbbbbbb",
    "bbcbbbbbbbbcbbbb",
    "bbbbbbabbbbbbbbb",
    "bbbbbbbbbbbbbbbb",
    "bbbbbccbbbbbbabb",
    "cbbbbbcbbbbbbbbb",
    "bbbbbbbbbbcbbbbb",
    "bbbabbbbbbbbbbbc",
    "bbbbbbbbbcbbbbbb",
];
const SNOW_PAL: &Pal = &[('a', 0xD3DEEE), ('b', 0xEEF3FA), ('c', 0xFFFFFF)];

const ICE: Map = [
    "bbbbbbbcbbbbbbbb",
    "bbbbbbcbbbbbbbba",
    "bbbbbcbbbbbbbbab",
    "bbbbcbbbbbbbbabb",
    "bbbbbbbbbbbbabbb",
    "bbbbbbbbbbbbbbbb",
    "cbbbbbbbbbbbbbbb",
    "bbbbbbbbbbbcbbbb",
    "bbbbbbbbbbcbbbbb",
    "bbbabbbbbcbbbbbb",
    "bbabbbbbcbbbbbbb",
    "babbbbbbbbbbbbbc",
    "abbbbbbbbbbbbbcb",
    "bbbbbbbbbbbbbcbb",
    "bbbbbbbbbbbbbbbb",
    "bbbbbbbbcbbbbbbb",
];
const ICE_PAL: &Pal = &[('a', 0x8FB8E0), ('b', 0xA9CDEE), ('c', 0xD4E8FA)];

const FARMLAND: Map = [
    "abcbabcbabcbabcb",
    "abcbabcbabcbabcb",
    "abcbabbbabcbabcb",
    "abcbabcbabcbabcb",
    "abcbabcbabcbacbb",
    "abbbabcbabcbabcb",
    "abcbabcbabcbabcb",
    "abcbabcbabbbabcb",
    "abcbabcbabcbabcb",
    "abcbacbbabcbabcb",
    "abcbabcbabcbabcb",
    "abcbabcbabcbabbb",
    "abcbabcbabcbabcb",
    "abbbabcbabcbabcb",
    "abcbabcbacbbabcb",
    "abcbabcbabcbabcb",
];
const FARMLAND_PAL: &Pal = &[('a', 0x3E2A1E), ('b', 0x553A2A), ('c', 0x6B4A36)];

// ---------------------------------------------------------------- eau, lave, minerais

const WATER: Map = [
    "bbbbbbbbbbbbbbbb",
    "bbccbbbbbbbbbbbb",
    "bcddcbbbbbbccbbb",
    "bbbbbbbbbbcddcbb",
    "abbbbbbbbbbbbbba",
    "aabbbbbbaabbbbbb",
    "bbbbbbbaaaabbbbb",
    "bbbbbbbbbbbbbbbb",
    "bbbbbbbccbbbbbbb",
    "bbbbbbcddcbbbbbb",
    "ccbbbbbbbbbbbbbc",
    "dcbbbbbbbbbbbbcd",
    "bbbbbbbbbbbbbbbb",
    "bbbaabbbbbbbaabb",
    "bbaaaabbbbbaaaab",
    "bbbbbbbbbbbbbbbb",
];
const WATER_PAL: &Pal = &[('a', 0x2F6FC0), ('b', 0x3F86D6), ('c', 0x6FAEEA), ('d', 0xA8D4F6)];

const LAVA: Map = [
    "bbbbccbbbbbbbbbb",
    "bbbcddcbbbbabbbb",
    "bbbbccbbbbaaabbb",
    "bbbbbbbbbbbabbbb",
    "abbbbbbbbbbbbbba",
    "aabbbbccbbbbbbaa",
    "abbbbcddcbbbbbba",
    "bbbbbcdddcbbbbbb",
    "bbbbbbccccbbbbbb",
    "bbabbbbbbbbbbbbb",
    "baaabbbbbbbbccbb",
    "bbabbbbbbbbcddcb",
    "bbbbbbbbbbbbccbb",
    "bbbbbbabbbbbbbbb",
    "bbbbbaaabbbbbbbb",
    "bbbbbbabbbbbbbbb",
];
const LAVA_PAL: &Pal = &[('a', 0xB8320E), ('b', 0xE4601A), ('c', 0xF79A2A), ('d', 0xFFD95A)];
const EMBER_PAL: &Pal = &[('a', 0x7A2A10), ('b', 0xF08A24), ('c', 0xFFC04A), ('d', 0xFFF0A8)];

/// Pepites incrustees dans la pierre.
const ORE: Map = [
    "................",
    "..kk............",
    ".kohk.......kk..",
    ".koook.....kohk.",
    "..kkk......kook.",
    "............kk..",
    "......kk........",
    ".....kohk.......",
    ".....koook......",
    "......kook......",
    ".......kk.......",
    "..kk............",
    ".kohk......kkk..",
    ".kook.....kohok.",
    "..kk.......kkk..",
    "................",
];
const COAL_PAL: &Pal = &[('o', 0x2A2C33), ('h', 0x4C505B), ('k', 0x1C1D22)];
const IRON_PAL: &Pal = &[('o', 0xD9A27A), ('h', 0xF2CBA8), ('k', 0x9C6B4C)];
const GOLD_PAL: &Pal = &[('o', 0xE8B93A), ('h', 0xFFE38A), ('k', 0xA87A1E)];
const DIAMOND_PAL: &Pal = &[('o', 0x4FD6CF), ('h', 0xB8F5EE), ('k', 0x2B8E8A)];
const VOLT_PAL: &Pal = &[('o', 0x9A6AE8), ('h', 0xD9C8FF), ('k', 0x5A3AA0)];

// ---------------------------------------------------------------- bois et feuillage

/// Ecorce de cedre : longues fibres verticales.
const BARK: Map = [
    "bcbabdbbcabbdbcb",
    "bcbabdbbcabbdbcb",
    "bcbabcbbcabbdbcb",
    "bcbabcbacabbcbcb",
    "bdbabcbacabbcbbb",
    "bdbbbcbacbbbcabb",
    "bdbbbcbbcbbacabd",
    "bcbbacbbdbbacabd",
    "bcbbacbbdbbacbbd",
    "acbbadbbdbbbcbbc",
    "acbbadbbcabbcbbc",
    "acbabdbbcabbdbbc",
    "bcbabdbbcabbdbcb",
    "bcbabdbacabbdbcb",
    "bcbabdbacabbdbcb",
    "bcbabdbbcabbdbcb",
];
const BARK_PAL: &Pal = &[('a', 0x4A2E22), ('b', 0x6B4433), ('c', 0x86583F), ('d', 0x9C6B4C)];

const LOG_TOP: Map = [
    "aaaaaaaaaaaaaaaa",
    "abbbbbbbbbbbbbba",
    "abddddddddddddba",
    "abdccccccccccdba",
    "abdcddddddddcdba",
    "abdcdccccccdcdba",
    "abdcdcddddcdcdba",
    "abdcdcdccdcdcdba",
    "abdcdcdccdcdcdba",
    "abdcdcddddcdcdba",
    "abdcdccccccdcdba",
    "abdcddddddddcdba",
    "abdccccccccccdba",
    "abddddddddddddba",
    "abbbbbbbbbbbbbba",
    "aaaaaaaaaaaaaaaa",
];
const LOG_TOP_PAL: &Pal = &[('a', 0x4A2E22), ('b', 0x86583F), ('c', 0xC9A26B), ('d', 0xE0BE86)];

/// Feuillage parseme de fleurs de cerisier.
const LEAVES: Map = [
    "abbcbbabbcbbabbc",
    "bcbbabbcpbbabbcb",
    "bbabbcbpqpbcbbab",
    "cbbcbbabpbbbabbc",
    "bbabbcbbbabcbbab",
    "abbcbabbcbbbabcb",
    "bcbbbbcbbabbcbbb",
    "bbabpbbbabbcbbab",
    "cbbpqpabbcbbabbc",
    "bbabpbbcbbabbcbb",
    "abbcbbabbcbbbbab",
    "bcbbabbbabbcbabb",
    "bbabbcbbcbbbbcbp",
    "cbbbabbabbabcbpq",
    "bbcbbbcbbcbbbabp",
    "babbabbbabbcbbab",
];
const LEAVES_PAL: &Pal = &[('a', 0x2F5E2E), ('b', 0x3F7A38), ('c', 0x55964A), ('p', 0xF4B6C8), ('q', 0xFBD9E3)];

/// Parquet de hinoki : lames claires, joints decales.
const PLANKS: Map = [
    "cccccccccccacccc",
    "bbbbdbbbbbbabbbb",
    "bbbbbbbddbbabbbb",
    "aaaaaaaaaaaaaaaa",
    "cccacccccccccccc",
    "bbbabbbbbdbbbbbb",
    "bdbabbbbbbbbbddb",
    "aaaaaaaaaaaaaaaa",
    "cccccccccccccacc",
    "bbbbbbdbbbbbbabb",
    "bbddbbbbbbbbbabb",
    "aaaaaaaaaaaaaaaa",
    "ccccccccaccccccc",
    "bbbbdbbbabbbbbbb",
    "bbbbbbbbabbddbbb",
    "aaaaaaaaaaaaaaaa",
];
const HINOKI_PAL: &Pal = &[('a', 0x9C7A4A), ('b', 0xC9A56C), ('c', 0xDDBB82), ('d', 0xB8935B)];
/// Cedre fonce, pour les meubles.
const CEDAR_PAL: &Pal = &[('a', 0x3E2A20), ('b', 0x6B4433), ('c', 0x86583F), ('d', 0x5A3A28)];

// ---------------------------------------------------------------- constructions japonaises

/// Paroi shoji : papier washi tendu sur un treillis de bois.
const SHOJI: Map = [
    "wwwwwwwwwwwwwwww",
    "wppppwppppwppppw",
    "wppppwppppwppppw",
    "wpppqwpppqwpppqw",
    "wppqqwppqqwppqqw",
    "wwwwwwwwwwwwwwww",
    "wppppwppppwppppw",
    "wppppwppppwppppw",
    "wpppqwpppqwpppqw",
    "wppqqwppqqwppqqw",
    "wwwwwwwwwwwwwwww",
    "wppppwppppwppppw",
    "wppppwppppwppppw",
    "wpppqwpppqwpppqw",
    "wppqqwppqqwppqqw",
    "wwwwwwwwwwwwwwww",
];
const SHOJI_PAL: &Pal = &[('w', 0x5A3A28), ('p', 0xF2E9D8), ('q', 0xE0D4BC)];

/// Tuiles kawara : ecailles bleu-gris en quinconce.
const KAWARA: Map = [
    "jaaaaaajjaaaaaaj",
    "abbbbbbaabbbbbba",
    "abccccbaabccccba",
    "abbbbbbaabbbbbba",
    "jabbbbajjabbbbaj",
    "jjaaaajjjjaaaajj",
    "jjjjjjjjjjjjjjjj",
    "jjjjjjjjjjjjjjjj",
    "aaajjaaaaaajjaaa",
    "bbbaabbbbbbaabbb",
    "ccbaabccccbaabcc",
    "bbbaabbbbbbaabbb",
    "bbajjabbbbajjabb",
    "aajjjjaaaajjjjaa",
    "jjjjjjjjjjjjjjjj",
    "jjjjjjjjjjjjjjjj",
];
const KAWARA_PAL: &Pal = &[('j', 0x2E3440), ('a', 0x4A5366), ('b', 0x66718A), ('c', 0x8C97AE)];

const BENCH_TOP: Map = [
    "aaaaaaaaaaaaaaaa",
    "accccdccccdcccca",
    "acbbbdcbbbdcbbba",
    "acbbbdcbbbdcbbba",
    "acbbbdcbbbdcbbba",
    "adddddddddddddda",
    "accccdccccdcccca",
    "acbbbdcbbbdcbbba",
    "acbbbdcbbbdcbbba",
    "acbbbdcbbbdcbbba",
    "adddddddddddddda",
    "accccdccccdcccca",
    "acbbbdcbbbdcbbba",
    "acbbbdcbbbdcbbba",
    "acbbbdcbbbdcbbba",
    "aaaaaaaaaaaaaaaa",
];

/// Scie et maillet accroches au flanc de l'etabli.
const TOOLS: Map = [
    "................",
    "................",
    "..ss......hhhh..",
    "..ss......hhhh..",
    "..ss.......tt...",
    "..ss.......tt...",
    "..ss.......tt...",
    "..ss.......tt...",
    "..ss.......tt...",
    "..tt.......tt...",
    "..tt.......tt...",
    "..tt............",
    "................",
    "................",
    "................",
    "................",
];
const TOOLS_PAL: &Pal = &[('s', 0xC8CED8), ('h', 0x8E96A4), ('t', 0x2A1C16)];

/// Gueule du fourneau ; 'f' et 'g' sont le feu (sombres quand il est eteint).
const HEARTH: Map = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "....kkkkkkkk....",
    "...kkddddddkk...",
    "...kddddddddk...",
    "...kddffffddk...",
    "...kdffggffdk...",
    "...kdfggggfdk...",
    "...kkkkkkkkkk...",
    "................",
    "................",
    "................",
];
const HEARTH_OFF: &Pal = &[('k', 0x2A2420), ('d', 0x14100E), ('f', 0x1E1814), ('g', 0x2A2018)];
const HEARTH_ON: &Pal = &[('k', 0x2A2420), ('d', 0x4A1C0C), ('f', 0xF08A24), ('g', 0xFFD95A)];

/// Coffre tansu : cedre sombre et ferrures.
const TANSU: Map = [
    "iiaaaaaaaaaaaaii",
    "icccccccccccccci",
    "abbbbbbbbbbbbbba",
    "abbbbbbiibbbbbba",
    "abbbbbbssbbbbbba",
    "abbbbbbbbbbbbbba",
    "aaaaaaaaaaaaaaaa",
    "acccccccccccccca",
    "abbbbbbbbbbbbbba",
    "abbbbbbiibbbbbba",
    "abbbbbbssbbbbbba",
    "abbbbbbbbbbbbbba",
    "aaaaaaaaaaaaaaaa",
    "acccccccccccccca",
    "ibbbbbbiibbbbbbi",
    "iiaaaaaassaaaaii",
];
const TANSU_PAL: &Pal = &[('a', 0x3E2A20), ('b', 0x5A3E2E), ('c', 0x74523C), ('i', 0x2A2C33), ('s', 0x8E96A4)];

/// Lanterne andon : cadre de bois, papier eclaire de l'interieur.
const ANDON: Map = [
    "wwwwwwwwwwwwwwww",
    "wppppppwwppppppw",
    "wppppppwwppppppw",
    "wppqqppwwppqqppw",
    "wpqqqqpwwpqqqqpw",
    "wppqqppwwppqqppw",
    "wppppppwwppppppw",
    "wwwwwwwwwwwwwwww",
    "wwwwwwwwwwwwwwww",
    "wppppppwwppppppw",
    "wppqqppwwppqqppw",
    "wpqqqqpwwpqqqqpw",
    "wppqqppwwppqqppw",
    "wppppppwwppppppw",
    "wppppppwwppppppw",
    "wwwwwwwwwwwwwwww",
];
const ANDON_OFF: &Pal = &[('w', 0x3E2A20), ('p', 0xB9AD94), ('q', 0xA39780)];
const ANDON_ON: &Pal = &[('w', 0x3E2A20), ('p', 0xFFE9A8), ('q', 0xFFF8DC)];

/// Baril de feux d'artifice.
const HANABI: Map = [
    "rrrrrrrrrrrrrrrr",
    "rsrrrrrrrrrrrrsr",
    "rrrrrrrrrrrrrrrr",
    "dddddddddddddddd",
    "wwwwwwwwwwwwwwww",
    "wwwwwwkwwkwwwwww",
    "wwwwwkkkkkkwwwww",
    "wwwwwwkwwkwwwwww",
    "wwwwwkkkkkkwwwww",
    "wwwwwwkwwkwwwwww",
    "wwwwwwwwwwwwwwww",
    "dddddddddddddddd",
    "rrrrrrrrrrrrrrrr",
    "rsrrrrrrrrrrrrsr",
    "rrrrrrrrrrrrrrrr",
    "qqqqqqqqqqqqqqqq",
];
const HANABI_PAL: &Pal = &[('r', 0xC8382A), ('s', 0xE86A4A), ('d', 0x7A1E18), ('w', 0xF2E9D8), ('k', 0x2A2420), ('q', 0x9A2A20)];

/// Futon indigo a petits motifs, oreiller blanc.
const FUTON: Map = [
    "wwwwwiiiiiiiiiii",
    "wqqqwiiiwiiiwiii",
    "wqqqwiiwiwiwiwii",
    "wqqqwiiiwiiiwiii",
    "wqqqwiiiiiiiiiii",
    "wqqqwiwiiiwiiiwi",
    "wqqqwwiwiwiwiwiw",
    "wqqqwiwiiiwiiiwi",
    "wqqqwiiiiiiiiiii",
    "wqqqwiiiwiiiwiii",
    "wqqqwiiwiwiwiwii",
    "wqqqwiiiwiiiwiii",
    "wqqqwiiiiiiiiiii",
    "wqqqwiwiiiwiiiwi",
    "wqqqwwiwiwiwiwiw",
    "wwwwwiwiiiwiiiwi",
];
const FUTON_PAL: &Pal = &[('w', 0xF2EEE4), ('q', 0xDAD4C6), ('i', 0x2B3A67)];

const FUR: Map = [
    "bbcbbbbbbcbbbbbb",
    "bcbbbabbcbbbbabb",
    "bbbbabbbbbbbabbb",
    "bbbbbbbcbbbbbbbc",
    "cbbbbbcbbbbbbbcb",
    "bbbabbbbbbabbbbb",
    "bbabbbbbbabbbcbb",
    "bbbbbcbbbbbbcbbb",
    "bbbbcbbbbbbbbbbb",
    "abbbbbbbabbbbbba",
    "bbbbbbbabbbcbbab",
    "bbcbbbbbbbcbbbbb",
    "bcbbbbabbbbbbbbb",
    "bbbbbabbbbbbbcbb",
    "bbbbbbbbbcbbcbbb",
    "bbbbbbbbcbbbbbbb",
];
const FUR_PAL: &Pal = &[('a', 0xB79C7C), ('b', 0xCDB596), ('c', 0xE2CFB4)];

/// Portail de l'Abime : un torii vermillon sur fond de nuit.
const TORII: Map = [
    "dddddddddddddddd",
    "drrrrrrrrrrrrrrd",
    "ddrrrrrrrrrrrrdd",
    "ddddrrddddrrdddd",
    "ddrrrrrrrrrrrrdd",
    "ddddrrddddrrdddd",
    "ddddrrdppdrrdddd",
    "ddddrrpppprrdddd",
    "ddddrrpqqprrdddd",
    "ddddrrpqqprrdddd",
    "ddddrrpppprrdddd",
    "ddddrrdppdrrdddd",
    "ddddrrddddrrdddd",
    "ddddrrddddrrdddd",
    "dddkrrkddkrrkddd",
    "dddddddddddddddd",
];
const TORII_PAL: &Pal = &[('d', 0x1E1830), ('r', 0xC8382A), ('p', 0x8A3AD0), ('q', 0xD8A0FF), ('k', 0x7A1E18)];

/// Portail du Zenith : pleine lune et etoiles.
const MOON_GATE: Map = [
    "tttttttttttttttt",
    "tnnnnnnnnnnnnnnt",
    "tnnsnnnnnnnnnsnt",
    "tnnnnnmmmmnnnnnt",
    "tnnnnmmmmmmnnnnt",
    "tnnnmmmmmwmmnnnt",
    "tnsnmmmmmmmmnnnt",
    "tnnnmmwmmmmmnnnt",
    "tnnnmmmmmmmmnsnt",
    "tnnnmmmmmwmmnnnt",
    "tnnnnmmmmmmnnnnt",
    "tnnnnnmmmmnnnnnt",
    "tnsnnnnnnnnnnnnt",
    "tnnnnnnnnnnsnnnt",
    "tnnnnnnnnnnnnnnt",
    "tttttttttttttttt",
];
const MOON_PAL: &Pal = &[('t', 0x3FA89A), ('n', 0x1A2450), ('s', 0xF2EEE4), ('m', 0xF4E8B0), ('w', 0xD9C98A)];

const STEEL: Map = [
    "aaaaaaaaaaaaaaaa",
    "acccccccccccccba",
    "acbbbbbbbbbbbbda",
    "acbsbbbbbbbbsbda",
    "acbbbbbbbbbbbbda",
    "acbbbbbbbbbbbbda",
    "acbbbbbbbbbbbbda",
    "acbbbbbbbbbbbbda",
    "acbbbbbbbbbbbbda",
    "acbbbbbbbbbbbbda",
    "acbbbbbbbbbbbbda",
    "acbbbbbbbbbbbbda",
    "acbsbbbbbbbbsbda",
    "acbbbbbbbbbbbbda",
    "abddddddddddddda",
    "aaaaaaaaaaaaaaaa",
];
const STEEL_PAL: &Pal = &[('a', 0x5A606B), ('b', 0xB8BEC8), ('c', 0xDCE0E6), ('d', 0x8E96A4), ('s', 0x6B727E)];

/// Porte fusuma : paysage peint a l'encre, soleil rouge.
const FUSUMA: Map = [
    "wwwwwwwwwwwwwwww",
    "wppppppppppppppw",
    "wppppppppprrpppw",
    "wpppppppprrrrppw",
    "wppppppppprrpppw",
    "wppppppppppppppw",
    "wpppkppppppppppw",
    "wppkkkpppppkpppw",
    "whkkkkkpppkkkppw",
    "whkkkkkkpkkkkkpw",
    "wkkkkkkkkkkkkkkw",
    "wqqqqqqqqqqqqqqw",
    "wppppppppppppppw",
    "wppppppppppppppw",
    "wppppppppppppppw",
    "wwwwwwwwwwwwwwww",
];
const FUSUMA_PAL: &Pal = &[('w', 0x3E2A20), ('p', 0xF2E9D8), ('q', 0xDDD2BE), ('k', 0x6B727E), ('r', 0xC8382A), ('h', 0x2A2420)];

const CACTUS: Map = [
    "abbcbbbaabbcbbba",
    "abscbbbaabbcbbba",
    "abbcbbbaabbcbsba",
    "abbcbbbaabbcbbba",
    "abbcbbsaabbcbbba",
    "abbcbbbaabscbbba",
    "abbcbbbaabbcbbba",
    "asbcbbbaabbcbbsa",
    "abbcbbbaabbcbbba",
    "abbcbsbaabbcbbba",
    "abbcbbbaasbcbbba",
    "abbcbbbaabbcbbba",
    "abscbbbaabbcbsba",
    "abbcbbbaabbcbbba",
    "abbcbbsaabbcbbba",
    "abbcbbbaabbcbbba",
];
const CACTUS_PAL: &Pal = &[('a', 0x2F6A3A), ('b', 0x44894A), ('c', 0x62A85E), ('s', 0xECE9D2)];

// ---------------------------------------------------------------- plantes et objets poses

/// Riz mur : tiges vertes, epis dores qui retombent.
const RICE: Map = [
    "....y.....y.....",
    "...yy....yy...y.",
    "..y.y...y.y..yy.",
    ".y..g..y..g.y.g.",
    ".y..g..y..g.y.g.",
    "....g.....g...g.",
    "..g.g...g.g...g.",
    "..g.gg..g.gg..g.",
    "..g.g...g.g..gg.",
    "..gg.g..gg.g.g..",
    "...g.g...g.g.g..",
    "...g.g...g.g.gg.",
    "...gg....gg...g.",
    "...g.g...g.g..g.",
    "..gg.g..gg.g.gg.",
    "..g..g..g..g.g..",
];
const RICE_GREEN: &Pal = &[('y', 0x6FA844), ('g', 0x5E9A3E)];
const RICE_RIPE: &Pal = &[('y', 0xE2B84A), ('g', 0x9AA04A)];

/// Radis daikon : fanes vertes, collet blanc qui sort de terre.
const DAIKON: Map = [
    "................",
    "................",
    "................",
    "................",
    "..g...g...g...g.",
    ".gg..gg..gg..gg.",
    ".g.g.g.g.g.g.g.g",
    ".g.g.g.g.g.g.g.g",
    "..gg..gg..gg..gg",
    "..g...g...g...g.",
    "..g...g...g...g.",
    ".ggg.ggg.ggg.ggg",
    "..g...g...g...g.",
    ".www.www.www.www",
    ".wrw.wrw.wrw.wrw",
    "..w...w...w...w.",
];
const DAIKON_LEAF: &Pal = &[('g', 0x4E9A3E), ('w', 0x4E9A3E), ('r', 0x3E7E34)];
const DAIKON_RIPE: &Pal = &[('g', 0x4E9A3E), ('w', 0xF2EEE4), ('r', 0xD8D2C4)];

/// Jeune cerisier en fleurs.
const SAKURA: Map = [
    "................",
    "......pp........",
    ".....pqqp..pp...",
    "....pqqqqppqqp..",
    "....pqqqqpqqqp..",
    ".....pqqp.pqp...",
    "......pp...p....",
    ".......t..t.....",
    ".......t.t......",
    ".......tt.......",
    ".......t........",
    "......tt........",
    "......t.........",
    "......t.........",
    ".....ttt........",
    "....ttttt.......",
];
const SAKURA_PAL: &Pal = &[('p', 0xE890AE), ('q', 0xF9C8D6), ('t', 0x5A3A28)];

/// Lanterne de papier au bout d'une perche.
const BONBORI: Map = [
    "................",
    "................",
    "......kkkk......",
    ".....kyyyyk.....",
    ".....kyffyk.....",
    ".....kyffyk.....",
    ".....kyyyyk.....",
    "......kkkk......",
    ".......tt.......",
    ".......tt.......",
    ".......tt.......",
    ".......tt.......",
    ".......tt.......",
    ".......tt.......",
    ".......tt.......",
    "......tttt......",
];
const BONBORI_PAL: &Pal = &[('k', 0x8A2A22), ('y', 0xF7E3A6), ('f', 0xFFB23A), ('t', 0x5A3A28)];
const SIGNAL_ON: &Pal = &[('k', 0x4A3A7A), ('y', 0xD9C8FF), ('f', 0xB48CFF), ('t', 0x5A3A28)];
const SIGNAL_OFF: &Pal = &[('k', 0x2E2A40), ('y', 0x5A5470), ('f', 0x443E58), ('t', 0x5A3A28)];

const WIRE: Map = [
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    "wwwwwwwwwwwwwwww",
    "wwwwwwwwwwwwwwww",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
];

/// Le fil est blanc : sa couleur vient de la puissance du signal.
const WIRE_PAL: &Pal = &[('w', 0xFFFFFF)];

const LEVER: Map = [
    "................",
    "................",
    "................",
    "..........rr....",
    ".........rrrr...",
    "..........rr....",
    ".........tt.....",
    "........tt......",
    ".......tt.......",
    ".......tt.......",
    "......tt........",
    "......tt........",
    "....ssssssss....",
    "...sccccccccs...",
    "...sbbbbbbbbs...",
    "...ssssssssss...",
];
const LEVER_PAL: &Pal = &[('r', 0xC8382A), ('t', 0x5A3A28), ('s', 0x565C67), ('c', 0x9BA2AD), ('b', 0x858C98)];

/// Herbes de la pampa (susuki).
const SUSUKI: Map = [
    "................",
    "................",
    "....h......h....",
    "...hg.....h.....",
    "...g..h...g..h..",
    "..gg..g..gg..g..",
    "..g..gg..g..gg..",
    "..g..g..gg..g...",
    ".gg..g..g...g..h",
    ".g..gg..g..gg..g",
    ".g..g..gg..g..gg",
    ".g.gg..g..gg..g.",
    "gg.g..gg..g..gg.",
    "g..g..g..gg..g..",
    "g.gg.gg..g..gg..",
    "g.g..g..gg..g...",
];
const SUSUKI_PAL: &Pal = &[('g', 0x5E9A3E), ('h', 0x9CCB66)];

/// Lycoris rouge (higanbana).
const HIGANBANA: Map = [
    "................",
    "...r.r..r.r.....",
    "..r..rrrr..r....",
    ".r..rrrrrr..r...",
    "...rrryyrrr.....",
    ".r.rrryyrrr.r...",
    "..r.rrrrrr.r....",
    "...r.rrrr.r.....",
    "..r...gg...r....",
    ".......g........",
    ".......g........",
    ".......g........",
    ".......g........",
    "......gg........",
    "......g.........",
    "......g.........",
];
const HIGANBANA_PAL: &Pal = &[('r', 0xD8322A), ('y', 0xF2C84A), ('g', 0x4E8A3A)];

// ---------------------------------------------------------------- circuits de reiseki

const WIRE_DOT: Map = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "......wwww......",
    ".....wwwwww.....",
    ".....wwwwww.....",
    ".....wwwwww.....",
    ".....wwwwww.....",
    "......wwww......",
    "................",
    "................",
    "................",
    "................",
    "................",
];
const WIRE_ARM: Map = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "........wwwwwwww",
    "........wwwwwwww",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
];
const WIRE_V: Map = [
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
    ".......ww.......",
];
/// Relais ofuda : talisman de papier sur une plaque laquee. La pointe indique la sortie (vers la droite).
const OFUDA: Map = [
    "ssssssssssssssss",
    "skkkkkkkkkkkkkks",
    "skkkkkkkkkkkkkks",
    "skkwwwwwwwwkkkks",
    "skkwwwwwwwwwkkks",
    "skkwrrwrwrwwwkks",
    "skkwwwwwwwwwwwks",
    "skkwrwrrwrwwwwws",
    "skkwwwwwwwwwwwks",
    "skkwrrwrwwwwwkks",
    "skkwwwwwwwwwkkks",
    "skkwwwwwwwwkkkks",
    "skkkkkkkkkkkkkks",
    "skggggggggkkkkks",
    "skkkkkkkkkkkkkks",
    "ssssssssssssssss",
];
const OFUDA_OFF: &Pal = &[('s', 0x565C67), ('k', 0x2A2030), ('w', 0xB8B0A0), ('r', 0x6A2A2A), ('g', 0x6A5428)];
const OFUDA_ON: &Pal = &[('s', 0x565C67), ('k', 0x2A2030), ('w', 0xFFF6E0), ('r', 0x9A5AFF), ('g', 0xE8B93A)];

/// Balance onmyo : le yin et le yang. Le temoin de droite s'allume en mode soustraction.
const ONMYO: Map = [
    "ssssssssssssssss",
    "skkkkkkkkkkkkkks",
    "skkkkwwwwwkkkkks",
    "skkkwwwwwwwkkkks",
    "skkwwwwbbwwwkkks",
    "skkwwwbbbbwwkkfs",
    "skwwwwbbbbbbkffs",
    "skwwwwwbbbbbkffs",
    "skwwwwwbbbbbkffs",
    "skwwwbbbbbbbkffs",
    "skkwwbbwwbbbkkfs",
    "skkwbbbwwbbbkkks",
    "skkkbbbbbbbkkkks",
    "skkkkbbbbbkkkkks",
    "skkkkkkkkkkkkkks",
    "ssssssssssssssss",
];

/// Sceau hanko : laque rouge.
const HANKO: Map = [
    "rrrrrrrrrrrrrrrr",
    "rrhrrrrdrrrrhrrr",
    "rrrrrrrrrrrrrrrr",
    "rrhrrrrdrrrrhrrr",
    "rrrrrrrrrrrrrrrr",
    "rrhrrrrdrrrrhrrr",
    "rrrrrrrrrrrrrrrr",
    "rrhrrrrdrrrrhrrr",
    "rrrrrrrrrrrrrrrr",
    "rrhrrrrdrrrrhrrr",
    "rrrrrrrrrrrrrrrr",
    "rrhrrrrdrrrrhrrr",
    "rrrrrrrrrrrrrrrr",
    "rrhrrrrdrrrrhrrr",
    "rrrrrrrrrrrrrrrr",
    "rrhrrrrdrrrrhrrr",
];
const HANKO_PAL: &Pal = &[('r', 0xB8322A), ('h', 0xE86A4A), ('d', 0x8A2A22)];

/// Cerclages de fer des mecanismes karakuri.
const BANDS: Map = [
    "................",
    "iiiiiiiiiiiiiiii",
    "iIiiiiiIiiiiiIii",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "iIiiiiiIiiiiiIii",
    "iiiiiiiiiiiiiiii",
    "................",
];
const GEAR: Map = [
    "................",
    "................",
    "................",
    ".......gg.......",
    "....gg.gg.gg....",
    "....gggggggg....",
    ".....gggggg.....",
    "...gggkkkkggg...",
    "...gggkkkkggg...",
    ".....gggggg.....",
    "....gggggggg....",
    "....gg.gg.gg....",
    ".......gg.......",
    "................",
    "................",
    "................",
];
const FRAME: Map = [
    "aaaaaaaaaaaaaaaa",
    "a..............a",
    "a..............a",
    "a..............a",
    "a..............a",
    "a..............a",
    "a..............a",
    "a..............a",
    "a..............a",
    "a..............a",
    "a..............a",
    "a..............a",
    "a..............a",
    "a..............a",
    "a..............a",
    "aaaaaaaaaaaaaaaa",
];
const GLUE: Map = [
    "................",
    "................",
    "................",
    ".....wwwww......",
    "...wwwwwwwww....",
    "...wwwhhwwwww...",
    "..wwwhhwwwwwww..",
    "..wwwwwwwwwwww..",
    "..wwwwwwwwgwww..",
    "...wwwwwwwwww...",
    "...wwwgwwwwww...",
    "....wwwwwwww....",
    "......wwww......",
    "................",
    "................",
    "................",
];
const HOLE: Map = [
    "aaaaaaaaaaaaaaaa",
    "akkkkkkkkkkkkkka",
    "akkkkkxxxxkkkkka",
    "akkkkkxxxxkkkkka",
    "akkkkkxxxxkkkkka",
    "akkkkkxxxxkkkkka",
    "akkkkkxxxxkkkkka",
    "akkkkkxxxxkkkkka",
    "akkkkkxxxxkkkkka",
    "akkkkkxxxxkkkkka",
    "akkkkkxxxxkkkkka",
    "akkkkkxxxxkkkkka",
    "akkkkkxxxxkkkkka",
    "akkkkkxxxxkkkkka",
    "akkkkkkkkkkkkkka",
    "aaaaaaaaaaaaaaaa",
];
const MOUTH: Map = [
    "................",
    "................",
    "................",
    "................",
    "....kkkkkkkk....",
    "...kddddddddk...",
    "...kddddddddk...",
    "...kddddddddk...",
    "...kddddddddk...",
    "...kddddddddk...",
    "....kkkkkkkk....",
    "................",
    "................",
    "................",
    "................",
    "................",
];
const KARAKURI_PAL: &Pal = &[
    ('i', 0x3E3A44),
    ('I', 0x8A919C),
    ('g', 0xE8B93A),
    ('k', 0x1E1830),
    ('a', 0x5A3A28),
    ('w', 0xE6EED2),
    ('h', 0xFFFFFF),
    ('x', 0x86583F),
    ('d', 0x0E0A16),
];

/// Daruma guetteur : le visage, le flanc et le dos (ou sort le signal).
const DARUMA: Map = [
    "rrrrrrrrrrrrrrrr",
    "rrrrrrrrrrrrrrrr",
    "rrrggrrrrrrggrrr",
    "rrwwwwwwwwwwwwrr",
    "rwwkkkwwwwkkkwwr",
    "rwkwwwkwwkwwwkwr",
    "rwkwkwkwwkwkwkwr",
    "rwkwwwkwwkwwwkwr",
    "rwwkkkwwwwkkkwwr",
    "rwwwwwwkkwwwwwwr",
    "rrwwkwwwwwwkwwrr",
    "rrwwwkkkkkkwwwrr",
    "rrrwwwwwwwwwwrrr",
    "rrrrggggggggrrrr",
    "rrrrrrrrrrrrrrrr",
    "rrrrrrrrrrrrrrrr",
];
const DARUMA_SIDE: Map = [
    "rrrrrrrrrrrrrrrr",
    "rdrrrrrrrrrrrrdr",
    "rrrrrrrrrrrrrrrr",
    "gggggggggggggggg",
    "rrrrrrrrrrrrrrrr",
    "rrrrrgrrrrgrrrrr",
    "rrrrggrrrrggrrrr",
    "rrrgggrrrrgggrrr",
    "rrrrggrrrrggrrrr",
    "rrrrrgrrrrgrrrrr",
    "rrrrrrrrrrrrrrrr",
    "rrdrrrrrrrrrrdrr",
    "gggggggggggggggg",
    "rrrrrrrrrrrrrrrr",
    "rdrrrrrrrrrrrrdr",
    "rrrrrrrrrrrrrrrr",
];
const DARUMA_BACK: Map = [
    "rrrrrrrrrrrrrrrr",
    "rdrrrrrrrrrrrrdr",
    "rrrrrrrrrrrrrrrr",
    "rrrrrrrrrrrrrrrr",
    "rrrrrrrrrrrrrrrr",
    "rrrrrggggggrrrrr",
    "rrrrrgkkkkgrrrrr",
    "rrrrrgkkkkgrrrrr",
    "rrrrrgkkkkgrrrrr",
    "rrrrrgkkkkgrrrrr",
    "rrrrrggggggrrrrr",
    "rrrrrrrrrrrrrrrr",
    "rrrrrrrrrrrrrrrr",
    "rrrrrrrrrrrrrrrr",
    "rdrrrrrrrrrrrrdr",
    "rrrrrrrrrrrrrrrr",
];
const DARUMA_PAL: &Pal = &[('r', 0xC8382A), ('d', 0x8A2A22), ('g', 0xE8B93A), ('w', 0xF2EEE4), ('k', 0x1E1830)];
const DARUMA_LIT: &Pal = &[('r', 0xC8382A), ('d', 0x8A2A22), ('g', 0xE8B93A), ('w', 0xF2EEE4), ('k', 0xD9C8FF)];
/// Bloc de reiseki : la plaque du bloc de fer, taillee dans la pierre d'esprit.
const REISEKI_PAL: &Pal = &[('a', 0x3A2470), ('b', 0x8A5AE0), ('c', 0xD9C8FF), ('d', 0x5A3AA0), ('s', 0x4A2E8A)];

/// Cadran solaire : un soleil d'or sur fond indigo ; en mode nuit, une lune pale.
const HIDOKEI: Map = [
    "ssssssssssssssss",
    "sbbbbbbbbbbbbbbs",
    "sbbbbbbggbbbbbbs",
    "sbbgbbbggbbbgbbs",
    "sbbbgbbbbbbgbbbs",
    "sbbbbbggggbbbbbs",
    "sbbbbggggggbbbbs",
    "sggbbggggggbbggs",
    "sggbbggggggbbggs",
    "sbbbbggggggbbbbs",
    "sbbbbbggggbbbbbs",
    "sbbbgbbbbbbgbbbs",
    "sbbgbbbggbbbgbbs",
    "sbbbbbbggbbbbbbs",
    "sbbbbbbbbbbbbbbs",
    "ssssssssssssssss",
];
const HIDOKEI_DAY: &Pal = &[('s', 0x565C67), ('b', 0x2E4A7A), ('g', 0xE8B93A)];
const HIDOKEI_NIGHT: &Pal = &[('s', 0x565C67), ('b', 0x1E1830), ('g', 0xC4CDE2)];
const LEVER_LIT: &Pal = &[('r', 0xB48CFF), ('t', 0x5A3A28), ('s', 0x565C67), ('c', 0x9BA2AD), ('b', 0x858C98)];

/// Dessin retourne de gauche a droite.
fn mirrored(map: &Map) -> Vec<String> {
    map.iter().map(|row| row.chars().rev().collect()).collect()
}

/// Rail droit, le long de l'axe X : deux fers sur des traverses de bois.
const RAIL: Map = [
    "................",
    ".ww..ww..ww..ww.",
    ".ww..ww..ww..ww.",
    "IIIIIIIIIIIIIIII",
    "iiiiiiiiiiiiiiii",
    ".ww..ww..ww..ww.",
    ".ww..ww..ww..ww.",
    ".ww..ww..ww..ww.",
    ".ww..ww..ww..ww.",
    ".ww..ww..ww..ww.",
    ".ww..ww..ww..ww.",
    "IIIIIIIIIIIIIIII",
    "iiiiiiiiiiiiiiii",
    ".ww..ww..ww..ww.",
    ".ww..ww..ww..ww.",
    "................",
];
const RAIL_POWERED: Map = [
    "................",
    ".ww..ww..ww..ww.",
    ".ww..ww..ww..ww.",
    "IIIIIIIIIIIIIIII",
    "iiiiiiiiiiiiiiii",
    ".ww..ww..ww..ww.",
    ".ww..ww..ww..ww.",
    ".wwvvwwvvwwvvww.",
    ".wwvvwwvvwwvvww.",
    ".ww..ww..ww..ww.",
    ".ww..ww..ww..ww.",
    "IIIIIIIIIIIIIIII",
    "iiiiiiiiiiiiiiii",
    ".ww..ww..ww..ww.",
    ".ww..ww..ww..ww.",
    "................",
];
const RAIL_DETECTOR: Map = [
    "................",
    ".ww..ww..ww..ww.",
    ".ww..ww..ww..ww.",
    "IIIIIIIIIIIIIIII",
    "iiiiiiiiiiiiiiii",
    ".ww..ww..ww..ww.",
    ".wwppppppppppww.",
    ".wwppppppppppww.",
    ".wwppppppppppww.",
    ".wwppppppppppww.",
    ".ww..ww..ww..ww.",
    "IIIIIIIIIIIIIIII",
    "iiiiiiiiiiiiiiii",
    ".ww..ww..ww..ww.",
    ".ww..ww..ww..ww.",
    "................",
];
const RAIL_PAL: &Pal = &[('w', 0x6B4433), ('I', 0xB6BCC6), ('i', 0x6B727E), ('v', 0x443E58), ('p', 0x86583F)];
const RAIL_GOLD_OFF: &Pal = &[('w', 0x6B4433), ('I', 0xE8B93A), ('i', 0xA87A1E), ('v', 0x443E58), ('p', 0x86583F)];
const RAIL_GOLD_ON: &Pal = &[('w', 0x6B4433), ('I', 0xFFE38A), ('i', 0xE8B93A), ('v', 0xB48CFF), ('p', 0x86583F)];
const RAIL_SEEN: &Pal = &[('w', 0x6B4433), ('I', 0xB6BCC6), ('i', 0x6B727E), ('v', 0x443E58), ('p', 0xC8382A)];

/// Corde shimenawa : paille tressee et bandelettes de papier (shide).
const ROPE: Map = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "yYyyYyyYyyYyyYyy",
    "yyYyyYyyYyyYyyYy",
    "..ww......ww....",
    "...ww......ww...",
    "..ww......ww....",
    "................",
    "................",
    "................",
    "................",
];
const ROPE_PAL: &Pal = &[('y', 0xC9A56C), ('Y', 0x9C7A4A), ('w', 0xF2EEE4)];

/// Tambour taiko : corps laque, clous et cordage en zigzag.
const TAIKO: Map = [
    "kkkkkkkkkkkkkkkk",
    "wwwwwwwwwwwwwwww",
    "gkgkgkgkgkgkgkgk",
    "rrrrrrrrrrrrrrrr",
    "yrrrrrryyrrrrrry",
    "ryrrrryrryrrrryr",
    "rryrryrrrryrryrr",
    "rrryyrrrrrryyrrr",
    "rrryyrrrrrryyrrr",
    "rryrryrrrryrryrr",
    "ryrrrryrryrrrryr",
    "yrrrrrryyrrrrrry",
    "rrrrrrrrrrrrrrrr",
    "gkgkgkgkgkgkgkgk",
    "wwwwwwwwwwwwwwww",
    "kkkkkkkkkkkkkkkk",
];
const TAIKO_PAL: &Pal = &[('k', 0x1E1830), ('w', 0xF2EAD8), ('g', 0xE8B93A), ('r', 0x8A2A22), ('y', 0xE2B84A)];

const CART_ICON: Map = [
    "................",
    "................",
    "................",
    "................",
    "................",
    ".kkkkkkkkkkkkkk.",
    ".kxXxxxxxxxxxdk.",
    ".kxxxxxxxxxxxdk.",
    ".kxxxxxxxxxxxdk.",
    "..kxxxxxxxxxdk..",
    "..kkkkkkkkkkkk..",
    "...kik....kik...",
    "...kkk....kkk...",
    "................",
    "................",
    "................",
];

/// Dessin calcule au pixel pres, a partir d'une fonction (x, y) -> couleur (0 = transparent).
fn drawn(wrap: bool, f: impl Fn(f32, f32) -> u32) -> Canvas {
    let mut px = vec![0u32; N * N];
    for y in 0..N {
        for x in 0..N {
            px[y * N + x] = f(x as f32 + 0.5, y as f32 + 0.5);
        }
    }
    Canvas { px, wrap, locked: Vec::new() }
}

/// Virage de rail qui relie le bord droit au bord du bas : deux arcs autour du coin.
fn rail_curve() -> Canvas {
    drawn(false, |x, y| {
        let (dx, dy) = (32.0 - x, 32.0 - y);
        let r = dx.hypot(dy);
        if (r - 24.0).abs() < 1.0 || (r - 8.0).abs() < 1.0 {
            0xFFB6_BCC6
        } else if (r - 26.0).abs() < 1.0 || (r - 10.0).abs() < 1.0 {
            0xFF6B_727E
        } else if (4.0..30.0).contains(&r) && ((dy.atan2(dx) / std::f32::consts::FRAC_PI_2 * 4.0 + 0.25).fract() < 0.5) {
            // Traverses en eventail.
            0xFF6B_4433
        } else {
            0
        }
    })
}

/// Cible de kyudo : anneaux noirs et blancs, mouche rouge, sur un paillasson.
fn target_face() -> Canvas {
    drawn(true, |x, y| {
        let r = (x - 16.0).hypot(y - 16.0);
        if r > 15.0 {
            0xFFC9_A56C
        } else if r < 2.5 {
            0xFFC8_382A
        } else if ((r - 2.5) / 3.2) as u32 % 2 == 0 {
            0xFFF2_EEE4
        } else {
            0xFF1E_1830
        }
    })
}

/// Peau du tambour, cerclee de laque et de clous.
fn drum_skin() -> Canvas {
    drawn(true, |x, y| {
        let r = (x - 16.0).hypot(y - 16.0);
        if r > 15.5 {
            0xFF1E_1830
        } else if r > 13.0 {
            if (((y - 16.0).atan2(x - 16.0) / std::f32::consts::TAU * 16.0).rem_euclid(1.0)) < 0.35 { 0xFFE8_B93A } else { 0xFF8A_2A22 }
        } else if r > 11.0 || ((x * 0.9 + y * 0.4) as u32 % 7 == 0 && r < 9.0) {
            0xFFDD_D2B8
        } else {
            0xFFF2_EAD8
        }
    })
}

// ---------------------------------------------------------------- ferme et cuisine

const SOY_PLANT: Map = [
    "................",
    "......g..g......",
    ".....ggg.gg.....",
    "..g..ggy.gg..g..",
    ".ggg..gg.g..ggg.",
    ".gyg..ggg...gyg.",
    "..gg...g...gg...",
    "...gg..g..gg....",
    "....gg.g.gg.....",
    "..g..ggggg..g...",
    ".gyg..ggg..gyg..",
    "..gg...g...gg...",
    "...gg..g..gg....",
    "....gg.g.gg.....",
    ".....ggggg......",
    ".......g........",
];
const SOY_GREEN: &Pal = &[('g', 0x5E9A3E), ('y', 0x7AB84A)];
const SOY_RIPE: &Pal = &[('g', 0x8A9A3E), ('y', 0xE2D9B0)];

/// Theier : 'l' marque les jeunes pousses, claires quand elles sont bonnes a cueillir.
const TEA_BUSH: Map = [
    "................",
    "................",
    "................",
    "................",
    ".....gggggg.....",
    "...gglgggglgg...",
    "..ggggglgggggg..",
    "..glggggggglgg..",
    ".gggggdggggggdg.",
    ".ggdgggggdggggg.",
    ".gggggggggggdgg.",
    ".gdggggdggggggg.",
    "..ggggggggdggg..",
    "..gggdggggggg...",
    "....ggg.tt.gg...",
    ".......tt.......",
];
const TEA_PLAIN: &Pal = &[('g', 0x2F6B3A), ('l', 0x2F6B3A), ('d', 0x1F4A2A), ('t', 0x5A3A28)];
const TEA_READY: &Pal = &[('g', 0x2F6B3A), ('l', 0xB8E06A), ('d', 0x1F4A2A), ('t', 0x5A3A28)];

const VINE: Map = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    ".......g........",
    "......gg..g.....",
    "..g...g..gg.....",
    "..gg..g.gg......",
    "...gg.ggg..gg...",
    "....ggggggggg...",
    "..gggggggg......",
    ".....gggg.......",
];
const KABOCHA_SIDE: Map = [
    "dddddddddddddddd",
    "dggGggdggGggdggG",
    "dggGggdggGggdggG",
    "dggGggdggGggdggG",
    "dggGggdggGggdggG",
    "dggGygdggGggdygG",
    "dggGggdggGggdggG",
    "dggGggdggGggdggG",
    "dggGggdggGggdggG",
    "dggGggdggGggdggG",
    "dgyGggdggGggdggG",
    "dggGggdggGggdggG",
    "dggGggdggGggdggG",
    "dggGggdggGggdggG",
    "dggGggdggGggdggG",
    "dddddddddddddddd",
];
const KABOCHA_PAL: &Pal = &[('d', 0x1F4A2A), ('g', 0x2F6B3A), ('G', 0x4A8A4A), ('y', 0xC9A56C)];

const BAMBOO_STALK: Map = [
    ".......bB.ll....",
    ".......bBll.....",
    ".......nn.......",
    ".......bB.......",
    "....ll.bB.......",
    ".....llbB.......",
    ".......nn.......",
    ".......bB.......",
    ".......bB..ll...",
    ".......bBll.....",
    ".......nn.......",
    ".......bB.......",
    "...ll..bB.......",
    "....lllbB.......",
    ".......nn.......",
    ".......bB.......",
];
const BAMBOO_PAL: &Pal = &[('b', 0x6FAE4A), ('B', 0x9AD06A), ('n', 0x3E6A2A), ('l', 0x4A8A3A)];

// Objets de cuisine. 'c' est le contenu d'un bol.
const BOWL: Map = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "..kkkkkkkkkkkk..",
    ".kcccccccccccck.",
    ".kxxxxxxxxxxxdk.",
    "..kxXxxxxxxxdk..",
    "..kxxxxxxxxxdk..",
    "...kxxxxxxxdk...",
    "....kkkkkkkk....",
    ".....kddddk.....",
    ".....kkkkkk.....",
    "................",
    "................",
];
const FISH: Map = [
    "................",
    "................",
    "................",
    "................",
    ".....kkkkk......",
    "...kkxxxxxkk..kk",
    "..kxXXxxxxxxkkxk",
    ".kxkxxxxxxxxxxxk",
    ".kxxxxxxxxxxxxdk",
    "..kxxxxxddddkkdk",
    "...kkdddddkk..kk",
    ".....kkkkk......",
    "................",
    "................",
    "................",
    "................",
];
const NIGIRI: Map = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "...kkkkkkkkkk...",
    "..kxXxxxxxxdk...",
    "..kxxxxxxxxxdk..",
    ".kkkkkkkkkkkkkk.",
    ".kwwwwwwwwwwwwk.",
    ".kwwwwwwwwwwwwk.",
    "..kkkkkkkkkkkk..",
    "................",
    "................",
    "................",
    "................",
];
const LEAF: Map = [
    "................",
    "................",
    "..........kk....",
    "........kkggk...",
    ".......kgggGk...",
    "......kggGggk...",
    ".....kggGgggk...",
    "....kggGgggk....",
    "....kgGgggk.....",
    "...kgGgggk......",
    "...kGgggk.......",
    "..kkgkkk........",
    "..kk............",
    "................",
    "................",
    "................",
];
const LEAF_PAL: &Pal = &[('k', 0x1E1830), ('g', 0x5EA83E), ('G', 0xB8E06A)];
const POD: Map = [
    "................",
    "................",
    "................",
    "................",
    "..........kkk...",
    "........kkxxk...",
    "......kkxXxxk...",
    "....kkxXxxxdk...",
    "...kxXxxxxdk....",
    "..kxXxxxddkk....",
    "..kxxxddkk......",
    "...kkkkk........",
    "................",
    "................",
    "................",
    "................",
];
const WEDGE: Map = [
    "................",
    "................",
    "................",
    "................",
    "..kk........kk..",
    "..kdk......kdk..",
    "..kdxk....kxdk..",
    "...kdxkkkkxdk...",
    "...kdxxxxxxdk...",
    "....kdxXXxdk....",
    ".....kddddk.....",
    "......kkkk......",
    "................",
    "................",
    "................",
    "................",
];
const BENTO_BOX: Map = [
    "................",
    "................",
    "................",
    ".kkkkkkkkkkkkkk.",
    ".kwwwwwwkyyyyyk.",
    ".kwwnwwwkyyyyyk.",
    ".kwwwwnwkkkkkkk.",
    ".kwwwwwwkgggggk.",
    ".kkkkkkkkgggggk.",
    ".kxxxxxxkgggggk.",
    ".kxxdxxxkgggggk.",
    ".kkkkkkkkkkkkkk.",
    "................",
    "................",
    "................",
    "................",
];
const SCISSORS: Map = [
    "................",
    "................",
    "...kk......kk...",
    "..kiik....kiik..",
    "..kiiik..kiiik..",
    "...kiiikkiiik...",
    "....kiiiiiik....",
    ".....kiiiik.....",
    "....kkikkikk....",
    "...ksk.kk.ksk...",
    "..ksk......ksk..",
    "..kk........kk..",
    "................",
    "................",
    "................",
    "................",
];
const ROD: Map = [
    "..............kk",
    ".............ksk",
    "............kskw",
    "...........ksk.w",
    "..........ksk..w",
    ".........ksk...w",
    "........ksk....w",
    ".......ksk.....w",
    "......ksk......w",
    ".....ksk.......w",
    "....ksk.......kw",
    "...ksk........kk",
    "..ksk...........",
    ".ksk............",
    "ksk.............",
    "kk..............",
];

/// Dessus du kabocha : cotes rayonnantes autour du pedoncule.
fn kabocha_top() -> Canvas {
    drawn(true, |x, y| {
        let (dx, dy) = (x - 16.0, y - 16.0);
        let r = dx.hypot(dy);
        if r < 3.0 {
            0xFF86_583F
        } else if (dy.atan2(dx) / std::f32::consts::TAU * 8.0).rem_euclid(1.0) < 0.22 || r > 15.0 {
            0xFF1F_4A2A
        } else if r < 9.0 {
            0xFF4A_8A4A
        } else {
            0xFF2F_6B3A
        }
    })
}

/// Dessus du foyer irori : cadre de bois, lit de cendres, braises au centre quand il cuit.
fn irori_top(lit: bool) -> Canvas {
    drawn(true, move |x, y| {
        let edge = x.min(y).min(32.0 - x).min(32.0 - y);
        let r = (x - 16.0).hypot(y - 16.0);
        let speck = hash(x as i32, y as i32, 0, 77) % 9;
        if edge < 2.0 {
            0xFF3E_2A20
        } else if edge < 5.0 {
            0xFF86_583F
        } else if lit && r < 4.0 {
            if speck < 4 { 0xFFFF_D95A } else { 0xFFF0_8A24 }
        } else if r < 7.0 {
            match (lit, speck) {
                (true, 0..=2) => 0xFFF0_8A24,
                (true, 3..=5) => 0xFFB8_320E,
                _ => 0xFF2E_2A30,
            }
        } else if speck == 0 {
            0xFF8E_96A4
        } else {
            0xFFB8_BEC8
        }
    })
}

// ---------------------------------------------------------------- armes

/// Arc long : poignee de cuir rouge, corde a droite.
const YUMI: Map = [
    "..........kk....",
    ".........kssk...",
    "........kssk.w..",
    ".......kssk..w..",
    "......kssk...w..",
    ".....kssk....w..",
    "....krrk.....w..",
    "....krrk.....w..",
    ".....kssk....w..",
    "......kssk...w..",
    ".......kssk..w..",
    "........kssk.w..",
    ".........ksskw..",
    "..........kssk..",
    "...........kk...",
    "................",
];
const YARI: Map = [
    ".............kkk",
    "............khlk",
    "...........khllk",
    "..........khlhkk",
    ".........kshhk..",
    "........kssk....",
    ".......kssk.....",
    "......kssk......",
    ".....kssk.......",
    "....kssk........",
    "...kssk.........",
    "..kssk..........",
    ".kssk...........",
    "kssk............",
    "kkk.............",
    "................",
];
const NAGINATA: Map = [
    "............kkk.",
    "...........khlk.",
    "..........khllk.",
    "..........khlk..",
    ".........khlk...",
    ".........kgk....",
    "........kssk....",
    ".......kssk.....",
    "......kssk......",
    ".....kssk.......",
    "....kssk........",
    "...kssk.........",
    "..kssk..........",
    ".kssk...........",
    "kssk............",
    "kkk.............",
];
const TANTO: Map = [
    "................",
    "................",
    "................",
    "................",
    "..........kk....",
    ".........khlk...",
    "........khlk....",
    ".......khlk.....",
    "......khlk......",
    "....kkgkk.......",
    "....ksk.........",
    "...ksk..........",
    "...kk...........",
    "................",
    "................",
    "................",
];
const TETSUBO: Map = [
    "..........kkkk..",
    ".........khlhhk.",
    "........khghhghk",
    "........khhhhhhk",
    ".......khghhghk.",
    ".......khhhhhk..",
    "......khghhk....",
    "......khhhk.....",
    ".....kkhkk......",
    "....kssk........",
    "...kssk.........",
    "..kssk..........",
    ".kssk...........",
    "kssk............",
    "kkk.............",
    "................",
];
const KUNAI: Map = [
    "................",
    "................",
    "................",
    "...........kk...",
    "..........khlk..",
    ".........khlk...",
    "........khlk....",
    ".......khhk.....",
    "......kkhk......",
    ".....kskk.......",
    "....ksk.........",
    "..kkkk..........",
    ".kk.kk..........",
    ".kkkk...........",
    "................",
    "................",
];
/// Bouclier tate : planches cerclees de fer, frappe d'un disque rouge.
const TATE: Map = [
    "................",
    "..kkkkkkkkkkkk..",
    "..kssssssssssk..",
    "..khhhhhhhhhhk..",
    "..kssssrrssssk..",
    "..kssrrrrrrssk..",
    "..kssrrrrrrssk..",
    "..ksrrrrrrrrsk..",
    "..ksrrrrrrrrsk..",
    "..kssrrrrrrssk..",
    "..kssrrrrrrssk..",
    "..kssssrrssssk..",
    "..khhhhhhhhhhk..",
    "..kssssssssssk..",
    "..kkkkkkkkkkkk..",
    "................",
];

// ---------------------------------------------------------------- grottes et biomes

/// Stalactite : large a la racine, effilee vers la pointe. Retournee, c'est une stalagmite.
const DRIP: Map = [
    "..aaabbbbbbaaa..",
    "...aabbcbbbaa...",
    "...aabbcbbba....",
    "....abbcbbba....",
    "....abbcbba.....",
    ".....abcbba.....",
    ".....abcbba.....",
    ".....abcba......",
    "......acba......",
    "......acba......",
    "......aca.......",
    "......aca.......",
    ".......ca.......",
    ".......ca.......",
    ".......c........",
    "................",
];
const DRIP_PAL: &Pal = &[('a', 0x6E5E50), ('b', 0x9A8670), ('c', 0xC4B098)];

/// Inscription de la stele maudite.
const CURSE: Map = [
    "................",
    "................",
    "....rrrrrrrr....",
    ".......rr.......",
    "..rrrrrrrrrrrr..",
    ".......rr.......",
    "....rr.rr.rr....",
    "...rr..rr..rr...",
    ".......rr.......",
    "....rrrrrrrr....",
    "....r..rr..r....",
    "....rrrrrrrr....",
    "....r..rr..r....",
    "....rrrrrrrr....",
    "................",
    "................",
];
const CURSE_PAL: &Pal = &[('r', 0xC8382A)];
const CEDAR_LEAF_PAL: &Pal = &[('a', 0x16382A), ('b', 0x1F4A34), ('c', 0x2E6442), ('p', 0x1F4A34), ('q', 0x2E6442)];
const MAPLE_LEAF_PAL: &Pal = &[('a', 0x8A2A1E), ('b', 0xB8382A), ('c', 0xD9562E), ('p', 0xF0A030), ('q', 0xFFD070)];
const MOSS_PAL: &Pal = &[('a', 0x3A5E2A), ('b', 0x4A7434), ('c', 0x5E8A40), ('d', 0x74A04E)];
const SLATE_PAL: &Pal = &[('a', 0x2A2C36), ('b', 0x3A3D4A), ('c', 0x4C505E), ('d', 0x1E2028), ('m', 0x3A3D4A)];
const BASALT_PAL: &Pal = &[('a', 0x34303A), ('b', 0x46414C), ('c', 0x5A5560), ('d', 0x26222C), ('m', 0x46414C)];

/// Cristal de roche : facettes claires qui accrochent la lumiere.
fn crystal() -> Canvas {
    drawn(true, |x, y| {
        let (u, v) = ((x + y * 0.5) / 8.0, (y - x * 0.5 + 32.0) / 8.0);
        let (fu, fv) = (u.fract(), v.fract());
        let cell = hash(u as i32, v as i32, 0, 61) % 3;
        if fu < 0.13 || fv < 0.13 {
            0xFF8A_6AD8
        } else if fu + fv < 0.75 {
            [0xFFF4_ECFF, 0xFFE0_D2FF, 0xFFFF_FFFF][cell as usize]
        } else {
            [0xFFC8_B4F8, 0xFFB4_9CF0, 0xFFD9_C8FF][cell as usize]
        }
    })
}

/// Toile : rayons et cercles de fil.
fn cobweb() -> Canvas {
    drawn(false, |x, y| {
        let (dx, dy) = (x - 16.0, y - 16.0);
        let r = dx.hypot(dy);
        let spoke = (dy.atan2(dx) / std::f32::consts::TAU * 8.0).rem_euclid(1.0);
        if r < 16.5 && (spoke < 0.09 || spoke > 0.91 || ((r / 5.0).fract() < 0.2 && r > 3.0)) {
            0xFFE6_E6EE
        } else {
            0
        }
    })
}

/// Lotus : feuille ronde fendue, fleur rose au centre.
fn lotus() -> Canvas {
    drawn(false, |x, y| {
        let (dx, dy) = (x - 16.0, y - 16.0);
        let r = dx.hypot(dy);
        let notch = dx > 0.0 && dy.abs() < dx * 0.22;
        if r < 3.0 {
            0xFFFF_E38A
        } else if r < 6.5 && ((dy.atan2(dx) / std::f32::consts::TAU * 7.0).rem_euclid(1.0) < 0.72) {
            if r < 5.0 { 0xFFFB_D9E3 } else { 0xFFF4_B6C8 }
        } else if r < 13.5 && !notch {
            if r > 12.0 { 0xFF2F_6B3A } else if (dx * 0.7 + dy) as i32 % 5 == 0 { 0xFF5E_9A3E } else { 0xFF4A_8A3A }
        } else {
            0
        }
    })
}

// ---------------------------------------------------------------- Yomi et Takamagahara

/// Onibi : flamme bleue des morts.
const FLAME: Map = [
    "................",
    "................",
    "................",
    ".......b........",
    "......bb........",
    "......bcb.......",
    ".....bbcb..b....",
    ".....bccbb.b....",
    "....bbcwcbbb....",
    "....bcwwccb.....",
    "....bcwwwcb.....",
    "....bbcwwcb.....",
    ".....bccccb.....",
    ".....bbccbb.....",
    "......bbbb......",
    "................",
];
const ONIBI_PAL: &Pal = &[('b', 0x2E6AD8), ('c', 0x6FC8FF), ('w', 0xE4F6FF)];

/// Robe de plumes celeste.
const ROBE: Map = [
    "................",
    "....kk....kk....",
    "...kwwkkkkwwk...",
    "..kwwwwwwwwwwk..",
    ".kwwXwwwwwwXwwk.",
    ".kwkkwwwwwwkkwk.",
    ".kk.kwwXXwwk.kk.",
    "....kwwwwwwk....",
    "....kwXwwXwk....",
    "....kwwwwwwk....",
    "...kwwwXXwwwk...",
    "...kwXwwwwXwk...",
    "..kwwwwwwwwwwk..",
    "..kwXwwXXwwXwk..",
    "..kkkkkkkkkkkk..",
    "................",
];
const ROBE_PAL: &Pal = &[('k', 0x1E1830), ('w', 0xF2EEE4), ('X', 0xF4B6C8)];
const CRIMSON_PAL: &Pal = &[('a', 0x6A1420), ('b', 0x8E1E2A), ('c', 0xB42E34), ('d', 0xD9483E)];
const SOUL_PAL: &Pal = &[('a', 0x3A2E26), ('b', 0x4E3E32), ('c', 0x62503F)];
const BONE_PAL: &Pal = &[('a', 0x9A9078), ('b', 0xDCD4BC), ('c', 0xF2EEDC), ('d', 0xB8AE94), ('s', 0x8A8068)];
const WISTERIA_PAL: &Pal = &[('a', 0x5A3A8A), ('b', 0x7A52B4), ('c', 0x9C74D8), ('p', 0xD9C8FF), ('q', 0xF2E8FF)];
const IRON_SAND_PAL: &Pal = &[('o', 0x2A2E3A), ('h', 0x8FA3D8), ('k', 0x14161E)];

/// Voile de torii : remous violets.
fn veil() -> Canvas {
    drawn(true, |x, y| {
        let swirl = ((x * 0.4).sin() * 3.0 + y * 0.5 + (y * 0.25).cos() * 4.0).rem_euclid(8.0);
        let spark = hash(x as i32, y as i32, 0, 83) % 23 == 0;
        if spark {
            0xFFF2_E8FF
        } else if swirl < 2.5 {
            0xFFB4_8CFF
        } else if swirl < 5.0 {
            0xFF7A_3AD0
        } else {
            0xFF4A_1E8A
        }
    })
}

/// Porte celeste : nuit etoilee cerclee d'or.
fn gateway() -> Canvas {
    drawn(true, |x, y| {
        let edge = x.min(y).min(32.0 - x).min(32.0 - y);
        let star = hash(x as i32, y as i32, 0, 87) % 19 == 0;
        if edge < 2.0 {
            0xFFE8_B93A
        } else if edge < 4.0 {
            0xFFA8_7A1E
        } else if star {
            0xFFF2_F4F8
        } else if (x - 16.0).hypot(y - 16.0) < 6.0 {
            0xFF2E_3A6A
        } else {
            0xFF12_1830
        }
    })
}

/// Koban : piece d'or ovale, frappee de traits.
const KOBAN: Map = [
    "................",
    "................",
    ".....kkkkkk.....",
    "....kxXXxxxk....",
    "...kxXxxxxxdk...",
    "...kxxddddxdk...",
    "...kxxxxxxxdk...",
    "...kxxddddxdk...",
    "...kxxxxxxxdk...",
    "...kxxddddxdk...",
    "...kxxxxxxxdk...",
    "...kxxxxxxddk...",
    "....kxxxdddk....",
    ".....kkkkkk.....",
    "................",
    "................",
];
const VERMILION_PAL: &Pal = &[('a', 0x7A1E18), ('b', 0xC8382A), ('c', 0xE0563E), ('d', 0xB02E24)];

// ---------------------------------------------------------------- ciel

const SUN: Map = [
    "................",
    ".....kkkkkk.....",
    "...kkrrrrrrkk...",
    "..krryyyyyyrrk..",
    "..kryyyyyyyyrk..",
    ".kryyyyyyyyyyrk.",
    ".kryyyywwyyyyrk.",
    ".kryyywwwwyyyrk.",
    ".kryyywwwwyyyrk.",
    ".kryyyywwyyyyrk.",
    ".kryyyyyyyyyyrk.",
    "..kryyyyyyyyrk..",
    "..krryyyyyyrrk..",
    "...kkrrrrrrkk...",
    ".....kkkkkk.....",
    "................",
];
const SUN_PAL: &Pal = &[('k', 0xC8382A), ('r', 0xF08A24), ('y', 0xFFD95A), ('w', 0xFFF6C8)];

const MOON: Map = [
    "................",
    ".....kkkkkk.....",
    "...kkmmmmmmkk...",
    "..kmmmmmmmmmmk..",
    "..kmmccmmmmmmk..",
    ".kmmmccmmmmmmmk.",
    ".kmmmmmmmmccmmk.",
    ".kmmmmmmmcccmmk.",
    ".kmmmmmmmmccmmk.",
    ".kmmcmmmmmmmmmk.",
    ".kmmmmmmmmmmmmk.",
    "..kmmmmmcmmmmk..",
    "..kmmmmmmmmmmk..",
    "...kkmmmmmmkk...",
    ".....kkkkkk.....",
    "................",
];
const MOON_DISC_PAL: &Pal = &[('k', 0xAAB4CC), ('m', 0xE6ECF8), ('c', 0xC4CDE2)];

// ---------------------------------------------------------------- icones de l'interface (8 x 8)

const HEART: [&str; 8] = [".kk..kk.", "krrkkrrk", "krhrrrrk", "krrrrrrk", ".krrrrk.", "..krrk..", "...kk...", "........"];
const HEART_PAL: &Pal = &[('k', 0x1E1830), ('r', 0xD8322A), ('h', 0xFFB0A0)];
const HEART_EMPTY_PAL: &Pal = &[('k', 0x1E1830), ('r', 0x3A2E4A), ('h', 0x3A2E4A)];

/// Cuirasse de la barre d'armure.
const CUIRASS: [&str; 8] = [".kk..kk.", "kaakkaak", "kahaaaak", ".kaaaak.", ".kllllk.", ".kaaaak.", ".kllllk.", "..kkkk.."];
const CUIRASS_PAL: &Pal = &[('k', 0x1E1830), ('a', 0xAEB6C8), ('h', 0xF2F4F8), ('l', 0xC0362C)];
const CUIRASS_EMPTY_PAL: &Pal = &[('k', 0x1E1830), ('a', 0x3A2E4A), ('h', 0x3A2E4A), ('l', 0x3A2E4A)];

/// Onigiri : boulette de riz et sa bande d'algue.
const ONIGIRI: [&str; 8] = ["...kk...", "..kwwk..", ".kwwwwk.", ".kwwwwk.", "kwwnnwwk", "kwwnnwwk", "kwwnnwwk", ".kkkkkk."];
const ONIGIRI_PAL: &Pal = &[('k', 0x1E1830), ('w', 0xF2EEE4), ('n', 0x2A3A2A)];
const ONIGIRI_EMPTY_PAL: &Pal = &[('k', 0x1E1830), ('w', 0x3A2E4A), ('n', 0x2A2238)];

const BUBBLE: [&str; 8] = ["..kkkk..", ".kbbbbk.", "kbhbbbbk", "kbbbbbbk", "kbbbbbbk", "kbbbbbbk", ".kbbbbk.", "..kkkk.."];
const BUBBLE_PAL: &Pal = &[('k', 0x1E1830), ('b', 0x6FAEEA), ('h', 0xE4F2FF)];

// ---------------------------------------------------------------- objets (16 x 16)

const PICKAXE: Map = [
    "................",
    "....kkkkkkk.....",
    "...khhhhhhhk....",
    "..khlllllhhhk...",
    "..kkkkkkskhhhk..",
    ".......kskkhhk..",
    "......kssk.khhk.",
    ".....kssk..khhk.",
    "....kssk....khk.",
    "...kssk.....kk..",
    "..kssk..........",
    ".kssk...........",
    "kssk............",
    "ksk.............",
    ".k..............",
    "................",
];
const AXE: Map = [
    "................",
    ".........kkk....",
    "........khhhk...",
    ".......khlhhhk..",
    "......kshlhhhhk.",
    ".....ksskhhhhhk.",
    "....kssk.khhhhk.",
    "...kssk...khhk..",
    "..kssk.....kk...",
    ".kssk...........",
    "kssk............",
    "ksk.............",
    ".k..............",
    "................",
    "................",
    "................",
];
const SHOVEL: Map = [
    "................",
    "..........kkkk..",
    ".........khhhhk.",
    "........khlhhhk.",
    "........khhlhhk.",
    "........khhhhk..",
    ".......kskhhk...",
    "......kssk.kk...",
    ".....kssk.......",
    "....kssk........",
    "...kssk.........",
    "..kssk..........",
    ".kssk...........",
    "kssk............",
    "kkk.............",
    "................",
];
/// Katana : lame legerement courbe, garde doree.
const KATANA: Map = [
    "..............kk",
    ".............khk",
    "............khlk",
    "...........khlk.",
    "..........khlk..",
    ".........khlk...",
    "........khlk....",
    ".......khlk.....",
    "......khlk......",
    "..kk.khlk.......",
    "..kgkhlk........",
    "...kggk.........",
    "..kskgk.........",
    ".kssk.kk........",
    "kssk............",
    "kkk.............",
];
const HOE: Map = [
    "................",
    "......kkkkkkk...",
    ".....khhhhhhhk..",
    ".....khllllhhk..",
    "......kkkkskhk..",
    ".........kskhk..",
    "........kssk.k..",
    ".......kssk.....",
    "......kssk......",
    ".....kssk.......",
    "....kssk........",
    "...kssk.........",
    "..kssk..........",
    ".kssk...........",
    "kssk............",
    "kkk.............",
];

const STICK: Map = [
    "................",
    "................",
    "............kk..",
    "...........kxXk.",
    "..........kxXk..",
    ".........kxxk...",
    "........kxXk....",
    ".......kxxk.....",
    "......kxXk......",
    ".....kxxk.......",
    "....kxXk........",
    "...kxxk.........",
    "..kxxk..........",
    "..kkk...........",
    "................",
    "................",
];
const LUMP: Map = [
    "................",
    "................",
    ".....kkkkk......",
    "...kkxxxxxkk....",
    "..kxxXXxxxxxk...",
    "..kxXXxxxxdxk...",
    ".kxxXxxxxxddxk..",
    ".kxxxxxxxxxdxk..",
    ".kxxxxxxddxxxk..",
    ".kxxxdxxxdxxxk..",
    "..kxddxxxxxxk...",
    "..kxxxxxxxdk....",
    "...kkxxxddk.....",
    ".....kkkkk......",
    "................",
    "................",
];
const INGOT: Map = [
    "................",
    "................",
    "................",
    "................",
    "....kkkkkkkkk...",
    "...kXXXXXXXXk...",
    "..kXxxxxxxxkdk..",
    ".kxxxxxxxxkddk..",
    ".kkkkkkkkkkddk..",
    ".kddddddddkdk...",
    ".kkkkkkkkkkk....",
    "................",
    "................",
    "................",
    "................",
    "................",
];
const GEM: Map = [
    "................",
    "................",
    ".....kkkkkk.....",
    "....kXXxxxxk....",
    "...kXXxxxxxdk...",
    "..kxXxxxxxxddk..",
    "..kkkkkkkkkkkk..",
    "..kxxxdxxdxxdk..",
    "...kxxdxxdxdk...",
    "....kxdxxddk....",
    ".....kxxddk.....",
    "......kxdk......",
    ".......kk.......",
    "................",
    "................",
    "................",
];
const DUST: Map = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "......kk........",
    ".....kxXk...kk..",
    "....kxxxxk.kxk..",
    "...kxXxxxxkxxxk.",
    "..kxxxxxdxxxxxdk",
    ".kxxxxxdxxxxxddk",
    ".kkkkkkkkkkkkkk.",
    "................",
    "................",
    "................",
];
const ORB: Map = [
    "................",
    "................",
    ".....kkkkkk.....",
    "....kxxxxxxk....",
    "...kxXXxxxxxk...",
    "..kxXXxxxxxxxk..",
    "..kxXxxxxxxxxk..",
    "..kxxxxxxxxxdk..",
    "..kxxxxxxxxxdk..",
    "..kxxxxxxxxddk..",
    "...kxxxxxxddk...",
    "....kxxxdddk....",
    ".....kkkkkk.....",
    "................",
    "................",
    "................",
];
/// Gerbe de riz.
const SHEAF: Map = [
    "................",
    "..y...y...y.....",
    ".yy..yy..yy.....",
    ".y.y.y.y.y.y....",
    "..yy..yy..yy....",
    "...y...y...y....",
    "...g...g..g.....",
    "....g..g.g......",
    ".....g.gg.......",
    "......ggg.......",
    ".....kkkkk......",
    "......ggg.......",
    "......g.g.......",
    ".....g..g.......",
    ".....g...g......",
    "................",
];
const RICE_BALL: Map = [
    "................",
    ".......kk.......",
    "......kwwk......",
    ".....kwwwwk.....",
    "....kwwwwwwk....",
    "....kwwwwwwk....",
    "...kwwwwwwwwk...",
    "...kwwwwwwwwk...",
    "..kwwwnnnnwwwk..",
    "..kwwwnnnnwwwk..",
    ".kwwwwnnnnwwwwk.",
    ".kwwwwnnnnwwwwk.",
    ".kwwwwnnnnwwwwk.",
    "..kkkkkkkkkkkk..",
    "................",
    "................",
];
const RADISH: Map = [
    "...........g.g..",
    "..........ggg...",
    ".........kgggk..",
    "........kwwgk...",
    ".......kwwwk....",
    "......kwwwwk....",
    ".....kwwdwk.....",
    "....kwwwwk......",
    "....kwdwwk......",
    "...kwwwwk.......",
    "...kwwdk........",
    "..kwwwk.........",
    "..kwwk..........",
    ".kwwk...........",
    ".kwk............",
    "..k.............",
];
const MEAT: Map = [
    "................",
    "................",
    "......kkkkk.....",
    "....kkxxxxxkk...",
    "...kxxXXxxxxxk..",
    "..kxxXxxxxxxxk..",
    "..kxxxxxxxxxdk..",
    "..kxxxxxxxxddk..",
    "...kxxxxxxddk...",
    "....kkxxxddkk...",
    "......kkkkwwk...",
    ".........kwwwk..",
    "..........kwwk..",
    "...........kk...",
    "................",
    "................",
];
const HIDE: Map = [
    "................",
    "..kk........kk..",
    ".kxxk......kxxk.",
    ".kxxxkkkkkkxxxk.",
    "..kxxxxxxxxxxk..",
    "...kxxXxxxxxk...",
    "...kxXxxxxxdk...",
    "...kxxxxxxxdk...",
    "...kxxxxxxddk...",
    "..kxxxxxxxxxxk..",
    ".kxxxkkkkkkxxxk.",
    ".kxxk......kxxk.",
    "..kk........kk..",
    "................",
    "................",
    "................",
];
const FEATHER: Map = [
    "............kk..",
    "..........kkwwk.",
    ".........kwwwwk.",
    "........kwwXwk..",
    ".......kwwXwwk..",
    "......kwwXwwk...",
    ".....kwwXwwk....",
    "....kwwXwwk.....",
    "...kwwXwwk......",
    "...kwXwwk.......",
    "..kkXwkk........",
    "..ksk...........",
    ".ksk............",
    ".kk.............",
    "................",
    "................",
];
const THREAD: Map = [
    "................",
    "................",
    "................",
    "....kkkkkk......",
    "...kwwwwwwk.....",
    "..kwwkkkkwwk....",
    "..kwk....kwk....",
    "..kwk....kwkkk..",
    "..kwwkkkkwwwwwk.",
    "...kwwwwwwkkwwk.",
    "....kkkkkk..kwk.",
    ".............k..",
    "................",
    "................",
    "................",
    "................",
];
const BONE: Map = [
    "...........kk.kk",
    "..........kwwkwk",
    "..........kwwwwk",
    "...........kwwk.",
    "..........kwwk..",
    ".........kwwk...",
    "........kwwk....",
    ".......kwwk.....",
    "......kwwk......",
    ".....kwwk.......",
    "....kwwk........",
    "...kwwk.........",
    ".kkwwk..........",
    "kwwwwk..........",
    "kwkwwk..........",
    "kk.kk...........",
];
/// Seau en bois cercle de fer (oke) ; 'c' est son contenu.
const PAIL: Map = [
    "................",
    "................",
    "..kkkkkkkkkkkk..",
    "..kcccccccccck..",
    "..kkkkkkkkkkkk..",
    "..kxxxXxxxxxdk..",
    "..kiiiiiiiiiik..",
    "..kxxxXxxxxxdk..",
    "...kxxXxxxxdk...",
    "...kxxXxxxxdk...",
    "...kiiiiiiiik...",
    "...kxxXxxxxdk...",
    "....kkkkkkkk....",
    "................",
    "................",
    "................",
];
/// Briquet : anneau d'acier et silex.
const STRIKER: Map = [
    "................",
    "................",
    "................",
    "..kkkk..........",
    ".kxxxxk....kk...",
    ".kx..xk...kddk..",
    ".kx..xk..kdddk..",
    ".kxxxxk.kdddk...",
    "..kkkk.kdddk....",
    ".......kddk.....",
    "........kk......",
    "................",
    "................",
    "................",
    "................",
    "................",
];
const ARROW: Map = [
    ".............kkk",
    "............kxxk",
    "...........kxxxk",
    "..........ksxxkk",
    ".........kssk...",
    "........kssk....",
    ".......kssk.....",
    "......kssk......",
    ".....kssk.......",
    "....kssk........",
    "..kkssk.........",
    ".kwkskk.........",
    "kwwkk...........",
    "kwk.............",
    "kk..............",
    "................",
];

/// Palette d'un objet : contour, teinte principale, reflet, ombre, contenu.
fn item_pal(x: u32, light: u32, dark: u32, content: u32) -> Vec<(char, u32)> {
    vec![
        ('k', 0x1E1830),
        ('x', x),
        ('X', light),
        ('d', dark),
        ('c', content),
        ('w', 0xF2EEE4),
        ('s', 0x86583F),
        ('g', 0x5E9A3E),
        ('y', 0xE2B84A),
        ('n', 0x2A3A2A),
        ('i', 0x3E3A44),
    ]
}

/// Nuage : boule blanche aux bords en escalier, ombre bleutee en bas.
fn puff() -> Canvas {
    let mut px = vec![0u32; N * N];
    // Dessine sur une grille de 16 : de gros pixels, lisibles de loin.
    for y in 0..16i32 {
        for x in 0..16i32 {
            let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
            let d = dx.hypot(dy);
            if d > 7.6 {
                continue;
            }
            let lit = dx.hypot(dy + 2.0) < 6.4;
            let c = if d > 6.6 && dy > 0.0 {
                0xFFA9_C4EC
            } else if lit {
                0xFFFF_FFFF
            } else if (x + y) % 2 == 0 && dx.hypot(dy + 2.0) < 7.2 {
                0xFFFF_FFFF
            } else {
                0xFFCE_E2FA
            };
            for k in 0..4 {
                px[((y * 2 + k / 2) * N as i32 + x * 2 + k % 2) as usize] = c;
            }
        }
    }
    Canvas { px, wrap: false, locked: Vec::new() }
}

/// Fissures de minage : des lignes brisees partent du centre, plus longues a chaque stade.
fn cracks(stage: u32) -> Canvas {
    let mut px = vec![0u32; N * N];
    for i in 0..(3 + stage as i32 * 2) {
        let h = hash(i, 0, 0, 52);
        let mut a = i as f32 * 2.4 + (h % 100) as f32 / 100.0;
        let len = 3 + stage as i32 * 2 - ((h >> 8) % 2) as i32;
        let (mut x, mut y) = (8.0f32, 8.0f32);
        for step in 0..len {
            let (gx, gy) = (x.floor() as i32, y.floor() as i32);
            if (0..16).contains(&gx) && (0..16).contains(&gy) {
                for k in 0..4 {
                    px[((gy * 2 + k / 2) * N as i32 + gx * 2 + k % 2) as usize] = 0xFF14_101E;
                }
            }
            // La fissure change un peu de direction a chaque pas.
            a += ((hash(i, step, 1, 52) % 100) as f32 / 100.0 - 0.5) * 0.9;
            x += a.cos();
            y += a.sin();
        }
    }
    Canvas { px, wrap: false, locked: Vec::new() }
}

/// Peint tout le pixel art par-dessus ce que le code avait genere.
pub fn paint(img: &mut Img) {
    // ---- Sols.
    let grass = Canvas::tiled(&GRASS, GRASS_PAL)
        .sprinkle(&["c", "b", "a"], GRASS_PAL, 30, 11)
        .sprinkle(&["c.c", ".a."], GRASS_PAL, 8, 14)
        .sprinkle(&["d"], GRASS_PAL, 16, 12)
        .sprinkle(&["a"], GRASS_PAL, 26, 13);
    let dirt = Canvas::tiled(&DIRT, DIRT_PAL)
        .sprinkle(&[".dd.", "dccd", ".aa."], DIRT_PAL, 5, 21)
        .sprinkle(&["a"], DIRT_PAL, 34, 22)
        .sprinkle(&["c"], DIRT_PAL, 24, 23)
        .sprinkle(&["ca"], DIRT_PAL, 8, 24);
    let snow = Canvas::tiled(&SNOW, SNOW_PAL)
        .sprinkle(&["c"], SNOW_PAL, 30, 26)
        .sprinkle(&["a"], SNOW_PAL, 14, 27)
        .sprinkle(&["cc", "c."], SNOW_PAL, 6, 28);
    grass.blit(img, t::GRASS_TOP, 255);
    dirt.blit(img, t::DIRT, 255);
    snow.blit(img, t::SNOW, 255);
    dirt.clone().over(&Canvas::cutout(&CAP, CAP_GRASS)).blit(img, t::GRASS_SIDE, 255);
    dirt.clone().over(&Canvas::cutout(&CAP, CAP_SNOW)).blit(img, t::SNOW_SIDE, 255);

    // ---- Roches : la meme pierre, recoloree.
    let rock = |pal: &Pal, seed: u32| {
        Canvas::tiled(&STONE, pal)
            .sprinkle(&["c"], pal, 26, seed)
            .sprinkle(&["a"], pal, 20, seed + 1)
            .sprinkle(&["cc", "c."], pal, 5, seed + 2)
            .sprinkle(&["d"], pal, 10, seed + 3)
            .sprinkle(&["m.", "mm"], pal, 3, seed + 4)
    };
    let stone = rock(STONE_PAL, 31);
    stone.blit(img, t::STONE, 255);
    rock(BEDROCK_PAL, 36).blit(img, t::BEDROCK, 255);
    rock(OBSIDIAN_PAL, 41).blit(img, t::OBSIDIAN, 255);
    rock(ABYSS_PAL, 46).blit(img, t::ABYSS_ROCK, 255);
    rock(ZENITH_PAL, 51).blit(img, t::ZENITH_STONE, 255);
    let wall = |pal: &Pal, seed: u32| {
        Canvas::tiled(&WALL, pal).lock(pal, "j").sprinkle(&["a"], pal, 30, seed).sprinkle(&["c"], pal, 20, seed + 1).sprinkle(&["b"], pal, 16, seed + 2)
    };
    wall(WALL_PAL, 56).blit(img, t::COBBLE, 255);
    Canvas::tiled(&SAND, SAND_PAL).sprinkle(&["a"], SAND_PAL, 30, 61).sprinkle(&["c"], SAND_PAL, 22, 62).blit(img, t::SAND, 255);
    Canvas::tiled(&GRAVEL, GRAVEL_PAL).sprinkle(&["c"], GRAVEL_PAL, 20, 63).sprinkle(&["j"], GRAVEL_PAL, 14, 64).blit(img, t::GRAVEL, 255);
    Canvas::tiled(&ICE, ICE_PAL).sprinkle(&["c.", ".c"], ICE_PAL, 6, 65).sprinkle(&["a"], ICE_PAL, 10, 66).blit(img, t::ICE, 255);
    Canvas::tiled(&FARMLAND, FARMLAND_PAL).sprinkle(&["c"], FARMLAND_PAL, 18, 67).sprinkle(&["a"], FARMLAND_PAL, 18, 68).blit(img, t::FARMLAND, 255);

    // ---- Fluides et minerais.
    Canvas::tiled(&WATER, WATER_PAL).sprinkle(&["d"], WATER_PAL, 8, 71).sprinkle(&["cc"], WATER_PAL, 8, 72).blit(img, t::WATER, 200);
    let molten = |pal: &Pal, seed: u32| Canvas::tiled(&LAVA, pal).sprinkle(&["d"], pal, 12, seed).sprinkle(&["a"], pal, 12, seed + 1);
    molten(LAVA_PAL, 73).blit(img, t::LAVA, 255);
    molten(EMBER_PAL, 75).blit(img, t::EMBER, 255);
    for (tile, pal) in [
        (t::COAL_ORE, COAL_PAL),
        (t::IRON_ORE, IRON_PAL),
        (t::GOLD_ORE, GOLD_PAL),
        (t::DIAMOND_ORE, DIAMOND_PAL),
        (t::VOLT_ORE, VOLT_PAL),
    ] {
        stone.clone().over(&Canvas::cutout(&ORE, pal)).blit(img, tile, 255);
    }

    // ---- Bois et feuillage.
    Canvas::tiled(&BARK, BARK_PAL)
        .fibers(color(BARK_PAL, 'a'), 30, 4, 10, true, 81)
        .fibers(color(BARK_PAL, 'd'), 22, 3, 8, true, 82)
        .blit(img, t::LOG_SIDE, 255);
    Canvas::tiled(&LOG_TOP, LOG_TOP_PAL).blit(img, t::LOG_TOP, 255);
    Canvas::tiled(&LEAVES, LEAVES_PAL)
        .sprinkle(&["cc.", "bcc", ".bb"], LEAVES_PAL, 10, 83)
        .sprinkle(&["a"], LEAVES_PAL, 24, 84)
        .sprinkle(&[".p.", "pqp", ".p."], LEAVES_PAL, 3, 85)
        .blit(img, t::LEAVES, 255);
    let boards = |pal: &Pal, seed: u32| {
        Canvas::tiled(&PLANKS, pal)
            .lock(pal, "a")
            .fibers(color(pal, 'd'), 28, 3, 9, false, seed)
            .fibers(color(pal, 'c'), 12, 2, 6, false, seed + 1)
    };
    boards(HINOKI_PAL, 86).blit(img, t::PLANKS, 255);

    // ---- Constructions.
    Canvas::tiled(&SHOJI, SHOJI_PAL).lock(SHOJI_PAL, "w").sprinkle(&["q"], SHOJI_PAL, 26, 91).blit(img, t::GLASS, 255);
    Canvas::tiled(&KAWARA, KAWARA_PAL).lock(KAWARA_PAL, "j").sprinkle(&["c"], KAWARA_PAL, 14, 92).sprinkle(&["a"], KAWARA_PAL, 14, 93).blit(img, t::BRICK, 255);
    Canvas::tiled(&BENCH_TOP, HINOKI_PAL).lock(HINOKI_PAL, "ad").fibers(color(HINOKI_PAL, 'd'), 14, 2, 5, false, 94).blit(img, t::CRAFT_TOP, 255);
    boards(CEDAR_PAL, 95).over(&Canvas::cutout(&TOOLS, TOOLS_PAL)).blit(img, t::CRAFT_SIDE, 255);
    let clay = wall(CLAY_PAL, 96);
    clay.blit(img, t::FURNACE_SIDE, 255);
    clay.clone().over(&Canvas::cutout(&HEARTH, HEARTH_OFF)).blit(img, t::FURNACE_FRONT, 255);
    clay.clone().over(&Canvas::cutout(&HEARTH, HEARTH_ON)).blit(img, t::FURNACE_ON, 255);
    Canvas::tiled(&TANSU, TANSU_PAL).lock(TANSU_PAL, "ais").fibers(color(TANSU_PAL, 'c'), 16, 3, 7, false, 97).blit(img, t::CHEST, 255);
    Canvas::tiled(&ANDON, ANDON_OFF).lock(ANDON_OFF, "w").sprinkle(&["q"], ANDON_OFF, 16, 98).blit(img, t::LAMP_OFF, 255);
    Canvas::tiled(&ANDON, ANDON_ON).blit(img, t::LAMP_ON, 255);
    Canvas::tiled(&HANABI, HANABI_PAL).blit(img, t::EXPLOSIVE, 255);
    Canvas::tiled(&FUTON, FUTON_PAL).blit(img, t::BED, 255);
    Canvas::tiled(&FUR, FUR_PAL).sprinkle(&["c", "b"], FUR_PAL, 26, 99).sprinkle(&["a"], FUR_PAL, 20, 100).blit(img, t::WOOL, 255);
    Canvas::tiled(&TORII, TORII_PAL).blit(img, t::ABYSS_PORTAL, 255);
    Canvas::tiled(&MOON_GATE, MOON_PAL).blit(img, t::ZENITH_PORTAL, 255);
    Canvas::tiled(&STEEL, STEEL_PAL).lock(STEEL_PAL, "as").sprinkle(&["c"], STEEL_PAL, 10, 101).blit(img, t::IRON_BLOCK, 255);
    Canvas::tiled(&FUSUMA, FUSUMA_PAL).blit(img, t::DOOR, 255);
    Canvas::tiled(&CACTUS, CACTUS_PAL).blit(img, t::CACTUS, 255);
    seigaiha().blit(img, t::SEIGAIHA, 255);
    puff().blit(img, t::PUFF, 255);
    for stage in 0..4u16 {
        cracks(stage as u32).blit(img, t::CRACK0 + stage, 220);
    }

    // ---- Circuits de reiseki.
    for (tile, map) in [(t::WIRE_DOT, &WIRE_DOT), (t::WIRE_ARM, &WIRE_ARM), (t::WIRE_V, &WIRE_V)] {
        Canvas::icon(map, WIRE_PAL).blit(img, tile, 255);
    }
    Canvas::cutout(&mirrored(&LEVER), LEVER_LIT).blit(img, t::LEVER_ON, 255);
    for on in 0..2u16 {
        let pal = if on == 1 { OFUDA_ON } else { OFUDA_OFF };
        for delay in 0..4u16 {
            // La perle glisse sur sa tringle : un cran par battement de retard.
            let mut c = Canvas::tiled(&OFUDA, pal);
            c.wrap = false;
            let bead = if on == 1 { 0xFFD9_C8FF } else { 0xFFC8_382A };
            for dy in 0..4 {
                for dx in 0..4 {
                    c.set(4 + delay as i32 * 4 + dx, 25 + dy, bead);
                }
            }
            c.blit(img, t::REPEATER0 + on * 4 + delay, 255);
        }
        for sub in 0..2u16 {
            let (white, black) = if on == 1 { (0xFFF6E0, 0x7A4AD0) } else { (0xB8B0A0, 0x3A3050) };
            let lamp = if sub == 1 { 0xE8322A } else { 0x4A3A3A };
            let pal = [('s', 0x565C67), ('k', 0x2A2030), ('w', white), ('b', black), ('f', lamp)];
            Canvas::tiled(&ONMYO, &pal).blit(img, t::COMPARATOR0 + sub * 2 + on, 255);
        }
    }
    Canvas::tiled(&HANKO, HANKO_PAL).blit(img, t::BUTTON, 255);
    boards(CEDAR_PAL, 111).blit(img, t::PLATE, 255);
    let box_side = boards(HINOKI_PAL, 112).over(&Canvas::cutout(&BANDS, KARAKURI_PAL));
    box_side.blit(img, t::PISTON_SIDE, 255);
    box_side.clone().over(&Canvas::cutout(&GEAR, KARAKURI_PAL)).blit(img, t::PISTON_BACK, 255);
    let face = boards(HINOKI_PAL, 113).over(&Canvas::cutout(&FRAME, KARAKURI_PAL));
    face.blit(img, t::PISTON_FACE, 255);
    face.clone().over(&Canvas::cutout(&GLUE, KARAKURI_PAL)).blit(img, t::STICKY_FACE, 255);
    Canvas::tiled(&HOLE, KARAKURI_PAL).blit(img, t::PISTON_OPEN, 255);
    boards(CEDAR_PAL, 114).blit(img, t::PISTON_ARM, 255);
    Canvas::tiled(&DARUMA, DARUMA_PAL).blit(img, t::OBSERVER_FRONT, 255);
    Canvas::tiled(&DARUMA_SIDE, DARUMA_PAL).blit(img, t::OBSERVER_SIDE, 255);
    Canvas::tiled(&DARUMA_BACK, DARUMA_PAL).blit(img, t::OBSERVER_BACK, 255);
    Canvas::tiled(&DARUMA_BACK, DARUMA_LIT).blit(img, t::OBSERVER_BACK_ON, 255);
    Canvas::tiled(&STEEL, REISEKI_PAL).lock(REISEKI_PAL, "as").sprinkle(&["c"], REISEKI_PAL, 14, 115).blit(img, t::VOLT_BLOCK, 255);
    Canvas::tiled(&HIDOKEI, HIDOKEI_DAY).blit(img, t::SUNDIAL_TOP, 255);
    Canvas::tiled(&HIDOKEI, HIDOKEI_NIGHT).blit(img, t::SUNDIAL_NIGHT, 255);
    boards(HINOKI_PAL, 116).blit(img, t::SUNDIAL_SIDE, 255);
    let chest_side = boards(CEDAR_PAL, 117).over(&Canvas::cutout(&BANDS, KARAKURI_PAL));
    chest_side.blit(img, t::DROPPER_SIDE, 255);
    chest_side.clone().over(&Canvas::cutout(&MOUTH, KARAKURI_PAL)).blit(img, t::DROPPER_FRONT, 255);
    rock(STONE_PAL, 118).blit(img, t::SLAB_SIDE, 255);

    // ---- Rails, tremie, tambour, cible, corde.
    Canvas::icon(&RAIL, RAIL_PAL).blit(img, t::RAIL, 255);
    rail_curve().blit(img, t::RAIL_CURVE, 255);
    Canvas::icon(&RAIL_POWERED, RAIL_GOLD_OFF).blit(img, t::POWERED_RAIL, 255);
    Canvas::icon(&RAIL_POWERED, RAIL_GOLD_ON).blit(img, t::POWERED_RAIL_ON, 255);
    Canvas::icon(&RAIL_DETECTOR, RAIL_PAL).blit(img, t::DETECTOR_RAIL, 255);
    Canvas::icon(&RAIL_DETECTOR, RAIL_SEEN).blit(img, t::DETECTOR_RAIL_ON, 255);
    Canvas::icon(&ROPE, ROPE_PAL).blit(img, t::ROPE, 255);
    boards(CEDAR_PAL, 121).blit(img, t::HOOK, 255);
    Canvas::tiled(&HOLE, KARAKURI_PAL).blit(img, t::HOPPER_TOP, 255);
    boards(HINOKI_PAL, 122).over(&Canvas::cutout(&FRAME, KARAKURI_PAL)).blit(img, t::HOPPER_SIDE, 255);
    Canvas::tiled(&TAIKO, TAIKO_PAL).blit(img, t::NOTE, 255);
    drum_skin().blit(img, t::NOTE_TOP, 255);
    target_face().blit(img, t::TARGET, 255);

    // ---- Ferme et cuisine.
    for stage in 0..4u16 {
        let drop = (3 - stage as usize) * 4;
        Canvas::cutout(&lowered(&SOY_PLANT, drop), if stage == 3 { SOY_RIPE } else { SOY_GREEN }).blit(img, t::SOY0 + stage, 255);
        Canvas::cutout(&lowered(&VINE, (3 - stage as usize) * 2), if stage == 3 { SOY_RIPE } else { SOY_GREEN }).blit(img, t::STEM0 + stage, 255);
        let (sink, pal) = [(8, TEA_PLAIN), (4, TEA_PLAIN), (0, TEA_PLAIN), (0, TEA_READY)][stage as usize];
        Canvas::cutout(&lowered(&TEA_BUSH, sink), pal).blit(img, t::TEA0 + stage, 255);
    }
    Canvas::tiled(&KABOCHA_SIDE, KABOCHA_PAL).blit(img, t::KABOCHA_SIDE, 255);
    kabocha_top().blit(img, t::KABOCHA_TOP, 255);
    Canvas::cutout(&BAMBOO_STALK, BAMBOO_PAL).blit(img, t::BAMBOO, 255);
    boards(CEDAR_PAL, 131).over(&Canvas::cutout(&BANDS, KARAKURI_PAL)).blit(img, t::COMPOST_SIDE, 255);
    let soil: &Pal = &[('a', 0x5A3A28), ('k', 0x4A3320), ('x', 0x6B4A2A)];
    let ripe: &Pal = &[('a', 0x5A3A28), ('k', 0x6B4A2A), ('x', 0xE6EED2)];
    Canvas::tiled(&HOLE, soil).sprinkle(&["x"], soil, 40, 132).blit(img, t::COMPOST_TOP, 255);
    Canvas::tiled(&HOLE, ripe).sprinkle(&["x"], ripe, 70, 133).blit(img, t::COMPOST_FULL, 255);
    let grill = wall(CLAY_PAL, 134).over(&Canvas::cutout(&BANDS, KARAKURI_PAL));
    grill.blit(img, t::SMOKER_SIDE, 255);
    grill.clone().over(&Canvas::cutout(&HEARTH, HEARTH_OFF)).blit(img, t::SMOKER_FRONT, 255);
    grill.clone().over(&Canvas::cutout(&HEARTH, HEARTH_ON)).blit(img, t::SMOKER_ON, 255);
    irori_top(false).blit(img, t::IRORI_TOP, 255);
    irori_top(true).blit(img, t::IRORI_ON, 255);
    boards(CEDAR_PAL, 135).blit(img, t::IRORI_SIDE, 255);

    // ---- Biomes et souterrains.
    Canvas::tiled(&LEAVES, CEDAR_LEAF_PAL).sprinkle(&["cc.", "bcc", ".bb"], CEDAR_LEAF_PAL, 10, 141).sprinkle(&["a"], CEDAR_LEAF_PAL, 30, 142).blit(img, t::CEDAR_LEAVES, 255);
    Canvas::tiled(&LEAVES, MAPLE_LEAF_PAL).sprinkle(&["cc.", "bcc", ".bb"], MAPLE_LEAF_PAL, 10, 143).sprinkle(&["p"], MAPLE_LEAF_PAL, 18, 144).blit(img, t::MAPLE_LEAVES, 255);
    let moss = Canvas::tiled(&GRASS, MOSS_PAL).sprinkle(&["c", "b", "a"], MOSS_PAL, 30, 145).sprinkle(&["d"], MOSS_PAL, 12, 146);
    moss.blit(img, t::MOSS, 255);
    let glow: &Pal = &[('g', 0xB8F06A), ('h', 0xECFFB0)];
    moss.clone().sprinkle(&[".g.", "ghg", ".g."], glow, 9, 147).sprinkle(&["g"], glow, 26, 148).blit(img, t::GLOW_MOSS, 255);
    Canvas::cutout(&DRIP, DRIP_PAL).blit(img, t::STALACTITE, 255);
    let upside: Vec<String> = DRIP.iter().rev().map(|r| r.to_string()).collect();
    Canvas::cutout(&upside, DRIP_PAL).blit(img, t::STALAGMITE, 255);
    let slate = rock(SLATE_PAL, 149);
    slate.blit(img, t::SLATE, 255);
    rock(BASALT_PAL, 154).blit(img, t::BASALT, 255);
    crystal().blit(img, t::CRYSTAL, 255);
    cobweb().blit(img, t::COBWEB, 255);
    lotus().blit(img, t::LOTUS, 255);
    wall(WALL_PAL, 159).over(&Canvas::cutout(&CURSE, CURSE_PAL)).blit(img, t::SPAWNER, 255);
    for (k, pal) in [COAL_PAL, IRON_PAL, GOLD_PAL, DIAMOND_PAL, VOLT_PAL].into_iter().enumerate() {
        slate.clone().over(&Canvas::cutout(&ORE, pal)).blit(img, t::SLATE_ORE0 + k as u16, 255);
    }

    // ---- Yomi et Takamagahara (tuiles logees a droite des modeles).
    Canvas::tiled(&GRASS, CRIMSON_PAL).sprinkle(&["c", "b", "a"], CRIMSON_PAL, 30, 161).sprinkle(&["d"], CRIMSON_PAL, 14, 162).blit(img, t::CRIMSON_MOSS, 255);
    Canvas::tiled(&SAND, SOUL_PAL).sprinkle(&["a"], SOUL_PAL, 40, 163).sprinkle(&["aa", "a."], SOUL_PAL, 10, 164).sprinkle(&["c"], SOUL_PAL, 24, 165).blit(img, t::SOUL_SAND, 255);
    Canvas::cutout(&FLAME, ONIBI_PAL).blit(img, t::ONIBI, 255);
    Canvas::tiled(&STEEL, BONE_PAL).lock(BONE_PAL, "as").sprinkle(&["d"], BONE_PAL, 20, 166).blit(img, t::BONE_BLOCK, 255);
    let yomi_rock = rock(ABYSS_PAL, 167);
    yomi_rock.clone().over(&Canvas::cutout(&ORE, IRON_SAND_PAL)).blit(img, t::SATETSU, 255);
    yomi_rock.over(&Canvas::cutout(&ORE, GOLD_PAL)).blit(img, t::ABYSS_GOLD, 255);
    Canvas::tiled(&LEAVES, WISTERIA_PAL).sprinkle(&["cc.", "bcc", ".bb"], WISTERIA_PAL, 10, 172).sprinkle(&[".p.", "pqp", ".p."], WISTERIA_PAL, 6, 173).blit(img, t::WISTERIA, 255);
    veil().blit(img, t::VEIL, 255);
    gateway().blit(img, t::GATEWAY, 255);
    Canvas::item(&LUMP, &item_pal(0x3A4A6A, 0x8FA3D8, 0x1E2A44, 0)).blit(img, items::icon(items::TAMA_SHARD), 255);
    Canvas::item(&ROBE, ROBE_PAL).blit(img, items::icon(items::HAGOROMO), 255);
    Canvas::item(&ORB, &item_pal(0xF7B8A0, 0xFFE0D0, 0xE0806A, 0)).blit(img, items::icon(items::PEACH), 255);

    // ---- Villages et sanctuaires.
    boards(VERMILION_PAL, 181).blit(img, t::VERMILION, 255);
    Canvas::item(&KOBAN, &item_pal(0xE8B93A, 0xFFE38A, 0xA87A1E, 0)).blit(img, items::icon(items::KOBAN), 255);

    // ---- Plantes et objets poses : fond transparent.
    for stage in 0..4u16 {
        let drop = (3 - stage as usize) * 4;
        let rice = if stage == 3 { RICE_RIPE } else { RICE_GREEN };
        Canvas::cutout(&lowered(&RICE, drop), rice).blit(img, t::WHEAT0 + stage, 255);
        let daikon = if stage == 3 { DAIKON_RIPE } else { DAIKON_LEAF };
        Canvas::cutout(&lowered(&DAIKON, drop), daikon).blit(img, t::CARROT0 + stage, 255);
    }
    for (tile, map, pal) in [
        (t::SAPLING, &SAKURA, SAKURA_PAL),
        (t::TORCH, &BONBORI, BONBORI_PAL),
        (t::SIGTORCH_ON, &BONBORI, SIGNAL_ON),
        (t::SIGTORCH_OFF, &BONBORI, SIGNAL_OFF),
        (t::WIRE, &WIRE, WIRE_PAL),
        (t::LEVER, &LEVER, LEVER_PAL),
        (t::TALLGRASS, &SUSUKI, SUSUKI_PAL),
        (t::FLOWER, &HIGANBANA, HIGANBANA_PAL),
        (t::SUN, &SUN, SUN_PAL),
        (t::MOON, &MOON, MOON_DISC_PAL),
    ] {
        Canvas::cutout(map, pal).blit(img, tile, 255);
    }

    // ---- Icones de l'interface.
    Canvas::icon(&CUIRASS, CUIRASS_PAL).blit(img, t::ARMOR, 255);
    Canvas::icon(&CUIRASS, CUIRASS_EMPTY_PAL).blit(img, t::ARMOR_EMPTY, 255);
    Canvas::icon(&HEART, HEART_PAL).blit(img, t::HEART, 255);
    Canvas::icon(&HEART, HEART_EMPTY_PAL).blit(img, t::HEART_EMPTY, 255);
    Canvas::icon(&ONIGIRI, ONIGIRI_PAL).blit(img, t::FOOD, 255);
    Canvas::icon(&ONIGIRI, ONIGIRI_EMPTY_PAL).blit(img, t::FOOD_EMPTY, 255);
    Canvas::icon(&BUBBLE, BUBBLE_PAL).blit(img, t::BUBBLE, 255);

    // ---- Outils : cinq formes, quatre matieres.
    let tiers = [(0xB8935B, 0xDDBB82), (0x8A919C, 0xB6BCC6), (0xC8CED8, 0xF2F4F8), (0x4FD6CF, 0xB8F5EE)];
    for (kind, map) in [&PICKAXE, &AXE, &SHOVEL, &KATANA, &HOE].into_iter().enumerate() {
        for (tier, (head, light)) in tiers.into_iter().enumerate() {
            let pal = [('k', 0x1E1830), ('h', head), ('l', light), ('s', 0x86583F), ('g', 0xE8B93A)];
            Canvas::item(map, &pal).blit(img, t::ITEM_TOOLS + (kind * 4 + tier) as u16, 255);
        }
    }

    // ---- Autres objets : (objet, forme, teinte, reflet, ombre, contenu).
    let list: [(u16, &Map, u32, u32, u32, u32); 30] = [
        (items::STICK, &STICK, 0x86583F, 0xB07A56, 0x5A3A28, 0),
        (items::COAL, &LUMP, 0x33363E, 0x5A5E6A, 0x1C1D22, 0),
        (items::IRON_INGOT, &INGOT, 0xC8CED8, 0xF2F4F8, 0x8E96A4, 0),
        (items::GOLD_INGOT, &INGOT, 0xE8B93A, 0xFFE38A, 0xA87A1E, 0),
        (items::DIAMOND, &GEM, 0x4FD6CF, 0xB8F5EE, 0x2B8E8A, 0),
        (items::VOLT_DUST, &DUST, 0x9A6AE8, 0xD9C8FF, 0x5A3AA0, 0),
        (items::SEEDS, &DUST, 0xE2D9B0, 0xF7F0D0, 0xB0A478, 0),
        (items::WHEAT_ITEM, &SHEAF, 0, 0, 0, 0),
        (items::BREAD, &RICE_BALL, 0, 0, 0, 0),
        (items::CARROT_ITEM, &RADISH, 0, 0, 0xD8D2C4, 0),
        (items::RAW_MEAT, &MEAT, 0xD85A66, 0xF2A0A8, 0x9A3440, 0),
        (items::COOKED_MEAT, &MEAT, 0xA8643A, 0xD09A6A, 0x6E3E22, 0),
        (items::LEATHER, &HIDE, 0xA8643A, 0xD09A6A, 0x6E3E22, 0),
        (items::FEATHER, &FEATHER, 0, 0xC4CDE2, 0, 0),
        (items::EGG, &ORB, 0xF2EAD8, 0xFFFFFF, 0xC8BEA6, 0),
        (items::STRING, &THREAD, 0, 0, 0, 0),
        (items::BONE, &BONE, 0, 0, 0, 0),
        (items::ROTTEN, &MEAT, 0xB8322A, 0xE86A4A, 0x6A1A1E, 0),
        (items::SPORE, &DUST, 0xF7E3A6, 0xFFF6D0, 0xC8A85A, 0),
        (items::PEARL, &ORB, 0x8FA3D8, 0xE4ECFF, 0x4A5C9A, 0),
        (items::EMBER_CRYSTAL, &GEM, 0xF08A24, 0xFFD95A, 0xB8320E, 0),
        (items::BUCKET, &PAIL, 0xB07A56, 0xD0A070, 0x7A5038, 0x5A3A28),
        (items::WATER_BUCKET, &PAIL, 0xB07A56, 0xD0A070, 0x7A5038, 0x3F86D6),
        (items::LAVA_BUCKET, &PAIL, 0xB07A56, 0xD0A070, 0x7A5038, 0xF08A24),
        (items::LIGHTER, &STRIKER, 0xC8CED8, 0xF2F4F8, 0x565C67, 0),
        (items::APPLE, &ORB, 0xD8322A, 0xFF9A8A, 0x8A1E1E, 0),
        (items::FLINT, &LUMP, 0x565C67, 0x8A919C, 0x33363E, 0),
        (items::WYRM_HEART, &ORB, 0x4FD6CF, 0xE4FFFA, 0x2B7E9A, 0),
        (items::ARROW, &ARROW, 0xC8CED8, 0, 0, 0),
        (items::CART, &CART_ICON, 0xB07A56, 0xD0A070, 0x7A5038, 0),
    ];
    for (id, map, x, light, dark, content) in list {
        Canvas::item(map, &item_pal(x, light, dark, content)).blit(img, items::icon(id), 255);
    }

    // ---- Cuisine et ferme : (objet, forme, teinte, reflet, ombre, contenu).
    let (wood, wood_l, wood_d) = (0x86583F, 0xB07A56, 0x5A3A28);
    let more: [(u16, &Map, u32, u32, u32, u32); 24] = [
        (items::COOKED_RICE, &BOWL, wood, wood_l, wood_d, 0xF2EEE4),
        (items::MISO_SOUP, &BOWL, wood, wood_l, wood_d, 0xC9853A),
        (items::NABE, &BOWL, 0x3E3A44, 0x6B727E, 0x1E1830, 0xB8322A),
        (items::GREEN_TEA, &BOWL, 0xE6E6E6, 0xFFFFFF, 0xA8A8B0, 0x8AB84A),
        (items::BOWL, &BOWL, wood, wood_l, wood_d, 0x3A2618),
        (items::SOY_BEANS, &POD, 0xC9B56C, 0xE2D9B0, 0x9C8A4A, 0),
        (items::EDAMAME, &POD, 0x6FAE4A, 0x9AD06A, 0x3E6A2A, 0),
        (items::TOFU, &INGOT, 0xF2EEE4, 0xFFFFFF, 0xC8BEA6, 0),
        (items::TAMAGOYAKI, &INGOT, 0xF2C84A, 0xFFE38A, 0xC8962A, 0),
        (items::NORI, &INGOT, 0x2A3A2A, 0x3E5A3E, 0x1A241A, 0),
        (items::RAW_BIRD, &MEAT, 0xF2B8A8, 0xFFD8CC, 0xC88878, 0),
        (items::YAKITORI, &MEAT, 0xC8843A, 0xE8B06A, 0x8A5422, 0),
        (items::RAW_FISH, &FISH, 0x6FA0C8, 0xB8D8F0, 0x3E6A94, 0),
        (items::COOKED_FISH, &FISH, 0xC8A070, 0xE8C89A, 0x8A6238, 0),
        (items::FUGU, &FISH, 0xE8C84A, 0xFFF0A0, 0xA8862A, 0),
        (items::SUSHI, &NIGIRI, 0xF08A6A, 0xFFB8A0, 0xC85A3E, 0),
        (items::DRIED_TEA, &DUST, 0x5A6A2A, 0x8A9A4A, 0x3A4A1A, 0),
        (items::KABOCHA_SLICE, &WEDGE, 0xF0A030, 0xFFD070, 0x2F6B3A, 0),
        (items::ROAST_KABOCHA, &WEDGE, 0xC87A22, 0xE8A84A, 0x4A3A1A, 0),
        (items::KABOCHA_SEEDS, &DUST, 0xE8E0C0, 0xFFF8E0, 0xB8AC84, 0),
        (items::FERTILIZER, &DUST, 0xE6E6E6, 0xFFFFFF, 0xA8A8B0, 0),
        (items::BENTO, &BENTO_BOX, 0xC8843A, 0xE8B06A, 0x8A5422, 0),
        (items::SHEARS, &SCISSORS, 0, 0, 0, 0),
        (items::FISHING_ROD, &ROD, 0, 0, 0, 0),
    ];
    for (id, map, x, light, dark, content) in more {
        Canvas::item(map, &item_pal(x, light, dark, content)).blit(img, items::icon(id), 255);
    }
    Canvas::item(&LEAF, LEAF_PAL).blit(img, items::icon(items::TEA_LEAF), 255);

    // ---- Outils de tamahagane (acier bleu nuit) et armes.
    let steel = [('k', 0x1E1830), ('h', 0x3A4A6A), ('l', 0x8FA3D8), ('s', 0x5A3A28), ('g', 0xE8B93A)];
    for (kind, map) in [&PICKAXE, &AXE, &SHOVEL, &KATANA, &HOE].into_iter().enumerate() {
        Canvas::item(map, &steel).blit(img, t::ITEM_TOOLS + 20 + kind as u16, 255);
    }
    let arms = [('k', 0x1E1830), ('h', 0xC8CED8), ('l', 0xF2F4F8), ('s', 0x86583F), ('g', 0xE8B93A), ('w', 0xF2EEE4), ('r', 0xC8382A)];
    let club = [('k', 0x1E1830), ('h', 0x565C67), ('l', 0x8A919C), ('s', 0x86583F), ('g', 0xC8CED8)];
    for (id, map) in [(items::YUMI, &YUMI), (items::YARI, &YARI), (items::NAGINATA, &NAGINATA), (items::TANTO, &TANTO), (items::KUNAI, &KUNAI), (items::SHIELD, &TATE)] {
        Canvas::item(map, &arms).blit(img, items::icon(id), 255);
    }
    Canvas::item(&TETSUBO, &club).blit(img, items::icon(items::TETSUBO), 255);
    Canvas::item(&INGOT, &item_pal(0x3A4A6A, 0x8FA3D8, 0x1E2A44, 0)).blit(img, items::icon(items::TAMAHAGANE), 255);
}
