//! Modes de jeu que l'on peut heberger : ceux du jeu (survie, creatif), ceux livres avec lui dans le
//! dossier `modes` (UHC, Bed Wars...) et ceux qu'apportent les mods installes. Un mode de jeu autre
//! que survie ou creatif est un mod comme un autre, dont le mod.json porte une rubrique « mode ».
use shared::mods::{self as sm, Pack};
use shared::rules::Rules;
use std::path::Path;

/// Dossier des modes de jeu livres avec le jeu.
pub const DIR: &str = "modes";

#[derive(Clone)]
pub struct Mode {
    pub id: String,
    pub name: String,
    pub about: String,
    pub tags: Vec<String>,
    /// Places proposees par defaut.
    pub players: u16,
    /// Le monde est garde d'une partie a l'autre.
    pub keep_world: bool,
    pub rules: Rules,
    /// Le mod qui fait ce mode ; None pour la survie et le creatif du jeu.
    pub pack: Option<Pack>,
}

fn builtin(id: &str, name: &str, about: &str, mode: u8, cheats: bool) -> Mode {
    Mode {
        id: id.into(),
        name: name.into(),
        about: about.into(),
        tags: vec![id.into()],
        players: 8,
        keep_world: true,
        rules: Rules { mode, cheats, ..Rules::default() }.checked(),
        pack: None,
    }
}

fn from_pack(pack: Pack) -> Option<Mode> {
    let def = pack.manifest.mode.clone()?;
    let game = shared::commands::index_of(&shared::commands::MODES, &def.game).unwrap_or(shared::rules::SURVIVAL);
    Some(Mode {
        id: pack.manifest.id.clone(),
        name: if def.name.trim().is_empty() { pack.manifest.name.clone() } else { def.name.trim().chars().take(32).collect() },
        about: if def.about.is_empty() { pack.manifest.description.clone() } else { def.about },
        tags: def.tags,
        players: if def.players == 0 { 16 } else { def.players.min(100) },
        keep_world: def.keep_world,
        rules: Rules { mode: game, ..Rules::default() }.checked(),
        pack: Some(pack),
    })
}

/// Tous les modes que ce joueur peut heberger, ceux du jeu d'abord.
pub fn available() -> Vec<Mode> {
    let mut all = vec![
        builtin("survie", "Survie", "Récolter, fabriquer, bâtir et se défendre, ensemble.", shared::rules::SURVIVAL, false),
        builtin("creatif", "Créatif", "Tous les objets, vol libre, aucun danger : pour construire.", shared::rules::CREATIVE, true),
    ];
    for dir in [DIR, sm::DIR] {
        let (packs, _) = sm::load_dir(Path::new(dir));
        for mode in packs.into_iter().filter_map(from_pack) {
            if !all.iter().any(|m| m.id == mode.id) {
                all.push(mode);
            }
        }
    }
    // Survie et Creatif d'abord, puis les autres par ordre alphabetique.
    all[2..].sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    all
}

/// Les mods installes qui ne sont pas des modes de jeu : ils accompagnent une partie de survie ou de creatif.
pub fn extras() -> Vec<Pack> {
    sm::load_dir(Path::new(sm::DIR)).0.into_iter().filter(|p| p.manifest.mode.is_none()).collect()
}
