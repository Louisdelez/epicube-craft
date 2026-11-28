//! Skins : l'apparence d'un joueur, une image de 64 x 64 pixels au format de Minecraft. Chaque partie
//! du corps (tete, buste, bras, jambes) y a ses six faces, et une seconde couche par-dessus (chapeau,
//! veste, manches...) dont les pixels transparents laissent voir la premiere.
//!
//! Les skins vivent dans l'atlas, dans deux rangees de cases de 64 pixels sous les tuiles des mods :
//! une case par joueur present, et quelques-unes pour feuilleter la bibliotheque.
use crate::atlas::AT;
use crate::models::{Model, Part};
use crate::ui::Ui;
use glam::{EulerRot, Mat3, Quat, Vec3};
use shared::mods as sm;

pub const SIZE: usize = 64;
pub const BYTES: usize = SIZE * SIZE * 4;
/// Nombre de cases de skin dans l'atlas.
pub const SLOTS: usize = 64;
/// Case du skin du joueur, case du skin par defaut, premiere case des joueurs presents, premiere case de la bibliotheque.
pub const OWN: usize = 0;
pub const DEFAULT: usize = 1;
pub const FIRST_REMOTE: usize = 2;
pub const FIRST_SHELF: usize = 40;
pub const FILE: &str = "saves/skin.png";

/// Ligne de l'atlas des mods ou commencent les cases de skin : juste sous ses tuiles.
fn base_y() -> usize {
    (sm::TILES as usize / 32) * 64
}

/// Coin haut-gauche, en pixels, de la case `slot` dans la moitie de l'atlas reservee aux mods.
pub fn origin(slot: usize) -> (usize, usize) {
    ((slot % 32) * SIZE, base_y() + (slot / 32) * SIZE)
}

/// Les six parties du corps.
pub const PARTS: [&str; 6] = ["Tête", "Buste", "Bras D", "Bras G", "Jambe D", "Jambe G"];
/// Les six faces d'une partie, dans l'ordre ou l'editeur les propose.
pub const FACES: [&str; 6] = ["Devant", "Derrière", "Droite", "Gauche", "Dessus", "Dessous"];

/// Une partie du corps dans l'image : ses dimensions en pixels (largeur, hauteur, profondeur), le coin
/// de sa premiere couche et celui de la seconde.
struct Zone {
    size: (usize, usize, usize),
    base: (usize, usize),
    over: (usize, usize),
}

const ZONES: [Zone; 6] = [
    Zone { size: (8, 8, 8), base: (0, 0), over: (32, 0) },
    Zone { size: (8, 12, 4), base: (16, 16), over: (16, 32) },
    Zone { size: (4, 12, 4), base: (40, 16), over: (40, 32) },
    Zone { size: (4, 12, 4), base: (32, 48), over: (48, 48) },
    Zone { size: (4, 12, 4), base: (0, 16), over: (0, 32) },
    Zone { size: (4, 12, 4), base: (16, 48), over: (0, 48) },
];

/// Rectangle (x, y, largeur, hauteur) d'une face dans l'image. `face` suit l'ordre de `FACES`.
pub fn face_rect(part: usize, face: usize, over: bool) -> (usize, usize, usize, usize) {
    let z = &ZONES[part];
    let (w, h, d) = z.size;
    let (ox, oy) = if over { z.over } else { z.base };
    match face {
        0 => (ox + d, oy + d, w, h),
        1 => (ox + d + w + d, oy + d, w, h),
        2 => (ox, oy + d, d, h),
        3 => (ox + d + w, oy + d, d, h),
        4 => (ox + d, oy, w, d),
        _ => (ox + d + w, oy, w, d),
    }
}

fn px(skin: &mut [u8], x: usize, y: usize, c: [u8; 4]) {
    let i = (y * SIZE + x) * 4;
    skin[i..i + 4].copy_from_slice(&c);
}

pub fn get(skin: &[u8], x: usize, y: usize) -> [u8; 4] {
    let i = (y * SIZE + x) * 4;
    [skin[i], skin[i + 1], skin[i + 2], skin[i + 3]]
}

pub fn set(skin: &mut [u8], x: usize, y: usize, c: [u8; 4]) {
    if x < SIZE && y < SIZE {
        px(skin, x, y, c);
    }
}

