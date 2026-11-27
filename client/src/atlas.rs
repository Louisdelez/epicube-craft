//! Atlas de textures genere par code, dans un style dessin anime : aplats de couleur,
//! taches peintes nettes, formes vectorielles lissees et contours a l'encre.
//! 32 tuiles de 64 pixels par ligne ; les modeles et la police occupent le bas.
use shared::blocks::t;
use shared::items;
use shared::util::hash;
use std::sync::OnceLock;

/// Cote de l'atlas en pixels.
pub const AT: usize = 2048;
/// Cote d'une tuile en pixels.
pub const TILE: usize = 64;
/// Zone de la police : cases de 64 px a partir de cette ligne.
const FONT_Y: usize = 1024;
const GLYPH_CELL: usize = 64;
/// Taille a laquelle la police est tracee dans l'atlas.
pub const FONT_PX: f32 = crate::font::HEIGHT as f32;


type C = [f32; 4];
const CLEAR: C = [0.0; 4];
/// Encre des contours.
const INK: C = [0.10, 0.08, 0.17, 1.0];
const PX: f32 = 1.0 / TILE as f32;

/// Coin haut-gauche d'une tuile, en coordonnees UV.
pub fn tile_uv(tile: u16) -> (f32, f32) {
    ((tile % 32) as f32 / 32.0, (tile / 32) as f32 / 32.0)
}

/// Un caractere de la police dans l'atlas. Tailles en pixels a la taille FONT_PX.
#[derive(Clone, Copy, Default)]
pub struct Glyph {
    pub uv: [f32; 4],
    pub w: f32,
    pub h: f32,
    /// Decalage du dessin par rapport au coin haut-gauche de la ligne.
    pub ox: f32,
    pub oy: f32,
    pub adv: f32,
}

static GLYPHS: OnceLock<Vec<Glyph>> = OnceLock::new();

/// Caractere a dessiner ; ce qui sort du Latin-1 devient un point d'interrogation.
pub fn glyph(ch: char) -> Glyph {
    let code = ch as u32;
    let i = if (32..256).contains(&code) { code - 32 } else { '?' as u32 - 32 };
    GLYPHS.get().and_then(|g| g.get(i as usize)).copied().unwrap_or_default()
}

/// Image RGBA de l'atlas, AT x AT pixels.
pub struct Img(pub Vec<u8>);

impl Img {
    pub fn set_px(&mut self, x: usize, y: usize, c: [u8; 4]) {
        let i = (y * AT + x) * 4;
        self.0[i..i + 4].copy_from_slice(&c);
    }

    /// Peint une tuile : `f(u, v)` recoit le centre de chaque pixel, de 0 a 1.
    fn tile(&mut self, tile: u16, f: impl Fn(f32, f32) -> C) {
        let (ox, oy) = ((tile as usize % 32) * TILE, (tile as usize / 32) * TILE);
        for y in 0..TILE {
            for x in 0..TILE {
                let c = f((x as f32 + 0.5) * PX, (y as f32 + 0.5) * PX);
                let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                self.set_px(ox + x, oy + y, [b(c[0]), b(c[1]), b(c[2]), b(c[3])]);
            }
        }
    }
}

fn rgb(r: u8, g: u8, b: u8) -> C {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

fn mix(a: C, b: C, k: f32) -> C {
    let k = k.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k, a[3] + (b[3] - a[3]) * k]
}

/// Eclaircit (k > 0) ou assombrit (k < 0) une couleur.
fn tone(c: C, k: f32) -> C {
    if k >= 0.0 {
        mix(c, [1.0, 1.0, 1.0, c[3]], k)
    } else {
        mix(c, [0.0, 0.0, 0.1, c[3]], -k)
    }
}

/// Pose `col` sur `dst` avec la couverture `a` (les tuiles a decoupe partent de CLEAR).
fn over(dst: C, col: C, a: f32) -> C {
    let a = (a * col[3]).clamp(0.0, 1.0);
    if dst[3] <= 0.001 {
        return [col[0], col[1], col[2], a];
    }
    [
        dst[0] + (col[0] - dst[0]) * a,
        dst[1] + (col[1] - dst[1]) * a,
        dst[2] + (col[2] - dst[2]) * a,
        dst[3] + a * (1.0 - dst[3]),
    ]
}

/// Couverture d'un pixel pour une distance signee (negative a l'interieur), lissee sur un pixel.
fn cov(sd: f32) -> f32 {
    (0.5 - sd * TILE as f32).clamp(0.0, 1.0)
}

/// Dessine une forme pleine avec son contour a l'encre.
fn shape(dst: C, sd: f32, fill: C, ink: f32) -> C {
    over(over(dst, INK, cov(sd - ink)), fill, cov(sd))
}

fn wrap(a: f32) -> f32 {
    let d = a.rem_euclid(1.0);
    d.min(1.0 - d)
}

/// Distance a un point, la tuile se repetant dans les deux sens.
fn pdist(u: f32, v: f32, cx: f32, cy: f32) -> f32 {
    wrap(u - cx).hypot(wrap(v - cy))
}

/// Rectangle aux coins arrondis (distance signee).
fn rbox(u: f32, v: f32, cx: f32, cy: f32, hx: f32, hy: f32, r: f32) -> f32 {
    let (dx, dy) = ((u - cx).abs() - hx + r, (v - cy).abs() - hy + r);
    dx.max(dy).min(0.0) + dx.max(0.0).hypot(dy.max(0.0)) - r
}

/// Distance a un segment, et position le long de celui-ci (0 au depart, 1 a l'arrivee).
fn seg(u: f32, v: f32, a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    let (px, py, dx, dy) = (u - a.0, v - a.1, b.0 - a.0, b.1 - a.1);
    let k = ((px * dx + py * dy) / (dx * dx + dy * dy).max(1e-6)).clamp(0.0, 1.0);
    ((px - dx * k).hypot(py - dy * k), k)
}

/// Brin effile : large a la base, pointu au sommet.
fn blade(u: f32, v: f32, base: (f32, f32), tip: (f32, f32), w: f32) -> f32 {
    let (d, k) = seg(u, v, base, tip);
    d - w * (1.0 - k * 0.9)
}

/// Bruit lisse qui se repete d'un bord a l'autre de la tuile, entre 0 et 1.
fn pnoise(u: f32, v: f32, freq: i32, seed: u32) -> f32 {
    let (x, y) = (u * freq as f32, v * freq as f32);
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let s = |k: f32| k * k * (3.0 - 2.0 * k);
    let (tx, ty) = (s(x - xi as f32), s(y - yi as f32));
    let l = |i: i32, j: i32| (hash(i.rem_euclid(freq), j.rem_euclid(freq), freq, seed) & 0xFFFF) as f32 / 65535.0;
    let a = l(xi, yi) + (l(xi + 1, yi) - l(xi, yi)) * tx;
    let b = l(xi, yi + 1) + (l(xi + 1, yi + 1) - l(xi, yi + 1)) * tx;
    a + (b - a) * ty
}

