//! Serveur dedie Epicube Craft.
//! Usage : epicube-craft-server [--port 25600] [--seed 1337] [--world monde] [--comptes obligatoires|facultatifs]
//!         [--nom "Mon serveur"] [--mode survie] [--etiquettes pvp,francais] [--joueurs 20] [--prive] [--sans-annuaire]
//! Le serveur s'inscrit a l'annuaire d'Epicube Hub : les joueurs le trouvent dans l'ecran Multijoueur.
//! Les regles d'entree (comptes des joueurs) sont lues dans comptes.txt, a cote du serveur.
//! Ctrl+C sauvegarde le monde puis arrete le serveur.
use std::net::TcpListener;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let port: u16 = arg("--port").and_then(|s| s.parse().ok()).unwrap_or(shared::DEFAULT_PORT);
    let seed: u32 = arg("--seed").and_then(|s| s.parse().ok()).unwrap_or(1337);
    let dir = PathBuf::from("saves").join(arg("--world").unwrap_or_else(|| "serveur".to_string()));

    // Comptes : le fichier a cote du serveur, cree avec ses explications s'il manque.
    let mut accounts = match std::fs::read_to_string(server::accounts::FILE) {
        Ok(text) => server::accounts::Accounts::parse(&text),
        Err(_) => {
            let _ = std::fs::write(server::accounts::FILE, server::accounts::Accounts::template());
            server::accounts::Accounts::open()
        }
    };
    if let Some(v) = arg("--comptes") {
        accounts.required = v.starts_with("oblig");
    }
    if let Some(problem) = accounts.problem() {
        eprintln!("Reglages des comptes ({}) : {problem}.", server::accounts::FILE);
        std::process::exit(1);
    }
    let entry = match (accounts.required, accounts.key.is_some()) {
        (true, _) => "comptes obligatoires",
        (false, true) => "comptes facultatifs",
        (false, false) => "sans verification des comptes (aucune cle du service de comptes)",
    };

    let listener = match TcpListener::bind(("0.0.0.0", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Impossible d'ouvrir le port {port} : {e}");
            std::process::exit(1);
        }
    };
    // Mods : tout ce qui se trouve dans le dossier mods, a cote du serveur.
    let (mods, problems) = shared::mods::load_dir(std::path::Path::new(shared::mods::DIR));
    for p in problems {
        eprintln!("Mod ignoré - {p}");
    }
    let flag = |name: &str| args.iter().any(|a| a == name);
    let mode = arg("--mode").unwrap_or_else(|| "survie".to_string());
    let party = server::party::Party {
        name: arg("--nom").unwrap_or_else(|| "Serveur Epicube Craft".to_string()),
        mode_name: arg("--nom-du-mode").unwrap_or_else(|| {
            let mut c = mode.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }),
        mode,
        tags: arg("--etiquettes").map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default(),
        max_players: arg("--joueurs").and_then(|s| s.parse().ok()).unwrap_or(20),
        host_name: String::new(),
        // Un serveur dedie prive figure dans l'annuaire avec son cadenas ; « --sans-annuaire » l'en retire.
        private: flag("--prive"),
        host: None,
        token: None,
        hub: if flag("--sans-annuaire") { String::new() } else { server::party::hub() },
        relay: None,
    };
    let mut handle = match server::start_party(listener, seed, Some(dir.clone()), accounts, mods, party) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Impossible de demarrer le serveur : {e}");
            std::process::exit(1);
        }
    };
    println!("Serveur Epicube Craft en ecoute sur le port {port} - monde : {} - {entry}", dir.display());
    let stop = handle.stopper();
    if let Err(e) = ctrlc::set_handler(move || {
        println!("Arret demande, sauvegarde...");
        stop();
    }) {
        eprintln!("Ctrl+C ne sauvegardera pas le monde : {e}");
    }
    handle.wait();
    println!("Monde sauvegarde. Au revoir.");
}
