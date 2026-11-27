//! Le chat : lignes recues, saisie, historique et suggestions de commandes.
//!
//! T ouvre la saisie, « / » l'ouvre avec la barre oblique deja tapee. Entree envoie, Echap ferme.
//! Tab complete avec la suggestion choisie ; les fleches parcourent les suggestions quand il y en a,
//! sinon les lignes deja envoyees.
use crate::ui::Ui;
use crate::{App, Input};
use shared::commands as cmd;
use shared::proto::{chat, ClientMsg};
use winit::keyboard::KeyCode;

/// Secondes pendant lesquelles une ligne reste a l'ecran quand le chat est ferme.
const SHOWN: f32 = 10.0;
/// Lignes gardees en memoire.
const KEPT: usize = 100;

#[derive(Default)]
pub struct Chat {
    pub open: bool,
    pub text: String,
    /// (ligne, nature, secondes depuis sa reception)
    pub log: Vec<(String, u8, f32)>,
    /// Lignes envoyees, de la plus ancienne a la plus recente, et celle que l'on est en train de revoir.
    sent: Vec<String>,
    recall: Option<usize>,
    /// Suggestion choisie dans la liste.
    pick: usize,
    /// Joueurs connectes, proposes comme arguments.
    pub roster: Vec<String>,
}

impl Chat {
    pub fn push(&mut self, text: String, kind: u8) {
        self.log.push((text, kind, 0.0));
        if self.log.len() > KEPT {
            self.log.remove(0);
        }
    }

    pub fn tick(&mut self, dt: f32) {
        for l in &mut self.log {
            l.2 += dt;
        }
    }

    pub fn show(&mut self, text: &str) {
        self.open = true;
        self.text = text.to_string();
        (self.recall, self.pick) = (None, 0);
    }

    fn close(&mut self) {
        self.open = false;
        self.text.clear();
        (self.recall, self.pick) = (None, 0);
    }

    fn suggestions(&self, cheats: bool) -> Vec<String> {
        cmd::suggest(&self.text, &self.roster, cheats)
    }

    /// Traite le clavier quand la saisie est ouverte. Renvoie la ligne a envoyer, s'il y en a une.
    pub fn keys(&mut self, inp: &Input, cheats: bool) -> Option<String> {
        let mut out = None;
        for k in &inp.pressed {
            let sug = self.suggestions(cheats);
            match k {
                KeyCode::Escape => self.close(),
                KeyCode::Enter | KeyCode::NumpadEnter => {
                    let line = self.text.trim().to_string();
                    if !line.is_empty() {
                        // On ne garde pas deux fois de suite la meme ligne.
                        if self.sent.last() != Some(&line) {
                            self.sent.push(line.clone());
                        }
                        out = Some(line);
                    }
                    self.close();
                }
                KeyCode::Tab if !sug.is_empty() => {
                    self.text = cmd::complete(&self.text, &sug[self.pick.min(sug.len() - 1)]);
                    self.pick = 0;
                }
                KeyCode::ArrowUp | KeyCode::ArrowDown if !sug.is_empty() => {
                    let n = sug.len();
                    self.pick = if *k == KeyCode::ArrowDown { (self.pick + 1) % n } else { (self.pick + n - 1) % n };
                }
                KeyCode::ArrowUp if !self.sent.is_empty() => {
                    let i = self.recall.map_or(self.sent.len() - 1, |i| i.saturating_sub(1));
                    self.recall = Some(i);
                    self.text = self.sent[i].clone();
                }
                KeyCode::ArrowDown => match self.recall {
                    Some(i) if i + 1 < self.sent.len() => {
                        self.recall = Some(i + 1);
                        self.text = self.sent[i + 1].clone();
                    }
                    Some(_) => {
                        self.recall = None;
                        self.text.clear();
                    }
                    None => {}
                },
                _ => {}
            }
        }
        if !self.open {
            return out;
        }
        let before = self.text.len();
        for _ in 0..inp.backspace {
            self.text.pop();
        }
        for ch in inp.text.chars().filter(|c| !c.is_control()) {
            if self.text.chars().count() < cmd::MAX_LEN {
                self.text.push(ch);
            }
        }
        if self.text.len() != before {
            self.pick = 0;
        }
        out
    }
}

/// Coupe un texte en lignes qui tiennent dans `width`, aux espaces de preference.
fn wrap(text: &str, s: f32, width: f32) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in text.split(' ') {
        let cur = lines.last_mut().unwrap();
        let candidate = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if Ui::text_width(&candidate, s) <= width || cur.is_empty() {
            *cur = candidate;
        } else {
            lines.push(format!("  {word}"));
        }
    }
    lines
}