fn fbm(u: f32, v: f32, freq: i32, seed: u32) -> f32 {
    0.62 * pnoise(u, v, freq, seed) + 0.38 * pnoise(u, v, freq * 2, seed + 5)
}

/// Matiere peinte : cinq tons aux bords nets (ombre profonde, ombre, aplat, lumiere, eclat),
/// plus de petites mouchetures pour que la surface ne paraisse jamais lisse.
fn toon(u: f32, v: f32, seed: u32, base: C, freq: i32, shadow: f32, light: f32) -> C {
    let edge = |x: f32| (x / 0.035 + 0.5).clamp(0.0, 1.0);
    let n = fbm(u, v, freq, seed);
    let m = fbm(u + 0.37, v + 0.71, freq, seed + 9);
    let d = fbm(u + 0.11, v + 0.53, freq * 3, seed + 21);
    let mut c = mix(base, tone(base, -0.16), edge(shadow - n));
    c = mix(c, tone(base, -0.30), edge(shadow - 0.12 - n));
    c = mix(c, tone(base, 0.17), edge(m - light));
    c = mix(c, tone(base, 0.33), edge(m - light - 0.11));
    c = mix(c, tone(c, -0.13), edge(0.33 - d));
    mix(c, tone(c, 0.11), edge(d - 0.70))
}

/// Petites taches rondes semees sur une grille qui se repete d'un bord a l'autre.
/// `chance` : pourcentage de cases occupees.
fn specks(u: f32, v: f32, grid: i32, seed: u32, radius: f32, chance: u32) -> f32 {
    let (x, y) = (u * grid as f32, v * grid as f32);
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let mut best = 0.0f32;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (cx, cy) = (xi + dx, yi + dy);
            let h = hash(cx.rem_euclid(grid), cy.rem_euclid(grid), grid, seed);
            if h % 100 >= chance {
                continue;
            }
            let (jx, jy) = (((h >> 8) & 255) as f32 / 255.0, ((h >> 16) & 255) as f32 / 255.0);
            let r = radius * (0.6 + ((h >> 24) & 15) as f32 / 30.0);
            let dist = (x - cx as f32 - jx).hypot(y - cy as f32 - jy) / grid as f32;
            best = best.max(cov(dist - r));
        }
    }
    best
}

/// Traits courts et effiles, semes au hasard a l'interieur de la tuile (coups de pinceau).
#[allow(clippy::too_many_arguments)]
fn strokes(u: f32, v: f32, seed: u32, count: i32, len: f32, width: f32, angle: f32, spread: f32) -> f32 {
    let mut best = 0.0f32;
    for i in 0..count {
        let h = hash(i, 0, 0, seed);
        let rnd = |k: u32| ((h >> (k * 8)) & 255) as f32 / 255.0;
        let a = angle + (rnd(2) - 0.5) * spread;
        let l = len * (0.6 + rnd(3) * 0.8) * 0.5;
        let (x, y) = (0.06 + rnd(0) * 0.88, 0.06 + rnd(1) * 0.88);
        let end = |k: f32| ((x + a.cos() * l * k).clamp(0.01, 0.99), (y + a.sin() * l * k).clamp(0.01, 0.99));
        best = best.max(cov(blade(u, v, end(-1.0), end(1.0), width)));
    }
    best
}

/// Fissure en ligne brisee : distance au trait.
fn crack(u: f32, v: f32, seed: u32, start: (f32, f32), dir: f32, steps: i32) -> f32 {
    let (mut p, mut a, mut d) = (start, dir, 1.0f32);
    for i in 0..steps {
        a += ((hash(i, 1, 0, seed) & 255) as f32 / 255.0 - 0.5) * 1.5;
        let q = ((p.0 + a.cos() * 0.11).clamp(0.02, 0.98), (p.1 + a.sin() * 0.11).clamp(0.02, 0.98));
        d = d.min(seg(u, v, p, q).0);
        p = q;
    }
    d
}

/// Feuillage : de vraies feuilles ovales, de trois verts, sur un fond sombre.
fn foliage(u: f32, v: f32) -> C {
    let mut c = toon(u, v, 6, rgb(34, 100, 52), 4, 0.5, 0.95);
    let grid = 5;
    let (x, y) = (u * grid as f32, v * grid as f32);
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (cx, cy) = (xi + dx, yi + dy);
            for leaf in 0..2 {
                let h = hash(cx.rem_euclid(grid), cy.rem_euclid(grid), leaf, 606);
                let rnd = |k: u32| ((h >> (k * 8)) & 255) as f32 / 255.0;
                let (lx, ly) = (x - cx as f32 - rnd(0), y - cy as f32 - rnd(1));
                let (sa, ca) = (rnd(2) * 6.28).sin_cos();
                let (px, py) = (lx * ca + ly * sa, -lx * sa + ly * ca);
                // Ovale allonge, avec sa nervure centrale.
                let sd = ((px / 0.52).hypot(py / 0.24) - 1.0) * 0.24 / grid as f32;
                let col = [rgb(60, 150, 68), rgb(92, 186, 82), rgb(46, 126, 60)][(h >> 24) as usize % 3];
                c = shape(c, sd, col, 0.008);
                c = over(c, tone(col, 0.22), cov(py.abs() / grid as f32 - 0.004) * cov(sd + 0.012));
            }
        }
    }
    c
}

fn grass(u: f32, v: f32) -> C {
    let base = rgb(100, 190, 76);
    let mut c = toon(u, v, 1, base, 4, 0.40, 0.66);
    // Brins clairs et sombres, puis quelques fleurettes.
    c = over(c, tone(base, 0.30), strokes(u, v, 71, 22, 0.13, 0.012, -1.4, 0.9));
    c = over(c, tone(base, -0.30), strokes(u, v, 72, 16, 0.11, 0.011, -1.7, 0.9));
    c = over(c, rgb(255, 240, 150), specks(u, v, 6, 73, 0.012, 14));
    c
}

fn dirt(u: f32, v: f32) -> C {
    let base = rgb(152, 106, 72);
    let mut c = toon(u, v, 2, base, 4, 0.38, 0.70);
    // Cailloux clairs cernes, grains sombres et radicelles.
    let pebble = specks(u, v, 5, 81, 0.034, 55);
    c = over(c, tone(base, -0.36), specks(u, v, 5, 81, 0.046, 55));
    c = over(c, rgb(196, 152, 112), pebble);
    c = over(c, tone(base, -0.34), specks(u, v, 9, 82, 0.012, 60));
    c = over(c, tone(base, 0.22), strokes(u, v, 83, 6, 0.16, 0.007, 0.4, 2.4));
    c
}

fn stone(u: f32, v: f32) -> C {
    let base = rgb(156, 162, 178);
    let mut c = toon(u, v, 3, base, 3, 0.42, 0.68);
    // Fissures (trait sombre double d'un filet clair), eclats et paillettes.
    for (seed, start, dir) in [(91, (0.12, 0.2), 0.5), (92, (0.85, 0.3), 2.4), (93, (0.4, 0.9), -1.2)] {
        let d = crack(u, v, seed, start, dir, 6);
        c = over(c, tone(base, 0.3), cov(crack(u + 0.012, v + 0.012, seed, start, dir, 6) - 0.006));
        c = over(c, tone(base, -0.45), cov(d - 0.007));
    }
    c = over(c, tone(base, -0.24), specks(u, v, 7, 94, 0.016, 45));
    c = over(c, tone(base, 0.34), specks(u, v, 8, 95, 0.009, 35));
    c
}