fn fill(skin: &mut [u8], r: (usize, usize, usize, usize), c: [u8; 3]) {
    for y in r.1..r.1 + r.3 {
        for x in r.0..r.0 + r.2 {
            // Un leger grain, pour que les aplats ne soient pas plats.
            let k = if (x * 7 + y * 13) % 5 == 0 { 0.93 } else { 1.0 };
            px(skin, x, y, [(c[0] as f32 * k) as u8, (c[1] as f32 * k) as u8, (c[2] as f32 * k) as u8, 255]);
        }
    }
}

/// Le skin de depart : le voyageur du jeu, cheveux bleu nuit, veste orange, pantalon bleu, sandales rouges.
pub fn default_skin() -> Vec<u8> {
    let mut s = vec![0u8; BYTES];
    let (skin, hair, jacket, pants, red, white, eye) = ([255, 214, 180], [44, 48, 96], [255, 150, 60], [50, 58, 104], [232, 72, 84], [250, 250, 250], [60, 140, 232]);
    for face in 0..6 {
        fill(&mut s, face_rect(0, face, false), if face == 4 || face == 1 { hair } else { skin });
        fill(&mut s, face_rect(1, face, false), jacket);
        for arm in 2..4 {
            fill(&mut s, face_rect(arm, face, false), jacket);
        }
        for leg in 4..6 {
            fill(&mut s, face_rect(leg, face, false), pants);
        }
    }
    // Le visage : une frange, deux grands yeux a reflet, une bouche.
    let (fx, fy, _, _) = face_rect(0, 0, false);
    fill(&mut s, (fx, fy, 8, 2), hair);
    fill(&mut s, (fx, fy + 2, 1, 2), hair);
    fill(&mut s, (fx + 7, fy + 2, 1, 2), hair);
    for ex in [1, 5] {
        fill(&mut s, (fx + ex, fy + 4, 2, 2), white);
        px(&mut s, fx + ex + 1, fy + 4, [eye[0], eye[1], eye[2], 255]);
        px(&mut s, fx + ex + 1, fy + 5, [30, 40, 90, 255]);
    }
    fill(&mut s, (fx + 3, fy + 6, 2, 1), [196, 92, 96]);
    // Les cotes de la tete : les cheveux descendent sur les tempes.
    for side in [2, 3] {
        let (x, y, w, _) = face_rect(0, side, false);
        fill(&mut s, (x, y, w, 3), hair);
        fill(&mut s, (x + if side == 2 { 0 } else { w - 3 }, y + 3, 3, 3), hair);
    }
    // Le buste : un col blanc, une ceinture sombre.
    let (bx, by, _, _) = face_rect(1, 0, false);
    fill(&mut s, (bx + 3, by, 2, 9), white);
    for face in 0..4 {
        let (x, y, w, _) = face_rect(1, face, false);
        fill(&mut s, (x, y + 10, w, 1), [60, 50, 76]);
        fill(&mut s, (x, y + 11, w, 1), pants);
    }
    // Les mains nues au bout des manches, les sandales au bout des jambes.
    for arm in 2..4 {
        for face in 0..4 {
            let (x, y, w, _) = face_rect(arm, face, false);
            fill(&mut s, (x, y + 9, w, 3), skin);
        }
        fill(&mut s, face_rect(arm, 5, false), skin);
    }
    for leg in 4..6 {
        for face in 0..4 {
            let (x, y, w, _) = face_rect(leg, face, false);
            fill(&mut s, (x, y + 10, w, 2), red);
        }
        fill(&mut s, face_rect(leg, 5, false), red);
    }
    // Seconde couche : des meches qui depassent sur le dessus de la tete.
    let (hx, hy, _, _) = face_rect(0, 4, true);
    for (x, y) in [(1, 1), (2, 1), (5, 2), (6, 2), (3, 5), (4, 5), (1, 6), (6, 6)] {
        px(&mut s, hx + x, hy + y, [hair[0] + 14, hair[1] + 14, hair[2] + 20, 255]);
    }
    s
}

