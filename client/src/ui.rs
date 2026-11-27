//! Dessin 2D de l'interface, en pixels (origine en haut a gauche), en pixel art :
//! police a points, cadres aux coins en escalier, tout aligne sur la grille.
use crate::atlas::{glyph, tile_uv, FONT_PX};
use crate::gfx::V2;
use shared::blocks::{self, t, Shape};
use shared::items::{self, Stack};

/// Encre des contours de l'interface.
pub const INK: [f32; 4] = [0.07, 0.06, 0.13, 1.0];

pub struct Ui {
    pub v: Vec<V2>,
    pub w: f32,
    pub h: f32,
}

fn mixc(a: [f32; 4], b: [f32; 4], k: f32) -> [f32; 4] {
    [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k, a[3]]
}

impl Ui {
    pub fn new(w: f32, h: f32) -> Ui {
        Ui { v: Vec::new(), w, h }
    }

    fn ndc(&self, p: [f32; 2]) -> [f32; 2] {
        [p[0] / self.w * 2.0 - 1.0, 1.0 - p[1] / self.h * 2.0]
    }

    /// Quadrilatere quelconque : quatre points, leurs UV et leurs couleurs.
    pub fn quad4(&mut self, p: [[f32; 2]; 4], uv: [[f32; 2]; 4], c: [[f32; 4]; 4]) {
        for k in [0, 1, 2, 0, 2, 3] {
            self.v.push(V2 { pos: self.ndc(p[k]), uv: uv[k], color: c[k] });
        }
    }

    fn quad(&mut self, x: f32, y: f32, w: f32, h: f32, uv: [f32; 4], color: [f32; 4]) {
        self.quad4(
            [[x, y], [x + w, y], [x + w, y + h], [x, y + h]],
            [[uv[0], uv[1]], [uv[2], uv[1]], [uv[2], uv[3]], [uv[0], uv[3]]],
            [color; 4],
        );
    }

    pub fn white() -> [f32; 2] {
        let (u, v) = tile_uv(t::WHITE);
        [u + 0.5 / 32.0, v + 0.5 / 32.0]
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        let [u, v] = Self::white();
        self.quad(x, y, w, h, [u, v, u, v], color);
    }

    /// Rectangle dont chaque coin a sa couleur (haut-gauche, haut-droit, bas-droit, bas-gauche).
    pub fn gradient(&mut self, x: f32, y: f32, w: f32, h: f32, c: [[f32; 4]; 4]) {
        let uv = Self::white();
        self.quad4([[x, y], [x + w, y], [x + w, y + h], [x, y + h]], [uv; 4], c);
    }

    /// Disque en pixel art : des bandes horizontales de `step` pixels de haut.
    pub fn disc(&mut self, cx: f32, cy: f32, radius: f32, step: f32, color: [f32; 4]) {
        let n = (radius / step).floor() as i32;
        for j in -n..n {
            let y = (j as f32 + 0.5) * step;
            let half = ((radius * radius - y * y).max(0.0).sqrt() / step).round() * step;
            self.rect(cx - half, cy + j as f32 * step, half * 2.0, step, color);
        }
    }

    /// Cadre en pixel art : contour a l'encre aux coins coupes, liseres clair en haut et sombre en bas.
    /// `t` est la taille d'un pixel d'interface.
    #[allow(clippy::too_many_arguments)]
    pub fn bevel(&mut self, x: f32, y: f32, w: f32, h: f32, t: f32, fill: [f32; 4], light: [f32; 4], dark: [f32; 4]) {
        let (x, y, w, h) = (x.floor(), y.floor(), w.floor(), h.floor());
        let ink = [INK[0], INK[1], INK[2], fill[3]];
        self.rect(x - t, y, w + 2.0 * t, h, ink);
        self.rect(x, y - t, w, h + 2.0 * t, ink);
        self.rect(x, y, w, h, fill);
        self.rect(x, y, w, t, mixc(fill, light, 0.7));
        self.rect(x, y + t, t, h - 2.0 * t, mixc(fill, light, 0.35));
        self.rect(x, y + h - t, w, t, mixc(fill, dark, 0.7));
        self.rect(x + w - t, y + t, t, h - 2.0 * t, mixc(fill, dark, 0.35));
    }

    /// Tuile de l'atlas ; `frac` < 1 n'en dessine que la partie gauche.
    pub fn tile(&mut self, x: f32, y: f32, size: f32, tile: u16, frac: f32, color: [f32; 4]) {
        let (u, v) = tile_uv(tile);
        let e = 1.0 / 32.0;
        self.quad(x.floor(), y.floor(), (size * frac).floor(), size.floor(), [u, v, u + e * frac, v + e], color);
    }

