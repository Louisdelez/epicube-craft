//! L'annuaire des parties en ligne, vu du jeu : toutes les parties publiques, quelle que soit la
//! machine qui les heberge. Les echanges avec Epicube Hub se font sur un fil a part.
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

const GAME: &str = "epicube-craft";
/// La liste se rafraichit toute seule a ce rythme tant que l'ecran est ouvert.
const REFRESH: Duration = Duration::from_secs(8);

/// Une partie en ligne.
#[derive(Clone)]
pub struct Listed {
    pub name: String,
    pub mode: String,
    pub mode_name: String,
    pub tags: Vec<String>,
    pub players: u16,
    pub max_players: u16,
    pub address: String,
    pub host_name: String,
    pub mods: u16,
    /// Partie privee ; `friend` : on est ami de celui qui l'heberge, on peut y entrer.
    pub private: bool,
    pub friend: bool,
}

impl Listed {
    pub fn full(&self) -> bool {
        self.players >= self.max_players
    }

    /// Une partie privee dont on n'est pas l'ami : on la voit, cadenassee, sans pouvoir y entrer.
    pub fn locked(&self) -> bool {
        self.private && !self.friend
    }
}

/// Un compte : son identifiant et son pseudo ; pour un ami, s'il est en ligne, ce qu'il fait, et le
/// nombre de ses messages que l'on n'a pas lus.
#[derive(Clone)]
pub struct Buddy {
    pub id: String,
    pub name: String,
    pub online: bool,
    pub status: String,
    pub unread: u32,
}

/// Un message d'une conversation : son numero, s'il est de nous, son texte.
#[derive(Clone)]
pub struct Line {
    pub id: i64,
    pub mine: bool,
    pub body: String,
}

/// Les amis du joueur, et les demandes en attente.
#[derive(Clone, Default)]
pub struct Friends {
    pub friends: Vec<Buddy>,
    pub received: Vec<Buddy>,
    pub sent: Vec<Buddy>,
}

enum Reply {
    List(Result<Vec<Listed>, String>),
    Friends(Result<Friends, String>),
    /// Une demande a abouti : ce qu'il faut en dire, puis la liste d'amis est relue.
    Done(Result<String, String>),
    /// Messages arrives dans la conversation avec ce compte.
    Messages(String, Vec<Line>),
    /// Un message est parti (ou non).
    Sent(Result<(), String>),
}

pub struct Browser {
    /// None : la liste n'est pas encore arrivee.
    pub list: Option<Result<Vec<Listed>, String>>,
    /// Texte cherche, mode de jeu retenu (vide : tous).
    pub query: String,
    pub mode: String,
    pub page: usize,
    /// Message a montrer au joueur.
    pub note: Option<String>,
    /// La rubrique Amis est ouverte ; None : les amis ne sont pas encore arrives.
    pub show_friends: bool,
    pub friends: Option<Result<Friends, String>>,
    /// Pseudo saisi pour une demande d'ami.
    pub friend_name: String,
    /// Conversation ouverte avec un ami, ses messages, et le texte en cours de frappe.
    pub chat_with: Option<Buddy>,
    pub chat: Vec<Line>,
    pub chat_input: String,
    chat_asked: Instant,
    /// Jeton du compte du joueur ; sans lui, pas d'amis.
    token: Option<String>,
    asked: Instant,
    rx: Receiver<Reply>,
    tx: Sender<Reply>,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(8)).timeout(Duration::from_secs(15)).build()
}

fn listed(v: &serde_json::Value) -> Listed {
    let text = |k: &str| v[k].as_str().unwrap_or_default().to_string();
    Listed {
        name: text("name"),
        mode: text("mode"),
        mode_name: text("mode_name"),
        tags: v["tags"].as_array().map(|a| a.iter().filter_map(|t| t.as_str().map(str::to_string)).collect()).unwrap_or_default(),
        players: v["players"].as_u64().unwrap_or(0) as u16,
        max_players: v["max_players"].as_u64().unwrap_or(0) as u16,
        address: text("address"),
        host_name: text("host_name"),
        mods: v["mods"].as_u64().unwrap_or(0) as u16,
        private: v["private"].as_bool() == Some(true),
        friend: v["friend"].as_bool() == Some(true),
    }
}

fn problem(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, _) => format!("l'annuaire a répondu {code}"),
        ureq::Error::Transport(_) => "annuaire injoignable".to_string(),
    }
}

/// Presente le compte du joueur, s'il en a un.
fn signed(r: ureq::Request, token: &Option<String>) -> ureq::Request {
    match token {
        Some(t) => r.set("Authorization", &format!("Bearer {t}")),
        None => r,
    }
}

fn fetch(path: &str, token: &Option<String>) -> Result<serde_json::Value, String> {
    let url = format!("{}/api/games/{GAME}/servers{path}", server::party::hub());
    signed(agent().get(&url), token).call().map_err(problem)?.into_json().map_err(|_| "réponse de l'annuaire illisible".to_string())
}