/// Le modele d'un joueur dont le skin occupe la case `slot` : six boites, et six autres a peine plus
/// grandes pour la seconde couche. Memes proportions que le joueur a toujours eues : les armures tombent juste.
pub fn model(slot: usize) -> Model {
    let (sx, sy) = origin(slot);
    // (centre, demi-tailles, attache, sens du balancement) de chaque partie, dans l'ordre de `PARTS`.
    let boxes: [(Vec3, Vec3, Vec3, i8); 6] = [
        (Vec3::new(0.0, 1.72, 0.0), Vec3::splat(0.22), Vec3::ZERO, 0),
        (Vec3::new(0.0, 1.1, 0.0), Vec3::new(0.25, 0.34, 0.125), Vec3::ZERO, 0),
        (Vec3::new(-0.36, 1.08, 0.0), Vec3::new(0.11, 0.36, 0.11), Vec3::new(-0.36, 1.44, 0.0), 1),
        (Vec3::new(0.36, 1.08, 0.0), Vec3::new(0.11, 0.36, 0.11), Vec3::new(0.36, 1.44, 0.0), -1),
        (Vec3::new(-0.125, 0.38, 0.0), Vec3::new(0.125, 0.38, 0.125), Vec3::new(-0.125, 0.76, 0.0), -1),
        (Vec3::new(0.125, 0.38, 0.0), Vec3::new(0.125, 0.38, 0.125), Vec3::new(0.125, 0.76, 0.0), 1),
    ];
    let uv = |part: usize, face: usize, over: bool| -> [f32; 4] {
        let (x, y, w, h) = face_rect(part, face, over);
        // Un cheveu a l'interieur du rectangle : les voisins ne bavent pas.
        let e = 0.02;
        let at = AT as f32;
        [(sx + x) as f32 + e, (sy + y) as f32 + e, (sx + x + w) as f32 - e, (sy + y + h) as f32 - e].map(|v| v / at).pipe(|r| [r[0], 1.0 + r[1], r[2], 1.0 + r[3]])
    };
    let mut parts = Vec::new();
    for over in [false, true] {
        for (k, (c, h, pivot, leg)) in boxes.iter().enumerate() {
            let grow = if over { if k == 0 { 0.022 } else { 0.012 } } else { 0.0 };
            // Faces d'une boite : +X (la gauche du personnage), -X (sa droite), dessus, dessous, devant, derriere.
            let faces = [uv(k, 3, over), uv(k, 2, over), uv(k, 4, over), uv(k, 5, over), uv(k, 0, over), uv(k, 1, over)];
            parts.push(Part { c: *c, h: *h + Vec3::splat(grow), rot: Vec3::ZERO, pivot: if *leg == 0 { *c } else { *pivot }, leg: *leg, uv: faces });
        }
    }
    Model { parts }
}

trait Pipe: Sized {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R {
        f(self)
    }
}
impl<T> Pipe for T {}

/// Lit un skin dans une image PNG de 64 x 64 (ou 64 x 32, l'ancien format : bras et jambes gauches
/// sont alors copies des droits).
pub fn decode(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut dec = png::Decoder::new(data);
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().map_err(|e| format!("image illisible : {e}"))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("image illisible : {e}"))?;
    if info.width != 64 || (info.height != 64 && info.height != 32) {
        return Err(format!("un skin fait 64 x 64 pixels (celui-ci : {} x {})", info.width, info.height));
    }
    let n = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        _ => return Err("format de couleurs non pris en charge".into()),
    };
    let mut skin = vec![0u8; BYTES];
    for y in 0..info.height as usize {
        for x in 0..SIZE {
            let p = &buf[(y * SIZE + x) * n..];
            let c = match n {
                4 => [p[0], p[1], p[2], p[3]],
                3 => [p[0], p[1], p[2], 255],
                2 => [p[0], p[0], p[0], p[1]],
                _ => [p[0], p[0], p[0], 255],
            };
            px(&mut skin, x, y, c);
        }
    }
    if info.height == 32 {
        for (from, to) in [(2, 3), (4, 5)] {
            for face in 0..6 {
                let (ax, ay, w, h) = face_rect(from, face, false);
                let (bx, by, _, _) = face_rect(to, face, false);
                for y in 0..h {
                    for x in 0..w {
                        let c = get(&skin, ax + x, ay + y);
                        px(&mut skin, bx + w - 1 - x, by + y, c);
                    }
                }
            }
        }
    }
    // La premiere couche est toujours pleine : un trou y laisserait voir a travers le corps.
    for part in 0..6 {
        for face in 0..6 {
            let (x0, y0, w, h) = face_rect(part, face, false);
            for y in y0..y0 + h {
                for x in x0..x0 + w {
                    skin[(y * SIZE + x) * 4 + 3] = 255;
                }
            }
        }
    }
    Ok(skin)
}

pub fn encode(skin: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, SIZE as u32, SIZE as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        if let Ok(mut w) = enc.write_header() {
            let _ = w.write_image_data(skin);
        }
    }
    out
}

