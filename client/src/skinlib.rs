//! La bibliotheque de skins : ce que les joueurs ont publie, a feuilleter et a porter. Elle vit sur
//! Epicube Hub ; les echanges se font sur un fil a part pour que l'ecran ne se fige jamais.
use crate::friends::{agent, signed};
use crate::skin;
use crate::ui::Ui;
use glam::Mat3;
use std::sync::mpsc::{self, Receiver, Sender};

/// Skins par page : trois rangees de quatre.
pub const PER_PAGE: usize = 12;
const COLS: usize = 4;
const ROWS: usize = 3;

#[derive(Clone)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub author: String,
    owner: String,
    pub px: Vec<u8>,
}

enum Reply {
    Page(Result<(Vec<Entry>, usize), String>),
    Me(String),
    Done(Result<String, String>),
}

/// Ce que la bibliotheque demande a qui l'affiche.
pub enum Ask {
    Nothing,
    /// Porter ce skin : il devient celui que l'on dessine.
    Wear(Vec<u8>),
    Close,
}

pub struct Shelf {
    /// None : la page n'est pas encore arrivee.
    pub list: Option<Result<Vec<Entry>, String>>,
    pages: usize,
    page: usize,
    pub query: String,
    /// Nom sous lequel on publiera son skin.
    pub name: String,
    mine_only: bool,
    picked: Option<usize>,
    /// Identifiant du compte du joueur, pour reconnaitre ses skins.
    me: Option<String>,
    token: Option<String>,
    pub note: String,
    /// Skins a ecrire dans l'atlas : (case, pixels).
    pub fresh: Vec<(usize, Vec<u8>)>,
    asked: String,
    rx: Receiver<Reply>,
    tx: Sender<Reply>,
}

fn base() -> String {
    format!("{}/api/skins", server::party::hub())
}

fn problem(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(_, r) => r.into_json::<serde_json::Value>().ok().and_then(|v| v["error"].as_str().map(str::to_string)).unwrap_or_else(|| "la bibliothèque a refusé".into()),
        ureq::Error::Transport(_) => "bibliothèque injoignable".to_string(),
    }
}

