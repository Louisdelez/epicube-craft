//! Les amis du joueur : ceux de son compte Epicube, la meme liste que dans Epicube Hub. On y demande
//! quelqu'un par son pseudo, on repond aux demandes, on voit qui est en ligne et l'on s'ecrit.
//! La barre des amis s'ouvre sur tous les ecrans, y compris en partie ; les echanges avec Epicube Hub
//! se font sur des fils a part.
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

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
pub struct List {
    pub friends: Vec<Buddy>,
    pub received: Vec<Buddy>,
    pub sent: Vec<Buddy>,
}

enum Reply {
    List(Result<List, String>),
    /// Une demande a abouti : ce qu'il faut en dire, puis la liste est relue.
    Done(Result<String, String>),
    /// Messages arrives dans la conversation avec ce compte.
    Messages(String, Vec<Line>),
    Sent(Result<(), String>),
}

pub struct Friends {
    /// La barre des amis est depliee.
    pub open: bool,
    /// None : la liste n'est pas encore arrivee.
    pub list: Option<Result<List, String>>,
    /// Pseudo saisi pour une demande d'ami.
    pub name: String,
    /// Conversation ouverte avec un ami, ses messages, et le texte en cours de frappe.
    pub chat_with: Option<Buddy>,
    pub chat: Vec<Line>,
    pub input: String,
    pub note: Option<String>,
    /// Jeton du compte du joueur ; sans lui, pas d'amis.
    token: Option<String>,
    asked: Instant,
    chat_asked: Instant,
    rx: Receiver<Reply>,
    tx: Sender<Reply>,
}

pub fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(8)).timeout(Duration::from_secs(15)).build()
}

/// Presente le compte du joueur, s'il en a un.
pub fn signed(r: ureq::Request, token: &Option<String>) -> ureq::Request {
    match token {
        Some(t) => r.set("Authorization", &format!("Bearer {t}")),
        None => r,
    }
}

fn url(path: &str) -> String {
    format!("{}/api/friends{path}", server::party::hub())
}

/// Message d'erreur du Hub, tel qu'il l'a ecrit pour le joueur.
fn said(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(_, r) => r.into_json::<serde_json::Value>().ok().and_then(|v| v["error"].as_str().map(str::to_string)).unwrap_or_else(|| "demande refusée".to_string()),
        ureq::Error::Transport(_) => "Epicube Hub est injoignable.".to_string(),
    }
}

fn fetch(token: &Option<String>) -> Result<List, String> {
    let v: serde_json::Value = signed(agent().get(&url("")), token).call().map_err(said)?.into_json().map_err(|e| e.to_string())?;
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
    Ok(List { friends: list("friends"), received: list("received"), sent: list("sent") })
}

impl Friends {
    /// `token` : le jeton du compte du joueur, s'il a lance le jeu depuis Epicube Hub.
    pub fn new(token: Option<String>) -> Friends {
        let (tx, rx) = mpsc::channel();
        let f = Friends {
            open: false,
            list: None,
            name: String::new(),
            chat_with: None,
            chat: Vec::new(),
            input: String::new(),
            note: None,
            token,
            asked: Instant::now(),
            chat_asked: Instant::now(),
            rx,
            tx,
        };
        f.refresh();
        f
    }

    /// Le joueur est-il connecte a son compte ?
    pub fn has_account(&self) -> bool {
        self.token.is_some()
    }

    fn refresh(&self) {
        if self.token.is_none() {
            return;
        }
        let (tx, token) = (self.tx.clone(), self.token.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Reply::List(fetch(&token)));
        });
    }

    /// Messages non lus et demandes d'amis en attente.
    pub fn waiting(&self) -> u32 {
        match &self.list {
            Some(Ok(l)) => l.friends.iter().map(|b| b.unread).sum::<u32>() + l.received.len() as u32,
            _ => 0,
        }
    }

    /// Deplie ou replie la barre.
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.note = None;
        if self.open {
            self.refresh();
        }
    }

    /// Prend les reponses arrivees ; relit la liste de temps en temps (plus souvent barre ouverte) et
    /// la conversation ouverte toutes les trois secondes.
    pub fn poll(&mut self) {
        while let Ok(reply) = self.rx.try_recv() {
            match reply {
                Reply::List(list) => self.list = Some(list),
                Reply::Done(result) => {
                    self.note = Some(result.unwrap_or_else(|e| e));
                    self.refresh();
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
        let every = Duration::from_secs(if self.open { 6 } else { 20 });
        if self.asked.elapsed() >= every {
            self.asked = Instant::now();
            self.refresh();
        }
        if self.open && self.chat_with.is_some() && self.chat_asked.elapsed() >= Duration::from_secs(3) {
            self.poll_chat();
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
    pub fn ask(&mut self) {
        let name = self.name.trim().to_string();
        if name.is_empty() {
            return;
        }
        self.act(agent().post(&url("")), Some(serde_json::json!({ "username": name })), format!("Demande envoyée à {name}."));
        self.name.clear();
    }

    pub fn accept(&mut self, b: &Buddy) {
        self.act(agent().post(&url(&format!("/{}/accept", b.id))), None, format!("{} est votre ami.", b.name));
    }

    /// Retire un ami, refuse une demande ou en annule une.
    pub fn remove(&mut self, b: &Buddy) {
        self.act(agent().delete(&url(&format!("/{}", b.id))), None, format!("{} : retiré.", b.name));
    }

    /// Ouvre la conversation avec un ami.
    pub fn open_chat(&mut self, buddy: &Buddy) {
        self.chat_with = Some(buddy.clone());
        self.chat.clear();
        self.input.clear();
        self.note = None;
        self.poll_chat();
    }

    pub fn close_chat(&mut self) {
        self.chat_with = None;
        self.refresh();
    }

    /// Demande ce qui est arrive dans la conversation ouverte depuis le dernier message connu.
    fn poll_chat(&mut self) {
        let Some(buddy) = &self.chat_with else { return };
        self.chat_asked = Instant::now();
        let (tx, token, id, after) = (self.tx.clone(), self.token.clone(), buddy.id.clone(), self.chat.last().map_or(0, |m| m.id));
        std::thread::spawn(move || {
            let got = signed(agent().get(&url(&format!("/{id}/messages"))).query("after", &after.to_string()), &token).call().ok().and_then(|r| r.into_json::<serde_json::Value>().ok());
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
    pub fn send(&mut self) {
        let (Some(buddy), body) = (&self.chat_with, self.input.trim().to_string()) else { return };
        if body.is_empty() {
            return;
        }
        self.input.clear();
        let (tx, token, id) = (self.tx.clone(), self.token.clone(), buddy.id.clone());
        std::thread::spawn(move || {
            let sent = signed(agent().post(&url(&format!("/{id}/messages"))), &token).send_json(serde_json::json!({ "body": body }));
            let _ = tx.send(Reply::Sent(sent.map(|_| ()).map_err(said)));
        });
    }
}

/// Dit aux amis du joueur qu'il est en jeu, tant que le jeu tourne. Sans compte, ne fait rien.
pub fn start_presence(token: Option<String>) {
    let Some(token) = token else { return };
    let _ = std::thread::Builder::new().name("presence".into()).spawn(move || loop {
        let _ = signed(agent().post(&url("/presence")), &Some(token.clone())).send_json(serde_json::json!({ "place": "game", "label": "Epicube Craft" }));
        std::thread::sleep(Duration::from_secs(30));
    });
}