/// Le skin enregistre du joueur, ou celui de depart.
pub fn load() -> Vec<u8> {
    std::fs::read(FILE).ok().and_then(|d| decode(&d).ok()).unwrap_or_else(default_skin)
}

pub fn save(skin: &[u8]) -> Result<(), String> {
    let _ = std::fs::create_dir_all("saves");
    std::fs::write(FILE, encode(skin)).map_err(|e| e.to_string())
}

/// Dessine un personnage dans l'interface : `view` l'oriente, `walk` balance ses membres, `feet` est la
/// place de ses pieds et `scale` la taille d'un bloc a l'ecran.
pub fn doll(ui: &mut Ui, model: &Model, view: Mat3, walk: f32, cx: f32, feet: f32, scale: f32) {
    let mut faces: Vec<(f32, [[f32; 2]; 4], [[f32; 2]; 4], f32)> = Vec::new();
    for p in &model.parts {
        let mut q = Quat::from_euler(EulerRot::ZYX, p.rot.z.to_radians(), p.rot.y.to_radians(), p.rot.x.to_radians());
        if p.leg != 0 {
            q = Quat::from_rotation_x(walk.sin() * 0.6 * p.leg as f32) * q;
        }
        let center = p.pivot + q * (p.c - p.pivot);
        for (f, (n, corners)) in crate::mesh::FACES.iter().enumerate() {
            let normal = view * (q * Vec3::new(n[0] as f32, n[1] as f32, n[2] as f32));
            if normal.z <= 0.01 {
                continue;
            }
            let (mut pts, mut uvs, mut depth) = ([[0.0; 2]; 4], [[0.0; 2]; 4], 0.0);
            for k in 0..4 {
                let v = view * (center + q * ((Vec3::from(corners[k]) * 2.0 - Vec3::ONE) * p.h));
                pts[k] = [cx + v.x * scale, feet - v.y * scale];
                depth += v.z / 4.0;
                let [a, b] = crate::mesh::UVS[k];
                uvs[k] = [p.uv[f][0] + a * (p.uv[f][2] - p.uv[f][0]), p.uv[f][1] + b * (p.uv[f][3] - p.uv[f][1])];
            }
            faces.push((depth, pts, uvs, crate::mesh::SHADE[f] as f32 / 255.0));
        }
    }
    faces.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, pts, uvs, shade) in faces {
        ui.quad4(pts, uvs, [[shade, shade, shade, 1.0]; 4]);
    }
}

// ---------------------------------------------------------------------------------------------
// L'editeur.

/// Ce que l'editeur demande a qui l'affiche.
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Ask {
    Nothing,
    /// Fermer l'editeur.
    Close,
    /// Le skin vient d'etre enregistre : il faut le montrer aux autres joueurs.
    Saved,
}

pub const TOOLS: [&str; 4] = ["Pinceau", "Gomme", "Pot", "Pipette"];

pub struct Editor {
    /// Le skin en cours de dessin.
    pub px: Vec<u8>,
    /// Il a change depuis la derniere fois qu'on l'a envoye dans l'atlas.
    pub dirty: bool,
    part: usize,
    face: usize,
    over: bool,
    tool: usize,
    color: [u8; 3],
    mirror: bool,
    undo: Vec<Vec<u8>>,
    /// Angle du personnage dans l'apercu ; il tourne tout seul tant qu'on ne le saisit pas.
    spin: f32,
    held: bool,
    /// Ce qui est enregistre : « Retour » sans enregistrer y revient.
    saved: Vec<u8>,
    pub note: String,
    /// Onglet : 0 l'editeur, 1 la bibliotheque.
    pub tab: usize,
}

/// La palette : des gris, puis huit teintes en cinq tons.
fn palette() -> Vec<[u8; 3]> {
    let mut out: Vec<[u8; 3]> = [0u8, 40, 80, 120, 160, 200, 230, 255].iter().map(|v| [*v, *v, *v]).collect();
    let hues: [[f32; 3]; 8] = [[1.0, 0.1, 0.1], [1.0, 0.5, 0.1], [1.0, 0.9, 0.2], [0.3, 0.8, 0.25], [0.15, 0.75, 0.75], [0.2, 0.4, 1.0], [0.6, 0.3, 0.9], [1.0, 0.4, 0.7]];
    for tone in [0.35f32, 0.6, 0.85, 1.0, 1.25] {
        for h in hues {
            // Au-dela de 1, le ton tire vers le blanc.
            out.push(h.map(|c| if tone <= 1.0 { (c * tone * 255.0) as u8 } else { ((c + (1.0 - c) * (tone - 1.0) * 2.4).min(1.0) * 255.0) as u8 }));
        }
    }
    // Quelques teintes de peau et de cuir.
    out.extend([[255, 224, 196], [255, 214, 180], [232, 180, 140], [196, 140, 100], [150, 100, 70], [110, 70, 48], [80, 52, 36], [50, 34, 26]]);
    out
}