fn snow(u: f32, v: f32) -> C {
    let mut c = toon(u, v, 4, rgb(236, 243, 255), 3, 0.40, 0.84);
    // Congeres bleutees et cristaux qui scintillent.
    c = over(c, rgb(206, 222, 250), strokes(u, v, 96, 7, 0.3, 0.012, 0.15, 0.5));
    over(c, rgb(255, 255, 255), specks(u, v, 9, 97, 0.01, 40))
}

/// Bord superieur ondule qui deborde sur le cote d'un bloc (herbe, neige).
fn cap(u: f32, v: f32, body: C, top: C) -> C {
    let tau = std::f32::consts::TAU;
    let edge = 0.27 + 0.055 * (u * tau * 3.0).sin() + 0.03 * (u * tau * 7.0 + 1.0).sin();
    let c = over(body, tone(top, -0.38), cov(v - edge - 0.035));
    over(c, top, cov(v - edge))
}

fn planks(u: f32, v: f32) -> C {
    let tau = std::f32::consts::TAU;
    let board = (v * 4.0).floor();
    let bi = board as i32;
    let bv = (v * 4.0).fract();
    let base = tone(rgb(220, 166, 100), ((hash(bi, 0, 0, 61) % 5) as f32 - 2.0) * 0.04);
    let mut c = base;
    // Veinage : trois lignes ondulees par planche, une claire entre deux sombres.
    for k in 0..3 {
        let y = 0.2 + k as f32 * 0.27 + 0.06 * (u * tau * (1.0 + k as f32) + board * 1.7).sin();
        let col = if k == 1 { tone(base, 0.18) } else { tone(base, -0.16) };
        c = over(c, col, cov((bv - y).abs() / 4.0 - 0.0035));
    }
    // Un noeud sur une planche sur deux.
    if hash(bi, 2, 0, 61) % 2 == 0 {
        let kx = 0.2 + (hash(bi, 1, 0, 61) % 60) as f32 / 100.0;
        let kd = (u - kx).hypot((bv - 0.5) / 4.0 * 1.7);
        c = over(c, tone(base, -0.3), cov(kd - 0.04));
        c = over(c, tone(base, -0.5), cov((kd - 0.024).abs() - 0.005));
    }
    let ink = rgb(112, 70, 40);
    let joint = wrap(u - board * 0.31);
    c = over(c, ink, cov(wrap(v * 4.0) / 4.0 - 0.014));
    c = over(c, ink, cov(joint - 0.012));
    // Clous de part et d'autre du joint.
    let nail = ((joint - 0.05).abs()).hypot((bv - 0.5).abs() / 4.0 - 0.07);
    over(c, rgb(70, 64, 76), cov(nail - 0.011))
}

/// Mur de pierres ou de briques arrondies : `rows` rangees decalees une sur deux.
fn masonry(u: f32, v: f32, rows: f32, cols: f32, mortar: C, body: C, seed: u32) -> C {
    let row = (v * rows).floor();
    let x = u * cols + if row as i32 % 2 == 0 { 0.0 } else { 0.5 };
    let (cx, cy) = (x.floor() + 0.5, row + 0.5);
    let sd = rbox(x, v * rows, cx, cy, 0.44, 0.40, 0.16) / rows;
    let k = (hash(cx.rem_euclid(cols) as i32, row as i32, 0, seed) % 5) as f32 * 0.035 - 0.07;
    let stone = tone(body, k + (fbm(u, v, 6, seed + 3) - 0.5) * 0.24);
    // Reflet en haut de chaque pierre, ombre en bas, eclats et joints mouchetes.
    let fy = (v * rows).fract();
    let mut lit = if fy < 0.3 { tone(stone, 0.17) } else if fy > 0.74 { tone(stone, -0.14) } else { stone };
    lit = over(lit, tone(body, -0.3), specks(u, v, 8, seed + 4, 0.012, 40));
    lit = over(lit, tone(body, 0.3), specks(u, v, 9, seed + 5, 0.008, 30));
    let joint = over(mortar, tone(mortar, 0.25), specks(u, v, 10, seed + 6, 0.01, 50));
    over(joint, lit, cov(sd))
}

/// Minerai : trois gemmes taillees dans la pierre.
fn ore(u: f32, v: f32, gem: C) -> C {
    let mut c = stone(u, v);
    for (cx, cy, r) in [(0.30, 0.30, 0.15), (0.70, 0.52, 0.17), (0.36, 0.76, 0.13)] {
        let sd = (u - cx).abs() + (v - cy).abs() - r;
        c = shape(c, sd, gem, 0.03);
        c = over(c, tone(gem, 0.45), cov(sd + r * 0.35) * if v < cy && u < cx { 1.0 } else { 0.0 });
        c = over(c, tone(gem, -0.3), cov(sd + r * 0.5) * if v > cy && u > cx { 1.0 } else { 0.0 });
        // Etincelle en croix.
        let (sx, sy) = ((u - cx + r * 0.3).abs(), (v - cy + r * 0.3).abs());
        c = over(c, [1.0; 4], cov(sx.min(sy) - 0.006) * cov(sx.max(sy) - r * 0.42));
    }
    c
}

/// Cadre plein sur le bord de la tuile.
fn frame(u: f32, v: f32, w: f32) -> f32 {
    cov(-rbox(u, v, 0.5, 0.5, 0.5 - w, 0.5 - w, 0.06))
}

/// Motif 8x8 transforme en autocollant : formes arrondies, contour a l'encre.
fn sprite(img: &mut Img, tile: u16, rows: [&str; 8], pal: &[(char, [u8; 3])]) {
    let cell = |x: i32, y: i32| -> Option<C> {
        if !(0..8).contains(&x) || !(0..8).contains(&y) {
            return None;
        }
        let ch = rows[y as usize].as_bytes()[x as usize] as char;
        pal.iter().find(|(p, _)| *p == ch).map(|(_, c)| rgb(c[0], c[1], c[2]))
    };
    img.tile(tile, |u, v| {
        // Champ d'occupation interpole entre les centres des cases, et couleur majoritaire.
        let (x, y) = (u * 8.0 - 0.5, v * 8.0 - 0.5);
        let (xi, yi) = (x.floor() as i32, y.floor() as i32);
        let (fx, fy) = (x - xi as f32, y - yi as f32);
        let (mut field, mut best, mut col) = (0.0f32, 0.0f32, CLEAR);
        for (dx, dy, w) in [(0, 0, (1.0 - fx) * (1.0 - fy)), (1, 0, fx * (1.0 - fy)), (0, 1, (1.0 - fx) * fy), (1, 1, fx * fy)] {
            if let Some(c) = cell(xi + dx, yi + dy) {
                field += w;
                if w > best {
                    best = w;
                    col = c;
                }
            }
        }
        let aa = 0.06;
        let ink = ((field - 0.2) / aa).clamp(0.0, 1.0);
        let fill = ((field - 0.5) / aa).clamp(0.0, 1.0);
        // Reflet discret en haut a gauche.
        let col = if fy + fx < 0.7 { tone(col, 0.12) } else { col };
        over(over(CLEAR, INK, ink), col, fill)
    });
}

