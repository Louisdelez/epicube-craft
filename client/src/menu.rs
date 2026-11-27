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
    Solo,
    Join,
    Quit,
    /// Quitter la partie et revenir a l'ecran-titre.
    Leave,
    Export,
    Reload,
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

fn address_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-')
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
        wd.ui.text(((w - tw) / 2.0).floor() + scale, y + scale, scale, text, [0.0, 0.0, 0.0, 0.5]);
        wd.ui.text(((w - tw) / 2.0).floor(), y, scale, text, [1.0, 0.86, 0.36, 1.0]);
    };

    match sh.screen {
        Screen::Title => {
            title(&mut wd, "EPICUBE CRAFT", (h * 0.16).floor(), 5.0 * s);
            let sub = "Un monde de blocs à explorer";
            wd.ui.text((w - Ui::text_width(sub, s)) / 2.0, (h * 0.16).floor() + 48.0 * s, s, sub, GREY);
            let mut y = (h * 0.44).floor();
            let solo = if sh.settings.lan { "Jouer (ouvert au réseau local)" } else { "Jouer en solo" };
            if wd.button(x, y, bw, bh, solo) {
                sh.screen = Screen::Solo;
                sh.field = 1;
            }
            y += bh + gap;
            if wd.button(x, y, bw, bh, "Multijoueur") {
                sh.screen = Screen::Multi;
                sh.field = 1;
            }
            y += bh + gap;
            if wd.button(x, y, bw, bh, "Paramètres") {
                sh.screen = Screen::Settings;
            }
            y += bh + gap;
            if wd.button(x, y, bw, bh, "Quitter") {
                action = Action::Quit;
            }
            wd.ui.text(4.0 * s, h - 12.0 * s, s, "Epicube Craft 0.9", GREY);
        }
        Screen::Solo => {
            title(&mut wd, "Monde", (h * 0.1).floor(), 3.0 * s);
            let mut y = (h * 0.27).floor();
            if wd.field(x, y, bw, bh, "Nom du monde", &sh.world, sh.field == 1, blink) {
                sh.field = 1;
            }
            y += bh + 16.0 * s;
            if wd.field(x, y, bw, bh, "Graine (chiffres ou texte, vide = au hasard)", &sh.seed_text, sh.field == 2, blink) {
                sh.field = 2;
            }
            y += bh + 6.0 * s;
            // Un monde deja cree garde sa graine : on le dit plutot que de laisser croire le contraire.
            let exists = std::path::Path::new("saves").join(&sh.world).join("monde.bin").exists();
            let hint = if exists { "Ce monde existe : il garde sa graine." } else { "Nouveau monde : la même graine redonne le même monde." };
            wd.ui.text((w - Ui::text_width(hint, s)) / 2.0, y, s, hint, GREY);
            y += 14.0 * s;
            if !exists {
                // Regles du nouveau monde : mode de jeu, difficulte, droit de changer de mode en cours de partie.
                let extreme = sh.new_mode == 4;
                let mode = if extreme { "Extrême (une seule vie)" } else { shared::rules::mode_name(sh.new_mode) };
                if wd.button(x, y, bw, bh, &format!("Mode de jeu : {mode}")) {
                    sh.new_mode = (sh.new_mode + 1) % 5;
                }
                y += bh + gap;
                let half = (bw / 2.0 - 2.0 * s).floor();
                let diff = shared::rules::difficulty_name(if extreme { shared::rules::HARD } else { sh.new_difficulty });
                if wd.button(x, y, half, bh, &format!("Difficulté : {diff}")) && !extreme {
                    sh.new_difficulty = (sh.new_difficulty + 1) % 4;
                }
                let cheats = !extreme && sh.new_cheats;
                if wd.button(x + bw - half, y, half, bh, &format!("Triche : {}", if cheats { "oui" } else { "non" })) && !extreme {
                    sh.new_cheats = !sh.new_cheats;
                }
                y += bh + 4.0 * s;
                let about = match sh.new_mode {
                    0 => "Survie : récolter, fabriquer, manger, se défendre.",
                    1 => "Créatif : tous les objets, vol libre, aucun danger.",
                    2 => "Aventure : comme la survie, sans casser ni poser de blocs.",
                    3 => "Spectateur : on traverse le monde sans y toucher.",
                    _ => "Extrême : survie en difficile ; à la mort, on reste spectateur.",
                };
                wd.ui.text((w - Ui::text_width(about, s)) / 2.0, y, s, about, GREY);
                y += 14.0 * s;
            }
            if wd.button(x, y, bw, bh, if exists { "Reprendre" } else { "Créer et jouer" }) && !sh.world.is_empty() {
                action = Action::Solo;
            }
            y += bh + gap;
            if wd.button(x, y, bw, bh, "Retour") {
                sh.screen = Screen::Title;
            }
            match sh.field {
                1 => type_into(&mut sh.world, inp, 24, name_char),
                2 => type_into(&mut sh.seed_text, inp, 32, seed_char),
                _ => {}
            }
        }
        Screen::Multi => {
            title(&mut wd, "Multijoueur", (h * 0.14).floor(), 3.0 * s);
            let mut y = (h * 0.36).floor();
            if wd.field(x, y, bw, bh, "Adresse du serveur", &sh.settings.address, sh.field == 1, blink) {
                sh.field = 1;
            }
            y += bh + 16.0 * s;
            if wd.field(x, y, bw, bh, "Pseudo", &sh.settings.name, sh.field == 2, blink) {
                sh.field = 2;
            }
            y += bh + 10.0 * s;
            let ready = !sh.settings.address.is_empty() && !sh.settings.name.is_empty();
            if wd.button(x, y, bw, bh, "Rejoindre") && ready {
                action = Action::Join;
            }
            y += bh + gap;
            if wd.button(x, y, bw, bh, "Retour") {
                sh.screen = Screen::Title;
            }
            match sh.field {
                1 => type_into(&mut sh.settings.address, inp, 40, address_char),
                2 => type_into(&mut sh.settings.name, inp, 16, name_char),
                _ => {}
            }
        }
        Screen::Settings | Screen::PauseSettings => {
            title(&mut wd, "Paramètres", (h * 0.06).floor(), 3.0 * s);
            // Deux colonnes.
            let cw = 190.0 * s;
            let (x1, x2) = ((w / 2.0 - cw - 3.0 * s).floor(), (w / 2.0 + 3.0 * s).floor());
            let y0 = (h * 0.06).floor() + 40.0 * s;
            let row = |k: f32| y0 + k * (bh + gap);
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
                wd.ui.text(xm, row(9.0) + 4.0 * s, s, "On entend les joueurs à moins de 48 blocs, d'où ils se trouvent.", GREY);
                if wd.button(x, row(10.0), bw, bh, "Retour") {
                    sh.sound_page = false;
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
                for (k, line) in [
                    "Pour dépasser la fréquence de l'écran : synchro verticale sur non.",
                    "Résolution auto : celle de l'écran sans sa haute densité (le plus rapide).",
                    "Le plein écran est souvent plus rapide que la fenêtre. F3 affiche la cadence.",
                ]
                .into_iter()
                .enumerate()
                {
                    wd.ui.text(x1, row(4.0) + 4.0 * s + k as f32 * 10.0 * s, s, line, GREY);
                }
                if wd.button(x, row(6.0) + 2.0 * s, bw, bh, "Retour") {
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
                wd.toggle(x1, row(4.0), cw, bh, "Solo en réseau local", &mut st.lan);
                if wd.field(x1, row(5.0) + 12.0 * s, cw, bh, "Pseudo", &st.name.clone(), sh.field == 2, blink) {
                    sh.field = 2;
                }
                if sh.field == 2 {
                    type_into(&mut st.name, inp, 16, name_char);
                }
            }
            // Ressources modifiables.
            let ry = row(7.0) + 14.0 * s;
            wd.ui.text(x1, ry - 10.0 * s, s, "Ressources (dossier assets) : textures PNG et modèles Blockbench", GREY);
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
            title(&mut wd, "Pause", (h * 0.2).floor(), 3.0 * s);
            let mut y = (h * 0.42).floor();
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
                let half = (bw / 2.0 - 2.0 * s).floor();
                if wd.button(x, y, half, bh, &format!("Mode : {}", shared::rules::mode_name(g.mode))) {
                    g.cycle_mode();
                }
                let diff = shared::rules::difficulty_name(g.rules.difficulty);
                if wd.button(x + bw - half, y, half, bh, &format!("Difficulté : {diff}")) {
                    g.cycle_difficulty();
                }
                y += bh + gap;
            }
            let leave = if sh.is_local() { "Sauvegarder et quitter" } else { "Se déconnecter" };
            if wd.button(x, y, bw, bh, leave) {
                action = Action::Leave;
            }
            let help = [
                "ZQSD (WASD en QWERTY) bouger - Espace sauter - Maj courir - E inventaire",
                "Clic gauche casser/frapper - clic droit poser/utiliser",
                "Molette ou 1-9 choisir - A jeter - T chat, / commande - V parler",
            ];
            for (i, line) in help.iter().enumerate() {
                let ty = y + bh + 14.0 * s + i as f32 * 10.0 * s;
                wd.ui.text((w - Ui::text_width(line, s)) / 2.0, ty, s, line, GREY);
            }
        }
        Screen::Game => {}
    }
    if let Some(err) = &sh.error {
        let tw = Ui::text_width(err, s);
        wd.ui.rect((w - tw) / 2.0 - 4.0 * s, h - 30.0 * s, tw + 8.0 * s, 14.0 * s, [0.4, 0.0, 0.0, 0.85]);
        wd.ui.text((w - tw) / 2.0, h - 27.0 * s, s, err, WHITE);
    }
    action
}
