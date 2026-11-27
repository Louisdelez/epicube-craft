//! Joue une vraie partie contre le serveur via TCP : miner, ramasser, ranger, poser,
//! puis verifie que le monde et le joueur survivent a un redemarrage.
use shared::chunk::{idx, Chunk};
use shared::proto::{read_msg, write_msg, ClientMsg, ServerMsg, SlotRef, WindowKind, PROTOCOL_VERSION};
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

struct Bot {
    stream: TcpStream,
    chunks: HashMap<(i32, i32), Chunk>,
    /// Objets au sol annonces par le serveur.
    drops: Vec<u32>,
    removed: Vec<u32>,
    id: u32,
}

impl Bot {
    fn connect(port: u16, name: &str) -> (Bot, [f64; 3]) {
        let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
        let mut bot = Bot { stream, chunks: HashMap::new(), drops: Vec::new(), removed: Vec::new(), id: 0 };
        bot.send(ClientMsg::Hello { version: PROTOCOL_VERSION, name: name.into(), view: 2, account: None });
        let (id, spawn) = bot.wait("Welcome", |m| match m {
            ServerMsg::Welcome { id, spawn, .. } => Some((*id, *spawn)),
            _ => None,
        });
        bot.id = id;
        (bot, spawn)
    }

    fn send(&mut self, msg: ClientMsg) {
        write_msg(&mut self.stream, &msg).unwrap();
    }

    /// Lit les messages jusqu'a ce que `pick` renvoie quelque chose (5 s au plus).
    fn wait<T>(&mut self, what: &str, mut pick: impl FnMut(&ServerMsg) -> Option<T>) -> T {
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end {
            let Ok(msg) = read_msg::<_, ServerMsg>(&mut self.stream) else { continue };
            if let ServerMsg::Chunk { cx, cz, data } = &msg {
                self.chunks.insert((*cx, *cz), Chunk::decode(data).expect("chunk valide"));
            }
            if let ServerMsg::DropSpawn { id, .. } = &msg {
                self.drops.push(*id);
            }
            if let ServerMsg::DropRemove { id } = &msg {
                self.removed.push(*id);
            }
            if let Some(v) = pick(&msg) {
                return v;
            }
        }
        panic!("jamais recu : {what}");
    }

    fn block(&self, p: [i32; 3]) -> u8 {
        let c = self.chunks.get(&(p[0] >> 4, p[2] >> 4)).expect("chunk recu");
        c.b[idx((p[0] & 15) as usize, p[1] as usize, (p[2] & 15) as usize)]
    }

    fn wait_chunk(&mut self, key: (i32, i32)) {
        if !self.chunks.contains_key(&key) {
            self.wait("chunk", |m| matches!(m, ServerMsg::Chunk { cx, cz, .. } if (*cx, *cz) == key).then_some(()));
        }
    }

    fn moved(&mut self, pos: [f64; 3]) {
        self.send(ClientMsg::Pos { pos, yaw: 0.0, pitch: 0.0, on_ground: true });
    }
}

