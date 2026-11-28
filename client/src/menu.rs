//! Menus : ecran-titre, multijoueur, parametres et pause.
use crate::ui::Ui;
use crate::{Input, Screen, Shell};
use shared::blocks::t;
use shared::util::hash;

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const GREY: [f32; 4] = [0.72, 0.74, 0.8, 1.0];

/// Ce que le joueur vient de demander dans un menu.
#[derive(PartialEq)]
pub enum Action {
    None,
    Quit,
    /// Quitter la partie et revenir a l'ecran-titre.
    Leave,
    Export,
    Reload,
    /// Creer une partie du mode choisi, publique ou privee selon l'interrupteur.
    Host,
    /// Rejoindre la partie a cette adresse.
    JoinAt(String),
}

struct W<'a> {
    ui: &'a mut Ui,
    mouse: (f32, f32),
    click: bool,
    down: bool,
    s: f32,
}

impl W<'_> {
    fn over(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.mouse.0 >= x && self.mouse.0 < x + w && self.mouse.1 >= y && self.mouse.1 < y + h
    }

    fn label(&mut self, x: f32, y: f32, w: f32, h: f32, text: &str, color: [f32; 4]) {
        let tw = Ui::text_width(text, self.s);
        self.ui.text((x + (w - tw) / 2.0).floor(), (y + (h - 8.0 * self.s) / 2.0).floor(), self.s, text, color);
    }

    fn button(&mut self, x: f32, y: f32, w: f32, h: f32, text: &str) -> bool {
        let hot = self.over(x, y, w, h);
        let (fill, light) = if hot {
            // Survol : plaque doree. Au repos : laque vermillon.
            ([0.85, 0.65, 0.16, 1.0], [1.0, 0.89, 0.54, 1.0])
        } else {
            ([0.72, 0.2, 0.165, 1.0], [0.91, 0.42, 0.29, 1.0])
        };
        self.ui.bevel(x, y, w, h, self.s, fill, light, [0.3, 0.07, 0.07, 1.0]);
        self.label(x, y, w, h, text, WHITE);
        if hot && self.click {
            crate::audio::play("clic", 0.6);
        }
        hot && self.click
    }

    /// Petit cadenas : une partie privee.
    fn lock(&mut self, x: f32, y: f32) {
        let (s, gold, dark) = (self.s, [0.95, 0.76, 0.26, 1.0], [0.35, 0.24, 0.05, 1.0]);
        // L'anse, puis le corps et son trou de serrure.
        self.ui.rect(x + s, y, 4.0 * s, s, gold);
        self.ui.rect(x + s, y, s, 3.0 * s, gold);
        self.ui.rect(x + 4.0 * s, y, s, 3.0 * s, gold);
        self.ui.rect(x, y + 3.0 * s, 6.0 * s, 5.0 * s, gold);
        self.ui.rect(x + 2.5 * s, y + 4.5 * s, s, 2.0 * s, dark);
    }

    fn toggle(&mut self, x: f32, y: f32, w: f32, h: f32, text: &str, value: &mut bool) {
        let label = format!("{text} : {}", if *value { "oui" } else { "non" });
        if self.button(x, y, w, h, &label) {
            *value = !*value;
        }
    }

    /// Curseur horizontal ; `v` va de 0 a 1. `drag` retient le curseur en cours de deplacement.
    #[allow(clippy::too_many_arguments)]
    fn slider(&mut self, id: u8, x: f32, y: f32, w: f32, h: f32, text: &str, v: &mut f32, drag: &mut Option<u8>) {
        let s = self.s;
        if self.click && self.over(x, y, w, h) {
            *drag = Some(id);
        }
        if !self.down && *drag == Some(id) {
            *drag = None;
        }
        if *drag == Some(id) {
            *v = ((self.mouse.0 - x - 4.0 * s) / (w - 8.0 * s)).clamp(0.0, 1.0);
        }
        self.ui.bevel(x, y, w, h, s, [0.12, 0.12, 0.15, 1.0], [0.06, 0.06, 0.08, 1.0], [0.36, 0.37, 0.44, 1.0]);
        self.ui.rect(x + s, y + s, (w - 2.0 * s) * *v, h - 2.0 * s, [0.2, 0.3, 0.46, 1.0]);
        // Poignee fine, sous le texte, pour ne pas le masquer.
        let kx = x + 4.0 * s + (w - 8.0 * s) * *v - 1.5 * s;
        self.ui.rect(kx, y + s, 3.0 * s, h - 2.0 * s, [0.62, 0.74, 0.95, 1.0]);
        self.label(x, y, w, h, text, WHITE);
    }

    /// Champ de saisie ; renvoie vrai si on vient de cliquer dedans.
    #[allow(clippy::too_many_arguments)]
    fn field(&mut self, x: f32, y: f32, w: f32, h: f32, title: &str, value: &str, focused: bool, blink: bool) -> bool {
        let s = self.s;
        self.ui.text(x, y - 10.0 * s, s, title, GREY);
        let edge = if focused { [0.55, 0.68, 0.9, 1.0] } else { [0.36, 0.37, 0.44, 1.0] };
        self.ui.bevel(x, y, w, h, s, [0.05, 0.05, 0.07, 1.0], [0.02, 0.02, 0.03, 1.0], edge);
        let cursor = if focused && blink { "_" } else { "" };
        self.ui.text(x + 5.0 * s, (y + (h - 8.0 * s) / 2.0).floor(), s, &format!("{value}{cursor}"), WHITE);
        self.click && self.over(x, y, w, h)
    }
}

/// Saisie clavier dans le champ actif : lettres, chiffres, ponctuation simple, retour arriere.
fn type_into(value: &mut String, inp: &Input, max: usize, allow: fn(char) -> bool) {
    for _ in 0..inp.backspace {
        value.pop();
    }
    for ch in inp.text.chars() {
        if allow(ch) && value.chars().count() < max {
            value.push(ch);
        }
    }
}

fn name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn seed_char(c: char) -> bool {
    !c.is_control()
}

