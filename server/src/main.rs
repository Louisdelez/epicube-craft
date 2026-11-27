//! Serveur dedie Epicube Craft.
//! Usage : epicube-craft-server [--port 25600] [--seed 1337] [--world monde]
//! Ctrl+C sauvegarde le monde puis arrete le serveur.
use std::net::TcpListener;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let port: u16 = arg("--port").and_then(|s| s.parse().ok()).unwrap_or(shared::DEFAULT_PORT);
    let seed: u32 = arg("--seed").and_then(|s| s.parse().ok()).unwrap_or(1337);
    let dir = PathBuf::from("saves").join(arg("--world").unwrap_or_else(|| "serveur".to_string()));

    let listener = match TcpListener::bind(("0.0.0.0", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Impossible d'ouvrir le port {port} : {e}");
            std::process::exit(1);
        }
    };
    let mut handle = match server::start(listener, seed, Some(dir.clone())) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Impossible de demarrer le serveur : {e}");
            std::process::exit(1);
        }
    };
    println!("Serveur Epicube Craft en ecoute sur le port {port} - monde : {}", dir.display());
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