#[rustfmt::skip]
const TOOLS: [[&str; 8]; 5] = [
    [".hhhhh..", ".....sh.", "....s.h.", "...s..h.", "..s...h.", ".s......", "s.......", "........"],
    ["....hh..", "...hhhh.", "...hhsh.", "....s.h.", "...s....", "..s.....", ".s......", "s......."],
    [".....hhh", "....hhhh", "....hhhh", "...shhh.", "..s.....", ".s......", "s.......", "........"],
    [".......h", "......hh", ".....hh.", "....hh..", ".g.hh...", "..gh....", ".sgg....", "s......."],
    ["...hhhh.", ".....sh.", "....s...", "...s....", "..s.....", ".s......", "s.......", "........"],
];
#[rustfmt::skip]
const STICK_S: [&str; 8] = ["........", "......s.", ".....s..", "....s...", "...s....", "..s.....", ".s......", "........"];
#[rustfmt::skip]
const LUMP: [&str; 8] = ["........", "..xxx...", ".xxxxx..", ".xxXxxx.", ".xxxxXx.", "..xxxx..", "...xx...", "........"];
#[rustfmt::skip]
const INGOT: [&str; 8] = ["........", "........", "..xxxxx.", ".xXXXXx.", ".xxxxxx.", ".xxxxx..", "........", "........"];
#[rustfmt::skip]
const GEM: [&str; 8] = ["........", "..xxxx..", ".xXXxxx.", ".xXxxxx.", "..xxxx..", "...xx...", "........", "........"];
#[rustfmt::skip]
const DUST: [&str; 8] = ["........", "..x..x..", ".x.xx...", "..xXx.x.", ".xxXxx..", "..x.x.x.", "...x....", "........"];
#[rustfmt::skip]
const ROUND: [&str; 8] = ["........", "..xxxx..", ".xXXxxx.", ".xXxxxx.", ".xxxxxx.", ".xxxxxx.", "..xxxx..", "........"];
#[rustfmt::skip]
const HEART_S: [&str; 8] = ["........", ".xx..xx.", "xXXxxxxx", "xXxxxxxx", ".xxxxxx.", "..xxxx..", "...xx...", "........"];
#[rustfmt::skip]
const WHEAT_S: [&str; 8] = ["......x.", ".....xx.", "...x.x..", "..xxx...", "..xX....", ".X.X....", "X..X....", "........"];
#[rustfmt::skip]
const BREAD_S: [&str; 8] = ["........", "........", "..xxxx..", ".xXxXxx.", ".xxxxxx.", ".xxxxxx.", "..xxxx..", "........"];
#[rustfmt::skip]
const CARROT_S: [&str; 8] = ["......X.", ".....XX.", "....xX..", "...xxx..", "..xxx...", ".xxx....", ".x......", "........"];
#[rustfmt::skip]
const MEAT: [&str; 8] = ["........", "..xxx...", ".xxXxx..", ".xXXXxw.", ".xxXxxww", "..xxx.w.", "........", "........"];
#[rustfmt::skip]
const HIDE: [&str; 8] = ["........", ".x.xx.x.", ".xxxxxx.", "..xxxx..", "..xxxx..", ".xxxxxx.", ".x.xx.x.", "........"];
#[rustfmt::skip]
const FEATHER_S: [&str; 8] = ["......x.", ".....xx.", "....xxx.", "...xxx..", "..xxx...", ".sx.....", "s.......", "........"];
#[rustfmt::skip]
const STRING_S: [&str; 8] = ["........", "..xx....", ".x..x...", ".x..x.x.", "..xx.x..", "....x...", "...x....", "........"];
#[rustfmt::skip]
const BONE_S: [&str; 8] = ["......xx", ".....xx.", "....x...", "...x....", "..x.....", ".xx.....", "xx......", "........"];
#[rustfmt::skip]
const BUCKET_S: [&str; 8] = ["........", ".x....x.", ".xXXXXx.", ".xXXXXx.", ".xxxxxx.", "..xxxx..", "..xxxx..", "........"];
#[rustfmt::skip]
const LIGHTER_S: [&str; 8] = ["........", "..xx....", ".x..x...", ".x..x.XX", "..xx.XX.", "....XX..", "........", "........"];
#[rustfmt::skip]
const ARROW_S: [&str; 8] = ["......xx", ".....sxx", "....s...", "...s....", "..s.....", "ws......", "ww......", "........"];
#[rustfmt::skip]
const DRUM: [&str; 8] = ["........", "...xxx..", "..xxxxx.", "..xxxxx.", "..xxxx..", ".wxx....", "ww......", "........"];

/// Trace la police a points dans l'atlas, a sa taille native de 16 pixels, sans lissage,
/// et remplit la table des caracteres.
fn bake_font(img: &mut Img) {
    use crate::font;
    // Chaque point de la police occupe un carre de DOT pixels dans l'atlas : la lecture au plus proche
    // voisin tombe toujours au coeur d'un point, jamais sur une frontiere.
    const DOT: usize = 4;
    let fallback = font::shape('?');
    let mut glyphs = Vec::with_capacity(224);
    for code in 32u32..256 {
        let ch = char::from_u32(code).unwrap_or('?');
        let k = (code - 32) as usize;
        let (cx, cy) = ((k % 32) * GLYPH_CELL + 1, FONT_Y + (k / 32) * GLYPH_CELL + 1);
        for y in 0..GLYPH_CELL - 2 {
            for x in 0..GLYPH_CELL - 2 {
                img.set_px(cx + x, cy + y, [255, 255, 255, 0]);
            }
        }
        let Some(shape) = font::shape(ch).or_else(|| font::shape('?')).or_else(|| fallback.as_ref().map(|f| font::Shape { rows: f.rows.clone(), top: f.top })) else {
            glyphs.push(Glyph::default());
            continue;
        };
        let (w, h) = (font::width(&shape), shape.rows.len());
        for (y, row) in shape.rows.iter().enumerate() {
            for (x, dot) in row.chars().enumerate() {
                if dot == '#' {
                    for py in 0..DOT {
                        for px in 0..DOT {
                            img.set_px(cx + x * DOT + px, cy + y * DOT + py, [255, 255, 255, 255]);
                        }
                    }
                }
            }
        }
        let s = AT as f32;
        glyphs.push(Glyph {
            uv: [cx as f32 / s, cy as f32 / s, (cx + w * DOT) as f32 / s, (cy + h * DOT) as f32 / s],
            w: w as f32,
            h: h as f32,
            ox: 0.0,
            oy: shape.top as f32,
            // Un point d'espace entre deux lettres.
            adv: w as f32 + 1.0,
        });
    }
    let _ = GLYPHS.set(glyphs);
}

