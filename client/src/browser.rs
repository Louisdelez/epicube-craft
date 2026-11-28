//! L'annuaire des parties en ligne, vu du jeu : toutes les parties, quelle que soit la machine qui
//! les heberge. Les echanges avec Epicube Hub se font sur un fil a part.
use crate::friends::{agent, signed};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

const GAME: &str = "epicube-craft";
/// La liste se rafraichit toute seule a ce rythme tant que l'ecran est ouvert.
const REFRESH: Duration = Duration::from_secs(8);
/// Le ping d'une partie se remesure a ce rythme, et jamais plus de tant a la fois.
const PING_EVERY: Duration = Duration::from_secs(24);
const PINGS_AT_ONCE: usize = 8;
const PING_WAIT: Duration = Duration::from_millis(2500);

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

pub struct Browser {
    /// None : la liste n'est pas encore arrivee.
    pub list: Option<Result<Vec<Listed>, String>>,
    /// Texte cherche parmi les parties, texte cherche parmi les modes de jeu, et mode retenu.
    pub query: String,
    pub mode_query: String,
    pub mode: String,
    /// Premier mode montre dans la liste de gauche : elle defile a la molette.
    pub mode_scroll: usize,
    pub page: usize,
    /// Jeton du compte du joueur : l'annuaire lui dit quelles parties privees lui sont ouvertes.
    token: Option<String>,
    asked: Instant,
    rx: Receiver<Result<Vec<Listed>, String>>,
    tx: Sender<Result<Vec<Listed>, String>>,
    /// Ping de chaque partie, par adresse : la mesure (None : sans reponse) et son heure.
    pings: HashMap<String, (Option<u32>, Instant)>,
    measuring: HashSet<String>,
    ping_rx: Receiver<(String, Option<u32>)>,
    ping_tx: Sender<(String, Option<u32>)>,
}

/// Mesure le ping d'une partie, en millisecondes : l'aller-retour d'un message jusqu'au serveur du jeu,
/// relais compris si la partie passe par lui. Une partie d'une version qui ne repond pas avant
/// l'entree donne le temps mis a la joindre.
fn measure(addr: &str) -> Option<u32> {
    use shared::proto::{read_msg, write_msg, ClientMsg, ServerMsg};
    use std::io::{Read, Write};
    use std::net::ToSocketAddrs;
    let (host, room) = addr.split_once('#').map_or((addr, None), |(h, r)| (h, Some(r)));
    let to = host.to_socket_addrs().ok()?.next()?;
    let start = Instant::now();
    let mut stream = std::net::TcpStream::connect_timeout(&to, PING_WAIT).ok()?;
    stream.set_nodelay(true).ok()?;
    stream.set_read_timeout(Some(PING_WAIT)).ok()?;
    stream.set_write_timeout(Some(PING_WAIT)).ok()?;
    if let Some(room) = room {
        stream.write_all(format!("JOIN {room}\n").as_bytes()).ok()?;
        let mut reply = [0u8; 3];
        if stream.read_exact(&mut reply).is_err() || &reply != b"OK\n" {
            return None;
        }
    }
    let reached = start.elapsed();
    // Deux allers-retours, le plus court des deux : le premier paie la mise en route.
    let mut best: Option<Duration> = None;
    for t in 0..2u64 {
        let sent = Instant::now();
        if write_msg(&mut stream, &ClientMsg::Ping { t }).is_err() {
            break;
        }
        match read_msg::<_, ServerMsg>(&mut stream) {
            Ok(ServerMsg::Pong { .. }) => best = Some(best.map_or(sent.elapsed(), |b| b.min(sent.elapsed()))),
            _ => break,
        }
    }
    Some(best.unwrap_or(reached).as_millis().clamp(1, 9999) as u32)
}

