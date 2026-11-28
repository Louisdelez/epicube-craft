//! Une partie ouverte aux autres : ce qu'elle annonce d'elle, et le fil qui la tient inscrite a
//! l'annuaire d'Epicube Hub. Toute partie s'y trouve, qu'elle tourne sur un serveur dedie ou dans le
//! jeu de celui qui l'heberge. Une partie privee y figure avec un cadenas : seuls les amis de celui
//! qui l'heberge peuvent y entrer.
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Nom du jeu dans l'annuaire.
const GAME: &str = "epicube-craft";
/// Adresse d'Epicube Hub : celle que le launcher donne au jeu, ou le service public.
pub const HUB_ENV: &str = "EPICUBE_HUB";
const DEFAULT_HUB: &str = "https://epicube.ch";
/// Rythme des nouvelles donnees a l'annuaire.
const BEAT: Duration = Duration::from_secs(25);

pub fn hub() -> String {
    std::env::var(HUB_ENV).ok().map(|u| u.trim().trim_end_matches('/').to_string()).filter(|u| u.starts_with("http")).unwrap_or_else(|| DEFAULT_HUB.to_string())
}

/// Ce qu'une partie dit d'elle aux joueurs qui cherchent ou jouer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Party {
    pub name: String,
    /// Mode de jeu : son identifiant (« survie », « uhc »...) et son nom affiche.
    pub mode: String,
    pub mode_name: String,
    pub tags: Vec<String>,
    /// Places ; au-dela, le serveur refuse l'entree.
    pub max_players: u16,
    /// Pseudo de celui qui heberge.
    pub host_name: String,
    /// Partie privee : reservee aux amis de celui qui l'heberge.
    pub private: bool,
    /// Compte de celui qui heberge, et son jeton : il prouve a l'annuaire que la partie est la sienne,
    /// et permet de lire la liste de ses amis.
    pub host: Option<shared::account::Id>,
    pub token: Option<String>,
    /// Adresse d'Epicube Hub ; vide : la partie ne s'annonce nulle part.
    pub hub: String,
    /// Adresse du relais par lequel on la rejoint, quand elle n'est pas joignable directement.
    pub relay: Option<String>,
}

/// Ou en est l'inscription de la partie, pour l'afficher a celui qui heberge.
#[derive(Default)]
pub struct Status {
    pub players: AtomicU32,
    /// Adresse sous laquelle les autres la voient.
    pub address: Mutex<String>,
    /// L'annuaire a pu s'y connecter depuis Internet.
    pub reachable: AtomicBool,
    /// L'annuaire a repondu au moins une fois.
    pub listed: AtomicBool,
    /// La partie passe par le relais du Hub : on la rejoint sans rien regler chez soi.
    pub relayed: AtomicBool,
    /// Amis de celui qui heberge : eux seuls entrent dans une partie privee.
    pub friends: Mutex<std::collections::HashSet<shared::account::Id>>,
    pub error: Mutex<Option<String>>,
}

impl Status {
    pub fn address(&self) -> String {
        self.address.lock().map(|c| c.clone()).unwrap_or_default()
    }
    pub fn error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|e| e.clone())
    }
}