    /// Bloc vu en perspective : dessus, face gauche et face droite.
    fn cube(&mut self, x: f32, y: f32, size: f32, b: u8) {
        let e = 1.0 / 32.0;
        let pt = |px: f32, py: f32| [x + px * size, y + py * size];
        let face = |ui: &mut Ui, p: [[f32; 2]; 4], side: usize, shade: f32| {
            let (u, v) = tile_uv(blocks::tile(b, 0, side));
            ui.quad4(p, [[u, v], [u + e, v], [u + e, v + e], [u, v + e]], [[shade, shade, shade, 1.0]; 4]);
        };
        // Silhouette a l'encre, puis les trois faces.
        let uvw = Self::white();
        let o = 0.04;
        let hexa = [pt(0.5, 0.04 - o), pt(0.96 + o, 0.27), pt(0.96 + o, 0.75), pt(0.5, 0.98 + o), pt(0.04 - o, 0.75), pt(0.04 - o, 0.27)];
        for k in 1..5 {
            for q in [hexa[0], hexa[k], hexa[k + 1]] {
                self.v.push(V2 { pos: self.ndc(q), uv: uvw, color: INK });
            }
        }
        face(self, [pt(0.04, 0.27), pt(0.5, 0.04), pt(0.96, 0.27), pt(0.5, 0.5)], 2, 1.0);
        face(self, [pt(0.04, 0.27), pt(0.5, 0.5), pt(0.5, 0.98), pt(0.04, 0.75)], 4, 0.8);
        face(self, [pt(0.5, 0.5), pt(0.96, 0.27), pt(0.96, 0.75), pt(0.5, 0.98)], 0, 0.62);
    }

    /// Icone d'un objet, sans quantite.
    pub fn icon(&mut self, x: f32, y: f32, size: f32, id: u16, tint: [f32; 4]) {
        if id < 256 && blocks::shape(id as u8) == Shape::Cube && tint[0] > 0.9 {
            self.cube(x, y, size, id as u8);
        } else {
            self.tile(x, y, size, items::icon(id), 1.0, tint);
        }
    }

    /// Icone d'une pile, avec sa quantite et sa barre d'usure.
    pub fn item(&mut self, x: f32, y: f32, size: f32, st: &Stack) {
        self.icon(x, y, size, st.id, [1.0; 4]);
        let s = size / 16.0;
        if st.n > 1 {
            let txt = st.n.to_string();
            self.text(x + size - Ui::text_width(&txt, s) + s, y + size - 8.0 * s, s, &txt, [1.0; 4]);
        }
        let max = items::max_dur(st.id);
        if max > 0 && st.dur < max {
            let k = st.dur as f32 / max as f32;
            self.rect(x + s, y + size - 3.0 * s, size - 2.0 * s, 2.0 * s, INK);
            self.rect(x + s, y + size - 3.0 * s, ((size - 2.0 * s) * k / s).ceil() * s, s, [1.0 - k, 0.3 + 0.6 * k, 0.2, 1.0]);
        }
    }

    /// Nombre de pixels d'ecran par point de la police, pour l'echelle `scale`.
    /// Toujours entier : les lettres restent nettes. La police fait 16 points de haut et les appelants
    /// prevoient une ligne de 8 x echelle : on arrondit donc vers le bas. A une echelle impaire, les
    /// lettres sont un peu plus petites que leur ligne, mais elles n'en debordent jamais (arrondir au
    /// plus proche les faisait depasser d'un tiers : lignes qui se chevauchent, texte coupe et decale).
    fn font_k(scale: f32) -> f32 {
        (scale / 2.0).floor().max(1.0)
    }

    pub fn text_width(s: &str, scale: f32) -> f32 {
        s.chars().map(|c| glyph(c).adv).sum::<f32>() * Self::font_k(scale)
    }

    fn glyphs(&mut self, x: f32, y: f32, k: f32, s: &str, color: [f32; 4]) {
        let mut pen = x;
        for ch in s.chars() {
            let g = glyph(ch);
            if g.w > 0.5 && ch != ' ' {
                self.quad(pen + g.ox * k, y + g.oy * k, g.w * k, g.h * k, g.uv, color);
            }
            pen += g.adv * k;
        }
    }

    /// Texte en police a points, cerne d'encre, lisible sur tous les fonds.
    pub fn text(&mut self, x: f32, y: f32, scale: f32, s: &str, color: [f32; 4]) {
        let k = Self::font_k(scale);
        // La ligne de 16 points est centree sur la hauteur prevue par les appelants (8 x echelle).
        let (x, y) = (x.floor(), (y + (8.0 * scale - FONT_PX * k) / 2.0).floor());
        let ink = [INK[0], INK[1], INK[2], color[3]];
        for (dx, dy) in [(-k, 0.0), (k, 0.0), (0.0, -k), (0.0, k), (-k, -k), (k, -k), (-k, k), (k, k)] {
            self.glyphs(x + dx, y + dy, k, s, ink);
        }
        self.glyphs(x, y, k, s, color);
    }
}