fn button(ui: &mut Ui, mouse: (f32, f32), x: f32, y: f32, w: f32, h: f32, s: f32, text: &str, on: bool) -> bool {
    let hot = mouse.0 >= x && mouse.0 < x + w && mouse.1 >= y && mouse.1 < y + h;
    let fill = match (on, hot) {
        (true, _) => [0.85, 0.65, 0.16, 1.0],
        (false, true) => [0.34, 0.34, 0.44, 1.0],
        (false, false) => [0.17, 0.17, 0.23, 1.0],
    };
    ui.bevel(x, y, w, h, s, fill, [0.4, 0.4, 0.5, 1.0], [0.06, 0.06, 0.09, 1.0]);
    ui.text((x + (w - Ui::text_width(text, s)) / 2.0).floor(), (y + (h - 8.0 * s) / 2.0).floor(), s, text, [1.0; 4]);
    hot
}

impl Editor {
    pub fn open() -> Editor {
        let px = load();
        Editor { saved: px.clone(), px, dirty: true, part: 0, face: 0, over: false, tool: 0, color: [232, 72, 84], mirror: false, undo: Vec::new(), spin: 0.5, held: false, note: String::new(), tab: 0 }
    }

    /// Prend un skin venu d'ailleurs (un fichier, la bibliotheque) comme point de depart.
    pub fn take(&mut self, px: Vec<u8>) {
        self.remember();
        self.px = px;
        self.dirty = true;
    }

    /// Ce qui etait enregistre : pour le remettre si l'on quitte sans enregistrer.
    pub fn saved(&self) -> &[u8] {
        &self.saved
    }

    fn remember(&mut self) {
        self.undo.push(self.px.clone());
        if self.undo.len() > 40 {
            self.undo.remove(0);
        }
    }

    /// Pose l'outil sur le pixel (x, y) de la face en cours ; `first` : le trait commence.
    fn paint(&mut self, x: usize, y: usize, first: bool) {
        let (x0, y0, w, h) = face_rect(self.part, self.face, self.over);
        let at = |px: &[u8], x: usize, y: usize| get(px, x0 + x, y0 + y);
        match self.tool {
            3 => {
                let c = at(&self.px, x, y);
                if c[3] > 0 {
                    self.color = [c[0], c[1], c[2]];
                }
                self.tool = 0;
            }
            2 => {
                if !first {
                    return;
                }
                // Remplit la plage de pixels de meme couleur qui touche celui-ci, sans sortir de la face.
                let (from, to) = (at(&self.px, x, y), [self.color[0], self.color[1], self.color[2], 255]);
                if from == to {
                    return;
                }
                let mut todo = vec![(x, y)];
                while let Some((cx, cy)) = todo.pop() {
                    if at(&self.px, cx, cy) != from {
                        continue;
                    }
                    set(&mut self.px, x0 + cx, y0 + cy, to);
                    if cx > 0 {
                        todo.push((cx - 1, cy));
                    }
                    if cy > 0 {
                        todo.push((cx, cy - 1));
                    }
                    if cx + 1 < w {
                        todo.push((cx + 1, cy));
                    }
                    if cy + 1 < h {
                        todo.push((cx, cy + 1));
                    }
                }
                self.dirty = true;
            }
            tool => {
                // La gomme n'efface que la seconde couche : la premiere reste pleine.
                if tool == 1 && !self.over {
                    return;
                }
                let c = if tool == 1 { [0, 0, 0, 0] } else { [self.color[0], self.color[1], self.color[2], 255] };
                set(&mut self.px, x0 + x, y0 + y, c);
                if self.mirror {
                    set(&mut self.px, x0 + w - 1 - x, y0 + y, c);
                }
                self.dirty = true;
            }
        }
    }