/// Blocs pleins.
fn paint_blocks(img: &mut Img) {
    let tau = std::f32::consts::TAU;
    img.tile(t::GRASS_TOP, grass);
    img.tile(t::DIRT, dirt);
    img.tile(t::GRASS_SIDE, |u, v| cap(u, v, dirt(u, v), grass(u, v)));
    img.tile(t::STONE, stone);
    img.tile(t::COBBLE, |u, v| masonry(u, v, 3.0, 2.0, rgb(84, 88, 104), rgb(166, 170, 184), 7));
    img.tile(t::BRICK, |u, v| masonry(u, v, 4.0, 2.0, rgb(232, 222, 206), rgb(208, 96, 80), 8));
    img.tile(t::SAND, |u, v| {
        let base = rgb(246, 222, 150);
        let mut c = toon(u, v, 5, base, 5, 0.34, 0.72);
        // Rides du vent et grains.
        let ripple = (v * tau * 5.0 + (u * tau * 2.0).sin() * 0.9).sin();
        c = mix(c, tone(base, -0.16), ((ripple - 0.72) / 0.1).clamp(0.0, 1.0));
        c = over(c, tone(base, -0.3), specks(u, v, 11, 101, 0.008, 55));
        over(c, [1.0; 4], specks(u, v, 10, 102, 0.006, 30))
    });
    img.tile(t::GRAVEL, |u, v| {
        // Galets ronds de plusieurs gris.
        let (x, y) = (u * 4.0, v * 4.0);
        let (ix, iy) = (x.floor() as i32, y.floor() as i32);
        let h = hash(ix, iy, 0, 21);
        let (jx, jy) = ((h & 255) as f32 / 255.0 * 0.3 + 0.35, ((h >> 8) & 255) as f32 / 255.0 * 0.3 + 0.35);
        let d = (x.fract() - jx).hypot(y.fract() - jy) / 4.0;
        let body = tone(rgb(170, 166, 170), ((h >> 16) % 5) as f32 * 0.06 - 0.12);
        shape(rgb(96, 94, 104), d - 0.085, body, 0.012)
    });
    img.tile(t::LOG_SIDE, |u, v| {
        // Ecorce en plaques verticales, crevasses sombres et stries.
        let base = rgb(134, 92, 58);
        let mut c = masonry(v, u, 5.0, 2.0, rgb(70, 44, 30), base, 31);
        c = over(c, tone(base, 0.22), strokes(u, v, 111, 14, 0.2, 0.007, 1.57, 0.3));
        c = over(c, tone(base, -0.4), strokes(u, v, 112, 10, 0.16, 0.006, 1.57, 0.3));
        c
    });
    img.tile(t::LOG_TOP, |u, v| {
        let r = (u - 0.5).hypot(v - 0.5);
        let a = (v - 0.5).atan2(u - 0.5);
        // Cernes serres, legerement irreguliers, et fentes de sechage.
        let wob = r * 58.0 + (a * 3.0).sin() * 0.8;
        let rings = if wob.sin() > 0.1 { rgb(228, 186, 126) } else { rgb(190, 144, 92) };
        let mut c = over(rgb(96, 64, 40), rings, cov(r - 0.40));
        for k in 0..5 {
            let ang = k as f32 * 1.3 + 0.4;
            let d = seg(u, v, (0.5 + ang.cos() * 0.08, 0.5 + ang.sin() * 0.08), (0.5 + ang.cos() * 0.34, 0.5 + ang.sin() * 0.34)).0;
            c = over(c, rgb(140, 98, 60), cov(d - 0.006));
        }
        c = over(c, rgb(70, 44, 30), cov((r - 0.41).abs() - 0.012));
        over(c, rgb(150, 104, 66), cov(r - 0.04))
    });
    img.tile(t::LEAVES, foliage);
    img.tile(t::PLANKS, planks);
    img.tile(t::BEDROCK, |u, v| toon(u, v, 9, rgb(70, 70, 86), 3, 0.5, 0.74));
    img.tile(t::WATER, |u, v| {
        // Deux jeux de vaguelettes et de l'ecume.
        let w1 = (v * tau * 3.0 + (u * tau).sin() * 1.3).sin();
        let w2 = (v * tau * 5.0 - (u * tau * 2.0).sin() * 0.9 + 1.7).sin();
        let mut c = mix(rgb(46, 132, 232), rgb(34, 108, 214), fbm(u, v, 3, 121));
        c = mix(c, rgb(150, 212, 255), ((w1 - 0.78) / 0.05).clamp(0.0, 1.0));
        c = mix(c, rgb(104, 180, 250), ((w2 - 0.86) / 0.05).clamp(0.0, 1.0));
        c = over(c, [1.0; 4], specks(u, v, 7, 122, 0.012, 22));
        [c[0], c[1], c[2], 0.76]
    });
    img.tile(t::LAVA, |u, v| {
        let base = rgb(255, 122, 30);
        let mut c = toon(u, v, 10, base, 3, 0.42, 0.62);
        c = mix(c, rgb(255, 232, 110), ((fbm(u + 0.2, v + 0.4, 3, 44) - 0.66) / 0.04).clamp(0.0, 1.0));
        // Croute sombre qui se craquelle.
        c = over(c, rgb(120, 40, 24), specks(u, v, 4, 123, 0.05, 45));
        over(c, rgb(255, 250, 200), specks(u, v, 8, 124, 0.009, 30))
    });
    img.tile(t::COAL_ORE, |u, v| ore(u, v, rgb(52, 52, 66)));
    img.tile(t::IRON_ORE, |u, v| ore(u, v, rgb(232, 172, 130)));
    img.tile(t::GOLD_ORE, |u, v| ore(u, v, rgb(255, 214, 64)));
    img.tile(t::DIAMOND_ORE, |u, v| ore(u, v, rgb(92, 232, 236)));
    img.tile(t::VOLT_ORE, |u, v| ore(u, v, rgb(170, 100, 255)));
    img.tile(t::GLASS, |u, v| {
        let mut c = over(CLEAR, rgb(214, 240, 255), frame(u, v, 0.07));
        // Reflets en diagonale.
        for (off, w) in [(0.62, 0.035), (0.76, 0.018)] {
            let d = ((u + v) - off).abs() / 1.414;
            let inside = (0.12..0.5).contains(&u);
            c = over(c, [1.0, 1.0, 1.0, 0.95], cov(d - w) * if inside { 1.0 } else { 0.0 });
        }
        c
    });
    img.tile(t::SNOW, snow);
    img.tile(t::SNOW_SIDE, |u, v| cap(u, v, dirt(u, v), snow(u, v)));
    img.tile(t::ICE, |u, v| {
        let mut c = toon(u, v, 11, rgb(170, 216, 255), 2, 0.4, 0.7);
        let d = (wrap(u + v - 0.3)).min(wrap(u + v - 0.42) * 2.0) / 1.414;
        c = over(c, [1.0, 1.0, 1.0, 0.9], cov(d - 0.016));
        c
    });
    img.tile(t::CRAFT_TOP, |u, v| {
        let mut c = planks(u, v);
        c = over(c, rgb(112, 70, 40), frame(u, v, 0.08));
        // Quadrillage de l'etabli.
        let g = wrap(u * 3.0).min(wrap(v * 3.0)) / 3.0;
        over(c, rgb(112, 70, 40), cov(g - 0.012))
    });
    img.tile(t::CRAFT_SIDE, |u, v| {
        let mut c = planks(u, v);
        c = over(c, rgb(112, 70, 40), frame(u, v, 0.08));
        // Marteau et scie stylises.
        c = shape(c, rbox(u, v, 0.32, 0.56, 0.035, 0.22, 0.02), rgb(170, 120, 70), 0.02);
        c = shape(c, rbox(u, v, 0.32, 0.32, 0.12, 0.07, 0.03), rgb(150, 156, 172), 0.02);
        c = shape(c, rbox(u, v, 0.68, 0.5, 0.07, 0.26, 0.03), rgb(190, 196, 210), 0.02);
        c
    });
    let furnace = |u: f32, v: f32, mouth: Option<bool>| {
        let mut c = masonry(u, v, 3.0, 2.0, rgb(70, 72, 86), rgb(146, 150, 166), 12);
        c = over(c, rgb(70, 72, 86), frame(u, v, 0.05));
        if let Some(lit) = mouth {
            let sd = rbox(u, v, 0.5, 0.66, 0.26, 0.17, 0.08);
            c = shape(c, sd, rgb(36, 34, 46), 0.025);
            if lit {
                // Flammes : orange puis jaune au coeur.
                let f = v - 0.62 - 0.07 * (u * tau * 4.0).sin().abs();
                c = over(c, rgb(255, 150, 40), cov(sd + 0.03) * cov(-f));
                c = over(c, rgb(255, 232, 110), cov(sd + 0.06) * cov(-f + 0.07));
            }
        }
        c
    };
    img.tile(t::FURNACE_SIDE, move |u, v| furnace(u, v, None));
    img.tile(t::FURNACE_FRONT, move |u, v| furnace(u, v, Some(false)));
    img.tile(t::FURNACE_ON, move |u, v| furnace(u, v, Some(true)));
    img.tile(t::CHEST, |u, v| {
        let mut c = tone(rgb(206, 140, 70), if v < 0.36 { 0.1 } else { 0.0 });
        c = over(c, rgb(120, 76, 40), frame(u, v, 0.07));
        c = over(c, rgb(120, 76, 40), cov((v - 0.36).abs() - 0.022));
        shape(c, rbox(u, v, 0.5, 0.38, 0.07, 0.10, 0.03), rgb(232, 236, 244), 0.022)
    });
    img.tile(t::OBSIDIAN, |u, v| toon(u, v, 13, rgb(58, 40, 96), 3, 0.42, 0.74));
    img.tile(t::FARMLAND, |u, v| {
        let base = rgb(118, 78, 52);
        let mut c = toon(u, v, 14, base, 4, 0.36, 0.9);
        // Sillons : creux sombre, crete eclairee, mottes.
        let furrow = (u * tau * 4.0).sin();
        c = mix(c, rgb(84, 54, 38), ((furrow - 0.45) / 0.1).clamp(0.0, 1.0));
        c = mix(c, tone(base, 0.2), ((-furrow - 0.8) / 0.1).clamp(0.0, 1.0));
        over(c, tone(base, 0.26), specks(u, v, 8, 131, 0.012, 40))
    });
    let lamp = |u: f32, v: f32, on: bool| {
        let glass = if on { rgb(255, 236, 140) } else { rgb(128, 104, 78) };
        let glow = if on { tone(glass, 0.5) } else { tone(glass, 0.12) };
        let mut c = mix(glass, glow, 1.0 - ((u - 0.5).hypot(v - 0.5) / 0.34).min(1.0));
        let bars = (u - 0.5).abs().min((v - 0.5).abs());
        c = over(c, rgb(84, 60, 44), cov(bars - 0.025));
        over(c, rgb(84, 60, 44), frame(u, v, 0.08))
    };
    img.tile(t::LAMP_OFF, move |u, v| lamp(u, v, false));
    img.tile(t::LAMP_ON, move |u, v| lamp(u, v, true));
    img.tile(t::EXPLOSIVE, |u, v| {
        let mut c = tone(rgb(232, 66, 60), if v < 0.2 { 0.16 } else { 0.0 });
        c = over(c, rgb(250, 246, 232), cov((v - 0.5).abs() - 0.15));
        // Eclair noir sur la bande.
        let z = seg(u, v, (0.38, 0.38), (0.56, 0.5)).0.min(seg(u, v, (0.56, 0.5), (0.44, 0.5)).0).min(seg(u, v, (0.44, 0.5), (0.62, 0.62)).0);
        c = over(c, INK, cov(z - 0.03));
        over(c, INK, cov((v - 0.5).abs() - 0.15).min(cov(0.135 - (v - 0.5).abs())))
    });
    img.tile(t::BED, |u, v| {
        let mut c = rgb(226, 74, 86);
        c = over(c, tone(c, 0.2), cov((v - 0.3).abs() - 0.04));
        c = shape(c, rbox(u, v, 0.17, 0.5, 0.13, 0.36, 0.08), rgb(250, 250, 255), 0.022);
        c
    });
    img.tile(t::WOOL, |u, v| toon(u, v, 15, rgb(248, 246, 240), 5, 0.34, 0.95));
    img.tile(t::ABYSS_ROCK, |u, v| toon(u, v, 16, rgb(150, 62, 70), 3, 0.44, 0.7));
    img.tile(t::EMBER, |u, v| {
        let c = toon(u, v, 17, rgb(255, 160, 50), 3, 0.38, 0.6);
        mix(c, rgb(255, 244, 170), ((fbm(u, v, 4, 51) - 0.68) / 0.04).clamp(0.0, 1.0))
    });
    img.tile(t::ABYSS_PORTAL, |u, v| {
        // Spirale mauve dans un cadre d'obsidienne.
        let (dx, dy) = (u - 0.5, v - 0.5);
        let sw = (dy.atan2(dx) * 2.0 + dx.hypot(dy) * 22.0).sin();
        let c = mix(rgb(128, 44, 200), rgb(236, 130, 255), ((sw - 0.1) / 0.1).clamp(0.0, 1.0));
        over(c, rgb(44, 30, 76), frame(u, v, 0.09))
    });
    img.tile(t::ZENITH_STONE, |u, v| toon(u, v, 18, rgb(238, 230, 190), 3, 0.42, 0.72));
    img.tile(t::ZENITH_PORTAL, |u, v| {
        let mut c = mix(rgb(20, 28, 70), rgb(52, 40, 110), v);
        for (cx, cy, r) in [(0.3, 0.3, 0.03), (0.68, 0.24, 0.02), (0.56, 0.6, 0.035), (0.26, 0.72, 0.02), (0.76, 0.78, 0.028)] {
            c = over(c, rgb(240, 250, 255), cov((u - cx).hypot(v - cy) - r));
        }
        over(c, rgb(80, 220, 200), frame(u, v, 0.09))
    });
    img.tile(t::CACTUS, |u, v| {
        let mut c = rgb(92, 184, 92);
        let rib = wrap(u * 4.0) / 4.0;
        c = over(c, rgb(60, 140, 72), cov(rib - 0.016));
        // Epines blanches.
        let d = pdist(u * 4.0, v * 4.0, 0.5, 0.5) / 4.0;
        over(c, rgb(250, 252, 240), cov(d - 0.016))
    });
    img.tile(t::IRON_BLOCK, |u, v| {
        let mut c = rgb(226, 230, 240);
        c = over(c, [1.0, 1.0, 1.0, 1.0], cov(((u + v) - 0.7).abs() / 1.414 - 0.05));
        over(c, rgb(160, 166, 184), frame(u, v, 0.07))
    });
    img.tile(t::DOOR, |u, v| {
        let mut c = planks(u, v);
        c = over(c, rgb(112, 70, 40), frame(u, v, 0.07));
        for cy in [0.3, 0.7] {
            c = shape(c, rbox(u, v, 0.5, cy, 0.28, 0.14, 0.04), rgb(190, 136, 80), 0.02);
        }
        shape(c, (u - 0.82).hypot(v - 0.5) - 0.045, rgb(244, 220, 110), 0.02)
    });
    img.tile(t::WHITE, |_, _| [1.0; 4]);
}

