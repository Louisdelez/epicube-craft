//! Interface en jeu : barre rapide, vie, faim, fenetres d'inventaire et liste des recettes.
use crate::models::Model;
use crate::ui::Ui;
use glam::{EulerRot, Mat3, Quat, Vec3};
use crate::{App, Input};
use shared::blocks::t;
use shared::craft::{self, Recipe};
use shared::items::{self, Slot};
use shared::proto::{ClientMsg, SlotRef, WindowKind};

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const PANEL: [f32; 4] = [0.137, 0.184, 0.333, 0.97];
const PANEL_LIGHT: [f32; 4] = [0.36, 0.44, 0.68, 1.0];
const PANEL_DARK: [f32; 4] = [0.06, 0.08, 0.17, 1.0];
const SLOT: [f32; 4] = [0.086, 0.114, 0.227, 1.0];
const SLOT_HOT: [f32; 4] = [0.24, 0.31, 0.53, 1.0];
/// Colonnes de la liste des recettes.
const RCOLS: usize = 7;

pub struct Layout {
    /// Inventaire creatif : (objet, x, y) des cases affichees.
    pub palette: Vec<(u16, f32, f32)>,
    pub cell: f32,
    pub panel: [f32; 4],
    pub slots: Vec<(SlotRef, f32, f32)>,
    pub rpanel: [f32; 4],
    /// (indice de la recette, x, y)
    pub recipes: Vec<(usize, f32, f32)>,
    /// Coin haut-gauche de la zone superieure (grille, four ou coffre).
    pub top: (f32, f32),
    /// Offres d'un villageois : (indice, x, y), chacune large de quatre cases.
    pub trades: Vec<(usize, f32, f32)>,
}

fn inside(m: (f32, f32), x: f32, y: f32, size: f32) -> bool {
    m.0 >= x && m.0 < x + size && m.1 >= y && m.1 < y + size
}

/// Case en creux : bord sombre en haut a gauche.
fn slot(ui: &mut Ui, x: f32, y: f32, size: f32, s: f32, hot: bool) {
    ui.bevel(x, y, size, size, s, if hot { SLOT_HOT } else { SLOT }, PANEL_DARK, PANEL_LIGHT);
}

impl App {
    /// Recettes proposees dans la fenetre ouverte.
    fn recipe_list(&self) -> Vec<(usize, &'static Recipe)> {
        craft::recipes()
            .iter()
            .enumerate()
            .filter(|(_, r)| match self.window {
                WindowKind::Table => true,
                WindowKind::Inventory => r.w <= 2 && r.h <= 2,
                _ => false,
            })
            .collect()
    }

    pub fn layout(&self, w: f32, h: f32, s: f32) -> Layout {
        let cell = 18.0 * s;
        // En creatif, l'inventaire propose tous les objets a la place des recettes.
        let creative = self.window == WindowKind::Inventory && self.mode == shared::rules::CREATIVE;
        let recipes = if creative { Vec::new() } else { self.recipe_list() };
        // L'inventaire a une rangee de plus en haut : quatre cases d'equipement et le personnage.
        let rows = if self.window == WindowKind::Inventory { 4.0 } else { 3.0 };
        let (pw, ph) = (9.0 * cell + 16.0 * s, (4.0 + rows) * cell + 38.0 * s);
        let rw = if recipes.is_empty() && !creative { 0.0 } else { RCOLS as f32 * cell + 12.0 * s + 4.0 * s };
        let (px, py) = (((w - pw - rw) / 2.0).floor(), ((h - ph) / 2.0).floor());
        let x0 = px + 8.0 * s;
        let yt = py + 18.0 * s;
        let mut slots = Vec::new();
        match self.window {
            WindowKind::Inventory => {
                for i in [0u8, 1, 3, 4] {
                    let (cx, cy) = ((i % 3) as f32, (i / 3) as f32);
                    slots.push((SlotRef::Craft(i), x0 + (4.5 + cx) * cell, yt + (1.0 + cy) * cell));
                }
                slots.push((SlotRef::CraftOut, x0 + 7.5 * cell, yt + 1.5 * cell));
                // Equipement : une colonne, de la tete aux pieds, a gauche du personnage.
                for i in 0..4u8 {
                    slots.push((SlotRef::Armor(i), x0, yt + i as f32 * cell));
                }
            }
            WindowKind::Table => {
                for i in 0..9u8 {
                    slots.push((SlotRef::Craft(i), x0 + (1.5 + (i % 3) as f32) * cell, yt + (i / 3) as f32 * cell));
                }
                slots.push((SlotRef::CraftOut, x0 + 6.5 * cell, yt + cell));
            }
            WindowKind::Furnace => {
                slots.push((SlotRef::Cont(0), x0 + 2.5 * cell, yt));
                slots.push((SlotRef::Cont(1), x0 + 2.5 * cell, yt + 2.0 * cell));
                slots.push((SlotRef::Cont(2), x0 + 6.0 * cell, yt + cell));
            }
            WindowKind::Chest => {
                for i in 0..27u8 {
                    slots.push((SlotRef::Cont(i), x0 + (i % 9) as f32 * cell, yt + (i / 9) as f32 * cell));
                }
            }
            WindowKind::Trade | WindowKind::None => {}
        }
        // Echanges : deux colonnes de trois offres, a la place de la grille.
        let trades = if self.window == WindowKind::Trade {
            (0..self.trades.len().min(6)).map(|i| (i, x0 + (i / 3) as f32 * 4.5 * cell, yt + (i % 3) as f32 * cell)).collect()
        } else {
            Vec::new()
        };
        let yi = yt + rows * cell + 8.0 * s;
        for i in 9..36u8 {
            let k = i - 9;
            slots.push((SlotRef::Inv(i), x0 + (k % 9) as f32 * cell, yi + (k / 9) as f32 * cell));
        }
        for i in 0..9u8 {
            slots.push((SlotRef::Inv(i), x0 + i as f32 * cell, yi + 3.0 * cell + 4.0 * s));
        }
        let rx = px + pw + 4.0 * s;
        let recipes = recipes
            .iter()
            .enumerate()
            .map(|(k, (idx, _))| {
                (*idx, rx + 6.0 * s + (k % RCOLS) as f32 * cell, py + 18.0 * s + (k / RCOLS) as f32 * cell)
            })
            .collect();
        let palette = if creative {
            let rows = ((ph - 30.0 * s) / cell).floor().max(1.0) as usize;
            App::palette()
                .into_iter()
                .skip(self.palette_row * RCOLS)
                .take(rows * RCOLS)
                .enumerate()
                .map(|(k, id)| (id, rx + 6.0 * s + (k % RCOLS) as f32 * cell, py + 18.0 * s + (k / RCOLS) as f32 * cell))
                .collect()
        } else {
            Vec::new()
        };
        Layout { palette, cell, panel: [px, py, pw, ph], slots, rpanel: [rx, py, rw - 4.0 * s, ph], recipes, top: (x0, yt), trades }
    }

