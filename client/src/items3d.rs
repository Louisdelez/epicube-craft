//! Modeles 3D des objets.
//!
//! Chaque objet est un vrai modele en boites, concu piece par piece (lame, garde et poignee d'un
//! katana, douves et cercles d'un seau...) dans `itemdefs`. Ce module leur peint une texture en pixel
//! art, matiere par matiere, puis tire de chaque modele son icone d'inventaire : ce qu'on voit dans
//! les cases est donc exactement ce qu'on tient en main et ce qui tourne au sol.
//!
//! Les objets s'exportent au format Blockbench (assets/items/<numero>_<nom>.bbmodel), texture
//! comprise. Un fichier modifie dans Blockbench est relu au demarrage, et son icone refaite.
//!
//! Les blocs qui ne sont pas des cubes (rails, pousses, lanternes...) gardent un modele plat : leur
//! dessin, auquel on donne un peu d'epaisseur.
use crate::assets;
use crate::atlas::{Img, AT, TILE};
use crate::itemdefs;
use crate::models::{margin, Model, Part};
use glam::{EulerRot, Mat3, Quat, Vec3};
use shared::blocks::{shape, Shape};
use shared::items;
use shared::util::hash;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Cote du dessin d'un objet, en pixels.
pub const N: usize = 32;
/// Ligne de l'atlas ou commencent les textures des modeles d'objets, et cote de la case de chacun.
pub const ITEM_Y: usize = 1472;
pub const ICELL: usize = 96;
const PER_ROW: usize = AT / ICELL;
/// Encre des contours.
const INK: [u8; 3] = [28, 24, 36];

pub type Shapes = HashMap<u16, Model>;

// ---------------------------------------------------------------------------------------------
// Matieres

/// Motif peint sur les faces d'une boite.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Pat {
    /// Uni, avec un simple biseau clair / sombre.
    Plain,
    /// Fil du bois : traits sombres dans le sens de la longueur.
    Grain,
    /// Metal poli : reflets en biais.
    Metal,
    /// Tressage en losanges (poignee de sabre), dans la seconde couleur.
    Wrap,
    /// Grain irregulier : points clairs et sombres.
    Speck,
    /// Bandes de la seconde couleur, en travers de la longueur.
    Bands,
    /// Points epars de la seconde couleur.
    Dots,
    /// Coeur lumineux.
    Glow,
    /// Ecailles : arcs de la seconde couleur.
    Scales,
}

#[derive(Clone, Copy, Debug)]
pub struct Mat {
    pub col: u32,
    pub alt: u32,
    pub pat: Pat,
    /// Biseau clair / sombre au bord des faces ; coupe sur les formes rondes, faites de pieces empilees.
    pub bevel: bool,
}

pub const fn m(col: u32) -> Mat {
    Mat { col, alt: col, pat: Pat::Plain, bevel: true }
}

pub const fn mp(col: u32, pat: Pat) -> Mat {
    Mat { col, alt: col, pat, bevel: true }
}

pub const fn ma(col: u32, alt: u32, pat: Pat) -> Mat {
    Mat { col, alt, pat, bevel: true }
}

/// Eclaircit (k > 0) ou assombrit (k < 0) une matiere.
pub fn shade(mut mat: Mat, k: f32) -> Mat {
    let f = |col: u32| {
        let c = [(col >> 16 & 255) as f32, (col >> 8 & 255) as f32, (col & 255) as f32];
        let v = c.map(|x| (if k >= 0.0 { x + (255.0 - x) * k } else { x * (1.0 + k) }) as u32);
        v[0] << 16 | v[1] << 8 | v[2]
    };
    mat.col = f(mat.col);
    mat.alt = f(mat.alt);
    mat
}

/// Ton d'une couleur : 0 la teinte, positif vers la lumiere (chaude), negatif vers l'ombre (froide).
fn tone(col: u32, t: i32) -> [u8; 3] {
    let c = [(col >> 16 & 255) as f32, (col >> 8 & 255) as f32, (col & 255) as f32];
    let (to, k, gain) = if t > 0 { ([255.0, 246.0, 214.0], 0.3 * t as f32, 1.0) } else { ([38.0, 28.0, 66.0], 0.17 * (-t) as f32, 0.84f32.powi(-t)) };
    let k = k.min(0.8);
    [0, 1, 2].map(|i| (c[i] * gain * (1.0 - k) + to[i] * k).clamp(0.0, 255.0) as u8)
}