/// Nombre de barres allumees (sur 4) pour un ping.
pub fn bars(ms: u32) -> usize {
    match ms {
        0..=59 => 4,
        60..=119 => 3,
        120..=219 => 2,
        _ => 1,
    }
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

fn fetch(token: &Option<String>) -> Result<Vec<Listed>, String> {
    // Seules les parties de notre version du jeu peuvent nous recevoir.
    let url = format!("{}/api/games/{GAME}/servers?protocol={}", server::party::hub(), shared::proto::PROTOCOL_VERSION);
    let v: serde_json::Value = signed(agent().get(&url), token)
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, _) => format!("l'annuaire a répondu {code}"),
            ureq::Error::Transport(_) => "annuaire injoignable".to_string(),
        })?
        .into_json()
        .map_err(|_| "réponse de l'annuaire illisible".to_string())?;
    Ok(v.as_array().map(|a| a.iter().map(listed).collect()).unwrap_or_default())
}

impl Browser {
    /// `token` : le jeton du compte du joueur, s'il a lance le jeu depuis Epicube Hub.
    pub fn open(token: Option<String>) -> Browser {
        let (tx, rx) = mpsc::channel();
        let (ping_tx, ping_rx) = mpsc::channel();
        let b = Browser {
            list: None,
            query: String::new(),
            mode_query: String::new(),
            mode: String::new(),
            mode_scroll: 0,
            page: 0,
            token,
            asked: Instant::now(),
            rx,
            tx,
            pings: HashMap::new(),
            measuring: HashSet::new(),
            ping_rx,
            ping_tx,
        };
        b.refresh();
        b
    }

    fn refresh(&self) {
        let (tx, token) = (self.tx.clone(), self.token.clone());
        std::thread::spawn(move || {
            let _ = tx.send(fetch(&token));
        });
    }

    /// Prend les reponses arrivees, et redemande la liste de temps en temps.
    pub fn poll(&mut self) {
        while let Ok(list) = self.rx.try_recv() {
            self.list = Some(list);
        }
        if self.asked.elapsed() >= REFRESH {
            self.asked = Instant::now();
            self.refresh();
        }
        while let Ok((addr, ms)) = self.ping_rx.try_recv() {
            self.measuring.remove(&addr);
            self.pings.insert(addr, (ms, Instant::now()));
        }
        // Les parties du mode regarde passent d'abord.
        let mut due: Vec<(bool, String)> = Vec::new();
        for s in self.list.iter().flatten().flatten() {
            let fresh = self.pings.get(&s.address).is_some_and(|p| p.1.elapsed() < PING_EVERY);
            if !fresh && !self.measuring.contains(&s.address) {
                due.push((s.mode != self.mode, s.address.clone()));
            }
        }
        due.sort();
        for (_, addr) in due.into_iter().take(PINGS_AT_ONCE.saturating_sub(self.measuring.len())) {
            self.measuring.insert(addr.clone());
            let tx = self.ping_tx.clone();
            std::thread::spawn(move || {
                let ms = measure(&addr);
                let _ = tx.send((addr, ms));
            });
        }
    }

    /// Ping d'une partie : None tant qu'il n'est pas mesure, Some(None) si elle ne repond pas.
    pub fn ping(&self, address: &str) -> Option<Option<u32>> {
        self.pings.get(address).map(|p| p.0)
    }

    /// Les parties du mode retenu qui correspondent au texte cherche.
    pub fn shown(&self) -> Vec<Listed> {
        let Some(Ok(list)) = &self.list else { return Vec::new() };
        let q = shared::commands::plain(self.query.trim());
        list.iter()
            .filter(|s| s.mode == self.mode)
            .filter(|s| q.is_empty() || [&s.name, &s.mode_name, &s.host_name].into_iter().chain(&s.tags).any(|f| shared::commands::plain(f).contains(&q)))
            .cloned()
            .collect()
    }

    /// Modes de jeu presents dans l'annuaire : (identifiant, nom, nombre de parties).
    pub fn categories(&self) -> Vec<(String, String, usize)> {
        let mut out: Vec<(String, String, usize)> = Vec::new();
        for s in self.list.iter().flatten().flatten() {
            match out.iter_mut().find(|c| c.0 == s.mode) {
                Some(c) => c.2 += 1,
                None => out.push((s.mode.clone(), s.mode_name.clone(), 1)),
            }
        }
        out.sort_by(|a, b| a.1.cmp(&b.1));
        out
    }
}