/// Le fil d'annonce ; le lacher (ou appeler `stop`) retire la partie de l'annuaire.
pub struct Announcer {
    stop: Sender<()>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl Announcer {
    pub fn stop(&mut self) {
        let _ = self.stop.send(());
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for Announcer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(8)).timeout(Duration::from_secs(15)).build()
}

fn body(p: &Party, port: u16, mods: u16, players: u32, secret: &str) -> serde_json::Value {
    serde_json::json!({
        "name": p.name, "mode": p.mode, "mode_name": p.mode_name, "tags": p.tags,
        "players": players, "max_players": p.max_players, "protocol": shared::proto::PROTOCOL_VERSION,
        "version": env!("CARGO_PKG_VERSION"), "private": p.private, "port": port, "mods": mods,
        "host_name": p.host_name, "relay": p.relay, "secret": secret,
    })
}

/// Inscrit la partie a l'annuaire et l'y maintient jusqu'a l'arret.
pub fn announce(mut party: Party, port: u16, mods: u16, status: Arc<Status>, door: crate::Door) -> Announcer {
    let (stop, stopped) = mpsc::channel::<()>();
    let join = std::thread::Builder::new().name("annuaire".into()).spawn(move || {
        // Le relais, s'il a fallu y passer : il s'arrete avec ce fil.
        let mut relay: Option<Arc<AtomicBool>> = None;
        let (http, base) = (agent(), format!("{}/api/games/{GAME}/servers", party.hub));
        // (identifiant, secret) une fois la partie inscrite.
        let mut ticket: Option<(String, String)> = None;
        let bearer = party.token.as_ref().map(|t| format!("Bearer {t}"));
        let signed = |r: ureq::Request| match &bearer {
            Some(b) => r.set("Authorization", b),
            None => r,
        };
        loop {
            // Partie privee : la liste des amis de l'hote dit qui peut entrer. Elle est relue a chaque
            // tour, pour qu'un ami accepte a l'instant puisse rejoindre.
            if party.private && bearer.is_some() {
                let friends = signed(http.get(&format!("{}/api/friends", party.hub))).call().ok().and_then(|r| r.into_json::<serde_json::Value>().ok());
                if let (Some(v), Ok(mut set)) = (friends, status.friends.lock()) {
                    *set = v["friends"].as_array().into_iter().flatten().filter_map(|f| shared::account::parse_id(f["id"].as_str()?)).collect();
                }
            }
            let players = status.players.load(Ordering::Relaxed);
            let sent = match &ticket {
                Some((id, secret)) => signed(http.put(&format!("{base}/{id}"))).send_json(body(&party, port, mods, players, secret)),
                None => signed(http.post(&base)).send_json(body(&party, port, mods, players, "")),
            };
            match sent.map_err(|e| e.to_string()).and_then(|r| r.into_json::<serde_json::Value>().map_err(|e| e.to_string())) {
                Ok(v) => {
                    let text = |k: &str| v[k].as_str().unwrap_or_default().to_string();
                    if ticket.is_none() {
                        ticket = Some((text("id"), text("secret")));
                        println!("Partie inscrite à l'annuaire : adresse {}{}", text("address"), if v["reachable"].as_bool() == Some(true) { "" } else { " (injoignable depuis Internet)" });
                    }
                    if let (Ok(mut a), Ok(mut e)) = (status.address.lock(), status.error.lock()) {
                        (*a, *e) = (text("address"), None);
                    }
                    // EPICUBE_RELAIS=1 fait passer la partie par le relais meme si elle est joignable (essais).
                    let reachable = v["reachable"].as_bool() == Some(true) && std::env::var_os("EPICUBE_RELAIS").is_none();
                    status.reachable.store(reachable, Ordering::Relaxed);
                    status.listed.store(true, Ordering::Relaxed);
                    // Injoignable de l'exterieur (une box, le plus souvent) : la partie passe par le relais du
                    // Hub. Elle s'y connecte elle-meme, puis s'annonce a cette adresse. Rien a regler chez soi.
                    if let (false, None, Some(at)) = (reachable, &relay, v["relay"].as_str().filter(|a| !a.is_empty())) {
                        let (room, secret) = (random_word(), random_word());
                        let (stop, up) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
                        relay_host(at.to_string(), room.clone(), secret, door.clone(), stop.clone(), up.clone());
                        // On attend que la salle soit ouverte avant de l'annoncer.
                        let opened = (0..50).any(|_| {
                            std::thread::sleep(Duration::from_millis(100));
                            up.load(Ordering::Relaxed)
                        });
                        relay = Some(stop);
                        if opened {
                            party.relay = Some(format!("{at}#{room}"));
                            status.relayed.store(true, Ordering::Relaxed);
                            ticket = None;
                            println!("Partie relayée par Epicube Hub : aucun port à ouvrir.");
                            continue;
                        }
                    }
                }
                Err(why) => {
                    // 404 : l'annuaire nous a oublies (il a redemarre) ; on s'inscrit de nouveau au prochain tour.
                    if ticket.take().is_none() {
                        if let Ok(mut e) = status.error.lock() {
                            *e = Some(if why.contains("status code") { format!("l'annuaire a refusé la partie ({why})") } else { "annuaire injoignable".into() });
                        }
                        status.listed.store(false, Ordering::Relaxed);
                    }
                }
            }
            // Sans inscription, on reessaie plus vite. Et des qu'un joueur entre ou sort, l'annuaire le sait.
            let wait = if ticket.is_some() { BEAT } else { Duration::from_secs(6) };
            let began = std::time::Instant::now();
            let mut stop = false;
            while began.elapsed() < wait && status.players.load(Ordering::Relaxed) == players {
                match stopped.recv_timeout(Duration::from_secs(2)) {
                    Err(RecvTimeoutError::Timeout) => {}
                    _ => {
                        stop = true;
                        break;
                    }
                }
            }
            if stop {
                break;
            }
        }
        if let Some(stop) = relay {
            stop.store(true, Ordering::Relaxed);
        }
        if let Some((id, secret)) = ticket {
            let _ = http.delete(&format!("{base}/{id}")).query("secret", &secret).call();
        }
    });
    Announcer { stop, join: join.ok() }
}

/// Un mot au hasard, de lettres et de chiffres : nom ou secret d'une salle du relais.
fn random_word() -> String {
    shared::account::random32()[..12].iter().map(|b| format!("{b:02x}")).collect()
}

/// Tient ouverte la salle de la partie sur le relais du Hub, et va y chercher chaque joueur qui arrive.
/// `up` passe a vrai quand la salle est ouverte ; `stop` arrete tout.
fn relay_host(at: String, room: String, secret: String, door: crate::Door, stop: Arc<AtomicBool>, up: Arc<AtomicBool>) {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;
    let _ = std::thread::Builder::new().name("relais".into()).spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            let session = (|| -> std::io::Result<()> {
                let mut control = TcpStream::connect(&at)?;
                control.set_read_timeout(Some(Duration::from_secs(2)))?;
                control.write_all(format!("HOST {room} {secret}\n").as_bytes())?;
                let mut lines = BufReader::new(control.try_clone()?);
                let mut quiet = 0;
                loop {
                    if stop.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    let mut text = String::new();
                    match lines.read_line(&mut text) {
                        Ok(0) => return Err(std::io::ErrorKind::ConnectionAborted.into()),
                        Ok(_) => {
                            let words: Vec<&str> = text.split_whitespace().collect();
                            match words[..] {
                                ["OK"] => up.store(true, Ordering::Relaxed),
                                ["CONN", number] => {
                                    // Un joueur attend au relais : on ouvre une connexion pour lui, et le serveur le recoit.
                                    let (at, line, door) = (at.clone(), format!("DATA {room} {number}\n"), door.clone());
                                    std::thread::spawn(move || {
                                        if let Ok(mut stream) = TcpStream::connect(&at) {
                                            if stream.write_all(line.as_bytes()).is_ok() {
                                                door.admit(stream);
                                            }
                                        }
                                    });
                                }
                                _ => {}
                            }
                        }
                        // Rien a lire pendant deux secondes : on en profite pour donner signe de vie.
                        Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                            quiet += 1;
                            if quiet >= 10 {
                                quiet = 0;
                                control.write_all(b"PING\n")?;
                            }
                        }
                        Err(e) => return Err(e),
                    }
                }
            })();
            if session.is_err() && !stop.load(Ordering::Relaxed) {
                // Coupure : on reprend la meme salle un peu plus tard.
                std::thread::sleep(Duration::from_secs(3));
            }
        }
    });
}
