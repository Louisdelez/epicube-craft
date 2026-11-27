//! Calcul de la lumiere et construction du maillage d'un chunk.
use crate::atlas::tile_uv;
use crate::gfx::{MeshData, Vtx};
use shared::blocks::*;
use shared::chunk::{idx, Chunk, H};
use std::collections::HashMap;

/// Largeur de la zone de travail : le chunk et ses 8 voisins.
const P: usize = 48;
const LAYER: usize = P * P;

/// (normale, 4 coins) par face. Ordre des coins : bas-gauche, bas-droite, haut-droite, haut-gauche.
pub const FACES: [([i32; 3], [[f32; 3]; 4]); 6] = [
    ([1, 0, 0], [[1., 0., 1.], [1., 0., 0.], [1., 1., 0.], [1., 1., 1.]]),
    ([-1, 0, 0], [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]]),
    ([0, 1, 0], [[0., 1., 1.], [1., 1., 1.], [1., 1., 0.], [0., 1., 0.]]),
    ([0, -1, 0], [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]]),
    ([0, 0, 1], [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]]),
    ([0, 0, -1], [[1., 0., 0.], [0., 0., 0.], [0., 1., 0.], [1., 1., 0.]]),
];
pub const UVS: [[f32; 2]; 4] = [[0., 1.], [1., 1.], [1., 0.], [0., 0.]];
pub const SHADE: [u8; 6] = [204, 204, 255, 128, 166, 166];
/// Retrait des UV pour eviter de deborder sur la tuile voisine.
const INSET: f32 = 0.0004;

fn flood(light: &mut [u8], bl: &[u8], q: &mut Vec<u32>) {
    let mut head = 0;
    while head < q.len() {
        let i = q[head] as usize;
        head += 1;
        let l = light[i];
        if l <= 1 {
            continue;
        }
        let (x, z, y) = (i % P, (i / P) % P, i / LAYER);
        let mut visit = |n: usize| {
            let op = opacity(bl[n]);
            if op < 15 {
                let nl = l.saturating_sub(1 + op);
                if nl > light[n] {
                    light[n] = nl;
                    q.push(n as u32);
                }
            }
        };
        if x > 0 {
            visit(i - 1);
        }
        if x < P - 1 {
            visit(i + 1);
        }
        if z > 0 {
            visit(i - P);
        }
        if z < P - 1 {
            visit(i + P);
        }
        if y > 0 {
            visit(i - LAYER);
        }
        if y < H - 1 {
            visit(i + LAYER);
        }
    }
}

/// Lumiere lissee d'un coin de face : moyenne des 4 cellules autour, les blocs pleins comptant pour 0.
#[allow(clippy::too_many_arguments)]
fn corner(bl: &[u8], sky: &[u8], blk: &[u8], x: i32, y: i32, z: i32, n: [i32; 3], c: [f32; 3]) -> (u8, u8) {
    let mut t = [0usize; 2];
    let mut k = 0;
    for a in 0..3 {
        if n[a] == 0 {
            t[k] = a;
            k += 1;
        }
    }
    let base = [x + n[0], y + n[1], z + n[2]];
    let o1 = if c[t[0]] > 0.5 { 1 } else { -1 };
    let o2 = if c[t[1]] > 0.5 { 1 } else { -1 };
    let (mut s, mut b) = (0u32, 0u32);
    let mut samp = |d1: i32, d2: i32| -> bool {
        let mut p = base;
        p[t[0]] += d1;
        p[t[1]] += d2;
        if p[1] < 0 {
            return true;
        }
        if p[1] >= H as i32 {
            s += 15;
            return false;
        }
        let i = (p[1] as usize * P + p[2] as usize) * P + p[0] as usize;
        if opaque(bl[i]) {
            return true;
        }
        s += sky[i] as u32;
        b += blk[i] as u32;
        false
    };
    samp(0, 0);
    let a1 = samp(o1, 0);
    let a2 = samp(0, o2);
    if !(a1 && a2) {
        samp(o1, o2);
    }
    ((s * 255 / 60) as u8, (b * 255 / 60) as u8)
}

fn quad(v: &mut Vec<Vtx>, i: &mut Vec<u32>, pos: [[f32; 3]; 4], tile: u16, color: [[u8; 4]; 4], light: [[u8; 4]; 4]) {
    quad_r(v, i, pos, tile, 0, color, light);
}

/// Quart de tour a donner a une texture dessinee « vers +X » pour qu'elle pointe vers DIRS4[k].
const TURN: [usize; 4] = [0, 2, 1, 3];