    /// Dessine l'editeur dans le cadre (x, y, w, h). `click` : le bouton vient d'etre enfonce ; `down` : il est tenu.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, ui: &mut Ui, model: &Model, mouse: (f32, f32), click: bool, down: bool, x: f32, y: f32, w: f32, h: f32, s: f32, dt: f32) -> Ask {
        let mut ask = Ask::Nothing;
        let bh = 14.0 * s;
        let hit = |bx: f32, by: f32, bw: f32, bhh: f32| mouse.0 >= bx && mouse.0 < bx + bw && mouse.1 >= by && mouse.1 < by + bhh;

        // ---- A gauche : le personnage, que l'on fait tourner en le saisissant.
        let (pw, ph) = (96.0 * s, h - 34.0 * s);
        ui.bevel(x, y, pw, ph, s, [0.04, 0.05, 0.11, 1.0], [0.06, 0.06, 0.09, 1.0], [0.3, 0.3, 0.38, 1.0]);
        if click && hit(x, y, pw, ph) {
            self.held = true;
        }
        if !down {
            self.held = false;
        }
        if self.held {
            self.spin = (mouse.0 - (x + pw / 2.0)) / (pw / 2.0) * 3.2;
        } else if let Some(fixed) = std::env::var("EPICUBE_SPIN").ok().and_then(|v| v.parse::<f32>().ok()) {
            // Aide au developpement : l'apercu fige a cet angle.
            self.spin = fixed;
        } else {
            self.spin += dt * 0.6;
        }
        let view = Mat3::from_rotation_x(0.14) * Mat3::from_rotation_y(self.spin);
        let scale = ((ph - 14.0 * s) / 2.15).min(pw / 1.25);
        doll(ui, model, view, (self.spin * 2.0).sin() * 0.25, x + pw / 2.0, y + ph - 7.0 * s, scale);

        // ---- Au milieu : les parties du corps, les faces, et la face choisie en grand.
        let mx = x + pw + 8.0 * s;
        for (k, name) in PARTS.iter().enumerate() {
            let (bx, by) = (mx + (k % 3) as f32 * 62.0 * s, y + (k / 3) as f32 * (bh + 2.0 * s));
            if button(ui, mouse, bx, by, 60.0 * s, bh, s, name, self.part == k) && click {
                self.part = k;
            }
        }
        let fy = y + 2.0 * (bh + 2.0 * s) + 2.0 * s;
        for (k, name) in FACES.iter().enumerate() {
            let bx = mx + (k % 3) as f32 * 62.0 * s;
            let by = fy + (k / 3) as f32 * (bh + 2.0 * s);
            if button(ui, mouse, bx, by, 60.0 * s, bh, s, name, self.face == k) && click {
                self.face = k;
            }
        }
        let (x0, y0, fw, fh) = face_rect(self.part, self.face, self.over);
        let cy = fy + 2.0 * (bh + 2.0 * s) + 4.0 * s;
        let room = (y + h - 34.0 * s - cy).max(40.0 * s);
        let cell = ((186.0 * s / fw as f32).min(room / fh as f32) / s).floor().max(2.0) * s;
        let cx = mx + ((186.0 * s - cell * fw as f32) / 2.0).floor();
        ui.rect(cx - s, cy - s, cell * fw as f32 + 2.0 * s, cell * fh as f32 + 2.0 * s, [0.0, 0.0, 0.0, 1.0]);
        for py in 0..fh {
            for px_ in 0..fw {
                let (qx, qy) = (cx + px_ as f32 * cell, cy + py as f32 * cell);
                // Un damier sous les pixels transparents de la seconde couche.
                let dark = (px_ + py) % 2 == 0;
                ui.rect(qx, qy, cell, cell, if dark { [0.22, 0.22, 0.26, 1.0] } else { [0.3, 0.3, 0.34, 1.0] });
                let c = get(&self.px, x0 + px_, y0 + py);
                if c[3] > 0 {
                    ui.rect(qx, qy, cell - s * 0.5, cell - s * 0.5, [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0]);
                } else if self.over {
                    // Sous un pixel vide de la seconde couche, on devine la premiere.
                    let (ux, uy, _, _) = face_rect(self.part, self.face, false);
                    let u = get(&self.px, ux + px_, uy + py);
                    ui.rect(qx + cell * 0.25, qy + cell * 0.25, cell * 0.5, cell * 0.5, [u[0] as f32 / 255.0, u[1] as f32 / 255.0, u[2] as f32 / 255.0, 0.55]);
                }
            }
        }
        if down && hit(cx, cy, cell * fw as f32, cell * fh as f32) {
            if click {
                self.remember();
            }
            let (px_, py) = (((mouse.0 - cx) / cell) as usize, ((mouse.1 - cy) / cell) as usize);
            self.paint(px_.min(fw - 1), py.min(fh - 1), click);
        }

