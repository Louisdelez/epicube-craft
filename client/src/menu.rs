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
    Quit,
    /// Quitter la partie et revenir a l'ecran-titre.
    Leave,
    Export,
    Reload,
    /// Heberger la partie preparee a l'ecran « Heberger ».
    Host,
    /// Rejoindre la partie a cette adresse.
    JoinAt(String),
    /// Jouer a ce mode : rejoindre une partie qui a de la place, ou en heberger une.
    Play(String),
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
            let sub = "Un monde de blocs à explorer";
            wd.ui.text((w - Ui::text_width(sub, s)) / 2.0, (h * 0.16).floor() + 48.0 * s, s, sub, GREY);
            let mut y = (h * 0.44).floor();
            if wd.button(x, y, bw, bh, "Jouer en solo") {
                sh.screen = Screen::Solo;
                sh.field = 1;
            }
            y += bh + gap;
            if wd.button(x, y, bw, bh, "Multijoueur") {
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
            wd.ui.text(4.0 * s, h - 12.0 * s, s, "Epicube Craft 0.12", GREY);
        }
        Screen::Solo => {
            title(&mut wd, "Monde", (h * 0.06).floor(), 3.0 * s);
            let mut y = (h * 0.22).floor();
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
                let diff = shared::rules::difficulty_name(if extreme { shared::rules::HARD } else { sh.new_difficulty });
                if wd.button(x, y, bw, bh, &format!("Difficulté : {diff}")) && !extreme {
                    sh.new_difficulty = (sh.new_difficulty + 1) % 4;
                }
                y += bh + gap;
                let cheats = !extreme && sh.new_cheats;
                if wd.button(x, y, bw, bh, &format!("Triche : {}", if cheats { "oui" } else { "non" })) && !extreme {
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
            title(&mut wd, "Multijoueur", (h * 0.03).floor(), 3.0 * s);
            let Some(b) = sh.browser.as_mut() else { return action };
            b.poll();
            // Une barre laterale a gauche (recherche, modes de jeu, hebergement), les parties a droite.
            let (sw, mw, top) = (150.0 * s, 440.0 * s, (h * 0.03).floor() + 40.0 * s);
            let sx = ((w - sw - mw - 8.0 * s) / 2.0).floor();
            let mx = sx + sw + 8.0 * s;
            let bottom = h - 22.0 * s;
            wd.ui.bevel(sx, top, sw, bottom - top, s, [0.07, 0.07, 0.1, 0.9], [0.04, 0.04, 0.06, 1.0], [0.3, 0.31, 0.38, 1.0]);
            wd.ui.bevel(mx, top, mw, bottom - top, s, [0.07, 0.07, 0.1, 0.9], [0.04, 0.04, 0.06, 1.0], [0.3, 0.31, 0.38, 1.0]);
            let (ix, iw) = (sx + 6.0 * s, sw - 12.0 * s);
            let mut y = top + 16.0 * s;
            if wd.field(ix, y, iw, bh, "Rechercher", &b.query, sh.field == 1, blink) {
                sh.field = 1;
            }
            y += bh + 8.0 * s;
            // Modes de jeu : ceux que l'on peut heberger, et ceux des parties en ligne.
            let online = b.categories();
            let mut chips: Vec<(String, String, usize)> = vec![(String::new(), "Toutes les parties".to_string(), online.iter().map(|c| c.2).sum())];
            for m in &sh.modes {
                chips.push((m.id.clone(), m.name.clone(), online.iter().find(|c| c.0 == m.id).map_or(0, |c| c.2)));
            }
            for c in &online {
                if !chips.iter().any(|k| k.0 == c.0) {
                    chips.push(c.clone());
                }
            }
            let room = ((bottom - 78.0 * s - y) / (bh + 2.0 * s)).floor().max(1.0) as usize;
            for (id, name, n) in chips.iter().take(room) {
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
                    b.show_friends = false;
                }
                y += bh + 2.0 * s;
            }
            let amis = match (b.show_friends, b.waiting()) {
                (true, _) => "Les parties".to_string(),
                (false, 0) => "Amis".to_string(),
                (false, n) => format!("Amis ({n})"),
            };
            if wd.button(ix, bottom - 74.0 * s, iw, bh, &amis) {
                b.toggle_friends();
                b.chat_with = None;
            }
            if wd.button(ix, bottom - 50.0 * s, iw, bh, "Héberger une partie") {
                sh.screen = Screen::Host;
                sh.field = 1;
                return action;
            }
            if wd.button(ix, bottom - 26.0 * s, iw, bh, "Retour") {
                sh.screen = Screen::Title;
                sh.browser = None;
                return action;
            }
            let (lx, lw) = (mx + 6.0 * s, mw - 12.0 * s);
            let head = top + 6.0 * s;
            if b.show_friends {
                // Rubrique Amis : demander quelqu'un par son pseudo, repondre aux demandes, retirer un ami.
                wd.ui.text(lx, head + 6.0 * s, s, "Amis", [1.0, 0.86, 0.36, 1.0]);
                if !b.has_account() {
                    let text = "Lancez le jeu depuis Epicube Hub, connecté à votre compte, pour avoir des amis.";
                    wd.ui.text((mx + (mw - Ui::text_width(text, s)) / 2.0).floor(), head + 60.0 * s, s, text, GREY);
                    return action;
                }
                let enter = inp.pressed.contains(&winit::keyboard::KeyCode::Enter) || inp.pressed.contains(&winit::keyboard::KeyCode::NumpadEnter);
                if let Some(buddy) = b.chat_with.clone() {
                    // Conversation avec un ami : les messages, et la saisie en bas.
                    let status = match &b.friends {
                        Some(Ok(f)) => f.friends.iter().find(|x| x.id == buddy.id).map_or(buddy.status.clone(), |x| x.status.clone()),
                        _ => buddy.status.clone(),
                    };
                    if wd.button(lx + lw - 70.0 * s, head, 70.0 * s, bh, "Retour") {
                        b.close_chat();
                        return action;
                    }
                    wd.ui.text(lx + 34.0 * s, head + 6.0 * s, s, &format!("{} - {status}", buddy.name), WHITE);
                    let iy = bottom - 26.0 * s;
                    wd.field(lx, iy, lw - 86.0 * s, bh, "", &b.chat_input, true, blink);
                    if wd.button(lx + lw - 80.0 * s, iy, 80.0 * s, bh, "Envoyer") || enter {
                        b.send_chat();
                    }
                    type_into(&mut b.chat_input, inp, 200, seed_char);
                    // Les derniers messages, du bas vers le haut ; les notres a droite.
                    let mut y = iy - 8.0 * s;
                    let top_limit = head + bh + 8.0 * s;
                    if b.chat.is_empty() {
                        wd.ui.text(lx, top_limit, s, "Aucun message pour le moment.", GREY);
                    }
                    'messages: for m in b.chat.iter().rev() {
                        let lines = crate::chatbox::wrap(&m.body, s, lw * 0.7);
                        let width = lines.iter().map(|l| Ui::text_width(l, s)).fold(0.0, f32::max) + 10.0 * s;
                        let height = lines.len() as f32 * 10.0 * s + 6.0 * s;
                        y -= height + 3.0 * s;
                        if y < top_limit {
                            break 'messages;
                        }
                        let x0 = if m.mine { lx + lw - width } else { lx };
                        wd.ui.rect(x0, y, width, height, if m.mine { [0.72, 0.2, 0.165, 1.0] } else { [0.18, 0.18, 0.24, 1.0] });
                        for (k, line) in lines.iter().enumerate() {
                            wd.ui.text(x0 + 5.0 * s, y + 3.0 * s + k as f32 * 10.0 * s, s, line, WHITE);
                        }
                    }
                    if let Some(note) = &b.note {
                        wd.ui.text(((w - Ui::text_width(note, s)) / 2.0).floor(), bottom + 6.0 * s, s, note, [1.0, 0.86, 0.36, 1.0]);
                    }
                    return action;
                }
                let fy = head + bh + 18.0 * s;
                wd.field(lx, fy, 200.0 * s, bh, "Ajouter un ami : son pseudo", &b.friend_name, true, blink);
                if wd.button(lx + 206.0 * s, fy, 80.0 * s, bh, "Demander") || enter {
                    b.ask_friend();
                }
                type_into(&mut b.friend_name, inp, 16, name_char);
                let mut y = fy + bh + 8.0 * s;
                let friends = match &b.friends {
                    None => {
                        wd.ui.text(lx, y, s, "Chargement...", GREY);
                        None
                    }
                    Some(Err(e)) => {
                        wd.ui.text(lx, y, s, e, GREY);
                        None
                    }
                    Some(Ok(f)) => Some(f.clone()),
                };
                if let Some(f) = friends {
                    // (titre, comptes, bouton qui accepte, bouton qui retire)
                    let groups: [(&str, &Vec<crate::browser::Buddy>, Option<&str>, &str); 3] =
                        [("Demandes reçues", &f.received, Some("Accepter"), "Refuser"), ("Mes amis", &f.friends, None, "Retirer"), ("Demandes envoyées", &f.sent, None, "Annuler")];
                    for (name, list, yes, no) in groups {
                        if list.is_empty() && name != "Mes amis" {
                            continue;
                        }
                        wd.ui.text(lx, y, s, &format!("{name} ({})", list.len()), [1.0, 0.86, 0.36, 1.0]);
                        y += 12.0 * s;
                        if list.is_empty() {
                            wd.ui.text(lx + 6.0 * s, y + 2.0 * s, s, "Personne pour le moment.", GREY);
                            y += 14.0 * s;
                        }
                        for buddy in list {
                            if y + bh > bottom - 6.0 * s {
                                break;
                            }
                            wd.ui.rect(lx, y, lw, bh, [0.12, 0.12, 0.16, 1.0]);
                            if name == "Mes amis" {
                                // Pastille (pleine : en ligne), pseudo, ce qu'il fait, messages non lus.
                                let dot = if buddy.online { [0.4, 0.9, 0.5, 1.0] } else { [0.35, 0.36, 0.42, 1.0] };
                                wd.ui.rect(lx + 6.0 * s, y + 8.0 * s, 4.0 * s, 4.0 * s, dot);
                                wd.ui.text(lx + 16.0 * s, y + 6.0 * s, s, &buddy.name, WHITE);
                                let nw = Ui::text_width(&buddy.name, s);
                                wd.ui.text(lx + 24.0 * s + nw, y + 6.0 * s, s, &buddy.status, GREY);
                                let write = if buddy.unread > 0 { format!("Écrire ({})", buddy.unread) } else { "Écrire".to_string() };
                                if wd.button(lx + lw - 148.0 * s, y, 70.0 * s, bh, &write) {
                                    b.open_chat(buddy);
                                }
                            } else {
                                wd.ui.text(lx + 6.0 * s, y + 6.0 * s, s, &buddy.name, WHITE);
                            }
                            if let Some(yes) = yes {
                                if wd.button(lx + lw - 148.0 * s, y, 70.0 * s, bh, yes) {
                                    b.accept_friend(buddy);
                                }
                            }
                            if wd.button(lx + lw - 74.0 * s, y, 70.0 * s, bh, no) {
                                b.remove_friend(buddy);
                            }
                            y += bh + 2.0 * s;
                        }
                        y += 6.0 * s;
                    }
                }
                if let Some(note) = &b.note {
                    wd.ui.text(((w - Ui::text_width(note, s)) / 2.0).floor(), bottom + 6.0 * s, s, note, [1.0, 0.86, 0.36, 1.0]);
                }
                return action;
            }
            // Les parties : nom, mode, etiquettes, joueurs.
            let rows = b.shown();
            let picked_name = chips.iter().find(|c| c.0 == b.mode).map_or(String::new(), |c| c.1.clone());
            let heading = if b.mode.is_empty() { format!("{} partie{} en ligne", rows.len(), if rows.len() > 1 { "s" } else { "" }) } else { format!("{picked_name} : {} partie{}", rows.len(), if rows.len() > 1 { "s" } else { "" }) };
            wd.ui.text(lx, head + 6.0 * s, s, &heading, [1.0, 0.86, 0.36, 1.0]);
            // Un mode retenu : on y joue d'un clic, dans une partie existante ou dans la sienne.
            if !b.mode.is_empty() && wd.button(lx + lw - 130.0 * s, head, 130.0 * s, bh, &format!("Jouer : {picked_name}")) {
                action = Action::Play(b.mode.clone());
            }
            let (y3, rh) = (head + bh + 6.0 * s, 26.0 * s);
            let per_page = (((bottom - 28.0 * s - y3) / (rh + 2.0 * s)).floor().max(1.0)) as usize;
            let pages = rows.len().div_ceil(per_page).max(1);
            b.page = b.page.min(pages - 1);
            let state = match &b.list {
                None => Some("Recherche des parties en ligne...".to_string()),
                Some(Err(e)) => Some(format!("Parties indisponibles : {e}.")),
                Some(Ok(_)) if rows.is_empty() => Some(if b.mode.is_empty() { "Aucune partie pour le moment. Hébergez la vôtre !".to_string() } else { format!("Aucune partie. « Jouer : {picked_name} » en lance une.") }),
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
                let nw = Ui::text_width(&row.name, s);
                wd.ui.text(nx + 8.0 * s + nw, y + 4.0 * s, s, &row.mode_name, [1.0, 0.86, 0.36, 1.0]);
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
            if let Some(note) = &b.note {
                wd.ui.text(((w - Ui::text_width(note, s)) / 2.0).floor(), bottom + 6.0 * s, s, note, [1.0, 0.86, 0.36, 1.0]);
            }
            let before = b.query.clone();
            type_into(&mut b.query, inp, 24, seed_char);
            if b.query != before {
                b.page = 0;
            }
            sh.field = 1;
        }
        Screen::Host => {
            title(&mut wd, "Héberger une partie", (h * 0.05).floor(), 3.0 * s);
            let form = &mut sh.host;
            let Some(mode) = sh.modes.get(form.pick).cloned() else {
                sh.screen = Screen::Multi;
                return action;
            };
            let mut y = (h * 0.05).floor() + 44.0 * s;
            if wd.button(x, y, bw, bh, &format!("Mode de jeu : {}", mode.name)) {
                form.pick = (form.pick + 1) % sh.modes.len().max(1);
                form.places = 0;
            }
            y += bh + 4.0 * s;
            for line in crate::chatbox::wrap(&mode.about, s, 300.0 * s).iter().take(2) {
                wd.ui.text(((w - Ui::text_width(line, s)) / 2.0).floor(), y, s, line, GREY);
                y += 10.0 * s;
            }
            y += 22.0 * s;
            if wd.field(x, y, bw, bh, "Nom de la partie (vide : « Partie de ... »)", &form.name, sh.field == 1, blink) {
                sh.field = 1;
            }
            y += bh + gap;
            let who = if form.public { "Partie publique : tout le monde peut la rejoindre" } else { "Partie privée : réservée à vos amis" };
            if wd.button(x - 30.0 * s, y, bw + 60.0 * s, bh, who) {
                form.public = !form.public;
            }
            y += bh + gap;
            let places = if form.places == 0 { mode.players } else { form.places };
            if wd.button(x, y, bw, bh, &format!("Places : {places}")) {
                let steps = [2u16, 4, 8, 12, 16, 24, 32, 50, 100];
                form.places = steps.iter().copied().find(|p| *p > places).unwrap_or(steps[0]);
            }
            y += bh + gap;
            if mode.keep_world {
                y += 12.0 * s;
                if wd.field(x, y, bw, bh, "Monde (gardé d'une partie à l'autre)", &form.world, sh.field == 2, blink) {
                    sh.field = 2;
                }
                y += bh + gap;
            } else {
                let note = "Chaque partie de ce mode commence dans un monde neuf.";
                wd.ui.text(((w - Ui::text_width(note, s)) / 2.0).floor(), y + 2.0 * s, s, note, GREY);
                y += 14.0 * s;
            }
            y += 6.0 * s;
            if wd.button(x, y, bw, bh, "Lancer la partie") {
                action = Action::Host;
            }
            y += bh + gap;
            if wd.button(x, y, bw, bh, "Retour") {
                sh.screen = Screen::Multi;
                sh.field = 1;
            }
            let note = "La partie tourne dans votre jeu. Rien à régler : ni port, ni box.";
            wd.ui.text(((w - Ui::text_width(note, s)) / 2.0).floor(), y + bh + 6.0 * s, s, note, GREY);
            match sh.field {
                1 => type_into(&mut form.name, inp, 32, seed_char),
                2 if mode.keep_world => type_into(&mut form.world, inp, 24, name_char),
                _ => {}
            }
        }
        Screen::Settings | Screen::PauseSettings => {
            title(&mut wd, if sh.keys_page { "Contrôles" } else { "Paramètres" }, (h * 0.06).floor(), 3.0 * s);
            // Deux colonnes.
            let cw = 190.0 * s;
            let (x1, x2) = ((w / 2.0 - cw - 3.0 * s).floor(), (w / 2.0 + 3.0 * s).floor());
            let y0 = (h * 0.06).floor() + 40.0 * s;
            let row = |k: f32| y0 + k * (bh + gap);
            if let Some(lib) = sh.library.as_mut() {
                // Page des mods : la bibliotheque d'Epicube Hub et ce qui est installe.
                lib.poll();
                let lw = 420.0 * s;
                let lx = ((w - lw) / 2.0).floor();
                let state = match &lib.list {
                    None => "Chargement de la bibliothèque...".to_string(),
                    Some(Err(e)) => format!("Bibliothèque indisponible : {e}."),
                    Some(Ok(l)) if l.is_empty() && lib.installed.is_empty() => "Aucun mod dans la bibliothèque pour le moment.".to_string(),
                    Some(Ok(_)) => "Les mods installés s'appliquent à vos parties solo.".to_string(),
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
                wd.ui.text(xm, row(9.0) + 4.0 * s, s, "On entend les joueurs à moins de 48 blocs, d'où ils se trouvent.", GREY);
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
                let note = if sh.rebinding.is_some() { "Appuyez sur la nouvelle touche (Échap : annuler)." } else { "Cliquez sur une touche pour la changer. Casser et frapper : clic gauche ; poser et utiliser : clic droit." };
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
                for (k, line) in [
                    "Pour dépasser la fréquence de l'écran : synchro verticale sur non.",
                    "Résolution : celle de la scène 3D. Le texte reste net dans tous les cas.",
                    "Interface basse résolution (Mac) : bien plus rapide, pour le même texte.",
                ]
                .into_iter()
                .enumerate()
                {
                    wd.ui.text(x1, row(lines_at) + 4.0 * s + k as f32 * 10.0 * s, s, line, GREY);
                }
                if wd.button(x, row(lines_at + 2.0) + 2.0 * s, bw, bh, "Retour") {
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
            if !in_game && wd.button(x2, row(6.0) + 12.0 * s, cw, bh, "Mods") {
                sh.library = Some(crate::library::Library::open());
            }
            // Ressources modifiables.
            let ry = row(8.0) + 14.0 * s;
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
                wd.ui.rect(px, py, pw, 30.0 * s, [0.0, 0.0, 0.02, 0.7]);
                for (k, line) in lines.iter().enumerate() {
                    wd.ui.text(px + 6.0 * s, py + 6.0 * s + k as f32 * 11.0 * s, s, line, if k == 0 { [1.0, 0.86, 0.36, 1.0] } else { WHITE });
                }
            }
            // L'aide-memoire suit les touches choisies par le joueur.
            use crate::controls::Act;
            let k = |a: Act| sh.settings.controls.shown_for(a);
            let help = [
                format!("{} {} {} {} bouger - {} sauter - {} courir - {} inventaire", k(Act::Forward), k(Act::Left), k(Act::Back), k(Act::Right), k(Act::Jump), k(Act::Sprint), k(Act::Inventory)),
                "Clic gauche casser/frapper - clic droit poser/utiliser".to_string(),
                format!("Molette ou 1-9 choisir - {} jeter - {} chat, / commande - {} parler", k(Act::Drop), k(Act::Chat), k(Act::Voice)),
            ];
            for (i, line) in help.iter().enumerate() {
                let ty = y + bh + 14.0 * s + i as f32 * 10.0 * s;
                wd.ui.text((w - Ui::text_width(line, s)) / 2.0, ty, s, line, GREY);
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