/// Plantes, torches et autres tuiles a decoupe.
fn paint_cutouts(img: &mut Img) {
    let tau = std::f32::consts::TAU;
    for s in 0..4u16 {
        let k = s as f32;
        // Ble : brins de plus en plus hauts, dores et garnis d'epis au dernier stade.
        img.tile(t::WHEAT0 + s, move |u, v| {
            let col = mix(rgb(96, 190, 80), rgb(240, 200, 80), ((k - 1.0) / 2.0).clamp(0.0, 1.0));
            let mut c = CLEAR;
            for i in 0..5 {
                let x = 0.14 + i as f32 * 0.18;
                let top = 0.78 - k * 0.22 - (i % 2) as f32 * 0.05;
                let lean = (i as f32 - 2.0) * 0.03;
                c = shape(c, blade(u, v, (x, 1.02), (x + lean, top), 0.04), col, 0.02);
                if s == 3 {
                    c = shape(c, rbox(u, v, x + lean, top + 0.06, 0.035, 0.09, 0.03), rgb(250, 220, 110), 0.018);
                }
            }
            c
        });
        // Carottes : touffe de fanes, puis le haut orange de la racine.
        img.tile(t::CARROT0 + s, move |u, v| {
            let mut c = CLEAR;
            for i in 0..4 {
                let x = 0.2 + i as f32 * 0.2;
                for lean in [-0.09, 0.0, 0.09] {
                    let top = 0.86 - k * 0.14;
                    c = shape(c, blade(u, v, (x, 1.02), (x + lean, top), 0.03), rgb(84, 180, 84), 0.018);
                }
                if s == 3 {
                    c = shape(c, rbox(u, v, x, 0.98, 0.055, 0.07, 0.04), rgb(255, 146, 50), 0.018);
                }
            }
            c
        });
    }
    img.tile(t::SAPLING, |u, v| {
        let mut c = shape(CLEAR, rbox(u, v, 0.5, 0.8, 0.03, 0.22, 0.02), rgb(140, 96, 60), 0.02);
        for (cx, cy, r) in [(0.36, 0.5, 0.15), (0.64, 0.46, 0.16), (0.5, 0.3, 0.17)] {
            c = shape(c, (u - cx).hypot(v - cy) - r, rgb(84, 184, 84), 0.022);
            c = over(c, rgb(150, 226, 120), cov((u - cx + 0.04).hypot(v - cy + 0.05) - r * 0.4));
        }
        c
    });
    let torch = |u: f32, v: f32, head: C, core: C| {
        let mut c = shape(CLEAR, rbox(u, v, 0.5, 0.72, 0.05, 0.28, 0.03), rgb(150, 104, 64), 0.022);
        // Flamme en goutte.
        let flame = ((u - 0.5).hypot(v - 0.34) - 0.13).min(blade(u, v, (0.5, 0.34), (0.5, 0.06), 0.12));
        c = shape(c, flame, head, 0.022);
        over(c, core, cov((u - 0.5).hypot(v - 0.36) - 0.06))
    };
    img.tile(t::TORCH, move |u, v| torch(u, v, rgb(255, 160, 50), rgb(255, 240, 150)));
    img.tile(t::SIGTORCH_ON, move |u, v| torch(u, v, rgb(190, 110, 255), rgb(240, 214, 255)));
    img.tile(t::SIGTORCH_OFF, move |u, v| torch(u, v, rgb(84, 66, 110), rgb(110, 92, 136)));
    img.tile(t::WIRE, |u, v| {
        // Croix aux bouts arrondis ; la couleur vient du signal.
        let d = rbox(u, v, 0.5, 0.5, 0.5, 0.07, 0.06).min(rbox(u, v, 0.5, 0.5, 0.07, 0.5, 0.06));
        over(over(CLEAR, [0.3, 0.3, 0.3, 1.0], cov(d - 0.03)), [1.0; 4], cov(d))
    });
    img.tile(t::LEVER, |u, v| {
        let mut c = shape(CLEAR, rbox(u, v, 0.5, 0.92, 0.22, 0.08, 0.04), rgb(150, 156, 172), 0.022);
        c = shape(c, seg(u, v, (0.5, 0.86), (0.62, 0.36)).0 - 0.04, rgb(150, 104, 64), 0.022);
        shape(c, (u - 0.62).hypot(v - 0.32) - 0.085, rgb(240, 70, 70), 0.022)
    });
    img.tile(t::TALLGRASS, |u, v| {
        let mut c = CLEAR;
        for i in 0..6 {
            let x = 0.1 + i as f32 * 0.16;
            let h = hash(i, 0, 0, 33);
            let top = 0.3 + (h % 30) as f32 / 100.0;
            let lean = ((h >> 8) % 20) as f32 / 100.0 - 0.1;
            let col = if i % 2 == 0 { rgb(104, 196, 78) } else { rgb(70, 160, 70) };
            c = shape(c, blade(u, v, (x, 1.02), (x + lean, top), 0.05), col, 0.02);
        }
        c
    });
    img.tile(t::FLOWER, |u, v| {
        let mut c = shape(CLEAR, blade(u, v, (0.5, 1.02), (0.5, 0.4), 0.035), rgb(84, 180, 84), 0.02);
        c = shape(c, blade(u, v, (0.5, 0.8), (0.72, 0.62), 0.05), rgb(104, 196, 78), 0.02);
        // Cinq petales ronds autour d'un coeur jaune.
        for i in 0..5 {
            let a = i as f32 * tau / 5.0 - 1.57;
            let (cx, cy) = (0.5 + a.cos() * 0.14, 0.32 + a.sin() * 0.14);
            c = shape(c, (u - cx).hypot(v - cy) - 0.105, rgb(255, 130, 170), 0.02);
        }
        shape(c, (u - 0.5).hypot(v - 0.32) - 0.08, rgb(255, 226, 90), 0.018)
    });
    img.tile(t::SUN, |u, v| {
        let r = (u - 0.5).hypot(v - 0.5);
        let c = over(CLEAR, [1.0, 0.9, 0.5, 0.35], cov(r - 0.48));
        let c = over(c, rgb(255, 222, 110), cov(r - 0.34));
        over(c, rgb(255, 250, 220), cov(r - 0.26))
    });
    img.tile(t::MOON, |u, v| {
        let r = (u - 0.5).hypot(v - 0.5);
        let mut c = over(CLEAR, rgb(234, 240, 255), cov(r - 0.4));
        for (cx, cy, cr) in [(0.4, 0.4, 0.07), (0.62, 0.56, 0.09), (0.44, 0.68, 0.05)] {
            c = over(c, rgb(198, 208, 236), cov((u - cx).hypot(v - cy) - cr) * cov(r - 0.38));
        }
        c
    });
    for s in 0..4u16 {
        // Fissures : des branches de plus en plus nombreuses partent du centre.
        img.tile(t::CRACK0 + s, move |u, v| {
            let mut d = 1.0f32;
            for i in 0..(3 + s as i32 * 2) {
                let h = hash(i, 0, 0, 52);
                let a = i as f32 * 2.4 + (h % 100) as f32 / 100.0;
                let len = 0.16 + (s as f32) * 0.07 + ((h >> 8) % 100) as f32 / 900.0;
                let mid = (0.5 + a.cos() * len * 0.5, 0.5 + a.sin() * len * 0.5);
                let end = (0.5 + (a + 0.35).cos() * len, 0.5 + (a + 0.35).sin() * len);
                d = d.min(seg(u, v, (0.5, 0.5), mid).0).min(seg(u, v, mid, end).0);
            }
            over(CLEAR, [0.06, 0.05, 0.1, 0.85], cov(d - 0.012))
        });
    }
    img.tile(t::CIRCLE, |u, v| over(CLEAR, [1.0; 4], cov((u - 0.5).hypot(v - 0.5) - 0.47)));
    img.tile(t::PUFF, |u, v| {
        // Boule de nuage : blanche, ombre bleutee en bas.
        let r = (u - 0.5).hypot(v - 0.5);
        let c = over(CLEAR, rgb(206, 226, 250), cov(r - 0.47));
        over(c, [1.0; 4], cov((u - 0.5).hypot(v - 0.42) - 0.42) * cov(r - 0.47))
    });
}