impl App {
    /// Clavier du chat ; a appeler quand la saisie est ouverte.
    pub fn chat_keys(&mut self, inp: &Input) {
        if let Some(line) = self.chat.keys(inp, self.rules.cheats) {
            self.net.send(ClientMsg::Chat { text: line });
        }
    }

    pub fn draw_chat(&self, ui: &mut Ui, s: f32) {
        let c = &self.chat;
        let h = ui.h;
        let (x, width) = (4.0 * s, (ui.w * 0.45).min(330.0 * s));
        let line_h = 10.0 * s;
        // Le bas du chat : juste au-dessus de la barre rapide et de la vie.
        let mut y = h - 50.0 * s;
        if c.open {
            // Ouvert, les lignes se rangent au-dessus de la saisie.
            y = h - 62.0 * s - line_h - 2.0 * s;
        }
        // Lignes, de la plus recente (en bas) a la plus ancienne.
        let limit = if c.open { 14 } else { 8 };
        let mut drawn = 0;
        for (text, kind, age) in c.log.iter().rev() {
            if !c.open && *age > SHOWN {
                break;
            }
            // Fermee, une ligne s'efface pendant sa derniere seconde.
            let a = if c.open { 1.0 } else { (SHOWN - age).clamp(0.0, 1.0) };
            let color = match *kind {
                chat::INFO => [1.0, 0.9, 0.5, a],
                chat::ERROR => [1.0, 0.45, 0.4, a],
                chat::WHISPER => [0.78, 0.74, 1.0, a],
                _ => [1.0, 1.0, 1.0, a],
            };
            for line in wrap(text, s, width - 6.0 * s).iter().rev() {
                if drawn >= limit {
                    break;
                }
                ui.rect(x, y, width, line_h, [0.0, 0.0, 0.02, 0.45 * a]);
                ui.text(x + 3.0 * s, y + 1.0 * s, s, line, color);
                y -= line_h;
                drawn += 1;
            }
            if drawn >= limit {
                break;
            }
        }
        if !c.open {
            return;
        }
        // Saisie : la ligne tapee, son curseur clignotant, et en gris la fin de la suggestion.
        let (ix, iy, iw) = (x, h - 62.0 * s, ui.w - 8.0 * s);
        ui.rect(ix, iy, iw, 12.0 * s, [0.0, 0.0, 0.02, 0.7]);
        let sug = c.suggestions(self.rules.cheats);
        let unknown = c.text.starts_with('/') && c.text.contains(' ') && cmd::find(c.text[1..].split(' ').next().unwrap_or("")).is_none();
        let color = if unknown { [1.0, 0.45, 0.4, 1.0] } else { [1.0, 1.0, 1.0, 1.0] };
        ui.text(ix + 3.0 * s, iy + 2.0 * s, s, &c.text, color);
        let tw = Ui::text_width(&c.text, s);
        let word = c.text.rsplit(' ').next().unwrap_or("").trim_start_matches('/');
        if let Some(choice) = sug.get(c.pick.min(sug.len().saturating_sub(1))) {
            // La suggestion prolonge ce qui est tape quand elle commence pareil.
            if cmd::plain(choice).starts_with(&cmd::plain(word)) {
                let rest: String = choice.chars().skip(word.chars().count()).collect();
                ui.text(ix + 3.0 * s + tw, iy + 2.0 * s, s, &rest, [0.6, 0.62, 0.7, 1.0]);
            }
        }
        if (self.clock * 2.0) as i32 % 2 == 0 {
            ui.rect(ix + 3.0 * s + tw, iy + 9.0 * s, 5.0 * s, s, [1.0, 1.0, 1.0, 1.0]);
        }
        // Liste des suggestions, au-dessus de la saisie, alignee sur le mot en cours.
        let mut top = iy;
        if !sug.is_empty() {
            let shown = 8.min(sug.len());
            // La fenetre de la liste suit la suggestion choisie.
            let first = c.pick.saturating_sub(shown - 1).min(sug.len() - shown);
            let bw = sug.iter().map(|w| Ui::text_width(w, s)).fold(0.0, f32::max) + 8.0 * s;
            let bx = (ix + 3.0 * s + tw - Ui::text_width(word, s) - 4.0 * s).clamp(ix, ui.w - bw - 4.0 * s);
            top = iy - shown as f32 * line_h - 2.0 * s;
            ui.rect(bx - s, top - s, bw + 2.0 * s, shown as f32 * line_h + 4.0 * s, [0.36, 0.44, 0.68, 1.0]);
            ui.rect(bx, top, bw, shown as f32 * line_h + 2.0 * s, [0.03, 0.04, 0.1, 1.0]);
            for (k, w) in sug.iter().enumerate().skip(first).take(shown) {
                let ly = top + s + (k - first) as f32 * line_h;
                let chosen = k == c.pick.min(sug.len() - 1);
                if chosen {
                    ui.rect(bx, ly - s, bw, line_h, [0.24, 0.31, 0.53, 1.0]);
                }
                ui.text(bx + 4.0 * s, ly, s, w, if chosen { [1.0, 0.9, 0.5, 1.0] } else { [0.85, 0.87, 0.95, 1.0] });
            }
            if sug.len() > shown {
                let more = format!("{} / {}", c.pick.min(sug.len() - 1) + 1, sug.len());
                ui.text(bx + bw + 4.0 * s, top + s, s, &more, [0.6, 0.62, 0.7, 1.0]);
            }
        }
        // Mode d'emploi de la commande reconnue, ou rappel des touches.
        let hint = cmd::hint(&c.text).unwrap_or_else(|| {
            if c.text.starts_with('/') { "Tab : compléter - flèches : choisir - Entrée : exécuter".into() } else { "Entrée : envoyer - / pour une commande - Échap : fermer".into() }
        });
        ui.rect(ix, top - line_h - s, Ui::text_width(&hint, s) + 6.0 * s, line_h, [0.0, 0.0, 0.02, 0.6]);
        ui.text(ix + 3.0 * s, top - line_h, s, &hint, if unknown { [1.0, 0.45, 0.4, 1.0] } else { [0.7, 0.72, 0.8, 1.0] });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(c: &mut Chat, keys: &[KeyCode], text: &str) -> Option<String> {
        let mut inp = Input::default();
        inp.pressed = keys.to_vec();
        inp.text = text.to_string();
        c.keys(&inp, true)
    }

    #[test]
    fn typing_completing_and_sending() {
        let mut c = Chat { roster: vec!["Aiko".into()], ..Default::default() };
        c.show("/");
        assert_eq!(press(&mut c, &[], "mo"), None);
        assert_eq!(c.suggestions(true), ["mode", "moi"]);
        // Tab prend la suggestion choisie ; la fleche du bas passe a la suivante.
        press(&mut c, &[KeyCode::ArrowDown], "");
        press(&mut c, &[KeyCode::ArrowUp], "");
        press(&mut c, &[KeyCode::Tab], "");
        assert_eq!(c.text, "/mode ");
        press(&mut c, &[], "cr");
        press(&mut c, &[KeyCode::Tab], "");
        assert_eq!(c.text, "/mode creatif ");
        assert_eq!(c.suggestions(true), ["Aiko"]);
        // Entree envoie et ferme.
        assert_eq!(press(&mut c, &[KeyCode::Enter], ""), Some("/mode creatif".to_string()));
        assert!(!c.open && c.text.is_empty());
        // La fleche du haut rappelle la derniere ligne envoyee ; celle du bas revient a une ligne vide.
        c.show("");
        press(&mut c, &[KeyCode::ArrowUp], "");
        assert_eq!(c.text, "/mode creatif");
        press(&mut c, &[KeyCode::ArrowDown], "");
        assert!(c.text.is_empty());
        // Echap ferme sans rien envoyer ; une ligne vide ne s'envoie pas.
        press(&mut c, &[], "salut");
        assert_eq!(press(&mut c, &[KeyCode::Escape], ""), None);
        assert!(!c.open);
        c.show("");
        assert_eq!(press(&mut c, &[KeyCode::Enter], ""), None);
        // Retour arriere et longueur maximale.
        c.show("abc");
        let mut inp = Input::default();
        inp.backspace = 2;
        c.keys(&inp, true);
        assert_eq!(c.text, "a");
        press(&mut c, &[], &"x".repeat(500));
        assert_eq!(c.text.chars().count(), cmd::MAX_LEN);
    }

    #[test]
    fn log_is_bounded_and_wraps() {
        let mut c = Chat::default();
        for i in 0..150 {
            c.push(format!("ligne {i}"), chat::SAY);
        }
        assert_eq!(c.log.len(), KEPT);
        assert_eq!(c.log.last().unwrap().0, "ligne 149");
        c.tick(3.0);
        assert!(c.log.iter().all(|l| l.2 == 3.0));
        // Le retour a la ligne ne perd aucun mot (la largeur des lettres depend de la police, absente ici).
        let text = "un message assez long pour devoir passer a la ligne plusieurs fois de suite";
        let joined: Vec<String> = wrap(text, 1.0, 120.0).iter().flat_map(|l| l.split_whitespace().map(str::to_string).collect::<Vec<_>>()).collect();
        assert_eq!(joined.join(" "), text);
    }
}
