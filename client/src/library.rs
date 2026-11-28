//! Bibliotheque de mods : la liste publiee par Epicube Hub, et les mods installes dans le dossier `mods`.
//! Les echanges avec le Hub se font sur un fil a part : le menu ne se fige jamais.
use shared::mods::{self as sm, Pack};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

/// Adresse d'Epicube Hub : celle que le launcher donne au jeu, ou le service public.
pub const HUB_ENV: &str = "EPICUBE_HUB";
const DEFAULT_HUB: &str = "https://epicube.ch";
const GAME: &str = "epicube-craft";

/// Un mod de la bibliotheque.
#[derive(Clone)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub size: u64,
    sha256: String,
}

/// Un mod present dans le dossier `mods`.
#[derive(Clone)]
pub struct Installed {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    /// Fichier .zip du mod ; None pour un dossier (un mod en cours d'ecriture, qu'on ne supprime pas d'ici).
    file: Option<PathBuf>,
}

enum Reply {
    List(Result<Vec<Entry>, String>),
    Installed(String, Result<(), String>),
}

pub struct Library {
    /// None : la liste n'est pas encore arrivee.
    pub list: Option<Result<Vec<Entry>, String>>,
    pub installed: Vec<Installed>,
    /// Mod en cours d'installation.
    pub busy: Option<String>,
    /// Dernier message a montrer au joueur.
    pub note: Option<String>,
    pub page: usize,
    rx: Receiver<Reply>,
    tx: mpsc::Sender<Reply>,
}

fn hub() -> String {
    std::env::var(HUB_ENV).ok().map(|u| u.trim().trim_end_matches('/').to_string()).filter(|u| u.starts_with("http")).unwrap_or_else(|| DEFAULT_HUB.to_string())
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(8)).timeout(Duration::from_secs(60)).build()
}

fn problem(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, _) => format!("le Hub a répondu {code}"),
        ureq::Error::Transport(_) => "Epicube Hub est injoignable".to_string(),
    }
}

fn fetch_list() -> Result<Vec<Entry>, String> {
    let text = agent().get(&format!("{}/api/games/{GAME}/mods", hub())).call().map_err(problem)?.into_string().map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| "réponse du Hub illisible".to_string())?;
    let field = |v: &serde_json::Value, k: &str| v[k].as_str().unwrap_or_default().to_string();
    Ok(value
        .as_array()
        .ok_or("réponse du Hub illisible")?
        .iter()
        .map(|v| Entry {
            id: field(v, "slug"),
            name: field(v, "name"),
            description: field(v, "description"),
            author: field(v, "author"),
            version: field(v, "version"),
            size: v["size_bytes"].as_u64().unwrap_or(0),
            sha256: field(v, "sha256"),
        })
        .filter(|e| sm::valid_id(&e.id))
        .collect())
}