/// Dessine le menu courant et renvoie l'action demandee.
pub fn draw(sh: &mut Shell, ui: &mut Ui, inp: &Input, s: f32) -> Action {
    let (w, h) = (ui.w, ui.h);
    let in_game = sh.game.is_some();
    if in_game {
        ui.rect(0.0, 0.0, w, h, [0.0, 0.0, 0.02, 0.62]);
    } else {
        // Fond : vagues seigaiha, grand soleil rouge et petales de cerisier qui tombent.
        let size = 32.0 * s;
        let (nx, ny) = ((w / size).ceil() as i32, (h / size).ceil() as i32);
        for j in 0..ny {
            for i in 0..nx {
                ui.tile(i as f32 * size, j as f32 * size, size, t::SEIGAIHA, 1.0, WHITE);
            }
        }
        ui.disc((w / 2.0).floor(), (h * 0.27).floor(), 62.0 * s, 2.0 * s, [0.784, 0.22, 0.165, 1.0]);
        for i in 0..30 {
            let rnd = |k: i32| (hash(i, k, 0, 808) & 0xFFFF) as f32 / 65535.0;
            let fall = 10.0 + rnd(2) * 14.0;
            let x = ((rnd(0) * w + sh.t * (6.0 + rnd(3) * 8.0) * s + (sh.t * 1.3 + rnd(4) * 6.0).sin() * 6.0 * s).rem_euclid(w) / s).floor() * s;
            let y = ((rnd(1) * h + sh.t * fall * s).rem_euclid(h) / s).floor() * s;
            ui.rect(x, y, 2.0 * s, 2.0 * s, [0.957, 0.714, 0.784, 1.0]);
            ui.rect(x + 2.0 * s, y + s, s, s, [0.984, 0.851, 0.89, 1.0]);
        }
    }
    let blink = (sh.t * 2.0) as i32 % 2 == 0;
    let mut wd = W { ui, mouse: inp.mouse, click: inp.lmb_click, down: inp.lmb, s };
    let (bw, bh, gap) = (200.0 * s, 20.0 * s, 4.0 * s);
    let x = ((w - bw) / 2.0).floor();
    let mut action = Action::None;

    let title = |wd: &mut W, text: &str, y: f32, scale: f32| {
        let tw = Ui::text_width(text, scale);
        // Une ombre fine : a cette taille, l'ombre d'un point entier empaterait le titre.
        wd.ui.title(((w - tw) / 2.0).floor(), y, scale, scale / 3.0, text, [1.0, 0.86, 0.36, 1.0]);
    };

    match sh.screen {
        Screen::Title => {
            title(&mut wd, "EPICUBE CRAFT", (h * 0.16).floor(), 5.0 * s);
            let mut y = (h * 0.44).floor();
            if wd.button(x, y, bw, bh, "Jouer") {
                sh.open_multi();
            }
            y += bh + gap;
            if wd.button(x, y, bw, bh, "Paramètres") {
                sh.screen = Screen::Settings;
            }
            y += bh + gap;
            if wd.button(x, y, bw, bh, "Quitter") {
                action = Action::Quit;
            }
            // Sous quel nom on joue : le compte du launcher, ou hors ligne.
            let who = match &sh.account {
                Some(a) if a.expired => format!("Compte : {} (session expirée, solo seulement)", a.name),
                Some(a) => format!("Compte : {}", a.name),
                None => format!("Hors ligne : {}", sh.settings.name),
            };
            wd.ui.text(w - Ui::text_width(&who, s) - 4.0 * s, h - 12.0 * s, s, &who, GREY);
            wd.ui.text(4.0 * s, h - 12.0 * s, s, "Epicube Craft 0.15", GREY);
        }
        Screen::Multi => {
            title(&mut wd, "Jouer", (h * 0.03).floor(), 3.0 * s);
            let Some(b) = sh.browser.as_mut() else { return action };
            b.poll();
            // A gauche les modes de jeu, a droite les parties du mode choisi.
            let (sw, mw, top) = (150.0 * s, 440.0 * s, (h * 0.03).floor() + 40.0 * s);
            let sx = ((w - sw - mw - 8.0 * s) / 2.0).floor();
            let mx = sx + sw + 8.0 * s;
            let bottom = h - 22.0 * s;
            wd.ui.bevel(sx, top, sw, bottom - top, s, [0.07, 0.07, 0.1, 0.9], [0.04, 0.04, 0.06, 1.0], [0.3, 0.31, 0.38, 1.0]);
            wd.ui.bevel(mx, top, mw, bottom - top, s, [0.07, 0.07, 0.1, 0.9], [0.04, 0.04, 0.06, 1.0], [0.3, 0.31, 0.38, 1.0]);
            let (ix, iw) = (sx + 6.0 * s, sw - 12.0 * s);
            // Les modes que l'on peut heberger, puis ceux que d'autres hebergent.
            let online = b.categories();
            let mut modes: Vec<(String, String, usize)> = sh.modes.iter().map(|m| (m.id.clone(), m.name.clone(), online.iter().find(|c| c.0 == m.id).map_or(0, |c| c.2))).collect();
            for c in &online {
                if !modes.iter().any(|k| k.0 == c.0) {
                    modes.push(c.clone());
                }
            }
            if !modes.iter().any(|m| m.0 == b.mode) {
                b.mode = modes.first().map(|m| m.0.clone()).unwrap_or_default();
            }
            // Recherche parmi les modes de jeu.
            if wd.field(ix, top + 6.0 * s, iw, bh, "", &b.mode_query, sh.field == 1, blink) {
                sh.field = 1;
            }
            if b.mode_query.is_empty() && sh.field != 1 {
                wd.ui.text(ix + 5.0 * s, top + 12.0 * s, s, "Rechercher un mode", GREY);
            }
            let wanted = shared::commands::plain(b.mode_query.trim());
            let picked_name = modes.iter().find(|c| c.0 == b.mode).map_or(String::new(), |c| c.1.clone());
            modes.retain(|m| wanted.is_empty() || shared::commands::plain(&m.1).contains(&wanted));
            let mut y = top + 12.0 * s + bh;
            let room = ((bottom - 32.0 * s - y) / (bh + 2.0 * s)).floor().max(1.0) as usize;
            // La liste defile a la molette quand elle depasse ; un repere a droite dit ou l'on en est.
            let most = modes.len().saturating_sub(room);
            if wd.over(ix, y, iw, room as f32 * (bh + 2.0 * s)) && inp.scroll != 0.0 {
                b.mode_scroll = (b.mode_scroll as i32 - inp.scroll.signum() as i32 * 2).clamp(0, most as i32) as usize;
            }
            b.mode_scroll = b.mode_scroll.min(most);
            if most > 0 {
                let track = room as f32 * (bh + 2.0 * s) - 2.0 * s;
                let knob = (track * room as f32 / modes.len() as f32).max(8.0 * s);
                wd.ui.rect(ix + iw + s, y, 2.0 * s, track, [0.1, 0.1, 0.14, 1.0]);
                wd.ui.rect(ix + iw + s, y + (track - knob) * b.mode_scroll as f32 / most as f32, 2.0 * s, knob, [0.85, 0.65, 0.16, 1.0]);
            }
            for (id, name, n) in modes.iter().skip(b.mode_scroll).take(room) {
                let picked = b.mode == *id;
                let hot = wd.over(ix, y, iw, bh);
                let fill = match (picked, hot) {
                    (true, _) => [0.85, 0.65, 0.16, 1.0],
                    (false, true) => [0.3, 0.3, 0.4, 1.0],
                    (false, false) => [0.14, 0.14, 0.19, 1.0],
                };
                wd.ui.rect(ix, y, iw, bh, fill);
                wd.ui.text(ix + 5.0 * s, y + 6.0 * s, s, name, WHITE);
                let count = n.to_string();
                wd.ui.text(ix + iw - 5.0 * s - Ui::text_width(&count, s), y + 6.0 * s, s, &count, if picked { WHITE } else { GREY });
                if hot && wd.click {
                    crate::audio::play("clic", 0.6);
                    b.mode = id.clone();
                    b.page = 0;
                }
                y += bh + 2.0 * s;
            }
            if wd.button(ix, bottom - 26.0 * s, iw, bh, "Retour") {
                sh.screen = Screen::Title;
                sh.browser = None;
                return action;
            }
            // Les parties du mode choisi, publiques ou privees.
            let (lx, lw) = (mx + 6.0 * s, mw - 12.0 * s);
            let head = top + 6.0 * s;
            let rows = b.shown();
            wd.ui.text(lx, head + 6.0 * s, s, &format!("{picked_name} : {} partie{}", rows.len(), if rows.len() > 1 { "s" } else { "" }), [1.0, 0.86, 0.36, 1.0]);
            // Recherche parmi les parties, puis le petit « + » : creer sa propre partie de ce mode.
            let hostable = sh.modes.iter().position(|m| m.id == b.mode);
            // A cote du « + », un interrupteur a glissiere : allume, la partie que l'on cree est privee (reservee aux amis).
            let label = "Privée";
            let (sw, sh_) = (22.0 * s, 12.0 * s);
            let tw = Ui::text_width(label, s) + 5.0 * s + sw;
            let endless = hostable.is_some_and(|k| sh.modes[k].keep_world);
            let disk_x = lx + lw - 2.0 * bh - 4.0 * s;
            let (plus_x, toggle_x) = (lx + lw - bh, if endless { disk_x } else { lx + lw - bh } - 8.0 * s - tw);
            if !endless {
                sh.saves_open = false;
            }
            let search_x = toggle_x - 8.0 * s - 150.0 * s;
            if wd.field(search_x, head, 150.0 * s, bh, "", &b.query, sh.field == 2, blink) {
                sh.field = 2;
            }
            if b.query.is_empty() && sh.field != 2 {
                wd.ui.text(search_x + 5.0 * s, head + 6.0 * s, s, "Rechercher une partie", GREY);
            }
            if hostable.is_some() {
                let private = sh.host.visibility == 1;
                wd.ui.text(toggle_x, head + 6.0 * s, s, label, if private { WHITE } else { GREY });
                // La glissiere : une pastille arrondie, grise eteinte, verte allumee, et son bouton blanc.
                let (x, y) = (toggle_x + tw - sw, head + ((bh - sh_) / 2.0).floor());
                let track = if private { [0.2, 0.78, 0.35, 1.0] } else { [0.34, 0.34, 0.4, 1.0] };
                let pill = |ui: &mut Ui, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]| {
                    ui.rect(x + 2.0 * s, y, w - 4.0 * s, h, col);
                    ui.rect(x + s, y + s, w - 2.0 * s, h - 2.0 * s, col);
                    ui.rect(x, y + 2.0 * s, w, h - 4.0 * s, col);
                };
                pill(wd.ui, x, y, sw, sh_, track);
                let kx = if private { x + sw - sh_ + s } else { x + s };
                pill(wd.ui, kx, y + s, sh_ - 2.0 * s, sh_ - 2.0 * s, WHITE);
                if wd.over(toggle_x, head, tw, bh) && wd.click {
                    crate::audio::play("clic", 0.6);
                    sh.host.visibility = if private { 0 } else { 1 };
                }
                // La disquette : les parties sauvegardees de ce mode, a reprendre.
                if endless {
                    let hot = wd.over(disk_x, head, bh, bh);
                    let on = sh.saves_open;
                    wd.ui.bevel(disk_x, head, bh, bh, s, if on || hot { [0.85, 0.65, 0.16, 1.0] } else { [0.16, 0.16, 0.22, 1.0] }, [0.4, 0.4, 0.5, 1.0], [0.06, 0.06, 0.09, 1.0]);
                    let (dx, dy, u) = (disk_x + ((bh - 10.0 * s) / 2.0).floor(), head + ((bh - 10.0 * s) / 2.0).floor(), s);
                    wd.ui.rect(dx, dy, 10.0 * u, 10.0 * u, [0.22, 0.32, 0.62, 1.0]);
                    wd.ui.rect(dx + 8.0 * u, dy, 2.0 * u, 2.0 * u, if on || hot { [0.85, 0.65, 0.16, 1.0] } else { [0.16, 0.16, 0.22, 1.0] });
                    wd.ui.rect(dx + 2.0 * u, dy, 5.0 * u, 3.0 * u, [0.82, 0.84, 0.9, 1.0]);
                    wd.ui.rect(dx + 5.0 * u, dy + u * 0.5, u, 2.0 * u, [0.22, 0.32, 0.62, 1.0]);
                    wd.ui.rect(dx + 2.0 * u, dy + 5.0 * u, 6.0 * u, 5.0 * u, [0.95, 0.95, 0.97, 1.0]);
                    wd.ui.rect(dx + 3.0 * u, dy + 7.0 * u, 4.0 * u, u * 0.6, [0.5, 0.52, 0.6, 1.0]);
                    if hot && wd.click {
                        crate::audio::play("clic", 0.6);
                        sh.saves_open = !sh.saves_open;
                        sh.doomed = None;
                    }
                }
                // Le « + » cree aussitot une partie du mode choisi. Dans un mode sans fin, c'est une partie
                // neuve, sauvegardee toute seule.
                if wd.button(plus_x, head, bh, bh, "+") {
                    sh.host.pick = hostable.unwrap_or(0);
                    sh.host.world.clear();
                    action = Action::Host;
                }
            }
            let (y3, rh) = (head + bh + 6.0 * s, 26.0 * s);
            if sh.saves_open {
                let mode = hostable.map(|k| (sh.modes[k].id.clone(), sh.modes[k].name.clone())).unwrap_or_default();
                let list = crate::saves::list(&mode.0, &mode.1);
                let now = crate::saves::now();
                // Par-dessus le compte des parties en ligne : celui des parties sauvegardees.
                wd.ui.rect(lx - s, head, search_x - lx - 4.0 * s, bh, [0.09, 0.09, 0.13, 1.0]);
                wd.ui.text(lx, head + 6.0 * s, s, &format!("{} : {} sauvegarde{}", mode.1, list.len(), if list.len() > 1 { "s" } else { "" }), [1.0, 0.86, 0.36, 1.0]);
                if list.is_empty() {
                    let text = "Aucune partie sauvegardée. Le « + » en crée une.";
                    wd.ui.text((mx + (mw - Ui::text_width(text, s)) / 2.0).floor(), y3 + 40.0 * s, s, text, GREY);
                }
                let room = (((bottom - 8.0 * s - y3) / (rh + 2.0 * s)).floor().max(1.0)) as usize;
                for (k, save) in list.iter().take(room).enumerate() {
                    let y = y3 + k as f32 * (rh + 2.0 * s);
                    let hot = wd.over(lx, y, lw, rh);
                    wd.ui.rect(lx, y, lw, rh, if hot { [0.2, 0.2, 0.28, 1.0] } else { [0.12, 0.12, 0.16, 1.0] });
                    wd.ui.text(lx + 6.0 * s, y + 4.0 * s, s, &save.name, WHITE);
                    wd.ui.text(lx + 6.0 * s, y + 15.0 * s, s, &crate::saves::ago(save.played, now), GREY);
                    if wd.button(lx + lw - 76.0 * s, y + 3.0 * s, 72.0 * s, bh, "Reprendre") {
                        sh.host.pick = hostable.unwrap_or(0);
                        sh.host.world = save.world.clone();
                        sh.saves_open = false;
                        action = Action::Host;
                    }
                    // Supprimer demande deux clics : le second confirme.
                    let sure = sh.doomed.as_deref() == Some(save.world.as_str());
                    if wd.button(lx + lw - 156.0 * s, y + 3.0 * s, 76.0 * s, bh, if sure { "Confirmer ?" } else { "Supprimer" }) {
                        if sure {
                            crate::saves::delete(&save.world);
                            sh.doomed = None;
                        } else {
                            sh.doomed = Some(save.world.clone());
                        }
                    }
                }
                match sh.field {
                    1 => type_into(&mut b.mode_query, inp, 16, seed_char),
                    _ => type_into(&mut b.query, inp, 20, seed_char),
                }
                return action;
            }
            let per_page = (((bottom - 28.0 * s - y3) / (rh + 2.0 * s)).floor().max(1.0)) as usize;
            let pages = rows.len().div_ceil(per_page).max(1);
            b.page = b.page.min(pages - 1);
            let state = match &b.list {
                None => Some("Recherche des parties en ligne...".to_string()),
                Some(Err(e)) => Some(format!("Parties indisponibles : {e}. Le « + » crée la vôtre.")),
                Some(Ok(_)) if rows.is_empty() => Some("Aucune partie pour le moment. Le « + » crée la vôtre.".to_string()),
                _ => None,
            };
            if let Some(text) = state {
                wd.ui.text((mx + (mw - Ui::text_width(&text, s)) / 2.0).floor(), y3 + 40.0 * s, s, &text, GREY);
            }
            for (k, row) in rows.iter().skip(b.page * per_page).take(per_page).enumerate() {
                let y = y3 + k as f32 * (rh + 2.0 * s);
                let hot = wd.over(lx, y, lw, rh);
                wd.ui.rect(lx, y, lw, rh, if hot { [0.2, 0.2, 0.28, 1.0] } else { [0.12, 0.12, 0.16, 1.0] });
                // Une partie privee porte un cadenas.
                let nx = if row.private {
                    wd.lock(lx + 6.0 * s, y + 4.0 * s);
                    lx + 16.0 * s
                } else {
                    lx + 6.0 * s
                };
                wd.ui.text(nx, y + 4.0 * s, s, &row.name, WHITE);
                let mut about: Vec<String> = Vec::new();
                if !row.host_name.is_empty() {
                    about.push(format!("chez {}", row.host_name));
                }
                about.extend(row.tags.iter().map(|t| format!("#{t}")));
                if row.mods > 0 {
                    about.push(format!("{} mod{}", row.mods, if row.mods > 1 { "s" } else { "" }));
                }
                wd.ui.text(lx + 6.0 * s, y + 15.0 * s, s, &about.join("  "), GREY);
                let count = format!("{} / {}", row.players, row.max_players);
                wd.ui.text(lx + lw - 84.0 * s - Ui::text_width(&count, s), y + 9.0 * s, s, &count, if row.full() { [1.0, 0.5, 0.5, 1.0] } else { WHITE });
                // Le ping : quatre barres, vertes selon sa qualite, et les millisecondes.
                // (Une partie privee qui ne nous est pas ouverte ne donne pas son adresse : pas de ping.)
                if !row.address.is_empty() {
                    let ping = b.ping(&row.address);
                    let lit = ping.flatten().map_or(0, crate::browser::bars);
                    let px = lx + lw - 190.0 * s;
                    for i in 0..4 {
                        let h = (3.0 + 3.0 * i as f32) * s;
                        wd.ui.rect(px + i as f32 * 4.0 * s, y + 19.0 * s - h, 3.0 * s, h, if i < lit { [0.3, 0.86, 0.36, 1.0] } else { [0.26, 0.26, 0.32, 1.0] });
                    }
                    let ms = match ping {
                        None => "...".to_string(),
                        Some(None) => "- ms".to_string(),
                        Some(Some(ms)) => format!("{ms} ms"),
                    };
                    wd.ui.text(px + 19.0 * s, y + 9.0 * s, s, &ms, if lit > 0 { WHITE } else { GREY });
                }
                if row.locked() {
                    // Privee, et l'on n'est pas ami de l'hote : on la voit, on n'y entre pas.
                    wd.ui.bevel(lx + lw - 76.0 * s, y + 3.0 * s, 72.0 * s, bh, s, [0.2, 0.2, 0.24, 1.0], [0.3, 0.3, 0.36, 1.0], [0.1, 0.1, 0.12, 1.0]);
                    wd.label(lx + lw - 76.0 * s, y + 3.0 * s, 72.0 * s, bh, "Privée", GREY);
                } else if wd.button(lx + lw - 76.0 * s, y + 3.0 * s, 72.0 * s, bh, if row.full() { "Complet" } else { "Rejoindre" }) && !row.full() {
                    action = Action::JoinAt(row.address.clone());
                }
            }
            if pages > 1 {
                let by = bottom - 24.0 * s;
                if wd.button(lx, by, 80.0 * s, bh, "Précédent") {
                    b.page = b.page.saturating_sub(1);
                }
                if wd.button(lx + lw - 80.0 * s, by, 80.0 * s, bh, "Suivant") {
                    b.page = (b.page + 1).min(pages - 1);
                }
                wd.label(lx, by, lw, bh, &format!("{} / {pages}", b.page + 1), GREY);
            }
            match sh.field {
                1 => type_into(&mut b.mode_query, inp, 16, seed_char),
                _ => {
                    let before = b.query.clone();
                    type_into(&mut b.query, inp, 20, seed_char);
                    if b.query != before {
                        b.page = 0;
                    }
                }
            }
        }
        Screen::Settings | Screen::PauseSettings => {
            if sh.skin.is_none() {
                title(&mut wd, if sh.keys_page { "Contrôles" } else { "Paramètres" }, (h * 0.06).floor(), 3.0 * s);
            }
            // Deux colonnes.
            let cw = 190.0 * s;
            let (x1, x2) = ((w / 2.0 - cw - 3.0 * s).floor(), (w / 2.0 + 3.0 * s).floor());
            let y0 = (h * 0.06).floor() + 40.0 * s;
            let row = |k: f32| y0 + k * (bh + gap);
            if sh.skin.is_some() {
                // L'editeur de skin : il occupe tout l'ecran des parametres.
                title(&mut wd, "Skin", (h * 0.03).floor(), 3.0 * s);
                let pw = (w - 24.0 * s).min(440.0 * s);
                let (px, py) = (((w - pw) / 2.0).floor(), (h * 0.03).floor() + 34.0 * s);
                let ph = h - py - 6.0 * s;
                wd.ui.rect(px - 6.0 * s, py - 6.0 * s, pw + 12.0 * s, ph + 6.0 * s, [0.07, 0.07, 0.1, 0.93]);
                // Deux onglets : dessiner son skin, ou feuilleter ceux que les joueurs ont publies.
                let tab = sh.skin.as_ref().map_or(0, |e| e.tab);
                for (k, name) in ["Éditeur", "Bibliothèque"].into_iter().enumerate() {
                    let (tx, tw) = (px + k as f32 * 92.0 * s, 90.0 * s);
                    let hot = wd.over(tx, py, tw, bh);
                    wd.ui.bevel(tx, py, tw, bh, s, if tab == k { [0.85, 0.65, 0.16, 1.0] } else if hot { [0.3, 0.3, 0.4, 1.0] } else { [0.14, 0.14, 0.19, 1.0] }, [0.4, 0.4, 0.5, 1.0], [0.06, 0.06, 0.09, 1.0]);
                    wd.label(tx, py, tw, bh, name, WHITE);
                    if hot && wd.click && tab != k {
                        if let Some(ed) = sh.skin.as_mut() {
                            ed.tab = k;
                        }
                        if k == 1 && sh.shelf.is_none() {
                            sh.shelf = Some(crate::skinlib::Shelf::open(sh.account.as_ref().map(|a| a.token.clone())));
                        }
                        sh.field = if k == 1 { 1 } else { 0 };
                    }
                }
                let (cy, chh) = (py + bh + 6.0 * s, ph - bh - 6.0 * s);
                let model = sh.models[crate::models::PLAYER].clone();
                let mut ask = crate::skin::Ask::Nothing;
                let tab = sh.skin.as_ref().map_or(0, |e| e.tab);
                if tab == 1 {
                    let mine = sh.skins[crate::skin::OWN].clone().unwrap_or_else(crate::skin::default_skin);
                    let mut field = sh.field as u8;
                    let mut wear = None;
                    if let Some(shelf) = sh.shelf.as_mut() {
                        shelf.poll();
                        match shelf.draw(wd.ui, &mine, wd.mouse, wd.click, px, cy, pw, chh, s, sh.t, &mut field) {
                            crate::skinlib::Ask::Wear(skin) => wear = Some(skin),
                            crate::skinlib::Ask::Close => ask = crate::skin::Ask::Close,
                            crate::skinlib::Ask::Nothing => {}
                        }
                        match field {
                            1 => type_into(&mut shelf.query, inp, 24, seed_char),
                            2 => type_into(&mut shelf.name, inp, 24, seed_char),
                            _ => {}
                        }
                        // Les skins de la page rejoignent l'atlas, chacun dans sa case.
                        for (slot, skin) in shelf.fresh.drain(..) {
                            sh.skins[slot] = Some(skin);
                            sh.skin_dirty.push(slot);
                        }
                    }
                    sh.field = field as _;
                    if let (Some(skin), Some(ed)) = (wear, sh.skin.as_mut()) {
                        // Le skin choisi devient celui que l'on dessine : on revient a l'editeur pour l'enregistrer.
                        ed.take(skin);
                        ed.tab = 0;
                        ed.note = "Skin de la bibliothèque : enregistrez pour le garder.".into();
                    }
                } else if let Some(ed) = sh.skin.as_mut() {
                    ask = ed.draw(wd.ui, &model, wd.mouse, wd.click, wd.down, px, cy, pw, chh, s, 1.0 / 60.0);
                }
                if let Some(ed) = sh.skin.as_mut() {
                    if ed.dirty {
                        // Le dessin en cours se voit aussitot sur le personnage.
                        ed.dirty = false;
                        sh.skins[crate::skin::OWN] = Some(ed.px.clone());
                        sh.skin_dirty.push(crate::skin::OWN);
                    }
                }
                match ask {
                    crate::skin::Ask::Close => sh.close_skin(),
                    crate::skin::Ask::Saved => sh.skin_saved(),
                    crate::skin::Ask::Nothing => {}
                }
                return action;
            }
            if let Some(lib) = sh.library.as_mut() {
                // Page des mods : la bibliotheque d'Epicube Hub et ce qui est installe.
                lib.poll();
                let lw = 420.0 * s;
                let lx = ((w - lw) / 2.0).floor();
                let state = match &lib.list {
                    None => "Chargement de la bibliothèque...".to_string(),
                    Some(Err(e)) => format!("Bibliothèque indisponible : {e}."),
                    Some(Ok(l)) if l.is_empty() && lib.installed.is_empty() => "Aucun mod dans la bibliothèque pour le moment.".to_string(),
                    Some(Ok(_)) => String::new(),
                };
                wd.ui.text(((w - Ui::text_width(&state, s)) / 2.0).floor(), y0 - 12.0 * s, s, &state, GREY);
                // Les mods de la bibliotheque, puis ceux qui ne sont que dans le dossier du joueur.
                let listed: Vec<crate::library::Entry> = lib.list.as_ref().and_then(|l| l.as_ref().ok()).cloned().unwrap_or_default();
                let mut rows: Vec<(String, String, String, Option<crate::library::Entry>)> = listed
                    .iter()
                    .map(|e| {
                        let by = if e.author.is_empty() { String::new() } else { format!(" - {}", e.author) };
                        (e.id.clone(), format!("{} {}{by} ({} ko)", e.name, e.version, e.size.div_ceil(1024)), e.description.clone(), Some(e.clone()))
                    })
                    .collect();
                for i in lib.installed.iter().filter(|i| !listed.iter().any(|e| e.id == i.id)) {
                    rows.push((i.id.clone(), format!("{} {} (hors bibliothèque)", i.name, i.version), i.description.clone(), None));
                }
                const PER_PAGE: usize = 6;
                let pages = rows.len().div_ceil(PER_PAGE).max(1);
                lib.page = lib.page.min(pages - 1);
                let rh = 30.0 * s;
                let mut clicked: Option<(String, Option<crate::library::Entry>, bool)> = None;
                for (k, (id, head, about, entry)) in rows.iter().skip(lib.page * PER_PAGE).take(PER_PAGE).enumerate() {
                    let y = y0 + k as f32 * (rh + 2.0 * s);
                    wd.ui.bevel(lx, y, lw, rh, s, [0.09, 0.09, 0.12, 0.92], [0.05, 0.05, 0.07, 1.0], [0.3, 0.31, 0.38, 1.0]);
                    wd.ui.text(lx + 6.0 * s, y + 5.0 * s, s, head, WHITE);
                    let lines = crate::chatbox::wrap(about, s, lw - 104.0 * s);
                    if let Some(first) = lines.first() {
                        let line = if lines.len() > 1 { format!("{first}...") } else { first.clone() };
                        wd.ui.text(lx + 6.0 * s, y + 17.0 * s, s, &line, GREY);
                    }
                    let have = lib.installed(id).map(|i| i.version.clone());
                    let update = matches!((&have, entry), (Some(v), Some(e)) if *v != e.version);
                    let label = if lib.busy.as_deref() == Some(id.as_str()) {
                        "Installation..."
                    } else if update {
                        "Mettre à jour"
                    } else if have.is_some() && lib.removable(id) {
                        "Supprimer"
                    } else if have.is_some() {
                        "Dossier"
                    } else {
                        "Installer"
                    };
                    if wd.button(lx + lw - 92.0 * s, y + 5.0 * s, 86.0 * s, bh, label) {
                        clicked = Some((id.clone(), entry.clone(), have.is_some() && !update));
                    }
                }
                if let Some((id, entry, installed)) = clicked {
                    match entry {
                        _ if installed => {
                            if lib.removable(&id) {
                                lib.remove(&id);
                            }
                        }
                        Some(e) => lib.install(&e),
                        None => {}
                    }
                }
                let by = y0 + PER_PAGE as f32 * (rh + 2.0 * s) + 2.0 * s;
                if pages > 1 {
                    if wd.button(lx, by, 80.0 * s, bh, "Précédent") {
                        lib.page = lib.page.saturating_sub(1);
                    }
                    if wd.button(lx + lw - 80.0 * s, by, 80.0 * s, bh, "Suivant") {
                        lib.page = (lib.page + 1).min(pages - 1);
                    }
                    wd.label(lx, by, lw, bh, &format!("{} / {pages}", lib.page + 1), GREY);
                }
                if let Some(note) = &lib.note {
                    wd.ui.text(((w - Ui::text_width(note, s)) / 2.0).floor(), by + bh + 5.0 * s, s, note, [0.6, 1.0, 0.6, 1.0]);
                }
                if wd.button(x, by + bh + 18.0 * s, bw, bh, "Retour") {
                    sh.library = None;
                }
                return action;
            }
            let st = &mut sh.settings;
            if sh.sound_page {
                // Page du son : quatre volumes.
                let xm = (w / 2.0 - cw / 2.0).floor();
                let rows: [(u8, &str, &mut f32); 4] = [
                    (5, "Volume général", &mut st.vol_master),
                    (6, "Musique", &mut st.vol_music),
                    (7, "Bruitages", &mut st.vol_sfx),
                    (8, "Ambiance", &mut st.vol_ambience),
                ];
                for (k, (id, label, value)) in rows.into_iter().enumerate() {
                    let mut v = *value;
                    wd.slider(id, xm, row(k as f32), cw, bh, &format!("{label} : {:.0} %", v * 100.0), &mut v, &mut sh.drag);
                    *value = (v * 20.0).round() / 20.0;
                }
                // Chat vocal : volume des voix, micro, sensibilite et test.
                let mut v = st.vol_voice;
                wd.slider(10, xm, row(4.0), cw, bh, &format!("Voix des joueurs : {:.0} %", v * 100.0), &mut v, &mut sh.drag);
                st.vol_voice = (v * 20.0).round() / 20.0;
                let mode = ["coupé", "appuyer sur V pour parler", "se déclenche à la voix", "ouvert en permanence"][st.mic.min(3) as usize];
                if wd.button(xm, row(5.0), cw, bh, &format!("Micro : {mode}")) {
                    // Dans l'ordre : coupe, appuyer pour parler, ouvert en permanence, declenchement a la voix.
                    st.mic = match st.mic {
                        0 => 1,
                        1 => 3,
                        3 => 2,
                        _ => 0,
                    };
                    sh.mic_error = None;
                }
                if st.mic == 2 {
                    let mut v = ((st.mic_gate - 0.002) / 0.098).clamp(0.0, 1.0);
                    wd.slider(11, xm, row(6.0), cw, bh, &format!("Seuil de la voix : {:.0} %", v * 100.0), &mut v, &mut sh.drag);
                    st.mic_gate = 0.002 + v * 0.098;
                }
                if st.mic != 0 {
                    wd.toggle(xm, row(7.0), cw, bh, "S'entendre (test du micro)", &mut sh.mic_echo);
                    // Niveau capte : une barre, verte quand la voix passe.
                    let (bx, by) = (xm, row(8.0));
                    let (level, live) = sh.mic.as_ref().map_or((0.0, false), |m| ((m.level() * 6.0).min(1.0), m.talking()));
                    wd.ui.rect(bx, by, cw, 5.0 * s, [0.05, 0.06, 0.12, 1.0]);
                    wd.ui.rect(bx, by, (cw * level).floor(), 5.0 * s, if live { [0.3, 0.85, 0.4, 1.0] } else { [0.6, 0.62, 0.7, 1.0] });
                    let note = match (&sh.mic_error, &sh.mic) {
                        (Some(e), _) => format!("Micro indisponible : {e}"),
                        (None, Some(m)) => format!("Micro : {}", m.device),
                        _ => "Ouverture du micro...".to_string(),
                    };
                    wd.ui.text(bx, by + 8.0 * s, s, &note, GREY);
                } else {
                    sh.mic_echo = false;
                }
                if wd.button(x, row(10.0), bw, bh, "Retour") {
                    sh.sound_page = false;
                }
                return action;
            }
            if sh.keys_page {
                // Page des controles : une touche par action, que l'on change d'un clic puis d'une touche.
                use crate::controls::{self, Controls};
                let (rh, kw) = (17.0 * s, 78.0 * s);
                let colw = 232.0 * s;
                let cols = [(w / 2.0 - colw - 6.0 * s).floor(), (w / 2.0 + 6.0 * s).floor()];
                let per_col = controls::COUNT.div_ceil(2);
                let top = y0 - 8.0 * s;
                // La touche enfoncee va a l'action qui l'attend.
                if let Some(index) = sh.rebinding {
                    if let Some(key) = inp.pressed.iter().copied().find(|k| controls::bindable(*k)) {
                        let label = inp.typed.iter().find(|t| t.0 == key).map(|t| t.1.as_str());
                        st.controls.set(index, key, label);
                        sh.rebinding = None;
                    }
                }
                for index in 0..controls::COUNT {
                    let (cx, y) = (cols[index / per_col], top + (index % per_col) as f32 * (rh + 2.0 * s));
                    wd.ui.rect(cx, y, colw, rh, [0.07, 0.07, 0.1, 0.85]);
                    wd.ui.text(cx + 5.0 * s, y + 4.5 * s, s, Controls::title(index), WHITE);
                    let waiting = sh.rebinding == Some(index);
                    let (kx, hot) = (cx + colw - kw - 2.0 * s, wd.over(cx + colw - kw - 2.0 * s, y + s, kw, rh - 2.0 * s));
                    let fill = match (waiting, hot) {
                        (true, _) => [0.85, 0.65, 0.16, 1.0],
                        (false, true) => [0.36, 0.37, 0.48, 1.0],
                        (false, false) => [0.2, 0.2, 0.27, 1.0],
                    };
                    wd.ui.rect(kx, y + s, kw, rh - 2.0 * s, fill);
                    // Une touche donnee a deux actions est ecrite en rouge.
                    let (text, color) = if waiting {
                        ("> ? <".to_string(), WHITE)
                    } else if st.controls.clash(index) {
                        (st.controls.shown(index), [1.0, 0.45, 0.45, 1.0])
                    } else {
                        (st.controls.shown(index), WHITE)
                    };
                    wd.label(kx, y + s, kw, rh - 2.0 * s, &text, color);
                    if hot && wd.click {
                        crate::audio::play("clic", 0.6);
                        sh.rebinding = if waiting { None } else { Some(index) };
                    }
                }
                let by = top + per_col as f32 * (rh + 2.0 * s) + 4.0 * s;
                let note = if sh.rebinding.is_some() { "Appuyez sur la nouvelle touche" } else { "" };
                wd.ui.text(((w - Ui::text_width(note, s)) / 2.0).floor(), by, s, note, GREY);
                let half = (bw / 2.0 - 2.0 * s).floor();
                if wd.button(x, by + 12.0 * s, half, bh, "Réinitialiser") {
                    st.controls.reset();
                    sh.rebinding = None;
                }
                if wd.button(x + bw - half, by + 12.0 * s, half, bh, "Retour") {
                    sh.keys_page = false;
                    sh.rebinding = None;
                }
                return action;
            }
            if sh.perf_page {
                // Page des performances : resolution, limite d'images, synchro.
                let xm = (w / 2.0 - cw / 2.0).floor();
                let res = match st.res {
                    0 => "auto".to_string(),
                    r => format!("{r} %"),
                };
                if wd.button(xm, row(0.0), cw, bh, &format!("Résolution : {res}")) {
                    st.res = match st.res {
                        0 => 100,
                        100 => 75,
                        75 => 50,
                        _ => 0,
                    };
                }
                let cap = if st.fps_max == 0 { "illimitées".to_string() } else { st.fps_max.to_string() };
                if wd.button(xm, row(1.0), cw, bh, &format!("Images par seconde : {cap}")) {
                    st.fps_max = match st.fps_max {
                        0 => 30,
                        30 => 60,
                        60 => 120,
                        120 => 144,
                        144 => 240,
                        _ => 0,
                    };
                }
                wd.toggle(xm, row(2.0), cw, bh, "Synchro verticale", &mut st.vsync);
                let mut v = (st.view - 4) as f32 / 8.0;
                wd.slider(9, xm, row(3.0), cw, bh, &format!("Distance de vue : {} chunks", st.view), &mut v, &mut sh.drag);
                st.view = 4 + (v * 8.0).round() as i32;
                let lines_at = if cfg!(target_os = "macos") {
                    wd.toggle(xm, row(4.0), cw, bh, "Interface basse résolution", &mut st.lowres);
                    5.0
                } else {
                    4.0
                };
                if wd.button(x, row(lines_at) + 12.0 * s, bw, bh, "Retour") {
                    sh.perf_page = false;
                }
                return action;
            }
            let half = (cw / 2.0 - 2.0 * s).floor();
            if wd.button(x2, row(5.0) + 12.0 * s, half, bh, "Son") {
                sh.sound_page = true;
            }
            if wd.button(x2 + cw - half, row(5.0) + 12.0 * s, half, bh, "Performances") {
                sh.perf_page = true;
            }

            let mut v = (st.view - 4) as f32 / 8.0;
            wd.slider(1, x1, row(0.0), cw, bh, &format!("Distance de vue : {} chunks", st.view), &mut v, &mut sh.drag);
            st.view = 4 + (v * 8.0).round() as i32;

            let mut v = (st.fov - 50.0) / 60.0;
            wd.slider(2, x1, row(1.0), cw, bh, &format!("Champ de vision : {:.0}", st.fov), &mut v, &mut sh.drag);
            st.fov = (50.0 + v * 60.0).round();

            let mut v = (st.sens - 0.2) / 2.8;
            wd.slider(3, x1, row(2.0), cw, bh, &format!("Sensibilité : {:.0} %", st.sens * 100.0), &mut v, &mut sh.drag);
            st.sens = ((0.2 + v * 2.8) * 20.0).round() / 20.0;

            let mut v = st.gui as f32 / 4.0;
            let size = if st.gui == 0 { "auto".to_string() } else { format!("x{}", st.gui) };
            wd.slider(4, x1, row(3.0), cw, bh, &format!("Taille de l'interface : {size}"), &mut v, &mut sh.drag);
            st.gui = (v * 4.0).round() as u32;

            wd.toggle(x2, row(0.0), cw, bh, "Synchro verticale", &mut st.vsync);
            wd.toggle(x2, row(1.0), cw, bh, "Plein écran", &mut st.fullscreen);
            wd.toggle(x2, row(2.0), cw, bh, "Nuages", &mut st.clouds);
            wd.toggle(x2, row(3.0), cw, bh, "Balancement de la caméra", &mut st.bobbing);

            wd.toggle(x2, row(4.0), cw, bh, "Contours à l'encre", &mut st.ink);
            if !in_game {
                if let Some(a) = &sh.account {
                    wd.field(x1, row(5.0) + 12.0 * s, cw, bh, "Compte", &a.name, false, false);
                    sh.field = 0;
                } else if wd.field(x1, row(5.0) + 12.0 * s, cw, bh, "Pseudo", &st.name.clone(), sh.field == 2, blink) {
                    sh.field = 2;
                }
                if sh.field == 2 {
                    type_into(&mut st.name, inp, 16, name_char);
                }
            }
            if wd.button(x1, row(6.0) + 12.0 * s, cw, bh, "Contrôles") {
                sh.keys_page = true;
            }
            if wd.button(x1, row(7.0) + 12.0 * s, cw, bh, "Skin") {
                sh.skin = Some(crate::skin::Editor::open());
            }
            if !in_game && wd.button(x2, row(6.0) + 12.0 * s, cw, bh, "Mods") {
                sh.library = Some(crate::library::Library::open());
            }
            // Ressources modifiables.
            let ry = row(8.0) + 14.0 * s;
            if wd.button(x1, ry, cw, bh, "Exporter les ressources") {
                action = Action::Export;
            }
            if wd.button(x2, ry, cw, bh, "Recharger les ressources") {
                action = Action::Reload;
            }
            if let Some(msg) = &sh.status {
                wd.ui.text(x1, ry + bh + 4.0 * s, s, msg, [0.6, 1.0, 0.6, 1.0]);
            }
            if wd.button(x, ry + bh + 18.0 * s, bw, bh, "Retour") {
                sh.field = 0;
                sh.screen = if sh.screen == Screen::PauseSettings { Screen::Pause } else { Screen::Title };
            }
        }
        Screen::Pause => {
            title(&mut wd, "Pause", (h * 0.14).floor(), 3.0 * s);
            let mut y = (h * 0.34).floor();
            if wd.button(x, y, bw, bh, "Reprendre") {
                sh.screen = Screen::Game;
            }
            y += bh + gap;
            if wd.button(x, y, bw, bh, "Paramètres") {
                sh.screen = Screen::PauseSettings;
            }
            y += bh + gap;
            // Regles de la partie : la difficulte se change ici ; le mode de jeu, si le monde l'autorise.
            if let Some(g) = sh.game.as_mut() {
                if wd.button(x, y, bw, bh, &format!("Mode de jeu : {}", shared::rules::mode_name(g.mode))) {
                    g.cycle_mode();
                }
                y += bh + gap;
                let diff = shared::rules::difficulty_name(g.rules.difficulty);
                if wd.button(x, y, bw, bh, &format!("Difficulté : {diff}")) {
                    g.cycle_difficulty();
                }
                y += bh + gap;
            }
            let hosting = sh.party_lines();
            let leave = match (&hosting, sh.is_local()) {
                (Some(_), _) => "Arrêter la partie",
                (None, true) => "Sauvegarder et quitter",
                (None, false) => "Se déconnecter",
            };
            if wd.button(x, y, bw, bh, leave) {
                action = Action::Leave;
            }
            // La partie que l'on heberge : ou en est sa mise en ligne.
            if let Some(lines) = hosting {
                let (px, py, pw) = (8.0 * s, 8.0 * s, 270.0 * s);
                let pw = lines.iter().map(|l| Ui::text_width(l, s)).fold(0.0, f32::max).min(pw) + 12.0 * s;
                wd.ui.rect(px, py, pw, (8.0 + 11.0 * lines.len() as f32) * s, [0.0, 0.0, 0.02, 0.7]);
                for (k, line) in lines.iter().enumerate() {
                    wd.ui.text(px + 6.0 * s, py + 6.0 * s + k as f32 * 11.0 * s, s, line, if k == 0 { [1.0, 0.86, 0.36, 1.0] } else { WHITE });
                }
            }
        }
        Screen::Game => {}
    }
    if let Some(err) = &sh.error {
        // Un motif long tient sur plusieurs lignes, la derniere au meme endroit qu'une ligne seule.
        let lines = crate::chatbox::wrap(err, s, w - 40.0 * s);
        let tw = lines.iter().map(|l| Ui::text_width(l, s)).fold(0.0, f32::max);
        let top = h - 30.0 * s - 11.0 * s * (lines.len() as f32 - 1.0);
        wd.ui.rect((w - tw) / 2.0 - 4.0 * s, top, tw + 8.0 * s, 3.0 * s + 11.0 * s * lines.len() as f32, [0.4, 0.0, 0.0, 0.85]);
        for (k, line) in lines.iter().enumerate() {
            wd.ui.text((w - Ui::text_width(line, s)) / 2.0, top + 3.0 * s + 11.0 * s * k as f32, s, line, WHITE);
        }
    }
    action
}