/// Garde les lettres, chiffres et quelques signes : de quoi ecrire un nom dans une adresse.
fn query(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn fetch(q: &str, page: usize, mine: Option<&str>) -> Result<(Vec<Entry>, usize), String> {
    use std::io::Read;
    let mut url = format!("{}?q={}&page={page}&per={PER_PAGE}", base(), query(q));
    if let Some(id) = mine {
        url += &format!("&mine={id}");
    }
    let v: serde_json::Value = agent().get(&url).call().map_err(problem)?.into_json().map_err(|_| "réponse illisible".to_string())?;
    let text = |e: &serde_json::Value, k: &str| e[k].as_str().unwrap_or_default().to_string();
    let mut out = Vec::new();
    for e in v["skins"].as_array().cloned().unwrap_or_default().iter().take(PER_PAGE) {
        let id = text(e, "id");
        let mut bytes = Vec::new();
        let Ok(res) = agent().get(&format!("{}/{id}/png", base())).call() else { continue };
        if res.into_reader().take(64 << 10).read_to_end(&mut bytes).is_err() {
            continue;
        }
        // Une image qui n'est pas un skin est passee sous silence.
        if let Ok(px) = skin::decode(&bytes) {
            out.push(Entry { id, name: text(e, "name"), author: text(e, "author"), owner: text(e, "owner"), px });
        }
    }
    Ok((out, v["pages"].as_u64().unwrap_or(1).max(1) as usize))
}

impl Shelf {
    /// `token` : le jeton du compte du joueur, s'il a lance le jeu depuis Epicube Hub.
    pub fn open(token: Option<String>) -> Shelf {
        let (tx, rx) = mpsc::channel();
        let mut shelf = Shelf { list: None, pages: 1, page: 0, query: String::new(), name: String::new(), mine_only: false, picked: None, me: None, token, note: String::new(), fresh: Vec::new(), asked: String::new(), rx, tx };
        if let Some(t) = shelf.token.clone() {
            let tx = shelf.tx.clone();
            std::thread::spawn(move || {
                let me = signed(agent().get(&format!("{}/me", base())), &Some(t)).call().ok().and_then(|r| r.into_json::<serde_json::Value>().ok()).and_then(|v| v["id"].as_str().map(str::to_string));
                if let Some(id) = me {
                    let _ = tx.send(Reply::Me(id));
                }
            });
        }
        shelf.refresh();
        shelf
    }

    fn refresh(&mut self) {
        self.list = None;
        self.picked = None;
        self.asked = self.query.clone();
        let (tx, q, page) = (self.tx.clone(), self.query.clone(), self.page);
        let mine = if self.mine_only { self.me.clone() } else { None };
        std::thread::spawn(move || {
            let _ = tx.send(Reply::Page(fetch(&q, page, mine.as_deref())));
        });
    }

    /// Prend les reponses arrivees depuis la derniere image.
    pub fn poll(&mut self) {
        while let Ok(reply) = self.rx.try_recv() {
            match reply {
                Reply::Me(id) => self.me = Some(id),
                Reply::Page(Ok((list, pages))) => {
                    self.pages = pages;
                    self.page = self.page.min(pages - 1);
                    // Chaque skin de la page prend sa case dans l'atlas.
                    self.fresh = list.iter().enumerate().map(|(k, e)| (skin::FIRST_SHELF + k, e.px.clone())).collect();
                    self.list = Some(Ok(list));
                }
                Reply::Page(Err(e)) => self.list = Some(Err(e)),
                Reply::Done(Ok(text)) => {
                    self.note = text;
                    self.refresh();
                }
                Reply::Done(Err(e)) => self.note = e,
            }
        }
        // La recherche part d'elle-meme quand on s'arrete de taper.
        if self.list.is_some() && self.asked != self.query {
            self.page = 0;
            self.refresh();
        }
    }

    fn publish(&mut self, px: &[u8]) {
        let name = self.name.trim().to_string();
        if self.token.is_none() {
            self.note = "Lancez le jeu depuis Epicube Hub, connecté à votre compte, pour publier.".into();
            return;
        }
        if name.is_empty() {
            self.note = "Donnez un nom à votre skin.".into();
            return;
        }
        let (tx, token, png) = (self.tx.clone(), self.token.clone(), skin::encode(px));
        self.note = "Publication...".into();
        std::thread::spawn(move || {
            let res = signed(agent().post(&format!("{}?name={}", base(), query(&name))), &token).set("Content-Type", "image/png").send_bytes(&png);
            let _ = tx.send(Reply::Done(res.map(|_| format!("« {name} » est publié.")).map_err(problem)));
        });
    }

    fn remove(&mut self, id: String) {
        let (tx, token) = (self.tx.clone(), self.token.clone());
        std::thread::spawn(move || {
            let res = signed(agent().delete(&format!("{}/{id}", base())), &token).call();
            let _ = tx.send(Reply::Done(res.map(|_| "Skin retiré de la bibliothèque.".to_string()).map_err(problem)));
        });
    }

    /// Dessine la bibliotheque dans le cadre (x, y, w, h). `mine` : le skin du joueur, pour le publier.
    /// `field` : le champ de texte qui a la main (1 la recherche, 2 le nom) ; il change au clic.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, ui: &mut Ui, mine: &[u8], mouse: (f32, f32), click: bool, x: f32, y: f32, w: f32, h: f32, s: f32, t: f32, field: &mut u8) -> Ask {
        let mut ask = Ask::Nothing;
        let bh = 14.0 * s;
        let hit = |bx: f32, by: f32, bw: f32, bhh: f32| mouse.0 >= bx && mouse.0 < bx + bw && mouse.1 >= by && mouse.1 < by + bhh;
        let blink = (t * 2.0) as i32 % 2 == 0;
        let button = |ui: &mut Ui, bx: f32, by: f32, bw: f32, text: &str, on: bool| -> bool {
            let hot = hit(bx, by, bw, bh);
            let fill = match (on, hot) {
                (true, _) => [0.85, 0.65, 0.16, 1.0],
                (false, true) => [0.34, 0.34, 0.44, 1.0],
                (false, false) => [0.17, 0.17, 0.23, 1.0],
            };
            ui.bevel(bx, by, bw, bh, s, fill, [0.4, 0.4, 0.5, 1.0], [0.06, 0.06, 0.09, 1.0]);
            ui.text((bx + (bw - Ui::text_width(text, s)) / 2.0).floor(), by + 3.0 * s, s, text, [1.0; 4]);
            hot && click
        };
        let input = |ui: &mut Ui, bx: f32, bw: f32, by: f32, value: &str, hint: &str, focused: bool| -> bool {
            ui.bevel(bx, by, bw, bh, s, [0.05, 0.05, 0.08, 1.0], [0.03, 0.03, 0.05, 1.0], [0.36, 0.37, 0.44, 1.0]);
            if value.is_empty() && !focused {
                ui.text(bx + 4.0 * s, by + 3.0 * s, s, hint, [0.55, 0.56, 0.62, 1.0]);
            } else {
                let cursor = if focused && blink { "_" } else { "" };
                ui.text(bx + 4.0 * s, by + 3.0 * s, s, &format!("{value}{cursor}"), [1.0; 4]);
            }
            hit(bx, by, bw, bh) && click
        };

        // ---- En haut : la recherche, « mes skins », les pages.
        if input(ui, x, 150.0 * s, y, &self.query, "Rechercher un skin ou un joueur", *field == 1) {
            *field = 1;
        }
        if self.me.is_some() && button(ui, x + 154.0 * s, y, 64.0 * s, "Mes skins", self.mine_only) {
            self.mine_only = !self.mine_only;
            self.page = 0;
            self.refresh();
        }
        let pages = format!("{} / {}", self.page + 1, self.pages);
        ui.text(x + w - 56.0 * s - Ui::text_width(&pages, s) / 2.0, y + 3.0 * s, s, &pages, [0.7, 0.72, 0.8, 1.0]);
        if button(ui, x + w - 98.0 * s, y, 16.0 * s, "<", false) && self.page > 0 {
            self.page -= 1;
            self.refresh();
        }
        if button(ui, x + w - 16.0 * s, y, 16.0 * s, ">", false) && self.page + 1 < self.pages {
            self.page += 1;
            self.refresh();
        }

        // ---- Les skins de la page : trois rangees de quatre personnages.
        let (gy, gh) = (y + bh + 5.0 * s, h - 2.0 * (bh + 5.0 * s) - 26.0 * s);
        let (cw, ch) = ((w / COLS as f32).floor(), (gh / ROWS as f32).floor());
        match &self.list {
            None => ui.text(x + 4.0 * s, gy + 8.0 * s, s, "Chargement de la bibliothèque...", [0.7, 0.72, 0.8, 1.0]),
            Some(Err(e)) => ui.text(x + 4.0 * s, gy + 8.0 * s, s, &format!("Bibliothèque indisponible : {e}."), [1.0, 0.6, 0.6, 1.0]),
            Some(Ok(list)) if list.is_empty() => ui.text(x + 4.0 * s, gy + 8.0 * s, s, "Aucun skin ici pour le moment : publiez le vôtre !", [0.7, 0.72, 0.8, 1.0]),
            Some(Ok(list)) => {
                let view = Mat3::from_rotation_x(0.12) * Mat3::from_rotation_y(0.45 + (t * 0.5).sin() * 0.25);
                for (k, e) in list.iter().enumerate() {
                    let (bx, by) = (x + (k % COLS) as f32 * cw, gy + (k / COLS) as f32 * ch);
                    let hot = hit(bx, by, cw - 2.0 * s, ch - 2.0 * s);
                    let fill = if self.picked == Some(k) { [0.3, 0.24, 0.1, 1.0] } else if hot { [0.16, 0.16, 0.22, 1.0] } else { [0.09, 0.09, 0.13, 1.0] };
                    ui.bevel(bx, by, cw - 2.0 * s, ch - 2.0 * s, s, fill, [0.06, 0.06, 0.09, 1.0], [0.3, 0.3, 0.38, 1.0]);
                    let scale = (ch - 12.0 * s) / 2.2;
                    skin::doll(ui, &skin::model(skin::FIRST_SHELF + k), view, 0.0, bx + scale * 0.75, by + ch - 6.0 * s, scale);
                    // Le nom du skin et de son auteur, coupes s'ils depassent.
                    let room = cw - scale * 1.5 - 8.0 * s;
                    let fit = |text: &str| -> String {
                        let mut out = text.to_string();
                        while Ui::text_width(&out, s) > room && out.chars().count() > 1 {
                            out.pop();
                        }
                        out
                    };
                    ui.text(bx + cw - room - 4.0 * s, by + 5.0 * s, s, &fit(&e.name), [1.0; 4]);
                    ui.text(bx + cw - room - 4.0 * s, by + 15.0 * s, s, &fit(&e.author), [0.6, 0.62, 0.7, 1.0]);
                    if hot && click {
                        self.picked = Some(k);
                    }
                }
            }
        }

        // ---- En bas : porter le skin choisi, retirer le sien, publier celui que l'on porte, revenir.
        let by = y + h - 2.0 * (bh + 3.0 * s) - 10.0 * s;
        let chosen = self.picked.and_then(|k| self.list.as_ref().and_then(|l| l.as_ref().ok()).and_then(|l| l.get(k)).cloned());
        if let Some(e) = &chosen {
            if button(ui, x, by, 92.0 * s, "Porter ce skin", false) {
                self.note = format!("« {} » est sur votre personnage : enregistrez dans l'éditeur pour le garder.", e.name);
                ask = Ask::Wear(e.px.clone());
            }
            if self.me.as_deref() == Some(e.owner.as_str()) && button(ui, x + 96.0 * s, by, 60.0 * s, "Retirer", false) {
                self.remove(e.id.clone());
            }
        }
        let py = by + bh + 3.0 * s;
        if input(ui, x, 150.0 * s, py, &self.name, "Nom de votre skin", *field == 2) {
            *field = 2;
        }
        if button(ui, x + 154.0 * s, py, 96.0 * s, "Publier mon skin", false) {
            self.publish(mine);
        }
        if button(ui, x + w - 70.0 * s, py, 70.0 * s, "Retour", false) {
            ask = Ask::Close;
        }
        ui.text(x, py + bh + 4.0 * s, s, &self.note, [0.6, 1.0, 0.6, 1.0]);
        ask
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_safe_in_an_address() {
        assert_eq!(query("Mon skin #1"), "Mon%20skin%20%231");
        assert_eq!(query("Samouraï"), "Samoura%C3%AF");
        assert_eq!(query("a-b_c.d"), "a-b_c.d");
    }
}