        // ---- A droite : la couche, les outils, la couleur.
        let rx = mx + 194.0 * s;
        let rw = (x + w - rx).max(96.0 * s);
        let half = (rw - 2.0 * s) / 2.0;
        if button(ui, mouse, rx, y, half, bh, s, "Couche 1", !self.over) && click {
            self.over = false;
        }
        if button(ui, mouse, rx + half + 2.0 * s, y, half, bh, s, "Couche 2", self.over) && click {
            self.over = true;
        }
        for (k, name) in TOOLS.iter().enumerate() {
            let (bx, by) = (rx + (k % 2) as f32 * (half + 2.0 * s), y + (1 + k / 2) as f32 * (bh + 2.0 * s));
            if button(ui, mouse, bx, by, half, bh, s, name, self.tool == k) && click {
                self.tool = k;
            }
        }
        let ty = y + 3.0 * (bh + 2.0 * s);
        if button(ui, mouse, rx, ty, half, bh, s, "Miroir", self.mirror) && click {
            self.mirror = !self.mirror;
        }
        if button(ui, mouse, rx + half + 2.0 * s, ty, half, bh, s, "Annuler", false) && click {
            if let Some(before) = self.undo.pop() {
                self.px = before;
                self.dirty = true;
            }
        }
        // La couleur en main, puis la palette.
        let py0 = ty + bh + 5.0 * s;
        ui.rect(rx, py0, rw, 9.0 * s, [0.0, 0.0, 0.0, 1.0]);
        ui.rect(rx + s, py0 + s, rw - 2.0 * s, 7.0 * s, [self.color[0] as f32 / 255.0, self.color[1] as f32 / 255.0, self.color[2] as f32 / 255.0, 1.0]);
        let sw = (rw / 8.0).floor();
        for (k, c) in palette().into_iter().enumerate() {
            let (qx, qy) = (rx + (k % 8) as f32 * sw, py0 + 12.0 * s + (k / 8) as f32 * sw);
            ui.rect(qx, qy, sw - s, sw - s, [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0]);
            if c == self.color {
                ui.rect(qx, qy + sw - 2.0 * s, sw - s, s, [1.0; 4]);
            }
            if click && hit(qx, qy, sw - s, sw - s) {
                self.color = c;
                if self.tool == 1 || self.tool == 3 {
                    self.tool = 0;
                }
            }
        }

        // ---- En bas : enregistrer, repartir du skin de depart, lire ou ecrire le fichier, revenir.
        let by = y + h - 28.0 * s;
        let bw = ((w - 8.0 * s) / 5.0).floor();
        let at = |k: usize| x + k as f32 * (bw + 2.0 * s);
        if button(ui, mouse, at(0), by, bw, bh, s, "Enregistrer", false) && click {
            match save(&self.px) {
                Ok(()) => {
                    self.saved = self.px.clone();
                    self.note = "Skin enregistré.".into();
                    ask = Ask::Saved;
                }
                Err(e) => self.note = format!("Enregistrement impossible : {e}"),
            }
        }
        if button(ui, mouse, at(1), by, bw, bh, s, "Skin de départ", false) && click {
            self.take(default_skin());
        }
        if button(ui, mouse, at(2), by, bw, bh, s, "Importer", false) && click {
            // Un skin de Minecraft pose dans le dossier du jeu sous le nom « skin.png ».
            match std::fs::read("skin.png").map_err(|_| "posez un fichier skin.png (64 x 64) dans le dossier du jeu".to_string()).and_then(|d| decode(&d)) {
                Ok(px) => {
                    self.take(px);
                    self.note = "skin.png importé : enregistrez pour le garder.".into();
                }
                Err(e) => self.note = format!("Import : {e}."),
            }
        }
        if button(ui, mouse, at(3), by, bw, bh, s, "Exporter", false) && click {
            self.note = match std::fs::write("skin.png", encode(&self.px)) {
                Ok(()) => "Écrit dans skin.png, dans le dossier du jeu.".into(),
                Err(e) => format!("Export impossible : {e}"),
            };
        }
        if button(ui, mouse, at(4), by, bw, bh, s, "Retour", false) && click {
            ask = Ask::Close;
        }
        ui.text(x, by + bh + 4.0 * s, s, &self.note, [0.6, 1.0, 0.6, 1.0]);
        ask
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_tile_the_image_without_overlap() {
        // Chaque pixel d'une face n'appartient qu'a elle, sur les deux couches.
        let mut owner = vec![0u16; SIZE * SIZE];
        for over in [false, true] {
            for part in 0..6 {
                for face in 0..6 {
                    let (x0, y0, w, h) = face_rect(part, face, over);
                    assert!(x0 + w <= SIZE && y0 + h <= SIZE);
                    for y in y0..y0 + h {
                        for x in x0..x0 + w {
                            assert_eq!(owner[y * SIZE + x], 0, "partie {part}, face {face}");
                            owner[y * SIZE + x] = 1;
                        }
                    }
                }
            }
        }
        // Le visage est bien la ou Minecraft le met : (8, 8), huit pixels de cote.
        assert_eq!(face_rect(0, 0, false), (8, 8, 8, 8));
        assert_eq!(face_rect(1, 0, false), (20, 20, 8, 12));
        assert_eq!(face_rect(3, 0, false), (36, 52, 4, 12));
    }