/// Largeur de la barre des amis, en points d'interface.
pub const FRIENDS_W: f32 = 176.0;

/// La barre des amis, a droite de tous les ecrans : repliee, une languette avec le nombre de messages
/// et de demandes en attente ; depliee, la liste des amis ou la conversation ouverte.
/// `free` : le curseur est libre (on n'est pas en train de jouer, souris capturee).
pub fn draw_friends(f: &mut crate::friends::Friends, ui: &mut Ui, inp: &Input, s: f32, t: f32, free: bool) {
    use winit::keyboard::KeyCode;
    let (w, h) = (ui.w, ui.h);
    let mut wd = W { ui, mouse: inp.mouse, click: inp.lmb_click && free, down: inp.lmb, s };
    let bh = 18.0 * s;
    if !f.open {
        // Repliee : une languette en haut a droite. En jeu, elle ne se montre que s'il y a du nouveau.
        let n = f.waiting();
        if !free && n == 0 {
            return;
        }
        let label = if n > 0 { format!("Amis ({n})") } else { "Amis".to_string() };
        let tw = Ui::text_width(&label, s) + 14.0 * s;
        if free {
            if wd.button(w - tw - 4.0 * s, 4.0 * s, tw, bh, &label) {
                f.toggle();
            }
        } else {
            wd.ui.rect(w - tw - 4.0 * s, 4.0 * s, tw, bh, [0.0, 0.0, 0.02, 0.6]);
            wd.label(w - tw - 4.0 * s, 4.0 * s, tw, bh, &label, [1.0, 0.86, 0.36, 1.0]);
        }
        return;
    }
    let pw = FRIENDS_W * s;
    let (px, ix, iw) = (w - pw, w - pw + 6.0 * s, pw - 12.0 * s);
    wd.ui.rect(px, 0.0, pw, h, [0.05, 0.05, 0.08, 0.96]);
    wd.ui.rect(px, 0.0, s, h, [0.3, 0.31, 0.38, 1.0]);
    let blink = (t * 2.0) as i32 % 2 == 0;
    let enter = inp.pressed.contains(&KeyCode::Enter) || inp.pressed.contains(&KeyCode::NumpadEnter);
    let small = |wd: &mut W, x: f32, y: f32, bw: f32, text: &str| -> bool { wd.button(x, y, bw, 16.0 * s, text) };
    if small(&mut wd, px + pw - 22.0 * s, 4.0 * s, 16.0 * s, ">") {
        f.toggle();
        return;
    }
    if !f.has_account() {
        wd.ui.text(ix, 8.0 * s, s, "Amis", [1.0, 0.86, 0.36, 1.0]);
        for (k, line) in crate::chatbox::wrap("Lancez le jeu depuis Epicube Hub, connecté à votre compte, pour retrouver vos amis.", s, iw).iter().enumerate() {
            wd.ui.text(ix, 30.0 * s + k as f32 * 10.0 * s, s, line, GREY);
        }
        return;
    }
    if let Some(buddy) = f.chat_with.clone() {
        // Conversation : l'ami et ce qu'il fait, les messages, la saisie en bas.
        let status = match &f.list {
            Some(Ok(l)) => l.friends.iter().find(|x| x.id == buddy.id).map_or(buddy.status.clone(), |x| x.status.clone()),
            _ => buddy.status.clone(),
        };
        if small(&mut wd, ix, 4.0 * s, 16.0 * s, "<") {
            f.close_chat();
            return;
        }
        wd.ui.text(ix + 22.0 * s, 8.0 * s, s, &buddy.name, [1.0, 0.86, 0.36, 1.0]);
        wd.ui.text(ix, 24.0 * s, s, &status, GREY);
        if small(&mut wd, ix + iw - 44.0 * s, 20.0 * s, 44.0 * s, "Retirer") {
            f.remove(&buddy);
            f.close_chat();
            return;
        }
        let iy = h - bh - 6.0 * s;
        wd.field(ix, iy, iw - 22.0 * s, bh, "", &f.input, true, blink);
        if small(&mut wd, ix + iw - 18.0 * s, iy + s, 18.0 * s, ">") || enter {
            f.send();
        }
        type_into(&mut f.input, inp, 200, seed_char);
        let (mut y, top) = (iy - 6.0 * s, 42.0 * s);
        if f.chat.is_empty() {
            wd.ui.text(ix, top, s, "Aucun message.", GREY);
        }
        for m in f.chat.iter().rev() {
            let lines = crate::chatbox::wrap(&m.body, s, iw * 0.82);
            let width = lines.iter().map(|l| Ui::text_width(l, s)).fold(0.0, f32::max) + 8.0 * s;
            let height = lines.len() as f32 * 10.0 * s + 5.0 * s;
            y -= height + 3.0 * s;
            if y < top {
                break;
            }
            // Les notres a droite, les siens a gauche.
            let x0 = if m.mine { ix + iw - width } else { ix };
            wd.ui.rect(x0, y, width, height, if m.mine { [0.72, 0.2, 0.165, 1.0] } else { [0.18, 0.18, 0.24, 1.0] });
            for (k, line) in lines.iter().enumerate() {
                wd.ui.text(x0 + 4.0 * s, y + 3.0 * s + k as f32 * 10.0 * s, s, line, WHITE);
            }
        }
        return;
    }
    // La liste : ajouter un ami, les demandes recues, les amis, les demandes envoyees.
    wd.ui.text(ix, 8.0 * s, s, "Amis", [1.0, 0.86, 0.36, 1.0]);
    let mut y = 36.0 * s;
    wd.field(ix, y, iw - 22.0 * s, bh, "Ajouter un ami : son pseudo", &f.name, true, blink);
    if small(&mut wd, ix + iw - 18.0 * s, y + s, 18.0 * s, "+") || enter {
        f.ask();
    }
    type_into(&mut f.name, inp, 16, name_char);
    y += bh + 4.0 * s;
    if let Some(note) = f.note.clone() {
        for line in crate::chatbox::wrap(&note, s, iw).iter().take(2) {
            wd.ui.text(ix, y, s, line, [0.6, 1.0, 0.6, 1.0]);
            y += 10.0 * s;
        }
    }
    y += 4.0 * s;
    let list = match &f.list {
        None => {
            wd.ui.text(ix, y, s, "Chargement...", GREY);
            return;
        }
        Some(Err(e)) => {
            for (k, line) in crate::chatbox::wrap(e, s, iw).iter().enumerate() {
                wd.ui.text(ix, y + k as f32 * 10.0 * s, s, line, GREY);
            }
            return;
        }
        Some(Ok(l)) => l.clone(),
    };
    let heading = |wd: &mut W, y: &mut f32, text: String| {
        wd.ui.text(ix, *y, s, &text, GREY);
        *y += 11.0 * s;
    };
    if !list.received.is_empty() {
        heading(&mut wd, &mut y, format!("Demandes reçues ({})", list.received.len()));
        for b in &list.received {
            wd.ui.rect(ix, y, iw, 18.0 * s, [0.12, 0.12, 0.16, 1.0]);
            wd.ui.text(ix + 4.0 * s, y + 5.0 * s, s, &b.name, WHITE);
            if small(&mut wd, ix + iw - 58.0 * s, y + s, 30.0 * s, "Oui") {
                f.accept(b);
            }
            if small(&mut wd, ix + iw - 26.0 * s, y + s, 26.0 * s, "Non") {
                f.remove(b);
            }
            y += 20.0 * s;
        }
        y += 4.0 * s;
    }
    let online = list.friends.iter().filter(|b| b.online).count();
    heading(&mut wd, &mut y, format!("Mes amis ({online} en ligne sur {})", list.friends.len()));
    if list.friends.is_empty() {
        wd.ui.text(ix, y, s, "Personne pour le moment.", GREY);
        y += 12.0 * s;
    }
    for b in &list.friends {
        if y + 24.0 * s > h - 4.0 * s {
            break;
        }
        // Un clic sur un ami ouvre la conversation.
        let hot = wd.over(ix, y, iw, 22.0 * s);
        wd.ui.rect(ix, y, iw, 22.0 * s, if hot { [0.22, 0.22, 0.3, 1.0] } else { [0.12, 0.12, 0.16, 1.0] });
        wd.ui.rect(ix + 4.0 * s, y + 5.0 * s, 4.0 * s, 4.0 * s, if b.online { [0.4, 0.9, 0.5, 1.0] } else { [0.35, 0.36, 0.42, 1.0] });
        wd.ui.text(ix + 12.0 * s, y + 3.0 * s, s, &b.name, WHITE);
        wd.ui.text(ix + 12.0 * s, y + 12.0 * s, s, &b.status, GREY);
        if b.unread > 0 {
            let n = b.unread.min(99).to_string();
            let nw = Ui::text_width(&n, s) + 8.0 * s;
            wd.ui.rect(ix + iw - nw - 3.0 * s, y + 5.0 * s, nw, 12.0 * s, [0.85, 0.65, 0.16, 1.0]);
            wd.ui.text(ix + iw - nw + s, y + 7.0 * s, s, &n, WHITE);
        }
        if hot && wd.click {
            crate::audio::play("clic", 0.6);
            f.open_chat(b);
        }
        y += 24.0 * s;
    }
    if !list.sent.is_empty() {
        y += 4.0 * s;
        heading(&mut wd, &mut y, format!("Demandes envoyées ({})", list.sent.len()));
        for b in &list.sent {
            if y + 20.0 * s > h - 4.0 * s {
                break;
            }
            wd.ui.rect(ix, y, iw, 18.0 * s, [0.12, 0.12, 0.16, 1.0]);
            wd.ui.text(ix + 4.0 * s, y + 5.0 * s, s, &b.name, GREY);
            if small(&mut wd, ix + iw - 46.0 * s, y + s, 46.0 * s, "Annuler") {
                f.remove(b);
            }
            y += 20.0 * s;
        }
    }
}