/// Message d'erreur du Hub, tel qu'il l'a ecrit pour le joueur.
fn said(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(_, r) => r.into_json::<serde_json::Value>().ok().and_then(|v| v["error"].as_str().map(str::to_string)).unwrap_or_else(|| "demande refusée".to_string()),
        ureq::Error::Transport(_) => "Epicube Hub est injoignable.".to_string(),
    }
}

fn fetch_friends(token: &Option<String>) -> Result<Friends, String> {
    let v: serde_json::Value = signed(agent().get(&format!("{}/api/friends", server::party::hub())), token).call().map_err(said)?.into_json().map_err(|e| e.to_string())?;
    let list = |k: &str| -> Vec<Buddy> {
        v[k].as_array()
            .into_iter()
            .flatten()
            .map(|b| Buddy {
                id: b["id"].as_str().unwrap_or_default().to_string(),
                name: b["username"].as_str().unwrap_or_default().to_string(),
                online: b["online"].as_bool() == Some(true),
                status: b["status"].as_str().unwrap_or_default().to_string(),
                unread: b["unread"].as_u64().unwrap_or(0) as u32,
            })
            .collect()
    };
    Ok(Friends { friends: list("friends"), received: list("received"), sent: list("sent") })
}

/// Minuscules sans accents : « Créatif » et « creatif » se trouvent.
fn plain(s: &str) -> String {
    shared::commands::plain(s)
}

impl Browser {
    /// `token` : le jeton du compte du joueur, s'il a lance le jeu depuis Epicube Hub.
    pub fn open(token: Option<String>) -> Browser {
        let (tx, rx) = mpsc::channel();
        let b = Browser {
            list: None,
            query: String::new(),
            mode: String::new(),
            page: 0,
            note: None,
            show_friends: false,
            friends: None,
            friend_name: String::new(),
            chat_with: None,
            chat: Vec::new(),
            chat_input: String::new(),
            chat_asked: Instant::now(),
            token,
            asked: Instant::now(),
            rx,
            tx,
        };
        b.refresh();
        // La liste d'amis se lit des l'ouverture : le bouton Amis dit combien de messages attendent.
        b.refresh_friends();
        b
    }

    /// Messages non lus et demandes d'amis en attente.
    pub fn waiting(&self) -> u32 {
        match &self.friends {
            Some(Ok(f)) => f.friends.iter().map(|b| b.unread).sum::<u32>() + f.received.len() as u32,
            _ => 0,
        }
    }

    /// Ouvre la conversation avec un ami.
    pub fn open_chat(&mut self, buddy: &Buddy) {
        self.chat_with = Some(buddy.clone());
        self.chat.clear();
        self.chat_input.clear();
        self.note = None;
        self.poll_chat();
    }

    pub fn close_chat(&mut self) {
        self.chat_with = None;
        self.refresh_friends();
    }