    fn slot_stack(&self, r: SlotRef) -> Slot {
        match r {
            SlotRef::Inv(i) => self.inv.get(i as usize).copied().flatten(),
            SlotRef::Armor(i) => self.armor.get(i as usize).copied().flatten(),
            SlotRef::Craft(i) => self.craft.get(i as usize).copied().flatten(),
            SlotRef::CraftOut => self.craft_out,
            SlotRef::Cont(i) => self.cont.get(i as usize).copied().flatten(),
        }
    }

    /// Clic dans une fenetre ouverte : case d'inventaire ou recette.
    pub fn ui_click(&mut self, inp: &Input, w: f32, h: f32, s: f32, right: bool) {
        let l = self.layout(w, h, s);
        if let Some((slot, _, _)) = l.slots.iter().find(|(_, x, y)| inside(inp.mouse, *x, *y, l.cell)) {
            crate::audio::play("case", 0.5);
            self.net.send(ClientMsg::Click { slot: *slot, right, shift: inp.shift() });
        } else if let Some((id, _, _)) = l.palette.iter().find(|(_, x, y)| inside(inp.mouse, *x, *y, l.cell)) {
            // Curseur vide : on prend une pile de l'objet. Curseur plein : on le jette.
            crate::audio::play("case", 0.5);
            self.net.send(ClientMsg::Pick { id: if self.cursor.is_some() { 0 } else { *id } });
        } else if let Some((idx, _, _)) = l.recipes.iter().find(|(_, x, y)| inside(inp.mouse, *x, *y, l.cell)) {
            crate::audio::play("fabrique", 0.5);
            self.net.send(ClientMsg::Craft { recipe: *idx as u16 });
        } else if let Some((idx, _, _)) = l.trades.iter().find(|(_, x, y)| inp.mouse.0 >= *x && inp.mouse.0 < x + 4.0 * l.cell && inp.mouse.1 >= *y && inp.mouse.1 < y + l.cell) {
            crate::audio::play("pieces", 0.7);
            self.net.send(ClientMsg::Trade { index: *idx as u8 });
        }
    }