/// Comme `quad`, la texture tournee de `rot` quarts de tour.
#[allow(clippy::too_many_arguments)]
fn quad_r(v: &mut Vec<Vtx>, i: &mut Vec<u32>, pos: [[f32; 3]; 4], tile: u16, rot: usize, color: [[u8; 4]; 4], light: [[u8; 4]; 4]) {
    let (u0, v0) = tile_uv(tile);
    let s = 1.0 / 32.0 - 2.0 * INSET;
    let base = v.len() as u32;
    for k in 0..4 {
        v.push(Vtx {
            pos: pos[k],
            uv: [u0 + INSET + UVS[(k + rot) % 4][0] * s, v0 + INSET + UVS[(k + rot) % 4][1] * s],
            color: color[k],
            light: light[k],
        });
    }
    i.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// Boite alignee sur les axes, de `mn` a `mx` dans le bloc pose en `o`. Chaque face lit la partie
/// de sa tuile qu'elle recouvre ; `rot` tourne la texture du dessus.
#[allow(clippy::too_many_arguments)]
fn cuboid(v: &mut Vec<Vtx>, i: &mut Vec<u32>, o: [f32; 3], mn: [f32; 3], mx: [f32; 3], tiles: [u16; 6], rot: usize, light: [u8; 4]) {
    let s = 1.0 / 32.0 - 2.0 * INSET;
    for (f, (_, corners)) in FACES.iter().enumerate() {
        let (u0, v0) = tile_uv(tiles[f]);
        let base = v.len() as u32;
        for c in corners {
            let q = [mn[0] + c[0] * (mx[0] - mn[0]), mn[1] + c[1] * (mx[1] - mn[1]), mn[2] + c[2] * (mx[2] - mn[2])];
            let (mut tu, mut tv) = match f {
                0 => (1.0 - q[2], 1.0 - q[1]),
                1 => (q[2], 1.0 - q[1]),
                2 => (q[0], q[2]),
                3 => (q[0], 1.0 - q[2]),
                4 => (q[0], 1.0 - q[1]),
                _ => (1.0 - q[0], 1.0 - q[1]),
            };
            if f == 2 {
                for _ in 0..rot {
                    (tu, tv) = (tv, 1.0 - tu);
                }
            }
            let sd = SHADE[f];
            v.push(Vtx {
                pos: [o[0] + q[0], o[1] + q[1], o[2] + q[2]],
                uv: [u0 + INSET + tu.clamp(0.0, 1.0) * s, v0 + INSET + tv.clamp(0.0, 1.0) * s],
                color: [sd, sd, sd, 255],
                light,
            });
        }
        i.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

/// Construit le maillage du chunk (cx, cz). Renvoie None tant qu'un voisin manque.
pub fn mesh_chunk(chunks: &HashMap<(i32, i32), Chunk>, cx: i32, cz: i32) -> Option<MeshData> {
    let mut bl = vec![0u8; LAYER * H];
    let mut me = vec![0u8; LAYER * H];
    for dz in 0..3usize {
        for dx in 0..3usize {
            let c = chunks.get(&(cx + dx as i32 - 1, cz + dz as i32 - 1))?;
            for y in 0..H {
                for z in 0..16 {
                    let s = idx(0, y, z);
                    let d = (y * P + dz * 16 + z) * P + dx * 16;
                    bl[d..d + 16].copy_from_slice(&c.b[s..s + 16]);
                    me[d..d + 16].copy_from_slice(&c.m[s..s + 16]);
                }
            }
        }
    }

    // Lumiere du ciel : plein jour au-dessus du premier obstacle de chaque colonne.
    let mut sky = vec![0u8; LAYER * H];
    let mut blk = vec![0u8; LAYER * H];
    let mut tops = vec![0usize; LAYER];
    let mut q: Vec<u32> = Vec::with_capacity(1 << 16);
    for z in 0..P {
        for x in 0..P {
            let mut y = H;
            while y > 0 {
                let i = ((y - 1) * P + z) * P + x;
                if opacity(bl[i]) != 0 {
                    break;
                }
                sky[i] = 15;
                y -= 1;
            }
            tops[z * P + x] = y;
        }
    }
    // Seules les cellules eclairees qui bordent une zone d'ombre ont besoin de se propager.
    for z in 0..P {
        for x in 0..P {
            let t = tops[z * P + x];
            let mut hi = t;
            if x > 0 {
                hi = hi.max(tops[z * P + x - 1]);
            }
            if x < P - 1 {
                hi = hi.max(tops[z * P + x + 1]);
            }
            if z > 0 {
                hi = hi.max(tops[(z - 1) * P + x]);
            }
            if z < P - 1 {
                hi = hi.max(tops[(z + 1) * P + x]);
            }
            for y in t..(hi + 1).min(H) {
                q.push(((y * P + z) * P + x) as u32);
            }
        }
    }
    flood(&mut sky, &bl, &mut q);

    // Lumiere des blocs.
    q.clear();
    for (i, &b) in bl.iter().enumerate() {
        if b != AIR {
            let e = emit(b, me[i]);
            if e > 0 {
                blk[i] = e;
                q.push(i as u32);
            }
        }
    }
    flood(&mut blk, &bl, &mut q);

    let mut out = MeshData::default();
    out.light = vec![0u8; 16 * 16 * H];
    for y in 0..H {
        for z in 0..16usize {
            for x in 0..16usize {
                let i = (y * P + z + 16) * P + x + 16;
                out.light[idx(x, y, z)] = sky[i] << 4 | blk[i];
            }
        }
    }
    for y in 0..H {
        for z in 0..16usize {
            for x in 0..16usize {
                let (px, pz) = (x + 16, z + 16);
                let i = (y * P + pz) * P + px;
                let b = bl[i];
                if b == AIR {
                    continue;
                }
                let m = me[i];
                let (wx, wy, wz) = ((cx * 16 + x as i32) as f32, y as f32, (cz * 16 + z as i32) as f32);
                let glow = emit(b, m) * 17;
                let own = [sky[i] * 17, (blk[i] * 17).max(glow), 0, 0];
                let sh = shape(b);
                match sh {
                    Shape::None => {}
                    Shape::Cube if b == ABYSS_PORTAL && m & 2 != 0 => {
                        // Voile d'un torii allume : une lame translucide au milieu du cadre.
                        let corners = if m & 1 == 0 { FACES[4].1 } else { FACES[0].1 };
                        let mut pos = [[0f32; 3]; 4];
                        for (n, c) in corners.iter().enumerate() {
                            pos[n] = if m & 1 == 0 { [wx + c[0], wy + c[1], wz + 0.5] } else { [wx + 0.5, wy + c[1], wz + c[2]] };
                        }
                        let glow = [own[0], 200, 0, 0];
                        quad(&mut out.tv, &mut out.ti, pos, t::VEIL, [[255, 255, 255, 200]; 4], [glow; 4]);
                    }
                    Shape::Cube if b == PISTON_HEAD => {
                        // Tete de poussoir : un plateau au bout d'une tige.
                        let face = (m & 7) as usize;
                        let (axis, far) = (face / 2, face % 2 == 0);
                        let (mut mn, mut mx) = ([0.0f32; 3], [1.0f32; 3]);
                        mn[axis] = if far { 0.75 } else { 0.0 };
                        mx[axis] = if far { 1.0 } else { 0.25 };
                        let mut tiles = [t::PISTON_FACE; 6];
                        tiles[face] = tile(b, m, face);
                        cuboid(&mut out.ov, &mut out.oi, [wx, wy, wz], mn, mx, tiles, 0, own);
                        let (mut mn, mut mx) = ([0.375f32; 3], [0.625f32; 3]);
                        mn[axis] = if far { 0.0 } else { 0.25 };
                        mx[axis] = if far { 0.75 } else { 1.0 };
                        cuboid(&mut out.ov, &mut out.oi, [wx, wy, wz], mn, mx, [t::PISTON_ARM; 6], 0, own);
                    }
                    Shape::Thin => {
                        let o = [wx, wy, wz];
                        match b {
                            REPEATER | COMPARATOR => {
                                let mut tiles = [t::SLAB_SIDE; 6];
                                tiles[2] = tile(b, m, 2);
                                let k = (m & 3) as usize;
                                cuboid(&mut out.ov, &mut out.oi, o, [0.0; 3], [1.0, 0.125, 1.0], tiles, TURN[k], own);
                                if b == REPEATER && m & 32 != 0 {
                                    // Verrou : une barre en travers.
                                    let (mn, mx) = if k < 2 { ([0.44, 0.125, 0.12], [0.56, 0.2, 0.88]) } else { ([0.12, 0.125, 0.44], [0.88, 0.2, 0.56]) };
                                    cuboid(&mut out.ov, &mut out.oi, o, mn, mx, [t::BEDROCK; 6], 0, own);
                                }
                            }
                            PLATE => {
                                let h = if m & 1 == 1 { 0.03 } else { 0.0625 };
                                cuboid(&mut out.ov, &mut out.oi, o, [0.0625, 0.0, 0.0625], [0.9375, h, 0.9375], [t::PLATE; 6], 0, own);
                            }
                            _ => {
                                // Sceau hanko : petit pave colle a son support, enfonce quand il est presse.
                                let a = shared::signal::attach_of(m);
                                let axis = (0..3).find(|i| a[*i] != 0).unwrap_or(1);
                                let depth = if m & 1 == 1 { 0.0625 } else { 0.125 };
                                let (mut mn, mut mx) = ([0.3125f32, 0.375, 0.3125], [0.6875f32, 0.625, 0.6875]);
                                if axis == 1 {
                                    (mn[2], mx[2]) = (0.375, 0.625);
                                }
                                (mn[axis], mx[axis]) = if a[axis] > 0 { (1.0 - depth, 1.0) } else { (0.0, depth) };
                                let skin = if b == HOOK { t::HOOK } else { t::BUTTON };
                                cuboid(&mut out.ov, &mut out.oi, o, mn, mx, [skin; 6], 0, own);
                            }
                        }
                    }
                    Shape::Cube | Shape::Fluid => {
                        let fluid = sh == Shape::Fluid;
                        let above = if y + 1 < H { bl[i + LAYER] } else { AIR };
                        let hgt = if fluid {
                            if above == b {
                                1.0
                            } else if m == 0 || m >= 8 {
                                0.875
                            } else {
                                0.875 * (8 - m) as f32 / 8.0
                            }
                        } else if b == BED {
                            0.5625
                        } else if b == SUNDIAL {
                            0.375
                        } else {
                            1.0
                        };
                        let open_door = b == DOOR && m & 1 == 1;
                        let trans = b == WATER || open_door;
                        let alpha = if open_door { 90 } else { 255 };
                        for (f, (n, corners)) in FACES.iter().enumerate() {
                            let ny = y as i32 + n[1];
                            if ny < 0 {
                                continue;
                            }
                            let nb = if ny >= H as i32 {
                                AIR
                            } else {
                                bl[(ny as usize * P + (pz as i32 + n[2]) as usize) * P + (px as i32 + n[0]) as usize]
                            };
                            let visible = if opaque(b) {
                                !opaque(nb)
                            } else {
                                (!opaque(nb) && nb != b) || (f == 2 && hgt < 1.0 && nb != b)
                            };
                            if !visible {
                                continue;
                            }
                            let mut pos = [[0f32; 3]; 4];
                            let mut light = [[0u8; 4]; 4];
                            for k in 0..4 {
                                let c = corners[k];
                                pos[k] = [wx + c[0], wy + c[1] * hgt, wz + c[2]];
                                let (s, bk) = corner(&bl, &sky, &blk, px as i32, y as i32, pz as i32, *n, c);
                                // 255 : le dessus de l'eau ondule ; 128 : les feuilles fremissent.
                                let wave = if b == WATER && c[1] > 0.5 && hgt < 1.0 {
                                    255
                                } else if matches!(b, LEAVES | CEDAR_LEAVES | MAPLE_LEAVES | WISTERIA) {
                                    128
                                } else {
                                    0
                                };
                                light[k] = [s, bk.max(glow), 0, wave];
                            }
                            let sd = SHADE[f];
                            let (v, ix) = if trans { (&mut out.tv, &mut out.ti) } else { (&mut out.ov, &mut out.oi) };
                            quad(v, ix, pos, tile(b, m, f), [[sd, sd, sd, alpha]; 4], light);
                        }
                    }
                    Shape::Cross => {
                        let tl = tile(b, m, 0);
                        // Levier et lanterne hitodama s'accrochent aussi aux murs (et le levier au plafond).
                        let a = if matches!(b, LEVER | SIGTORCH) { shared::signal::attach_of(m) } else { [0, -1, 0] };
                        let (ox, oz) = (a[0] as f32 * 0.3, a[2] as f32 * 0.3);
                        let lift = if a[1] == 0 { 0.2 } else { 0.0 };
                        let (y0, y1) = if a[1] > 0 { (wy + 1.0, wy) } else { (wy + lift, wy + 1.0 + lift) };
                        for (p0, p1) in [((0.15, 0.15), (0.85, 0.85)), ((0.15, 0.85), (0.85, 0.15))] {
                            let pos = [
                                [wx + p0.0 + ox, y0, wz + p0.1 + oz],
                                [wx + p1.0 + ox, y0, wz + p1.1 + oz],
                                [wx + p1.0 + ox, y1, wz + p1.1 + oz],
                                [wx + p0.0 + ox, y1, wz + p0.1 + oz],
                            ];
                            // Seul le haut des plantes se balance ; lanternes et leviers restent fixes.
                            let sway = if matches!(b, TORCH | SIGTORCH | LEVER) { 0 } else { 128 };
                            let top = [own[0], own[1], 0, sway];
                            quad(&mut out.ov, &mut out.oi2, pos, tl, [[255; 4]; 4], [own, own, top, top]);
                        }
                    }
                    Shape::Flat if b != WIRE => {
                        let flat = |yy: f32| [[wx, yy, wz + 1.0], [wx + 1.0, yy, wz + 1.0], [wx + 1.0, yy, wz], [wx, yy, wz]];
                        if b == TRIPWIRE {
                            // Corde tendue a mi-hauteur, dans l'axe de ses voisines.
                            let along_x = matches!(bl[i + 1], TRIPWIRE | HOOK) || matches!(bl[i - 1], TRIPWIRE | HOOK);
                            let col = if m & 1 == 1 { [255, 210, 210, 255] } else { [255; 4] };
                            quad_r(&mut out.ov, &mut out.oi2, flat(wy + 0.45), t::ROPE, if along_x { 0 } else { 1 }, [col; 4], [own; 4]);
                        } else {
                            // Rails : droits dans un axe, ou l'un des quatre virages.
                            let shape = (m & 7) as usize;
                            let rot = if shape >= 2 { [0, 3, 1, 2][(shape - 2).min(3)] } else { shape };
                            let tl = if b == RAIL || shape < 2 { tile(b, m, 2) } else { tile(b, m & 8, 2) };
                            quad_r(&mut out.ov, &mut out.oi2, flat(wy + 0.04), tl, rot, [[255; 4]; 4], [own; 4]);
                        }
                    }
                    Shape::Flat => {
                        // Fil de reiseki : un point central, un bras vers chaque cote relie,
                        // et un trait vertical quand il grimpe sur le bloc voisin.
                        let get = |q: [i32; 3]| -> (B, u8) {
                            if q[1] < 0 || q[1] >= H as i32 {
                                return (AIR, 0);
                            }
                            let j = (q[1] as usize * P + q[2] as usize) * P + q[0] as usize;
                            (bl[j], me[j])
                        };
                        let links = shared::signal::links(&get, [px as i32, y as i32, pz as i32]);
                        let arms = shared::signal::pointed(links);
                        let pw = (m & 15) as f32 / 15.0;
                        let col = [(70.0 + 185.0 * pw) as u8, (40.0 + 120.0 * pw) as u8, (110.0 + 145.0 * pw) as u8, 255];
                        let lit = [own[0], own[1].max((pw * 150.0) as u8), 0, 0];
                        let flat = |yy: f32| [[wx, yy, wz + 1.0], [wx + 1.0, yy, wz + 1.0], [wx + 1.0, yy, wz], [wx, yy, wz]];
                        quad(&mut out.ov, &mut out.oi2, flat(wy + 0.034), t::WIRE_DOT, [col; 4], [lit; 4]);
                        for k in 0..4 {
                            if arms[k] {
                                quad_r(&mut out.ov, &mut out.oi2, flat(wy + 0.03), t::WIRE_ARM, TURN[k], [col; 4], [lit; 4]);
                            }
                            if links[k] == 2 {
                                let d = shared::signal::DIRS4[k];
                                let corners = FACES[[1, 0, 5, 4][k]].1;
                                let mut pos = [[0f32; 3]; 4];
                                for (n, c) in corners.iter().enumerate() {
                                    pos[n] = [wx + c[0] + d[0] as f32 * 0.97, wy + c[1], wz + c[2] + d[2] as f32 * 0.97];
                                }
                                quad(&mut out.ov, &mut out.oi2, pos, t::WIRE_V, [col; 4], [lit; 4]);
                            }
                        }
                    }
                }
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mesure, pas un vrai test : `cargo test --release -p client -- --ignored --nocapture vitesse`.
    #[test]
    #[ignore]
    fn vitesse_de_maillage() {
        let mut chunks = HashMap::new();
        for cz in -3..4 {
            for cx in -3..4 {
                chunks.insert((cx, cz), shared::gen::generate(0, 5, cx, cz));
            }
        }
        let t = std::time::Instant::now();
        let (mut n, mut tris) = (0, 0);
        for cz in -2..3 {
            for cx in -2..3 {
                let m = mesh_chunk(&chunks, cx, cz).unwrap();
                tris += (m.oi.len() + m.oi2.len() + m.ti.len()) / 3;
                n += 1;
            }
        }
        println!("maillage : {:.2} ms par chunk, {} triangles en moyenne", t.elapsed().as_secs_f64() * 1000.0 / n as f64, tris / n);
    }
}