    /// Demande ce qui est arrive dans la conversation ouverte depuis le dernier message connu.
    fn poll_chat(&mut self) {
        let Some(buddy) = &self.chat_with else { return };
        self.chat_asked = Instant::now();
        let (tx, token, id, after) = (self.tx.clone(), self.token.clone(), buddy.id.clone(), self.chat.last().map_or(0, |m| m.id));
        std::thread::spawn(move || {
            let url = format!("{}/api/friends/{id}/messages", server::party::hub());
            let got = signed(agent().get(&url).query("after", &after.to_string()), &token).call().ok().and_then(|r| r.into_json::<serde_json::Value>().ok());
            if let Some(v) = got {
                let lines = v
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|m| Line { id: m["id"].as_i64().unwrap_or(0), mine: m["mine"].as_bool() == Some(true), body: m["body"].as_str().unwrap_or_default().to_string() })
                    .collect();
                let _ = tx.send(Reply::Messages(id, lines));
            }
        });
    }

    /// Envoie le message en cours de frappe a l'ami de la conversation ouverte.
    pub fn send_chat(&mut self) {
        let (Some(buddy), body) = (&self.chat_with, self.chat_input.trim().to_string()) else { return };
        if body.is_empty() {
            return;
        }
        self.chat_input.clear();
        let (tx, token, id) = (self.tx.clone(), self.token.clone(), buddy.id.clone());
        std::thread::spawn(move || {
            let url = format!("{}/api/friends/{id}/messages", server::party::hub());
            let sent = signed(agent().post(&url), &token).send_json(serde_json::json!({ "body": body }));
            let _ = tx.send(Reply::Sent(sent.map(|_| ()).map_err(said)));
        });
    }

    /// Le joueur est-il connecte a son compte ?
    pub fn has_account(&self) -> bool {
        self.token.is_some()
    }

    fn refresh_friends(&self) {
        if self.token.is_none() {
            return;
        }
        let (tx, token) = (self.tx.clone(), self.token.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Reply::Friends(fetch_friends(&token)));
        });
    }

    /// Ouvre ou ferme la rubrique Amis.
    pub fn toggle_friends(&mut self) {
        self.show_friends = !self.show_friends;
        self.note = None;
        if self.show_friends {
            self.refresh_friends();
        }
    }

    fn act(&self, request: ureq::Request, body: Option<serde_json::Value>, done: String) {
        let (tx, token) = (self.tx.clone(), self.token.clone());
        std::thread::spawn(move || {
            let request = signed(request, &token);
            let sent = match body {
                Some(b) => request.send_json(b),
                None => request.call(),
            };
            let _ = tx.send(Reply::Done(sent.map(|_| done).map_err(said)));
        });
    }

    /// Demande en ami le compte dont le pseudo est saisi.
    pub fn ask_friend(&mut self) {
        let name = self.friend_name.trim().to_string();
        if name.is_empty() {
            return;
        }
        self.act(agent().post(&format!("{}/api/friends", server::party::hub())), Some(serde_json::json!({ "username": name })), format!("Demande envoyée à {name}."));
        self.friend_name.clear();
    }

    pub fn accept_friend(&mut self, b: &Buddy) {
        self.act(agent().post(&format!("{}/api/friends/{}/accept", server::party::hub(), b.id)), None, format!("{} est maintenant votre ami.", b.name));
    }

    /// Retire un ami, refuse une demande ou en annule une.
    pub fn remove_friend(&mut self, b: &Buddy) {
        self.act(agent().delete(&format!("{}/api/friends/{}", server::party::hub(), b.id)), None, format!("{} : retiré.", b.name));
    }

    fn refresh(&self) {
        let (tx, token) = (self.tx.clone(), self.token.clone());
        std::thread::spawn(move || {
            // Seules les parties de notre version du jeu peuvent nous recevoir.
            let list = fetch(&format!("?protocol={}", shared::proto::PROTOCOL_VERSION), &token).map(|v| v.as_array().map(|a| a.iter().map(listed).collect()).unwrap_or_default());
            let _ = tx.send(Reply::List(list));
        });
    }

    /// Prend les reponses arrivees, et redemande la liste de temps en temps.
    pub fn poll(&mut self) {
        while let Ok(reply) = self.rx.try_recv() {
            match reply {
                Reply::List(list) => self.list = Some(list),
                Reply::Friends(f) => self.friends = Some(f),
                Reply::Done(result) => {
                    self.note = Some(result.unwrap_or_else(|e| e));
                    self.refresh_friends();
                }
                Reply::Messages(with, lines) => {
                    // Une reponse arrivee apres qu'on a change de conversation ne compte plus.
                    if self.chat_with.as_ref().is_some_and(|b| b.id == with) {
                        let last = self.chat.last().map_or(0, |m| m.id);
                        self.chat.extend(lines.into_iter().filter(|m| m.id > last));
                    }
                }
                Reply::Sent(result) => {
                    if let Err(e) = result {
                        self.note = Some(e);
                    }
                    self.poll_chat();
                }
            }
        }
        if self.asked.elapsed() >= REFRESH {
            self.asked = Instant::now();
            self.refresh();
            self.refresh_friends();
        }
        if self.chat_with.is_some() && self.chat_asked.elapsed() >= Duration::from_secs(3) {
            self.poll_chat();
        }
    }

    /// Les parties qui correspondent au mode retenu et au texte cherche.
    pub fn shown(&self) -> Vec<Listed> {
        let Some(Ok(list)) = &self.list else { return Vec::new() };
        let q = plain(self.query.trim());
        list.iter()
            .filter(|s| self.mode.is_empty() || s.mode == self.mode)
            .filter(|s| q.is_empty() || [&s.name, &s.mode, &s.mode_name, &s.host_name].into_iter().chain(&s.tags).any(|f| plain(f).contains(&q)))
            .cloned()
            .collect()
    }

    /// Modes de jeu presents dans l'annuaire : (identifiant, nom, nombre de parties), les plus fournis d'abord.
    pub fn categories(&self) -> Vec<(String, String, usize)> {
        let mut out: Vec<(String, String, usize)> = Vec::new();
        for s in self.list.iter().flatten().flatten() {
            match out.iter_mut().find(|c| c.0 == s.mode) {
                Some(c) => c.2 += 1,
                None => out.push((s.mode.clone(), s.mode_name.clone(), 1)),
            }
        }
        out.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.1.cmp(&b.1)));
        out
    }

    /// La meilleure partie a rejoindre pour ce mode : la plus peuplee qui a encore de la place.
    pub fn best(&self, mode: &str) -> Option<Listed> {
        self.list.iter().flatten().flatten().filter(|s| s.mode == mode && !s.full() && !s.locked()).max_by_key(|s| s.players).cloned()
    }
}

/// Dit aux amis du joueur qu'il est en jeu, tant que le jeu tourne. Sans compte, ne fait rien.
pub fn start_presence(token: Option<String>) {
    let Some(token) = token else { return };
    let _ = std::thread::Builder::new().name("presence".into()).spawn(move || loop {
        let url = format!("{}/api/friends/presence", server::party::hub());
        let _ = signed(agent().post(&url), &Some(token.clone())).send_json(serde_json::json!({ "place": "game", "label": "Epicube Craft" }));
        std::thread::sleep(Duration::from_secs(30));
    });
}
