//! Un joueur rejoint un serveur qui a des mods : il les recoit par la connexion du jeu, puis il entre.
use shared::mods::{hash_of, load_dir, Pack};
use shared::proto::{read_msg, write_msg, ClientMsg, ServerMsg, PROTOCOL_VERSION};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::time::{Duration, Instant};

fn wait<T>(stream: &mut TcpStream, what: &str, mut pick: impl FnMut(ServerMsg) -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end {
        let Ok(msg) = read_msg::<_, ServerMsg>(stream) else { continue };
        if let Some(v) = pick(msg) {
            return v;
        }
    }
    panic!("jamais recu : {what}");
}

#[test]
fn mods_are_downloaded_on_join() {
    let (packs, problems) = load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../exemples/mods"));
    assert!(problems.is_empty(), "{problems:?}");
    assert!(packs.iter().any(|p| p.manifest.id == "bienvenue"));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut srv = server::start_with_mods(listener, 1337, None, server::accounts::Accounts::default(), packs.clone()).unwrap();

    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
    write_msg(&mut stream, &ClientMsg::Hello { version: PROTOCOL_VERSION, name: "Aki".into(), view: 2, account: None }).unwrap();
    // Le serveur annonce ses mods avant de laisser entrer.
    let infos = wait(&mut stream, "Mods", |m| match m {
        ServerMsg::Mods { mods, .. } => Some(mods),
        ServerMsg::Welcome { .. } => panic!("entre sans avoir recu les mods"),
        _ => None,
    });
    assert_eq!(infos.len(), packs.len());
    // Chaque mod arrive par morceaux ; l'archive recue est exactement celle annoncee.
    for info in &infos {
        let mut bytes: Vec<u8> = Vec::new();
        while bytes.len() < info.size as usize {
            write_msg(&mut stream, &ClientMsg::ModWant { hash: info.hash, offset: bytes.len() as u32 }).unwrap();
            let data = wait(&mut stream, "ModPart", |m| match m {
                ServerMsg::ModPart { hash, offset, total, data } if hash == info.hash && offset as usize == bytes.len() && total == info.size => Some(data),
                _ => None,
            });
            assert!(!data.is_empty());
            bytes.extend(data);
        }
        assert_eq!(hash_of(&bytes), info.hash);
        assert_eq!(Pack::from_zip(bytes).unwrap().manifest.id, info.id);
    }
    write_msg(&mut stream, &ClientMsg::ModsReady).unwrap();
    wait(&mut stream, "Welcome", |m| matches!(m, ServerMsg::Welcome { .. }).then_some(()));
    // Le mod Lua salue le nouveau venu, et sa commande repond.
    wait(&mut stream, "salut du mod", |m| matches!(m, ServerMsg::Chat { text, .. } if text.contains("bienvenue")).then_some(()));
    write_msg(&mut stream, &ClientMsg::Chat { text: "/soin".into() }).unwrap();
    wait(&mut stream, "reponse de /soin", |m| matches!(m, ServerMsg::Chat { text, .. } if text.contains("gueri")).then_some(()));
    srv.stop();
    shared::mods::clear_content();
}