/// Couleur du pixel (px, py) d'une face de `w` x `h` pixels.
fn texel(mat: Mat, px: usize, py: usize, w: usize, h: usize, seed: u32) -> [u8; 3] {
    let mut t = 0i32;
    // Biseau : lumiere en haut a gauche, ombre en bas a droite.
    if !mat.bevel {
    } else if w >= 3 && h >= 3 {
        if py == h - 1 || px == w - 1 {
            t = -1;
        } else if py == 0 || px == 0 {
            t = 1;
        }
    } else if (w == 2 && px == 1 && h > 2) || (h == 2 && py == 1 && w > 2) {
        t = -1;
    }
    // (along, across) : position le long du grand cote, puis en travers.
    let (along, across, len) = if h >= w { (py, px, h) } else { (px, py, w) };
    let (ia, ic) = (along as i32, across as i32);
    let mut col = mat.col;
    match mat.pat {
        Pat::Plain => {}
        Pat::Grain => {
            if hash(ic, ia / 5, 1, seed) % 4 == 0 && hash(ic, ia, 2, seed) % 5 != 0 {
                t -= 1;
            }
        }
        Pat::Metal => {
            let d = (px + py + (seed % 5) as usize) % 9;
            if d == 2 || d == 3 {
                t += 1;
            } else if d == 7 && w.min(h) >= 3 {
                t -= 1;
            }
        }
        Pat::Wrap => {
            // Losanges : la seconde couleur apparait au creux du tressage.
            let a = along % 4;
            if matches!((a, across % 2), (1, 0) | (2, 0) | (0, 1) | (3, 1)) {
                col = mat.alt;
            } else if a == 3 {
                t -= 1;
            }
        }
        Pat::Speck => match hash(px as i32, py as i32, 3, seed) % 9 {
            0 => t -= 1,
            1 => t += 1,
            _ => {}
        },
        Pat::Bands => {
            if along % 4 == 1 && len >= 4 {
                col = mat.alt;
            }
        }
        Pat::Dots => {
            if hash(px as i32, py as i32, 4, seed) % 6 == 0 {
                col = mat.alt;
                t = 0;
            }
        }
        Pat::Glow => {
            let (dx, dy) = ((px as f32 + 0.5) / w as f32 - 0.42, (py as f32 + 0.5) / h as f32 - 0.4);
            let d = (dx * dx + dy * dy).sqrt();
            if w.min(h) >= 3 {
                t = if d < 0.16 {
                    2
                } else if d < 0.34 {
                    1
                } else if d > 0.56 {
                    -1
                } else {
                    0
                };
            }
        }
        Pat::Scales => {
            let k = (px + (py / 2 % 2) * 2) % 4;
            if (k == 0 && py % 2 == 0) || (k == 1 && py % 2 == 1) {
                col = mat.alt;
            }
        }
    }
    tone(col, t.clamp(-2, 2))
}

// ---------------------------------------------------------------------------------------------
// Boites de construction

/// Boite d'un modele en cours de conception. Unite : le seizieme de bloc ; x a droite, y vers le
/// haut (0 = la base), z vers celui qui regarde.
#[derive(Clone, Copy, Debug)]
pub struct Bx {
    pub c: Vec3,
    pub s: Vec3,
    pub q: Quat,
    pub m: Mat,
}

fn euler(rot: [f32; 3]) -> Quat {
    Quat::from_euler(EulerRot::ZYX, rot[2].to_radians(), rot[1].to_radians(), rot[0].to_radians())
}

#[derive(Default)]
pub struct Kit {
    pub v: Vec<Bx>,
}

impl Kit {
    /// Boite droite : centre, dimensions, matiere.
    pub fn b(&mut self, c: [f32; 3], s: [f32; 3], m: Mat) {
        self.v.push(Bx { c: c.into(), s: s.into(), q: Quat::IDENTITY, m });
    }

    /// Boite inclinee de `rot` degres (x, y, z) autour de son centre.
    pub fn r(&mut self, c: [f32; 3], s: [f32; 3], m: Mat, rot: [f32; 3]) {
        self.v.push(Bx { c: c.into(), s: s.into(), q: euler(rot), m });
    }