/// Icones des objets et de l'interface.
fn paint_icons(img: &mut Img) {
    let wood = [150, 104, 64];
    let tiers = [[196, 146, 90], [160, 164, 178], [236, 240, 248], [96, 232, 236]];
    for kind in 0..5usize {
        for (tier, col) in tiers.iter().enumerate() {
            let tile = t::ITEM_TOOLS + (kind * 4 + tier) as u16;
            sprite(img, tile, TOOLS[kind], &[('h', *col), ('s', wood), ('g', [90, 76, 70])]);
        }
    }
    let misc: [(u16, [&str; 8], [u8; 3], [u8; 3]); 29] = [
        (items::STICK, STICK_S, wood, wood),
        (items::COAL, LUMP, [56, 56, 70], [96, 96, 112]),
        (items::IRON_INGOT, INGOT, [214, 218, 230], [250, 250, 255]),
        (items::GOLD_INGOT, INGOT, [250, 200, 50], [255, 240, 140]),
        (items::DIAMOND, GEM, [90, 226, 230], [210, 252, 255]),
        (items::VOLT_DUST, DUST, [170, 100, 255], [216, 176, 255]),
        (items::SEEDS, DUST, [124, 196, 80], [180, 226, 120]),
        (items::WHEAT_ITEM, WHEAT_S, [244, 206, 84], [180, 164, 70]),
        (items::BREAD, BREAD_S, [214, 150, 76], [246, 200, 124]),
        (items::CARROT_ITEM, CARROT_S, [255, 146, 50], [84, 180, 84]),
        (items::RAW_MEAT, MEAT, [240, 110, 120], [255, 214, 214]),
        (items::COOKED_MEAT, MEAT, [176, 106, 62], [224, 170, 124]),
        (items::LEATHER, HIDE, [176, 110, 66], [176, 110, 66]),
        (items::FEATHER, FEATHER_S, [250, 250, 255], wood),
        (items::EGG, ROUND, [250, 242, 222], [255, 255, 255]),
        (items::STRING, STRING_S, [244, 244, 250], [244, 244, 250]),
        (items::BONE, BONE_S, [246, 242, 226], [246, 242, 226]),
        (items::ROTTEN, MEAT, [130, 146, 80], [184, 110, 90]),
        (items::SPORE, DUST, [100, 216, 136], [186, 246, 204]),
        (items::PEARL, ROUND, [40, 120, 124], [140, 240, 232]),
        (items::EMBER_CRYSTAL, GEM, [255, 160, 50], [255, 238, 156]),
        (items::BUCKET, BUCKET_S, [190, 194, 208], [110, 112, 128]),
        (items::WATER_BUCKET, BUCKET_S, [190, 194, 208], [52, 140, 236]),
        (items::LAVA_BUCKET, BUCKET_S, [190, 194, 208], [255, 122, 30]),
        (items::LIGHTER, LIGHTER_S, [214, 218, 230], [76, 76, 92]),
        (items::APPLE, ROUND, [236, 60, 70], [255, 150, 150]),
        (items::FLINT, LUMP, [76, 76, 92], [130, 130, 150]),
        (items::WYRM_HEART, HEART_S, [170, 70, 230], [230, 160, 255]),
        (items::ARROW, ARROW_S, [214, 218, 230], wood),
    ];
    for (id, pattern, a, b) in misc {
        sprite(img, items::icon(id), pattern, &[('x', a), ('X', b), ('s', wood), ('w', [250, 250, 246])]);
    }
    sprite(img, t::HEART, HEART_S, &[('x', [255, 70, 96]), ('X', [255, 170, 186])]);
    sprite(img, t::HEART_EMPTY, HEART_S, &[('x', [70, 50, 78]), ('X', [70, 50, 78])]);
    sprite(img, t::FOOD, DRUM, &[('x', [214, 130, 66]), ('w', [250, 250, 246])]);
    sprite(img, t::FOOD_EMPTY, DRUM, &[('x', [70, 50, 78]), ('w', [70, 50, 78])]);
    sprite(img, t::BUBBLE, ROUND, &[('x', [90, 176, 255]), ('X', [220, 242, 255])]);
}

/// Construit les tuiles et la police. Les textures des modeles sont peintes ensuite par `models`.
pub fn build() -> Img {
    let mut img = Img(vec![0u8; AT * AT * 4]);
    paint_blocks(&mut img);
    paint_cutouts(&mut img);
    paint_icons(&mut img);
    // Les blocs sont ensuite repeints en pixel art dessine a la main.
    crate::pixels::paint(&mut img);
    bake_font(&mut img);
    img
}