    #[test]
    fn skins_survive_the_png() {
        let skin = default_skin();
        assert_eq!(decode(&encode(&skin)).unwrap(), skin);
        assert!(decode(b"pas une image").is_err());
        // Le skin de depart couvre toute la premiere couche.
        for part in 0..6 {
            for face in 0..6 {
                let (x, y, _, _) = face_rect(part, face, false);
                assert_eq!(get(&skin, x, y)[3], 255);
            }
        }
        // Les cases de l'atlas tiennent sous les tuiles des mods, au-dessus de leurs modeles.
        let (_, y) = origin(SLOTS - 1);
        assert!(y + SIZE <= AT - 2 * 256);
    }

    #[test]
    fn the_editor_paints_fills_mirrors_and_undoes() {
        let mut ed = Editor { px: default_skin(), saved: default_skin(), dirty: false, part: 1, face: 0, over: false, tool: 0, color: [1, 2, 3], mirror: true, undo: Vec::new(), spin: 0.0, held: false, note: String::new(), tab: 0 };
        let (x0, y0, w, _) = face_rect(1, 0, false);
        // Le pinceau, en miroir : le pixel et son symetrique.
        ed.remember();
        ed.paint(1, 2, true);
        assert_eq!(get(&ed.px, x0 + 1, y0 + 2), [1, 2, 3, 255]);
        assert_eq!(get(&ed.px, x0 + w - 2, y0 + 2), [1, 2, 3, 255]);
        // La gomme ne troue pas la premiere couche ; elle efface la seconde.
        ed.tool = 1;
        ed.paint(1, 2, true);
        assert_eq!(get(&ed.px, x0 + 1, y0 + 2)[3], 255);
        ed.over = true;
        let (ox, oy, _, _) = face_rect(1, 0, true);
        ed.tool = 0;
        ed.paint(0, 0, true);
        ed.tool = 1;
        ed.paint(0, 0, true);
        assert_eq!(get(&ed.px, ox, oy)[3], 0);
        // Le pot remplit une plage sans sortir de la face ; la pipette reprend une couleur.
        ed.over = false;
        ed.part = 4;
        ed.tool = 2;
        ed.color = [9, 9, 9];
        let before = ed.px.clone();
        ed.paint(0, 0, true);
        let (lx, ly, lw, lh) = face_rect(4, 0, false);
        assert_eq!(get(&ed.px, lx, ly), [9, 9, 9, 255]);
        for y in 0..SIZE {
            for x in 0..SIZE {
                let inside = x >= lx && x < lx + lw && y >= ly && y < ly + lh;
                assert!(inside || get(&ed.px, x, y) == get(&before, x, y), "le pot a deborde en {x}, {y}");
            }
        }
        ed.tool = 3;
        ed.color = [0, 0, 0];
        ed.paint(0, 0, true);
        assert_eq!((ed.color, ed.tool), ([9, 9, 9], 0));
        // Annuler revient au skin d'avant le premier trait.
        let first = ed.undo.first().cloned().unwrap();
        ed.px = first.clone();
        assert_eq!(ed.px, default_skin());
    }
}