    /// Barre tendue du point `a` au point `b` du plan de face, large de `w`, epaisse de `d`.
    pub fn seg(&mut self, a: [f32; 2], b: [f32; 2], w: f32, d: f32, m: Mat) {
        self.segz(a, b, 0.0, w, d, m);
    }

    /// Comme `seg`, a la profondeur `z`.
    pub fn segz(&mut self, a: [f32; 2], b: [f32; 2], z: f32, w: f32, d: f32, m: Mat) {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        let c = Vec3::new((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, z);
        self.v.push(Bx { c, s: Vec3::new(w, len, d), q: Quat::from_rotation_z((-dx).atan2(dy)), m });
    }

    /// Ligne brisee de barres passant par `pts`.
    pub fn line(&mut self, pts: &[[f32; 2]], w: f32, d: f32, m: Mat) {
        for p in pts.windows(2) {
            // Leger recouvrement pour fermer les coudes.
            let (dx, dy) = (p[1][0] - p[0][0], p[1][1] - p[0][1]);
            let l = (dx * dx + dy * dy).sqrt().max(0.01);
            let e = w * 0.2 / l;
            self.seg([p[0][0] - dx * e, p[0][1] - dy * e], [p[1][0] + dx * e, p[1][1] + dy * e], w, d, m);
        }
    }

    /// Fait tourner autour de `pivot` toutes les boites ajoutees depuis l'indice `from`.
    pub fn turn(&mut self, from: usize, rot: [f32; 3], pivot: [f32; 3]) {
        let (q, p) = (euler(rot), Vec3::from(pivot));
        for b in &mut self.v[from..] {
            b.c = p + q * (b.c - p);
            b.q = q * b.q;
        }
    }

    /// Deplace toutes les boites ajoutees depuis l'indice `from`.
    pub fn shift(&mut self, from: usize, by: [f32; 3]) {
        for b in &mut self.v[from..] {
            b.c += Vec3::from(by);
        }
    }

    /// Disque (octogone) de diametre `d` et de hauteur `h` : deux paves croises a 45 degres.
    pub fn disc(&mut self, c: [f32; 3], d: f32, h: f32, mut m: Mat) {
        m.bevel = false;
        let s = d * 0.82;
        self.b(c, [s, h, s], m);
        self.r(c, [s, h, s], m, [0.0, 45.0, 0.0]);
    }

    /// Boule de rayon `r` : six disques empiles, plus clairs en haut, plus sombres en bas.
    pub fn ball(&mut self, c: [f32; 3], r: f32, m: Mat) {
        const LAYERS: usize = 6;
        let h = 2.0 * r / LAYERS as f32;
        for i in 0..LAYERS {
            let y = -r + (i as f32 + 0.5) * h;
            let d = 2.0 * (r * r - y * y).sqrt().max(r * 0.42);
            let lit = [-0.16, -0.08, 0.0, 0.0, 0.07, 0.14][i];
            self.disc([c[0], c[1] + y, c[2]], d, h + 0.02, shade(m, lit));
        }
    }

    /// Couronne carree : quatre parois d'epaisseur `t`, de cote exterieur `side`, hautes de `h`, centrees en `c`.
    pub fn ring(&mut self, c: [f32; 3], side: f32, h: f32, t: f32, m: Mat) {
        let o = (side - t) / 2.0;
        for s in [-1.0, 1.0] {
            self.b([c[0] + s * o, c[1], c[2]], [t, h, side], m);
            self.b([c[0], c[1], c[2] + s * o], [side - 2.0 * t, h, t], m);
        }
    }

    /// Couronne ronde (octogone) de diametre `d` : deux couronnes carrees croisees a 45 degres.
    pub fn hoop(&mut self, c: [f32; 3], d: f32, h: f32, t: f32, mut m: Mat) {
        m.bevel = false;
        self.ring(c, d * 0.82, h, t, m);
        let from = self.v.len();
        self.ring(c, d * 0.82, h, t, m);
        self.turn(from, [0.0, 45.0, 0.0], c);
    }
}

/// Facon de presenter un objet : dans sa case d'inventaire et dans la main.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Pose {
    /// Outil ou arme : dresse en diagonale.
    Tool,
    /// Objet pose, vu de trois quarts par-dessus.
    Thing,
    /// Objet plat presente de face.
    Flat,
    /// De face, penche de tant de degres.
    Roll(f32),
}