/// Telecharge un mod, le verifie, et le range dans le dossier `mods`.
fn fetch_mod(e: &Entry) -> Result<(), String> {
    use std::io::Read;
    let res = agent().get(&format!("{}/api/games/{GAME}/mods/{}/download", hub(), e.id)).call().map_err(problem)?;
    let mut bytes = Vec::new();
    res.into_reader().take(sm::MAX_SIZE as u64 + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() > sm::MAX_SIZE {
        return Err("mod trop gros".into());
    }
    if sm::hex(&sm::hash_of(&bytes)) != e.sha256 {
        return Err("le fichier reçu n'est pas celui annoncé par le Hub".into());
    }
    let pack = Pack::from_zip(bytes)?;
    if pack.manifest.id != e.id {
        return Err("l'archive ne contient pas le mod attendu".into());
    }
    std::fs::create_dir_all(sm::DIR).map_err(|e| e.to_string())?;
    // Ecrit a cote puis renomme : une coupure ne laisse pas un mod a moitie ecrit.
    let (tmp, dest) = (Path::new(sm::DIR).join(format!(".{}.part", e.id)), Path::new(sm::DIR).join(format!("{}.zip", e.id)));
    std::fs::write(&tmp, &pack.bytes).and_then(|_| std::fs::rename(&tmp, &dest)).map_err(|e| e.to_string())
}

/// Les mods du dossier `mods`, par ordre de nom.
fn scan() -> Vec<Installed> {
    let Ok(entries) = std::fs::read_dir(sm::DIR) else { return Vec::new() };
    let mut out: Vec<Installed> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let (pack, file) = if path.is_dir() && path.join("mod.json").exists() {
                (Pack::from_dir(&path).ok()?, None)
            } else if path.is_file() && name.ends_with(".zip") {
                (Pack::from_zip(std::fs::read(&path).ok()?).ok()?, Some(path))
            } else {
                return None;
            };
            let m = pack.manifest;
            Some(Installed { id: m.id, name: m.name, description: m.description, version: m.version, file })
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

impl Library {
    /// Ouvre la bibliotheque : lit les mods installes et demande la liste au Hub.
    pub fn open() -> Library {
        let (tx, rx) = mpsc::channel();
        let lib = Library { list: None, installed: scan(), busy: None, note: None, page: 0, rx, tx };
        lib.refresh();
        lib
    }

    fn refresh(&self) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Reply::List(fetch_list()));
        });
    }

    /// Prend les reponses arrivees depuis la derniere image.
    pub fn poll(&mut self) {
        while let Ok(reply) = self.rx.try_recv() {
            match reply {
                Reply::List(list) => self.list = Some(list),
                Reply::Installed(id, result) => {
                    self.busy = None;
                    self.installed = scan();
                    self.note = Some(match result {
                        Ok(()) => format!("« {id} » est installé."),
                        Err(e) => format!("« {id} » n'a pas pu être installé : {e}"),
                    });
                }
            }
        }
    }

    pub fn install(&mut self, e: &Entry) {
        if self.busy.is_some() {
            return;
        }
        // Une autre copie du meme mod (ancienne version sous un autre nom de fichier) lui cede la place.
        for old in self.installed.iter().filter(|i| i.id == e.id) {
            if let Some(file) = &old.file {
                let _ = std::fs::remove_file(file);
            }
        }
        self.busy = Some(e.id.clone());
        self.note = None;
        let (tx, e) = (self.tx.clone(), e.clone());
        std::thread::spawn(move || {
            let result = fetch_mod(&e);
            let _ = tx.send(Reply::Installed(e.id, result));
        });
    }

    pub fn remove(&mut self, id: &str) {
        for old in self.installed.iter().filter(|i| i.id == id) {
            if let Some(file) = &old.file {
                let _ = std::fs::remove_file(file);
            }
        }
        self.installed = scan();
        self.note = Some(format!("« {id} » est supprimé."));
    }

    pub fn installed(&self, id: &str) -> Option<&Installed> {
        self.installed.iter().find(|i| i.id == id)
    }

    /// Un mod installe peut-il etre supprime d'ici ? (Pas un dossier : c'est le travail de son createur.)
    pub fn removable(&self, id: &str) -> bool {
        self.installed.iter().any(|i| i.id == id && i.file.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Contre un vrai Hub : EPICUBE_HUB=http://... cargo test -p client library -- --ignored
    /// Installe puis supprime le premier mod de la bibliotheque, dans un dossier temporaire.
    #[test]
    #[ignore]
    fn install_from_a_running_hub() {
        let dir = std::env::temp_dir().join(format!("epicube-library-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let mut lib = Library::open();
        let wait = |lib: &mut Library, done: &dyn Fn(&Library) -> bool| {
            for _ in 0..200 {
                lib.poll();
                if done(lib) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            panic!("le Hub n'a pas répondu");
        };
        wait(&mut lib, &|l| l.list.is_some());
        let list = lib.list.clone().unwrap().expect("liste des mods");
        println!("{} mod(s) : {:?}", list.len(), list.iter().map(|e| format!("{} {}", e.id, e.version)).collect::<Vec<_>>());
        for e in &list {
            lib.install(e);
            wait(&mut lib, &|l| l.busy.is_none());
            println!("{}", lib.note.clone().unwrap_or_default());
            let got = lib.installed(&e.id).expect("mod installe");
            assert_eq!(got.version, e.version);
        }
        // Les mods installes sont ceux que la partie solo chargera.
        let (packs, problems) = sm::load_dir(Path::new(sm::DIR));
        assert!(problems.is_empty() && packs.len() == list.len());
        let first = list[0].id.clone();
        assert!(lib.removable(&first));
        lib.remove(&first);
        assert!(lib.installed(&first).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