#[test]
fn miner_ranger_poser_sauvegarder() {
    let dir = std::env::temp_dir().join(format!("epicube_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut srv = server::start(listener, 1337, Some(dir.clone()), server::accounts::Accounts::default()).unwrap();
    let (mut bot, spawn) = Bot::connect(srv.port, "Testeur");

    let under = [spawn[0].floor() as i32, spawn[1].floor() as i32 - 1, spawn[2].floor() as i32];
    let side = [under[0] + 1, under[1], under[2]];
    bot.wait_chunk((under[0] >> 4, under[2] >> 4));
    bot.wait_chunk((side[0] >> 4, side[2] >> 4));
    let original = bot.block(under);
    assert_ne!(original, 0, "le joueur apparait sur un bloc");

    // Trop tot : le serveur refuse un minage plus rapide que la durete du bloc.
    bot.send(ClientMsg::Dig { pos: under });
    std::thread::sleep(Duration::from_millis(1600));
    bot.send(ClientMsg::Dig { pos: under });
    bot.wait("bloc casse", |m| matches!(m, ServerMsg::BlockChange { pos, b: 0, .. } if *pos == under).then_some(()));
    // L'objet est annonce avant le changement de bloc : il est deja note.
    let drop_id = *bot.drops.first().expect("un objet est tombe du bloc");

    // On descend dans le trou pour ramasser l'objet.
    bot.moved([spawn[0], spawn[1] - 1.0, spawn[2]]);
    let got = bot.wait("ramassage", |m| match m {
        ServerMsg::Inventory { inv, .. } => inv[0],
        _ => None,
    });
    assert_eq!(got.n, 1);
    assert!(bot.removed.contains(&drop_id), "l'objet ramasse disparait du sol");

    // Inventaire : prendre la pile, la poser dans la case 5.
    bot.send(ClientMsg::OpenInventory);
    bot.wait("fenetre", |m| matches!(m, ServerMsg::Window { kind: WindowKind::Inventory }).then_some(()));
    bot.send(ClientMsg::Click { slot: SlotRef::Inv(0), right: false, shift: false });
    bot.wait("pile en main", |m| matches!(m, ServerMsg::Inventory { cursor: Some(_), .. }).then_some(()));
    bot.send(ClientMsg::Click { slot: SlotRef::Inv(5), right: false, shift: false });
    bot.wait("pile rangee", |m| match m {
        ServerMsg::Inventory { inv, cursor: None, .. } if inv[5].is_some() && inv[0].is_none() => Some(()),
        _ => None,
    });
    // Une case de la grille 3x3 n'est pas accessible depuis l'inventaire 2x2.
    bot.send(ClientMsg::Click { slot: SlotRef::Inv(5), right: false, shift: false });
    bot.send(ClientMsg::Click { slot: SlotRef::Craft(8), right: false, shift: false });
    bot.send(ClientMsg::CloseWindow);
    bot.wait("retour de la pile", |m| match m {
        ServerMsg::Inventory { inv, cursor: None, craft, .. } if craft.iter().all(|s| s.is_none()) => {
            inv.iter().flatten().next().map(|_| ())
        }
        _ => None,
    });

    // Poser le bloc dans le trou : refuse tant que le joueur est dedans.
    let below = [under[0], under[1] - 1, under[2]];
    bot.send(ClientMsg::Select(0));
    bot.send(ClientMsg::Use { target: Some((below, under)) });
    bot.moved(spawn);
    std::thread::sleep(Duration::from_millis(150));
    bot.send(ClientMsg::Use { target: Some((below, under)) });
    let placed = bot.wait("bloc pose", |m| match m {
        ServerMsg::BlockChange { pos, b, .. } if *pos == under && *b != 0 => Some(*b),
        _ => None,
    });
    assert_eq!(placed as u16, got.id);

    // Un second trou, laisse ouvert, pour verifier la sauvegarde.
    std::thread::sleep(Duration::from_millis(1600));
    bot.send(ClientMsg::Dig { pos: side });
    bot.wait("second bloc casse", |m| matches!(m, ServerMsg::BlockChange { pos, b: 0, .. } if *pos == side).then_some(()));
    let last = [spawn[0], spawn[1] + 2.0, spawn[2]];
    bot.send(ClientMsg::Pos { pos: last, yaw: 0.0, pitch: 0.0, on_ground: false });
    std::thread::sleep(Duration::from_millis(200));
    drop(bot);
    std::thread::sleep(Duration::from_millis(200));
    srv.stop();

    assert!(dir.join("monde.bin").exists());
    assert!(dir.join("joueurs").join(format!("{}.bin", shared::account::format_id(&shared::account::offline_id("Testeur")))).exists());

    // Redemarrage avec une autre graine : celle du monde sauvegarde doit primer.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut srv = server::start(listener, 999, Some(dir.clone()), server::accounts::Accounts::default()).unwrap();
    let (mut bot, back) = Bot::connect(srv.port, "Testeur");
    assert_eq!(back, last, "le joueur revient ou il etait");
    bot.wait_chunk((under[0] >> 4, under[2] >> 4));
    bot.wait_chunk((side[0] >> 4, side[2] >> 4));
    assert_eq!(bot.block(under), placed, "le bloc pose est toujours la");
    assert_eq!(bot.block(side), 0, "le trou est toujours la");
    drop(bot);
    srv.stop();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deux_joueurs_se_voient() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut srv = server::start(listener, 1337, None, server::accounts::Accounts::default()).unwrap();
    let (mut a, spawn) = Bot::connect(srv.port, "Alice");
    let (mut b, _) = Bot::connect(srv.port, "Bob");
    let (a_id, b_id) = (a.id, b.id);

    // Chacun apprend la presence de l'autre.
    a.wait("arrivee de Bob", |m| matches!(m, ServerMsg::PlayerJoin { id, name, .. } if *id == b_id && name == "Bob").then_some(()));
    b.wait("presence d'Alice", |m| matches!(m, ServerMsg::PlayerJoin { id, name, .. } if *id == a_id && name == "Alice").then_some(()));

    // Alice bouge : Bob recoit sa position.
    let moved = [spawn[0] + 2.0, spawn[1], spawn[2] + 1.0];
    a.moved(moved);
    b.wait("position d'Alice", |m| matches!(m, ServerMsg::PlayerPos { id, pos, .. } if *id == a_id && *pos == moved).then_some(()));

    // Un bond impossible est refuse : Alice est remise a sa place et Bob ne voit rien.
    a.moved([spawn[0] + 500.0, spawn[1], spawn[2]]);
    a.wait("rappel a l'ordre", |m| matches!(m, ServerMsg::Teleport { pos } if *pos == moved).then_some(()));

    // Alice casse un bloc : Bob voit le changement.
    let under = [spawn[0].floor() as i32, spawn[1].floor() as i32 - 1, spawn[2].floor() as i32];
    std::thread::sleep(Duration::from_millis(1600));
    a.send(ClientMsg::Dig { pos: under });
    b.wait("bloc casse par Alice", |m| {
        if let ServerMsg::PlayerPos { id, pos, .. } = m {
            assert!(*id != a_id || pos[0] < spawn[0] + 100.0, "le bond refuse ne doit pas etre diffuse");
        }
        matches!(m, ServerMsg::BlockChange { pos, b: 0, .. } if *pos == under).then_some(())
    });

    // Un deuxieme "Alice" recoit un autre pseudo.
    let (c, _) = Bot::connect(srv.port, "Alice");
    b.wait("homonyme renomme", |m| matches!(m, ServerMsg::PlayerJoin { id, name, .. } if *id == c.id && name != "Alice").then_some(()));
    drop(c);

    // Bob part : Alice en est informee.
    drop(b);
    a.wait("depart de Bob", |m| matches!(m, ServerMsg::PlayerLeave { id } if *id == b_id).then_some(()));
    drop(a);
    srv.stop();
}

mod comptes {
    use super::*;
    use server::accounts::Accounts;
    use shared::account::{self, Claims};

    const SERVICE: [u8; 32] = [21; 32];
    const SESSION: [u8; 32] = [22; 32];
    const ADDRESS: &str = "127.0.0.1:25600";

    fn token(name: &str, expires: u64) -> String {
        let id = account::parse_id("0190a8e2-7d5c-4b1e-9f3a-5c2d8e4f6a1b").unwrap();
        account::issue(&SERVICE, &Claims { id, name: name.into(), key: account::public_of(&SESSION), expires })
    }

    fn rules(required: bool) -> Accounts {
        Accounts { required, key: Some(account::public_of(&SERVICE)), ..Accounts::default() }
    }

    fn open(port: u16) -> Bot {
        let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
        Bot { stream, chunks: HashMap::new(), drops: Vec::new(), removed: Vec::new(), id: 0 }
    }

    /// Se presente, releve le defi s'il y en a un, et renvoie le verdict : identifiant de joueur, ou motif du refus.
    fn join(port: u16, name: &str, token: Option<String>, session: &[u8; 32], address: &str) -> (Bot, Result<u32, String>) {
        let mut bot = open(port);
        bot.send(ClientMsg::Hello { version: PROTOCOL_VERSION, name: name.into(), view: 2, account: token });
        let verdict = loop {
            let step = bot.wait("verdict", |m| match m {
                ServerMsg::Challenge { nonce } => Some(Err(Ok(*nonce))),
                ServerMsg::Welcome { id, .. } => Some(Ok(*id)),
                ServerMsg::Refused { reason } => Some(Err(Err(reason.clone()))),
                _ => None,
            });
            match step {
                Ok(id) => break Ok(id),
                Err(Err(reason)) => break Err(reason),
                Err(Ok(nonce)) => bot.send(ClientMsg::Proof { address: address.into(), sig: account::prove(session, &nonce, address) }),
            }
        };
        (bot, verdict)
    }

    fn start(accounts: Accounts, dir: Option<std::path::PathBuf>) -> (server::ServerHandle, u16) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let srv = server::start(listener, 1337, dir, accounts).unwrap();
        let port = srv.port;
        (srv, port)
    }

    #[test]
    fn serveur_a_comptes_obligatoires() {
        let (mut srv, port) = start(rules(true), None);
        let later = account::now() + 3600;
        // Sans compte : dehors. Avec un compte et la bonne cle de session : dedans.
        assert!(join(port, "Intrus", None, &SESSION, ADDRESS).1.is_err());
        let (_hanako, ok) = join(port, "peu importe", Some(token("Hanako", later)), &SESSION, ADDRESS);
        assert!(ok.is_ok(), "{ok:?}");
        // Le meme compte ne peut pas etre connecte deux fois.
        assert!(join(port, "x", Some(token("Hanako", later)), &SESSION, ADDRESS).1.is_err());
        srv.stop();
    }

    #[test]
    fn jeton_vole_ou_perime_refuse() {
        let (mut srv, port) = start(rules(true), None);
        let later = account::now() + 3600;
        // Un jeton intercepte ne sert a rien sans la cle de session.
        assert!(join(port, "x", Some(token("Hanako", later)), &[99; 32], ADDRESS).1.is_err());
        // Jeton perime, ou signe par un autre service.
        assert!(join(port, "x", Some(token("Hanako", 1000)), &SESSION, ADDRESS).1.is_err());
        let fake = account::issue(&[66; 32], &Claims { id: [4; 16], name: "Hanako".into(), key: account::public_of(&SESSION), expires: later });
        assert!(join(port, "x", Some(fake), &SESSION, ADDRESS).1.is_err());
        srv.stop();
    }

    #[test]
    fn connexion_relayee_refusee() {
        let accounts = Accounts { addresses: vec!["jeu.exemple.fr:25600".into()], ..rules(true) };
        let (mut srv, port) = start(accounts, None);
        let later = account::now() + 3600;
        // Le joueur a compose l'adresse d'un autre serveur, qui relaie : sa signature ne vaut pas ici.
        assert!(join(port, "x", Some(token("Hanako", later)), &SESSION, "pirate.exemple.fr:25600").1.is_err());
        assert!(join(port, "x", Some(token("Hanako", later)), &SESSION, "Jeu.Exemple.fr").1.is_ok());
        srv.stop();
    }

    #[test]
    fn partie_locale_accepte_un_jeton_perime() {
        let (mut srv, port) = start(Accounts { lenient: true, ..rules(false) }, None);
        assert!(join(port, "x", Some(token("Hanako", 1000)), &SESSION, ADDRESS).1.is_ok());
        srv.stop();
    }

    #[test]
    fn le_pseudo_d_un_compte_lui_revient() {
        let (mut srv, port) = start(rules(false), None);
        // Comptes facultatifs : un joueur sans compte entre, mais cede le pseudo a son proprietaire.
        let (mut squatter, ok) = join(port, "Hanako", None, &SESSION, ADDRESS);
        assert!(ok.is_ok());
        assert!(join(port, "x", Some(token("Hanako", account::now() + 3600)), &SESSION, ADDRESS).1.is_ok());
        squatter.wait("renvoi", |m| matches!(m, ServerMsg::Refused { .. }).then_some(()));
        srv.stop();
    }

    #[test]
    fn la_sauvegarde_suit_le_compte() {
        let dir = std::env::temp_dir().join(format!("epicube_comptes_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // Une sauvegarde d'avant les comptes, rangee sous le pseudo.
        let (mut srv, port) = start(Accounts::default(), Some(dir.clone()));
        let (bot, ok) = join(port, "Hanako", None, &SESSION, ADDRESS);
        assert!(ok.is_ok());
        drop(bot);
        srv.stop();
        let players = dir.join("joueurs");
        let offline = players.join(format!("{}.bin", account::format_id(&account::offline_id("Hanako"))));
        assert!(offline.exists(), "sauvegarde hors ligne absente");
        std::fs::rename(&offline, players.join("Hanako.bin")).unwrap();
        // Le compte « Hanako » la reprend, sous son identifiant ; un changement de pseudo n'y touche plus.
        let (mut srv, port) = start(rules(true), Some(dir.clone()));
        let (bot, ok) = join(port, "x", Some(token("Hanako", account::now() + 3600)), &SESSION, ADDRESS);
        assert!(ok.is_ok());
        drop(bot);
        srv.stop();
        let owned = players.join("0190a8e2-7d5c-4b1e-9f3a-5c2d8e4f6a1b.bin");
        assert!(owned.exists() && !players.join("Hanako.bin").exists());
        let (mut srv, port) = start(rules(true), Some(dir.clone()));
        assert!(join(port, "x", Some(token("Sakura", account::now() + 3600)), &SESSION, ADDRESS).1.is_ok());
        srv.stop();
        assert!(owned.exists() && !players.join("Sakura.bin").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