impl Pose {
    /// Rotation du modele devant l'oeil pour dessiner l'icone (on regarde vers -z).
    fn view(self) -> Mat3 {
        let r = |x: f32, y: f32, z: f32| Mat3::from_rotation_x(x.to_radians()) * Mat3::from_rotation_y(y.to_radians()) * Mat3::from_rotation_z(z.to_radians());
        match self {
            Pose::Tool => r(10.0, -26.0, -42.0),
            Pose::Thing => r(26.0, -34.0, 0.0),
            Pose::Flat => r(8.0, -20.0, 0.0),
            Pose::Roll(a) => r(8.0, -20.0, a),
        }
    }

    /// Rotation du modele dans la main, dans le repere (droite, haut, avant).
    pub fn hand(self) -> Mat3 {
        let r = |x: f32, y: f32, z: f32| Mat3::from_rotation_z(z.to_radians()) * Mat3::from_rotation_x(x.to_radians()) * Mat3::from_rotation_y(y.to_radians());
        match self {
            Pose::Tool => r(20.0, -104.0, 6.0),
            Pose::Thing => r(-12.0, 205.0, 0.0),
            Pose::Flat | Pose::Roll(_) => r(-6.0, 200.0, 0.0),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Textures

/// Objets qui ont un vrai modele : tout ce qui ne se pose pas comme un bloc.
pub fn ids() -> Vec<u16> {
    (256..items::ITEM_END)
        .chain(items::TOOL_BASE..items::TOOL_BASE + 20)
        .chain(items::COOKED_RICE..items::MORE_END)
        .chain(items::TOOL4_BASE..=items::SHIELD)
        .chain(items::TAMA_SHARD..items::REALM_END)
        .chain(items::ARMOR_BASE..items::ARMOR_END)
        .collect()
}

/// Blocs qui ne sont pas des cubes : leur modele est leur dessin, en epaisseur.
pub fn flat_ids() -> Vec<u16> {
    (1..256u16).filter(|id| !matches!(shape(*id as u8), Shape::Cube | Shape::None | Shape::Fluid) && items::place_block(*id).is_some()).collect()
}

/// Case de texture d'un objet dans l'atlas : coin haut-gauche et cote.
pub fn cell(id: u16) -> Option<(usize, usize, usize)> {
    let k = ids().iter().position(|i| *i == id)?;
    Some(((k % PER_ROW) * ICELL, ITEM_Y + (k / PER_ROW) * ICELL, ICELL))
}

/// Taille en pixels des six faces d'une boite, a `density` pixels par unite.
fn face_px(s: Vec3, density: f32) -> [(usize, usize); 6] {
    let n = |v: f32| ((v * density).round() as usize).max(1);
    [(n(s.z), n(s.y)), (n(s.z), n(s.y)), (n(s.x), n(s.z)), (n(s.x), n(s.z)), (n(s.x), n(s.y)), (n(s.x), n(s.y))]
}

/// Range les faces dans une case, des plus hautes aux plus basses, en rangees ; None si elles n'y tiennent pas.
fn pack(v: &[Bx], density: f32) -> Option<Vec<[[usize; 4]; 6]>> {
    let mut faces: Vec<(usize, usize, usize, usize)> = Vec::with_capacity(v.len() * 6);
    for (k, b) in v.iter().enumerate() {
        for (f, (w, h)) in face_px(b.s, density).into_iter().enumerate() {
            faces.push((k, f, w, h));
        }
    }
    faces.sort_by(|a, b| b.3.cmp(&a.3).then(b.2.cmp(&a.2)));
    let (mut x, mut y, mut row) = (0usize, 0usize, 0usize);
    let mut out = vec![[[0usize; 4]; 6]; v.len()];
    for (k, f, w, h) in faces {
        if w > ICELL || h > ICELL {
            return None;
        }
        if x + w > ICELL {
            x = 0;
            y += row;
            row = 0;
        }
        if y + h > ICELL {
            return None;
        }
        out[k][f] = [x, y, w, h];
        x += w;
        row = row.max(h);
    }
    Some(out)
}

/// Peint la texture d'un objet dans sa case et renvoie son modele ; `None` si la case deborde.
pub fn bake(img: &mut Img, id: u16, kit: &Kit) -> Option<Model> {
    let (ox, oy, _) = cell(id)?;
    for y in 0..ICELL {
        for x in 0..ICELL {
            img.set_px(ox + x, oy + y, [0, 0, 0, 0]);
        }
    }
    let rects = [2.0, 1.5, 1.0].into_iter().find_map(|d| pack(&kit.v, d))?;
    let mut model = Model::default();
    for (k, (b, faces)) in kit.v.iter().zip(rects).enumerate() {
        let mut uv = [[0f32; 4]; 6];
        for (f, [x, y, w, h]) in faces.into_iter().enumerate() {
            let seed = id as u32 * 7919 + (k * 6 + f) as u32;
            for py in 0..h {
                for px in 0..w {
                    let c = texel(b.m, px, py, w, h, seed);
                    img.set_px(ox + x + px, oy + y + py, [c[0], c[1], c[2], 255]);
                }
            }
            let (ex, ey) = (margin(w as f32), margin(h as f32));
            let s = AT as f32;
            uv[f] = [((ox + x) as f32 + ex) / s, ((oy + y) as f32 + ey) / s, ((ox + x + w) as f32 - ex) / s, ((oy + y + h) as f32 - ey) / s];
        }
        // L'objet est centre sur l'origine et tient dans un cube de cote 1.
        let c = (b.c - Vec3::new(0.0, 8.0, 0.0)) / 16.0;
        let (rz, ry, rx) = b.q.to_euler(EulerRot::ZYX);
        let rot = Vec3::new(rx.to_degrees(), ry.to_degrees(), rz.to_degrees());
        model.parts.push(Part { c, h: b.s / 32.0, rot, pivot: c, leg: 0, uv });
    }
    Some(model)
}

// ---------------------------------------------------------------------------------------------
// Modeles plats des blocs minces

/// Coin haut-gauche, dans l'atlas, de la tuile d'un objet.
fn tile_origin(id: u16) -> (usize, usize) {
    let tile = items::icon(id) as usize;
    ((tile % 32) * TILE, (tile / 32) * TILE)
}

/// Rectangle de l'atlas couvrant les pixels [x0, x1[ x [y0, y1[ du dessin, a peine rentre pour ne pas baver.
fn rect(o: (usize, usize), x0: f32, y0: f32, x1: f32, y1: f32) -> [f32; 4] {
    let k = (TILE / N) as f32;
    let e = 0.04;
    let u = |x: f32, grow: f32| (o.0 as f32 + x * k + e * grow) / AT as f32;
    let v = |y: f32, grow: f32| (o.1 as f32 + y * k + e * grow) / AT as f32;
    [u(x0, (x1 - x0).signum()), v(y0, (y1 - y0).signum()), u(x1, (x0 - x1).signum()), v(y1, (y0 - y1).signum())]
}

/// Donne de l'epaisseur au dessin d'un bloc mince : ses pixels opaques, regroupes en grands rectangles.
pub fn extrude(img: &Img, id: u16) -> Model {
    let o = tile_origin(id);
    let k = TILE / N;
    let n = N as f32;
    let solid = |x: usize, y: usize| img.0[((o.1 + y * k) * AT + o.0 + x * k) * 4 + 3] > 127;
    let mut used = [[false; N]; N];
    let mut model = Model::default();
    for y in 0..N {
        for x in 0..N {
            if used[y][x] || !solid(x, y) {
                continue;
            }
            let mut x1 = x;
            while x1 < N && solid(x1, y) && !used[y][x1] {
                x1 += 1;
            }
            let mut y1 = y + 1;
            while y1 < N && (x..x1).all(|i| solid(i, y1) && !used[y1][i]) {
                y1 += 1;
            }
            for row in used.iter_mut().take(y1).skip(y) {
                row[x..x1].fill(true);
            }
            let (fx0, fy0, fx1, fy1) = (x as f32, y as f32, x1 as f32, y1 as f32);
            let c = Vec3::new((fx0 + fx1) / 2.0 / n - 0.5, 0.5 - (fy0 + fy1) / 2.0 / n, 0.0);
            let uv = [
                rect(o, fx1 - 1.0, fy0, fx1, fy1),
                rect(o, fx0, fy0, fx0 + 1.0, fy1),
                rect(o, fx0, fy0, fx1, fy0 + 1.0),
                rect(o, fx0, fy1 - 1.0, fx1, fy1),
                rect(o, fx0, fy0, fx1, fy1),
                rect(o, fx1, fy0, fx0, fy1),
            ];
            model.parts.push(Part { c, h: Vec3::new((fx1 - fx0) / 2.0 / n, (fy1 - fy0) / 2.0 / n, 1.0 / 32.0), rot: Vec3::ZERO, pivot: c, leg: 0, uv });
        }
    }
    model
}

// ---------------------------------------------------------------------------------------------
// Icones

/// Texel de l'atlas sous le point (a, b) d'un rectangle de texture.
fn sample(img: &Img, r: [f32; 4], a: f32, b: f32) -> [u8; 4] {
    let x = ((r[0] + a * (r[2] - r[0])) * AT as f32) as usize;
    let y = ((r[1] + b * (r[3] - r[1])) * AT as f32) as usize;
    let i = (y.min(AT - 1) * AT + x.min(AT - 1)) * 4;
    [img.0[i], img.0[i + 1], img.0[i + 2], img.0[i + 3]]
}

type Hit = ([u8; 3], f32, usize);

/// Dessine l'icone d'un objet d'apres son modele : N x N pixels RGBA.
pub fn draw_icon(img: &Img, id: u16, model: &Model) -> Vec<[u8; 4]> {
    let view = itemdefs::pose(id).view();
    let inv = view.transpose();
    let parts: Vec<(Vec3, Quat, &Part)> = model
        .parts
        .iter()
        .map(|p| {
            let q = Quat::from_euler(EulerRot::ZYX, p.rot.z.to_radians(), p.rot.y.to_radians(), p.rot.x.to_radians());
            (p.pivot + q * (p.c - p.pivot), q, p)
        })
        .collect();
    // Cadre : l'objet projete, agrandi pour remplir la case.
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for (c, q, p) in &parts {
        for k in 0..8 {
            let corner = Vec3::new(if k & 1 == 0 { -p.h.x } else { p.h.x }, if k & 2 == 0 { -p.h.y } else { p.h.y }, if k & 4 == 0 { -p.h.z } else { p.h.z });
            let v = view * (*c + *q * corner);
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    let span = (hi.x - lo.x).max(hi.y - lo.y).max(0.05);
    let ppu = (N as f32 - 4.0) * itemdefs::fill(id) / span;
    let mid = (lo + hi) / 2.0;
    let light = Vec3::new(-0.45, 0.75, 0.5).normalize();

    // Lance un rayon : couleur, profondeur et piece touchee.
    let trace = |sx: f32, sy: f32| -> Option<Hit> {
        let (o, d) = (inv * Vec3::new(sx, sy, 50.0), inv * Vec3::NEG_Z);
        let mut best: Option<Hit> = None;
        for (k, (c, q, p)) in parts.iter().enumerate() {
            let qi = q.inverse();
            let (lo_, ld) = (qi * (o - *c), qi * d);
            let (mut t0, mut t1, mut axis) = (f32::MIN, f32::MAX, 0);
            let mut miss = false;
            for a in 0..3 {
                if ld[a].abs() < 1e-6 {
                    if lo_[a].abs() > p.h[a] {
                        miss = true;
                    }
                    continue;
                }
                let (ta, tb) = ((-p.h[a] - lo_[a]) / ld[a], (p.h[a] - lo_[a]) / ld[a]);
                let (near, far) = (ta.min(tb), ta.max(tb));
                if near > t0 {
                    t0 = near;
                    axis = a;
                }
                t1 = t1.min(far);
            }
            if miss || t0 > t1 || best.is_some_and(|b| b.1 <= t0) {
                continue;
            }
            let hit = lo_ + ld * t0;
            let n = |a: usize| ((hit[a] + p.h[a]) / (2.0 * p.h[a])).clamp(0.0, 0.9999);
            let plus = ld[axis] < 0.0;
            let (f, a, b) = match (axis, plus) {
                (0, true) => (0, 1.0 - n(2), 1.0 - n(1)),
                (0, false) => (1, n(2), 1.0 - n(1)),
                (1, true) => (2, n(0), n(2)),
                (1, false) => (3, n(0), 1.0 - n(2)),
                (_, true) => (4, n(0), 1.0 - n(1)),
                (_, false) => (5, 1.0 - n(0), 1.0 - n(1)),
            };
            let px = sample(img, p.uv[f], a.clamp(0.0, 0.9999), b.clamp(0.0, 0.9999));
            if px[3] < 128 {
                continue;
            }
            let mut normal = Vec3::ZERO;
            normal[axis] = if plus { 1.0 } else { -1.0 };
            let lit = 0.76 + 0.36 * (view * (*q * normal)).dot(light).max(0.0);
            let col = [0, 1, 2].map(|i| (px[i] as f32 * lit).min(255.0) as u8);
            best = Some((col, t0, k));
        }
        best
    };

    const SS: usize = 3;
    let mut px = vec![None::<Hit>; N * N];
    for y in 0..N {
        for x in 0..N {
            let mut hits: Vec<Hit> = Vec::with_capacity(SS * SS);
            for j in 0..SS {
                for i in 0..SS {
                    let sx = mid.x + (x as f32 + (i as f32 + 0.5) / SS as f32 - N as f32 / 2.0) / ppu;
                    let sy = mid.y - (y as f32 + (j as f32 + 0.5) / SS as f32 - N as f32 / 2.0) / ppu;
                    hits.extend(trace(sx, sy));
                }
            }
            if hits.len() * 3 >= SS * SS {
                // Couleur la plus frequente parmi les rayons : pas de degrade flou.
                px[y * N + x] = hits.iter().max_by_key(|h| hits.iter().filter(|o| o.0 == h.0).count()).copied();
            }
        }
    }
    let around = |x: usize, y: usize| {
        [(1i32, 0i32), (0, 1), (-1, 0), (0, -1)].into_iter().filter_map(move |(dx, dy)| {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            ((0..N as i32).contains(&nx) && (0..N as i32).contains(&ny)).then(|| ny as usize * N + nx as usize)
        })
    };
    let mut out = vec![[0u8; 4]; N * N];
    for y in 0..N {
        for x in 0..N {
            match px[y * N + x] {
                Some((mut c, depth, part)) => {
                    // Trait d'ombre la ou une autre piece passe nettement devant.
                    if around(x, y).any(|i| px[i].is_some_and(|(_, d2, p2)| p2 != part && depth - d2 > 1.6 / 16.0)) {
                        c = c.map(|v| (v as u32 * 62 / 100) as u8);
                    }
                    out[y * N + x] = [c[0], c[1], c[2], 255];
                }
                // Contour a l'encre.
                None if around(x, y).any(|i| px[i].is_some()) => out[y * N + x] = [INK[0], INK[1], INK[2], 255],
                None => {}
            }
        }
    }
    out
}

/// Refait l'icone de chaque objet d'apres son modele, dans sa tuile de l'atlas.
pub fn icons(img: &mut Img, shapes: &Shapes) {
    let k = TILE / N;
    for id in ids() {
        let Some(model) = shapes.get(&id) else { continue };
        let icon = draw_icon(img, id, model);
        let o = tile_origin(id);
        for y in 0..TILE {
            for x in 0..TILE {
                img.set_px(o.0 + x, o.1 + y, icon[(y / k) * N + x / k]);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Construction, export et relecture

/// Construit tous les modeles : les objets piece par piece, les blocs minces d'apres leur dessin.
pub fn build(img: &mut Img) -> Shapes {
    let mut shapes = Shapes::new();
    for id in flat_ids() {
        shapes.insert(id, extrude(img, id));
    }
    for id in ids() {
        if let Some(model) = bake(img, id, &itemdefs::design(id)) {
            shapes.insert(id, model);
        }
    }
    shapes
}

/// Nom de fichier d'un objet : son numero (qui fait foi a la relecture), puis son nom sans accents ni espaces.
pub fn file_name(id: u16) -> String {
    let slug: String = items::name(id).to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    format!("{id:03}_{}.bbmodel", slug.trim_matches('_'))
}

/// Exporte tous les objets dans `dir`/items. Renvoie le nombre de fichiers ecrits.
pub fn export(dir: &Path, img: &Img, shapes: &Shapes) -> Result<usize, String> {
    let out = dir.join("items");
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let mut n = 0;
    for id in ids() {
        if let (Some(model), Some(cell)) = (shapes.get(&id), cell(id)) {
            fs::write(out.join(file_name(id)), assets::bbmodel(&items::name(id), cell, 0.5, model, img)?).map_err(|e| e.to_string())?;
            n += 1;
        }
    }
    Ok(n)
}

/// Applique les fichiers de `dir`/items par-dessus les modeles integres. Renvoie un compte rendu par fichier.
pub fn load(dir: &Path, img: &mut Img, shapes: &mut Shapes) -> Vec<String> {
    let mut log = Vec::new();
    let Ok(files) = fs::read_dir(dir.join("items")) else { return log };
    for f in files.flatten() {
        let name = f.file_name().to_string_lossy().to_string();
        if !name.ends_with(".bbmodel") {
            continue;
        }
        // Le numero en tete du nom dit de quel objet il s'agit.
        let Some((id, cell)) = name.split('_').next().and_then(|n| n.parse::<u16>().ok()).and_then(|id| Some((id, cell(id)?))) else {
            log.push(format!("{name} : numero d'objet inconnu, fichier ignore"));
            continue;
        };
        match fs::read_to_string(f.path()).map_err(|e| e.to_string()).and_then(|t| assets::read_bbmodel(&t, cell, 0.5, img)) {
            Ok(model) => {
                shapes.insert(id, model);
            }
            Err(e) => log.push(format!("{name} : {e}")),
        }
    }
    log
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_item_has_a_real_model() {
        let mut img = crate::atlas::build();
        let shapes = build(&mut img);
        icons(&mut img, &shapes);
        // La derniere rangee de cases partage sa hauteur avec des tuiles, a partir de la douzieme case.
        assert!(ids().len() <= PER_ROW * 5 + 11, "plus d'objets que de cases de texture libres");
        let mut names = std::collections::HashSet::new();
        for id in ids() {
            let kit = itemdefs::design(id);
            let name = items::name(id);
            assert!(kit.v.len() >= 3, "{name} : un vrai modele a plusieurs pieces ({})", kit.v.len());
            assert!([2.0, 1.5].into_iter().any(|d| pack(&kit.v, d).is_some()), "{name} : la texture ne tient pas dans sa case");
            // Tout tient dans le cube de l'objet, a peu de chose pres.
            for p in &shapes[&id].parts {
                assert!(p.c.abs().max_element() <= 0.75, "{name} : piece hors du cube ({:?})", p.c);
            }
            // L'icone est dessinee, avec assez de pixels pour se lire.
            let o = tile_origin(id);
            let k = TILE / N;
            let opaque = (0..N * N).filter(|i| img.0[((o.1 + i / N * k) * AT + o.0 + i % N * k) * 4 + 3] > 127).count();
            assert!(opaque >= 90, "{name} : icone presque vide ({opaque} pixels)");
            assert!(names.insert(file_name(id)), "deux objets auraient le meme fichier : {}", file_name(id));
        }
        for id in flat_ids() {
            assert!(!shapes[&id].parts.is_empty(), "{} n'a aucun pixel", items::name(id));
        }
    }

    #[test]
    fn blockbench_roundtrip_keeps_models() {
        let mut img = crate::atlas::build();
        let shapes = build(&mut img);
        for id in [items::tool_id(items::SWORD, 2), items::YUMI, items::BENTO, items::KOBAN, items::BUCKET] {
            let cell = cell(id).unwrap();
            let text = assets::bbmodel(&items::name(id), cell, 0.5, &shapes[&id], &img).unwrap();
            let mut copy = Img(img.0.clone());
            let back = assets::read_bbmodel(&text, cell, 0.5, &mut copy).unwrap();
            assert_eq!(copy.0, img.0, "la texture de {} doit revenir identique", items::name(id));
            assert_eq!(back.parts.len(), shapes[&id].parts.len());
            for (a, b) in shapes[&id].parts.iter().zip(&back.parts) {
                assert!(a.c.distance(b.c) < 1e-4 && a.h.distance(b.h) < 1e-4 && a.rot.distance(b.rot) < 1e-2, "piece deplacee dans {}", items::name(id));
                for f in 0..6 {
                    for k in 0..4 {
                        assert!((a.uv[f][k] - b.uv[f][k]).abs() < 1e-5, "UV modifie dans {}", items::name(id));
                    }
                }
            }
        }
    }
}