    /// Le personnage en volume dans l'inventaire : il porte sa tenue, respire, et suit la souris du regard.
    #[allow(clippy::too_many_arguments)]
    fn draw_doll(&self, ui: &mut Ui, inp: &Input, models: &[Model], x: f32, y: f32, w: f32, h: f32, s: f32) {
        ui.bevel(x, y, w, h, s, [0.04, 0.05, 0.11, 1.0], PANEL_DARK, PANEL_LIGHT);
        let Some(model) = models.get(crate::models::PLAYER) else { return };
        let (cx, feet) = (x + w / 2.0, y + h - 5.0 * s);
        let scale = (h - 10.0 * s) / 2.3;
        // Il se tourne vers le curseur, et leve ou baisse un peu la tete avec lui.
        let yaw = ((inp.mouse.0 - cx) / (w * 1.6)).clamp(-1.0, 1.0) * 0.95;
        let pitch = ((inp.mouse.1 - (feet - 1.7 * scale)) / (h * 1.6)).clamp(-1.0, 1.0) * 0.3;
        let view = Mat3::from_rotation_x(0.12 + pitch) * Mat3::from_rotation_y(yaw);
        // Au repos, les bras se balancent a peine : il respire.
        let walk = (self.clock * 1.7).sin() * 0.14;
        let gear = [0, 1, 2, 3].map(|i| self.armor.get(i).copied().flatten().map_or(0, |st| st.id));

        // Faces visibles de toutes les boites, a trier de la plus lointaine a la plus proche.
        let mut faces: Vec<(f32, [[f32; 2]; 4], [[f32; 2]; 4], [f32; 4])> = Vec::new();
        let white = Ui::white();
        let mut add = |center: Vec3, q: Quat, half: Vec3, uv: Option<&[[f32; 4]; 6]>, col: [f32; 3]| {
            for (f, (n, corners)) in crate::mesh::FACES.iter().enumerate() {
                let normal = view * (q * Vec3::new(n[0] as f32, n[1] as f32, n[2] as f32));
                if normal.z <= 0.01 {
                    continue;
                }
                let shade = crate::mesh::SHADE[f] as f32 / 255.0;
                let (mut pts, mut uvs, mut depth) = ([[0.0; 2]; 4], [[0.0; 2]; 4], 0.0);
                for k in 0..4 {
                    let v = view * (center + q * ((Vec3::from(corners[k]) * 2.0 - Vec3::ONE) * half));
                    pts[k] = [cx + v.x * scale, feet - v.y * scale];
                    depth += v.z / 4.0;
                    let [a, b] = crate::mesh::UVS[k];
                    uvs[k] = uv.map_or(white, |r| [r[f][0] + a * (r[f][2] - r[f][0]), r[f][1] + b * (r[f][3] - r[f][1])]);
                }
                faces.push((depth, pts, uvs, [col[0] * shade, col[1] * shade, col[2] * shade, 1.0]));
            }
        };
        for p in &model.parts {
            let mut q = Quat::from_euler(EulerRot::ZYX, p.rot.z.to_radians(), p.rot.y.to_radians(), p.rot.x.to_radians());
            if p.leg != 0 {
                q = Quat::from_rotation_x(walk.sin() * 0.6 * p.leg as f32) * q;
            }
            add(p.pivot + q * (p.c - p.pivot), q, p.h, Some(&p.uv), [1.0; 3]);
        }
        crate::gear_boxes(walk, gear, &mut |center, q, half, col| {
            add(center, q, half, None, [col[0] as f32 / 255.0, col[1] as f32 / 255.0, col[2] as f32 / 255.0]);
        });
        faces.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, pts, uvs, col) in faces {
            ui.quad4(pts, uvs, [col; 4]);
        }
    }

    fn draw_window(&self, ui: &mut Ui, inp: &Input, models: &[Model], s: f32) {
        let (w, h) = (ui.w, ui.h);
        let l = self.layout(w, h, s);
        let cell = l.cell;
        ui.rect(0.0, 0.0, w, h, [0.0, 0.0, 0.02, 0.55]);
        let [px, py, pw, ph] = l.panel;
        ui.bevel(px, py, pw, ph, s, PANEL, PANEL_LIGHT, PANEL_DARK);
        let title = match self.window {
            WindowKind::Inventory => "Inventaire",
            WindowKind::Table => "Établi",
            WindowKind::Furnace => "Four",
            WindowKind::Chest => "Coffre",
            WindowKind::Trade => &self.trade_title,
            WindowKind::None => "",
        };
        ui.text(px + 8.0 * s, py + 6.0 * s, s, title, [1.0, 0.9, 0.6, 1.0]);

        let mut tip: Vec<String> = Vec::new();
        for (r, x, y) in &l.slots {
            let hover = inside(inp.mouse, *x, *y, cell);
            slot(ui, x + s, y + s, cell - 2.0 * s, s, hover);
            if let Some(st) = self.slot_stack(*r) {
                ui.item(x + s, y + s, cell - 2.0 * s, &st);
                if hover && self.cursor.is_none() {
                    tip.push(items::name(st.id));
                    let points = items::armor_points(st.id);
                    if points > 0.0 {
                        let tough = items::armor_toughness(st.id);
                        tip.push(if tough > 0.0 { format!("Armure +{points}, robustesse +{tough}") } else { format!("Armure +{points}") });
                    }
                }
            } else if let SlotRef::Armor(i) = r {
                // Case d'equipement vide : la silhouette de la piece attendue.
                ui.icon(x + s, y + s, cell - 2.0 * s, items::armor_id(*i, 2), [0.0, 0.0, 0.0, 0.28]);
            }
        }

        // Offres du villageois : ce qu'il demande, une fleche, ce qu'il donne. Vert si l'on a de quoi payer.
        for (idx, x, y) in &l.trades {
            let (cost, out) = self.trades[*idx];
            let ok = items::count(&self.inv, cost.id) >= cost.n as u32;
            let hover = inp.mouse.0 >= *x && inp.mouse.0 < x + 4.0 * cell && inp.mouse.1 >= *y && inp.mouse.1 < y + cell;
            let bg = match (ok, hover) {
                (true, true) => [0.30, 0.66, 0.38, 1.0],
                (true, false) => [0.20, 0.44, 0.26, 1.0],
                (false, true) => SLOT_HOT,
                (false, false) => SLOT,
            };
            ui.bevel(x + s, y + s, 4.0 * cell - 2.0 * s, cell - 2.0 * s, s, bg, PANEL_DARK, PANEL_LIGHT);
            ui.item(x + 4.0 * s, y + s, cell - 2.0 * s, &cost);
            ui.text(x + 1.55 * cell, y + 5.0 * s, s, "=>", WHITE);
            ui.item(x + 2.7 * cell, y + s, cell - 2.0 * s, &out);
            if hover {
                tip.push(format!("{} x{}", items::name(out.id), out.n));
                tip.push(format!("  contre {} {} ({})", cost.n, items::name(cost.id), items::count(&self.inv, cost.id)));
            }
        }

        let (x0, yt) = l.top;
        match self.window {
            WindowKind::Inventory | WindowKind::Table => {
                let table = self.window == WindowKind::Table;
                let (ax, ay) = if table { (x0 + 5.0 * cell, yt + cell) } else { (x0 + 6.6 * cell, yt + 1.5 * cell) };
                ui.text(ax, ay + 5.0 * s, s, "=>", WHITE);
                if !table {
                    self.draw_doll(ui, inp, models, x0 + cell + 2.0 * s, yt + s, 3.0 * cell - 2.0 * s, 4.0 * cell - 2.0 * s, s);
                }
            }
            WindowKind::Furnace => {
                // Flamme restante, puis avancement de la cuisson.
                let [flame, cook] = self.cont_prog;
                let (fx, fy) = (x0 + 2.5 * cell + 4.0 * s, yt + cell + 3.0 * s);
                ui.rect(fx, fy, 10.0 * s, 12.0 * s, PANEL_DARK);
                let (hot, ember) = ([1.0, 0.85, 0.2, 1.0], [0.95, 0.3, 0.05, 1.0]);
                ui.gradient(fx, fy + 12.0 * s * (1.0 - flame), 10.0 * s, 12.0 * s * flame, [hot, hot, ember, ember]);
                let (bx, by) = (x0 + 4.0 * cell, yt + cell + 7.0 * s);
                ui.rect(bx, by, 1.6 * cell, 4.0 * s, PANEL_DARK);
                ui.rect(bx, by, 1.6 * cell * cook.clamp(0.0, 1.0), 4.0 * s, [0.92, 0.92, 0.92, 1.0]);
            }
            _ => {}
        }

        if !l.palette.is_empty() {
            let [rx, ry, rw, rh] = l.rpanel;
            ui.bevel(rx, ry, rw, rh, s, PANEL, PANEL_LIGHT, PANEL_DARK);
            ui.text(rx + 6.0 * s, ry + 6.0 * s, s, "Tous les objets", [1.0, 0.9, 0.6, 1.0]);
            ui.text(rx + 6.0 * s, ry + rh - 11.0 * s, s, "Molette : défiler", [0.7, 0.72, 0.8, 1.0]);
            for (id, x, y) in &l.palette {
                let hover = inside(inp.mouse, *x, *y, cell);
                slot(ui, x + s, y + s, cell - 2.0 * s, s, hover);
                ui.icon(x + s, y + s, cell - 2.0 * s, *id, WHITE);
                if hover {
                    tip.push(items::name(*id));
                }
            }
        }
        if !l.recipes.is_empty() {
            let [rx, ry, rw, rh] = l.rpanel;
            ui.bevel(rx, ry, rw, rh, s, PANEL, PANEL_LIGHT, PANEL_DARK);
            ui.text(rx + 6.0 * s, ry + 6.0 * s, s, "Recettes", [1.0, 0.9, 0.6, 1.0]);
            let all = craft::recipes();
            for (idx, x, y) in &l.recipes {
                let r = &all[*idx];
                let ok = craft::can_craft(r, &self.inv);
                let hover = inside(inp.mouse, *x, *y, cell);
                let bg = match (ok, hover) {
                    (true, true) => [0.30, 0.66, 0.38, 1.0],
                    (true, false) => [0.20, 0.44, 0.26, 1.0],
                    (false, true) => SLOT_HOT,
                    (false, false) => SLOT,
                };
                ui.bevel(x + s, y + s, cell - 2.0 * s, cell - 2.0 * s, s, bg, PANEL_DARK, PANEL_LIGHT);
                let a = if ok { 1.0 } else { 0.4 };
                ui.icon(x + s, y + s, cell - 2.0 * s, r.out, [a, a, a, 1.0]);
                if hover {
                    let n = if r.n > 1 { format!(" x{}", r.n) } else { String::new() };
                    tip.push(format!("{}{n}", items::name(r.out)));
                    for (id, need) in craft::ingredients(r) {
                        tip.push(format!("  {need} {} ({})", items::name(id), items::count(&self.inv, id)));
                    }
                    if r.w > 2 || r.h > 2 {
                        tip.push("  (établi)".to_string());
                    }
                }
            }
        }

        if let Some(st) = self.cursor {
            ui.item(inp.mouse.0 - 8.0 * s, inp.mouse.1 - 8.0 * s, 16.0 * s, &st);
        }
        if !tip.is_empty() {
            let tw = tip.iter().map(|t| Ui::text_width(t, s)).fold(0.0, f32::max) + 8.0 * s;
            let th = tip.len() as f32 * 10.0 * s + 6.0 * s;
            let tx = (inp.mouse.0 + 12.0 * s).min(w - tw);
            let ty = (inp.mouse.1 - 4.0 * s).min(h - th).max(0.0);
            ui.bevel(tx, ty, tw, th, s, [0.06, 0.03, 0.12, 0.96], [0.36, 0.2, 0.62, 1.0], [0.16, 0.08, 0.3, 1.0]);
            for (i, line) in tip.iter().enumerate() {
                let col = if i == 0 { WHITE } else { [0.75, 0.75, 0.8, 1.0] };
                ui.text(tx + 4.0 * s, ty + 4.0 * s + i as f32 * 10.0 * s, s, line, col);
            }
        }
    }

    /// Ecran d'informations (F3), a la maniere de celui de Minecraft : a gauche le jeu et le lieu,
    /// a droite le reseau et ce que l'on vise, en bas la duree des dernieres images.
    fn draw_debug(&self, ui: &mut Ui, backend: &str, meshes: usize, s: f32) {
        use shared::gen;
        let (w, h) = (ui.w, ui.h);
        let p = self.abs(self.pos);
        let (bx, by, bz) = (p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32);
        let hours = (self.time * 24.0 + 6.0) % 24.0;
        // Images : moyenne lissee, duree de la derniere, pire des deux dernieres secondes.
        let last = self.frames.last().copied().unwrap_or(0.0);
        let worst = self.frames.iter().cloned().fold(0.0, f32::max);
        // Regard : meme convention que Minecraft, +Z au sud et +X a l'est.
        let (sx, sz) = (self.yaw.sin(), self.yaw.cos());
        let facing = if sz.abs() >= sx.abs() {
            if sz > 0.0 { "sud (+Z)" } else { "nord (-Z)" }
        } else if sx > 0.0 {
            "est (+X)"
        } else {
            "ouest (-X)"
        };
        let light = self.light_at(self.pos + glam::Vec3::Y * 0.5);
        let world = ["Surface", "Yomi, le pays des morts", "Takamagahara, la plaine céleste"][self.dim.min(2) as usize];
        let mut left = vec![
            format!("Epicube Craft 0.9 - {:.0} images/s ({:.1} ms, pire {:.1} ms)", self.fps, last, worst),
            format!("Rendu : {backend} - {:.0} x {:.0}", w, h),
            format!("Chunks : {meshes} affichés, {} chargés - vue {}", self.world.chunks.len(), self.view),
            format!(
                "Partie : {}, difficulté {}{}",
                shared::rules::mode_name(self.mode).to_lowercase(),
                shared::rules::difficulty_name(self.rules.difficulty).to_lowercase(),
                if self.rules.hardcore { " (extrême)" } else { "" }
            ),
            format!("Entités : {} créatures, {} objets, {} projectiles, {} particules", self.mobs.len(), self.drops.len(), self.projs.len(), self.particles.len()),
            String::new(),
            world.to_string(),
            format!("XYZ : {:.3} / {:.3} / {:.3}", p[0], p[1], p[2]),
            format!("Bloc : {bx} {by} {bz}  (dans le chunk : {} {})", bx.rem_euclid(16), bz.rem_euclid(16)),
            format!("Chunk : {} {}  (région {} {})", bx >> 4, bz >> 4, bx.div_euclid(shared::structures::REGION), bz.div_euclid(shared::structures::REGION)),
            format!("Regard : {facing} - lacet {:.1}, tangage {:.1}", self.yaw.to_degrees().rem_euclid(360.0), self.pitch.to_degrees()),
            format!("Lumière : ciel {}, blocs {}", light[0] / 17, light[1] / 17),
        ];
        match self.dim {
            0 => {
                let (ground, biome) = gen::column(self.world_seed, bx, bz);
                left.push(format!("Biome : {} - sol à {ground}", biome.name()));
                if by < ground - 4 {
                    left.push(format!("Sous-sol : {}", gen::cavern(self.world_seed, bx, bz).name()));
                }
                // La structure de la region, comme le ferait une commande de localisation.
                let (rx, rz) = (bx.div_euclid(shared::structures::REGION), bz.div_euclid(shared::structures::REGION));
                left.push(match shared::structures::site(self.world_seed, rx, rz) {
                    Some(site) => {
                        let d = ((site.x - bx) as f64).hypot((site.z - bz) as f64);
                        format!("Structure de la région : {} à {:.0} blocs ({} {})", site.kind.name(), d, site.x, site.z)
                    }
                    None => "Structure de la région : aucune".to_string(),
                });
            }
            1 => left.push(format!("Contrée : {}", gen::yomi_biome(self.world_seed, bx, bz).name())),
            _ => {}
        }
        left.push(format!("Heure : {:02}:{:02}", hours as u32, (hours.fract() * 60.0) as u32));
        left.push(format!("Graine : {}", self.seed_label.as_deref().unwrap_or("?")));
        left.push(format!("Origine d'affichage : {} {}", self.origin[0], self.origin[1]));

        let names: Vec<&str> = self.others.values().map(|r| r.name.as_str()).collect();
        let held = self.inv.get(self.sel).copied().flatten();
        let mut right = vec![
            format!("Ping : {:.1} ms", self.ping_ms),
            format!("Serveur : {:.1} ticks/s - {:.2} ms par tick", self.server_tps, self.server_mspt),
            format!("Autres joueurs : {} {}", names.len(), names.join(", ")),
            String::new(),
            format!("Vie {:.0} / 20 - faim {:.0} / 20", self.hp, self.hunger),
        ];
        if self.effects != 0 {
            let mut e = Vec::new();
            if self.effects & 1 != 0 {
                e.push("poison");
            }
            if self.effects & 2 != 0 {
                e.push("nausée");
            }
            right.push(format!("Effets : {}", e.join(", ")));
        }
        if let Some(st) = held {
            let max = items::max_dur(st.id);
            right.push(if max > 0 { format!("Tenu : {} ({} / {max})", items::name(st.id), st.dur) } else { format!("Tenu : {} x{}", items::name(st.id), st.n) });
        }
        if let Some((t, _)) = self.target {
            let (b, m) = self.world.block(t.x, t.y, t.z);
            right.push(String::new());
            right.push(format!("Bloc visé : {} ({b}:{m})", items::name(b as u16)));
            right.push(format!("en {} {} {}", t.x + self.origin[0], t.y, t.z + self.origin[1]));
        }
        right.push(String::new());
        right.push("F3 infos - F4 vol - F6 bordures de chunks".to_string());
        if self.fly {
            right.push("Vol activé".to_string());
        }

        // Chaque ligne sur son bandeau sombre, pour rester lisible sur tous les fonds.
        let back = [0.07, 0.06, 0.13, 0.62];
        for (i, l) in left.iter().enumerate().filter(|(_, l)| !l.is_empty()) {
            let y = (3.0 + i as f32 * 10.0) * s;
            ui.rect(2.0 * s, y, Ui::text_width(l, s) + 4.0 * s, 10.0 * s, back);
            ui.text(4.0 * s, y + s, s, l, WHITE);
        }
        for (i, l) in right.iter().enumerate().filter(|(_, l)| !l.is_empty()) {
            let (y, tw) = ((3.0 + i as f32 * 10.0) * s, Ui::text_width(l, s));
            ui.rect(w - tw - 6.0 * s, y, tw + 4.0 * s, 10.0 * s, back);
            ui.text(w - tw - 4.0 * s, y + s, s, l, WHITE);
        }

        // Graphique des images : une barre par image, verte sous 16,7 ms (60 images/s), rouge au-dela de 33 ms.
        let (gx, gh) = (4.0 * s, 30.0 * s);
        let gy = h - 6.0 * s - gh;
        ui.rect(gx - 2.0 * s, gy - 2.0 * s, 124.0 * s, gh + 4.0 * s, back);
        for (i, ms) in self.frames.iter().enumerate() {
            let bar = ((ms / 33.4).min(1.0) * gh / s).ceil() * s;
            let col = if *ms <= 16.8 { [0.35, 0.85, 0.4, 1.0] } else if *ms <= 33.4 { [0.95, 0.8, 0.25, 1.0] } else { [0.9, 0.25, 0.2, 1.0] };
            ui.rect(gx + i as f32 * s, gy + gh - bar, s, bar, col);
        }
        ui.rect(gx, gy + gh / 2.0, 120.0 * s, 1.0, [1.0, 1.0, 1.0, 0.5]);
        ui.text(gx + 124.0 * s, gy + gh / 2.0 - 4.0 * s, s, "16,7 ms", WHITE);
    }

    /// Voiles plein ecran : vignette, eau, coup recu.
    fn draw_overlays(&self, ui: &mut Ui) {
        let (w, h) = (ui.w, ui.h);
        if self.underwater {
            ui.rect(0.0, 0.0, w, h, [0.05, 0.2, 0.55, 0.28]);
        }
        let (e, c) = ([0.0, 0.0, 0.0, 0.34], [0.0, 0.0, 0.0, 0.0]);
        let (bx, by) = (w * 0.2, h * 0.24);
        ui.gradient(0.0, 0.0, bx, h, [e, c, c, e]);
        ui.gradient(w - bx, 0.0, bx, h, [c, e, e, c]);
        ui.gradient(0.0, 0.0, w, by, [e, e, c, c]);
        ui.gradient(0.0, h - by, w, by, [c, c, e, e]);
        if self.hurt_t > 0.0 {
            let a = (self.hurt_t / 0.4).min(1.0) * 0.38;
            ui.rect(0.0, 0.0, w, h, [0.85, 0.05, 0.05, a]);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_ui(&self, ui: &mut Ui, inp: &Input, grabbed: bool, backend: &str, meshes: usize, models: &[Model], s: f32) {
        let (w, h) = (ui.w, ui.h);
        self.draw_overlays(ui);

        // Pseudos des autres joueurs.
        for (x, y, name) in &self.labels {
            let tw = Ui::text_width(name, s);
            ui.rect(x - tw / 2.0 - 2.0 * s, y - 5.0 * s, tw + 4.0 * s, 10.0 * s, [0.0, 0.0, 0.0, 0.4]);
            ui.text(x - tw / 2.0, y - 4.0 * s, s, name, WHITE);
        }

        if grabbed {
            // Viseur : croix blanche cernee de noir, lisible sur tous les fonds.
            let (cx, cy) = ((w / 2.0).floor(), (h / 2.0).floor());
            for (o, col) in [(s, [0.0, 0.0, 0.0, 0.5]), (0.0, [1.0, 1.0, 1.0, 0.9])] {
                ui.rect(cx - 5.0 * s + o, cy - 0.5 * s + o, 10.0 * s, s, col);
                ui.rect(cx - 0.5 * s + o, cy - 5.0 * s + o, s, 10.0 * s, col);
            }
            // Jauge sous le viseur : tension de l'arc, ou retour en garde de l'arme. Doree quand le coup portera a fond.
            let held = self.inv.get(self.sel).copied().flatten().map(|st| st.id);
            let weapon = shared::items::weapon(held);
            let gauge = if self.draw_t > 0.0 {
                Some(shared::items::draw_power(self.draw_t as f64) as f32)
            } else {
                let k = self.since_hit * weapon.speed as f32;
                (weapon.speed < 3.9 && k < 1.0).then_some(k)
            };
            if let Some(k) = gauge {
                let (gx, gy) = (cx - 8.0 * s, cy + 8.0 * s);
                ui.rect(gx - s, gy - s, 18.0 * s, 4.0 * s, [0.07, 0.06, 0.13, 0.8]);
                let full = k >= 0.92;
                ui.rect(gx, gy, (16.0 * k.clamp(0.0, 1.0)).floor() * s, 2.0 * s, if full { [0.91, 0.73, 0.23, 1.0] } else { [0.9, 0.9, 0.95, 1.0] });
            }
        }

        // Un spectateur n'a ni barre rapide, ni vie, ni faim.
        if self.mode == shared::rules::SPECTATOR {
            self.draw_messages(ui, s);
            return self.draw_end(ui, inp, grabbed, backend, meshes, models, s);
        }
        // Barre rapide.
        let cell = 20.0 * s;
        let (bx, by) = (((w - 9.0 * cell) / 2.0).floor(), h - cell - 3.0 * s);
        let bar = [0.1, 0.1, 0.13, 0.75];
        ui.bevel(bx - 2.0 * s, by - 2.0 * s, 9.0 * cell + 4.0 * s, cell + 4.0 * s, s, bar, PANEL_LIGHT, PANEL_DARK);
        for i in 0..9 {
            let x = bx + i as f32 * cell;
            slot(ui, x + s, by + s, cell - 2.0 * s, s, false);
            if let Some(st) = self.inv.get(i).copied().flatten() {
                ui.item(x + 2.0 * s, by + 2.0 * s, 16.0 * s, &st);
            }
        }
        // Case choisie : cadre clair par-dessus.
        let sx = bx + self.sel as f32 * cell;
        for (x, y, rw, rh) in [
            (sx - s, by - s, cell + 2.0 * s, 2.0 * s),
            (sx - s, by + cell - s, cell + 2.0 * s, 2.0 * s),
            (sx - s, by - s, 2.0 * s, cell + 2.0 * s),
            (sx + cell - s, by - s, 2.0 * s, cell + 2.0 * s),
        ] {
            ui.rect(x, y, rw, rh, [1.0, 0.95, 0.7, 1.0]);
        }

        // Vie a gauche, faim a droite : dix icones de deux points chacune.
        let iy = by - 12.0 * s;
        let mortal = !shared::rules::invulnerable(self.mode);
        // Armure au-dessus de la vie, seulement quand on en porte.
        let points = items::defense(&self.armor).0 as f32;
        if points > 0.0 && mortal {
            for i in 0..10 {
                let x = bx + i as f32 * 8.0 * s;
                ui.hud(x, iy - 10.0 * s, s, t::ARMOR_EMPTY, 0, 9, WHITE);
                let fill = ((points - i as f32 * 2.0) / 2.0).clamp(0.0, 1.0);
                if fill > 0.0 {
                    ui.hud(x, iy - 10.0 * s, s, t::ARMOR, 0, if fill < 1.0 { 5 } else { 9 }, WHITE);
                }
            }
        }
        for i in (0..10).filter(|_| mortal) {
            // Les coeurs tremblent quand la vie est basse.
            let shake = if self.hp <= 6.0 { ((self.clock * 14.0 + i as f32 * 1.7).sin() * s).round() } else { 0.0 };
            let x = bx + i as f32 * 8.0 * s;
            ui.hud(x, iy + shake, s, t::HEART_EMPTY, 0, 9, WHITE);
            let fill = ((self.hp - i as f32 * 2.0) / 2.0).clamp(0.0, 1.0);
            if fill > 0.0 {
                // Empoisonne : les coeurs virent au vert.
                // Mode extreme : des coeurs sombres, pour se rappeler qu'il n'y en aura pas d'autres.
                let tint = if self.effects & 1 != 0 {
                    [0.55, 0.85, 0.3, 1.0]
                } else if self.rules.hardcore {
                    [0.72, 0.42, 0.62, 1.0]
                } else {
                    WHITE
                };
                ui.hud(x, iy + shake, s, t::HEART, 0, if fill < 1.0 { 5 } else { 9 }, tint);
            }
            let fx = bx + 9.0 * cell - 8.0 * s - i as f32 * 8.0 * s;
            ui.hud(fx, iy, s, t::FOOD_EMPTY, 0, 9, WHITE);
            let fill = ((self.hunger - i as f32 * 2.0) / 2.0).clamp(0.0, 1.0);
            if fill > 0.0 {
                // Nausee : les onigiris verdissent.
                let tint = if self.effects & 2 != 0 { [0.7, 0.85, 0.4, 1.0] } else { WHITE };
                if fill < 1.0 {
                    ui.hud(fx, iy, s, t::FOOD, 4, 5, tint);
                } else {
                    ui.hud(fx, iy, s, t::FOOD, 0, 9, tint);
                }
            }
        }
        // Bulles d'air sous l'eau.
        if self.air < 10.0 && mortal {
            for i in 0..self.air.ceil().max(0.0) as usize {
                let x = bx + 9.0 * cell - 8.0 * s - i as f32 * 8.0 * s;
                ui.hud(x, iy - 10.0 * s, s, t::BUBBLE, 0, 9, WHITE);
            }
        }
        if self.name_t > 0.0 {
            if let Some(st) = self.inv.get(self.sel).copied().flatten() {
                let name = items::name(st.id);
                let a = self.name_t.min(1.0);
                ui.text((w - Ui::text_width(&name, s)) / 2.0, iy - 14.0 * s, s, &name, [1.0, 1.0, 1.0, a]);
            }
        }
        self.draw_messages(ui, s);
        // Barre de vie du boss.
        if let Some(hp) = self.boss {
            let (bw, bh) = ((w * 0.4).floor(), 5.0 * s);
            let (x, y) = (((w - bw) / 2.0).floor(), 16.0 * s);
            let title = "Ryûjin, dragon de Takamagahara";
            ui.text((w - Ui::text_width(title, s)) / 2.0, y - 11.0 * s, s, title, [0.9, 0.6, 1.0, 1.0]);
            ui.bevel(x - s, y - s, bw + 2.0 * s, bh + 2.0 * s, s, [0.0, 0.0, 0.0, 0.7], PANEL_DARK, PANEL_LIGHT);
            ui.rect(x, y, bw * hp.clamp(0.0, 1.0), bh, [0.72, 0.25, 0.9, 1.0]);
        }
        // Avancement du minage.
        if let Some(frac) = self.dig_frac {
            ui.rect(w / 2.0 - 12.0 * s, h / 2.0 + 9.0 * s, 24.0 * s, 3.0 * s, [0.0, 0.0, 0.0, 0.6]);
            ui.rect(w / 2.0 - 12.0 * s, h / 2.0 + 9.0 * s, 24.0 * s * frac, 3.0 * s, WHITE);
        }

        self.draw_end(ui, inp, grabbed, backend, meshes, models, s);
    }

    /// Message d'information au milieu de l'ecran (mode de jeu, entree dans un monde...).
    fn draw_messages(&self, ui: &mut Ui, s: f32) {
        let (w, h) = (ui.w, ui.h);
        if let Some((text, left)) = &self.info {
            let a = left.min(1.0);
            let tw = Ui::text_width(text, s);
            ui.rect((w - tw) / 2.0 - 4.0 * s, h * 0.25 - 3.0 * s, tw + 8.0 * s, 14.0 * s, [0.0, 0.0, 0.0, 0.4 * a]);
            ui.text((w - tw) / 2.0, h * 0.25, s, text, [1.0, 0.95, 0.6, a]);
        }
    }

    /// Ce qui se dessine par-dessus tout, quel que soit le mode : informations, fenetre ouverte, messages de fin.
    #[allow(clippy::too_many_arguments)]
    fn draw_end(&self, ui: &mut Ui, inp: &Input, grabbed: bool, backend: &str, meshes: usize, models: &[Model], s: f32) {
        let (w, h) = (ui.w, ui.h);
        let _ = grabbed;
        self.draw_chat(ui, s);
        // Chat vocal : qui parle en ce moment (soi-meme compris).
        let mut talkers: Vec<String> = self.others.values().filter(|r| r.talk > 0.0).map(|r| r.name.clone()).collect();
        talkers.sort_unstable();
        if self.mic_live {
            talkers.insert(0, "Vous".to_string());
        }
        for (k, name) in talkers.iter().enumerate() {
            let line = format!("(( {name} ))");
            let (tx, ty) = (w - Ui::text_width(&line, s) - 8.0 * s, 30.0 * s + k as f32 * 12.0 * s);
            ui.rect(tx - 3.0 * s, ty - 2.0 * s, Ui::text_width(&line, s) + 6.0 * s, 11.0 * s, [0.0, 0.0, 0.02, 0.5]);
            ui.text(tx, ty, s, &line, if name == "Vous" { [0.6, 1.0, 0.6, 1.0] } else { [1.0, 0.9, 0.5, 1.0] });
        }
        if self.debug {
            self.draw_debug(ui, backend, meshes, s);
        }

        if self.window != WindowKind::None {
            self.draw_window(ui, inp, models, s);
        }

        let dead = self.dead();
        let msg: Option<(String, &str)> = if let Some(l) = &self.lost {
            Some((l.clone(), "Échap pour ouvrir le menu"))
        } else if !self.joined {
            Some(("Connexion au serveur...".to_string(), ""))
        } else if dead {
            let next = if self.rules.hardcore { "Mode extrême : Entrée pour continuer en spectateur" } else { "Appuyez sur Entrée pour réapparaître" };
            Some(("Vous êtes mort".to_string(), next))
        } else {
            None
        };
        if let Some((title, sub)) = msg {
            let s2 = s * 2.0;
            let bg = if dead { [0.45, 0.0, 0.0, 0.55] } else { [0.0, 0.0, 0.0, 0.5] };
            ui.rect(0.0, h / 2.0 - 30.0 * s, w, 50.0 * s, bg);
            ui.text((w - Ui::text_width(&title, s2)) / 2.0, h / 2.0 - 22.0 * s, s2, &title, WHITE);
            ui.text((w - Ui::text_width(sub, s)) / 2.0, h / 2.0 + 4.0 * s, s, sub, [0.9, 0.9, 0.9, 1.0]);
        }
    }
}
