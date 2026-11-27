//! Outil d'essai des comptes : joue le role du service de comptes et du launcher tant qu'ils n'existent pas.
//!
//!   epicube-craft-jeton cles [fichier]
//!       Cree la paire de cles d'un service de comptes d'essai. La cle privee va dans le fichier
//!       (service-essai.cle par defaut), la cle publique s'affiche : c'est elle que l'on donne aux serveurs.
//!
//!   epicube-craft-jeton jeton --nom Hanako [--cle fichier] [--id uuid] [--heures 24]
//!       Fabrique un jeton pour ce joueur et affiche les deux variables a fournir au jeu.
//!
//! Ne jamais utiliser ces cles d'essai pour de vrais joueurs.
use shared::account::{self, Claims};

const DEFAULT_FILE: &str = "service-essai.cle";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    match args.first().map(String::as_str) {
        Some("cles") => {
            let file = args.get(1).cloned().unwrap_or_else(|| DEFAULT_FILE.to_string());
            if std::path::Path::new(&file).exists() {
                fail(&format!("{file} existe deja : supprimez-le pour en creer un autre"));
            }
            let secret = account::random32();
            if let Err(e) = std::fs::write(&file, account::format_key(&secret)) {
                fail(&format!("impossible d'ecrire {file} : {e}"));
            }
            println!("Cle privee du service d'essai : {file} (a garder pour soi)");
            println!("Cle publique, a donner aux serveurs :");
            println!("  dans comptes.txt :  cle={}", account::format_key(&account::public_of(&secret)));
            println!("  ou en variable   :  {}={}", account::KEY_ENV, account::format_key(&account::public_of(&secret)));
        }
        Some("jeton") => {
            let file = arg("--cle").unwrap_or_else(|| DEFAULT_FILE.to_string());
            let Some(secret) = std::fs::read_to_string(&file).ok().and_then(|t| account::parse_key(&t)) else {
                fail(&format!("cle du service illisible dans {file} : lancez d'abord « epicube-craft-jeton cles »"));
            };
            let name = account::clean_name(&arg("--nom").unwrap_or_default());
            if name.is_empty() {
                fail("il faut un pseudo : --nom Hanako");
            }
            // Sans --id, un identifiant tire du pseudo : le meme joueur d'essai retrouve sa sauvegarde.
            let id = match arg("--id") {
                Some(text) => account::parse_id(&text).unwrap_or_else(|| fail("--id n'est pas un UUID")),
                None => {
                    let mut id = account::offline_id(&format!("essai:{name}"));
                    id[6] = (id[6] & 0x0f) | 0x40;
                    id
                }
            };
            let hours: u64 = arg("--heures").and_then(|h| h.parse().ok()).unwrap_or(24);
            let session = account::random32();
            let claims = Claims { id, name, key: account::public_of(&session), expires: account::now() + hours * 3600 };
            println!("# {} - {} - valable {hours} h", claims.name, account::format_id(&claims.id));
            println!("{}={}", account::TOKEN_ENV, account::issue(&secret, &claims));
            println!("{}={}", account::SESSION_ENV, account::format_key(&session));
        }
        _ => {
            println!("Usage :");
            println!("  epicube-craft-jeton cles [fichier]");
            println!("  epicube-craft-jeton jeton --nom Hanako [--cle fichier] [--id uuid] [--heures 24]");
        }
    }
}

fn fail(why: &str) -> ! {
    eprintln!("{why}");
    std::process::exit(1);
}
