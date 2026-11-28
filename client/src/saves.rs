//! Les parties sauvegardees : dans les modes sans fin (survie, creatif, SMP, PvP Faction...), chaque
//! partie creee garde son monde, et se reprend plus tard. Une partie est un dossier de `saves`, avec
//! une fiche `partie.txt` qui dit son mode, son nom et quand on y a joue.
use std::path::{Path, PathBuf};

const DIR: &str = "saves";
const NOTE: &str = "partie.txt";

#[derive(Clone, Debug, PartialEq)]
pub struct Saved {
    /// Nom du dossier du monde.
    pub world: String,
    /// Nom montre au joueur.
    pub name: String,
    /// Derniere fois qu'on y a joue, en secondes depuis 1970 (0 : inconnu).
    pub played: u64,
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn read(dir: &Path) -> Option<(String, String, u64)> {
    let text = std::fs::read_to_string(dir.join(NOTE)).ok()?;
    let field = |key: &str| text.lines().find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('='))).map(str::trim).unwrap_or_default().to_string();
    Some((field("mode"), field("nom"), field("joue").parse().unwrap_or(0)))
}

/// Les parties sauvegardees du mode `mode` (dont le nom affiche est `mode_name`), la plus recente d'abord.
pub fn list(mode: &str, mode_name: &str) -> Vec<Saved> {
    list_in(Path::new(DIR), mode, mode_name)
}

fn list_in(root: &Path, mode: &str, mode_name: &str) -> Vec<Saved> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    let mut out: Vec<Saved> = entries
        .flatten()
        .filter_map(|e| {
            let (dir, world) = (e.path(), e.file_name().to_string_lossy().to_string());
            if !dir.is_dir() || world.starts_with('.') || !(dir.join("monde.bin").exists() || dir.join("graine.txt").exists()) {
                return None;
            }
            match read(&dir) {
                Some((m, name, played)) => (m == mode).then(|| Saved { name: if name.is_empty() { world.clone() } else { name }, world, played }),
                // Un monde d'avant les fiches : « monde » est la survie, les autres portent le nom de leur mode.
                None => {
                    let legacy = if world == "monde" { "survie" } else { world.as_str() };
                    let played = std::fs::metadata(dir.join("monde.bin")).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
                    (legacy == mode).then(|| Saved { name: format!("{mode_name} (monde d'origine)"), world, played })
                }
            }
        })
        .collect();
    out.sort_by(|a, b| b.played.cmp(&a.played).then(a.name.cmp(&b.name)));
    out
}

/// Prepare une partie neuve du mode : rend le nom de son dossier et le nom a montrer (« SMP 3 »).
pub fn fresh(mode: &str, mode_name: &str) -> (String, String) {
    fresh_in(Path::new(DIR), mode, mode_name)
}

fn fresh_in(root: &Path, mode: &str, mode_name: &str) -> (String, String) {
    let taken: Vec<String> = list_in(root, mode, mode_name).into_iter().map(|s| s.name).collect();
    let mut n = 1;
    loop {
        let (world, name) = (format!("{mode}-{n}"), format!("{mode_name} {n}"));
        if !root.join(&world).exists() && !taken.contains(&name) {
            return (world, name);
        }
        n += 1;
    }
}

/// Note que l'on joue a cette partie maintenant ; ecrit sa fiche si elle n'en avait pas.
pub fn touch(world: &str, mode: &str, name: &str) {
    touch_in(Path::new(DIR), world, mode, name, now());
}

fn touch_in(root: &Path, world: &str, mode: &str, name: &str, at: u64) {
    let dir = root.join(world);
    let (name, made) = match read(&dir) {
        Some((_, old, _)) if !old.is_empty() => (old, None),
        _ => (name.to_string(), Some(at)),
    };
    let made = made.or_else(|| std::fs::read_to_string(dir.join(NOTE)).ok().and_then(|t| t.lines().find_map(|l| l.strip_prefix("cree=").and_then(|v| v.trim().parse().ok())))).unwrap_or(at);
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(NOTE), format!("mode={mode}\nnom={name}\ncree={made}\njoue={at}\n"));
}

/// Supprime une partie sauvegardee, monde compris.
pub fn delete(world: &str) -> bool {
    // Un nom de dossier simple, rien d'autre : on ne sort jamais du dossier des sauvegardes.
    if world.is_empty() || world.starts_with('.') || world.contains(['/', '\\']) {
        return false;
    }
    std::fs::remove_dir_all(PathBuf::from(DIR).join(world)).is_ok()
}

/// « il y a 3 h », « il y a 2 jours »...
pub fn ago(played: u64, now: u64) -> String {
    if played == 0 {
        return "jamais jouée".to_string();
    }
    let s = now.saturating_sub(played);
    match s {
        0..=89 => "à l'instant".to_string(),
        90..=5399 => format!("il y a {} min", s / 60),
        5400..=129_599 => format!("il y a {} h", s / 3600),
        _ => format!("il y a {} jours", s / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_games_are_listed_by_mode_most_recent_first() {
        let root = std::env::temp_dir().join(format!("epicube-saves-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let world = |name: &str| {
            std::fs::create_dir_all(root.join(name)).unwrap();
            std::fs::write(root.join(name).join("graine.txt"), "1").unwrap();
        };
        // Deux parties de SMP, une de faction, et le vieux monde de survie sans fiche.
        let (w1, n1) = fresh_in(&root, "smp", "SMP");
        assert_eq!((w1.as_str(), n1.as_str()), ("smp-1", "SMP 1"));
        world(&w1);
        touch_in(&root, &w1, "smp", &n1, 100);
        let (w2, n2) = fresh_in(&root, "smp", "SMP");
        assert_eq!((w2.as_str(), n2.as_str()), ("smp-2", "SMP 2"));
        world(&w2);
        touch_in(&root, &w2, "smp", &n2, 200);
        world("faction-1");
        touch_in(&root, "faction-1", "faction", "PvP Faction 1", 150);
        world("monde");
        let smp = list_in(&root, "smp", "SMP");
        assert_eq!(smp.iter().map(|s| (s.name.as_str(), s.played)).collect::<Vec<_>>(), vec![("SMP 2", 200), ("SMP 1", 100)]);
        assert_eq!(list_in(&root, "faction", "PvP Faction").len(), 1);
        assert_eq!(list_in(&root, "survie", "Survie").iter().map(|s| s.world.as_str()).collect::<Vec<_>>(), vec!["monde"]);
        assert!(list_in(&root, "creatif", "Créatif").is_empty());
        // Rejouer a une partie garde son nom et la remonte en tete.
        touch_in(&root, &w1, "smp", "autre nom", 300);
        assert_eq!(list_in(&root, "smp", "SMP")[0], Saved { world: "smp-1".into(), name: "SMP 1".into(), played: 300 });
        // Un dossier de parties sans lendemain, ou sans monde, n'est pas une partie sauvegardee.
        std::fs::create_dir_all(root.join(".parties").join("uhc-1")).unwrap();
        std::fs::create_dir_all(root.join("vide")).unwrap();
        assert_eq!(list_in(&root, "smp", "SMP").len(), 2);
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(ago(0, 10), "jamais jouée");
        assert_eq!((ago(1000, 1030), ago(1000, 1000 + 600), ago(1000, 1000 + 7200), ago(1000, 1000 + 3 * 86_400)), ("à l'instant".to_string(), "il y a 10 min".to_string(), "il y a 2 h".to_string(), "il y a 3 jours".to_string()));
        assert!(!delete("../ailleurs") && !delete("") && !delete(".parties"));
    }
}
