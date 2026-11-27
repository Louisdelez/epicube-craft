//! Modes de jeu et difficulte.
//!
//! Le mode appartient a chaque joueur : survie, creatif, aventure ou spectateur. La difficulte, le
//! mode extreme (une seule vie) et le droit de changer de mode en cours de partie appartiennent au
//! monde ; ils sont notes dans le fichier `regles.txt` de sa sauvegarde.
use serde::{Deserialize, Serialize};

/// On recolte, on fabrique, on a faim et on peut mourir.
pub const SURVIVAL: u8 = 0;
/// Blocs a volonte, casse instantanee, vol libre, aucun degat.
pub const CREATIVE: u8 = 1;
/// Comme la survie, mais on ne casse ni ne pose de blocs : on parcourt un monde bati par d'autres.
pub const ADVENTURE: u8 = 2;
/// On vole a travers les murs, invisible, sans rien pouvoir toucher.
pub const SPECTATOR: u8 = 3;

/// Aucune creature hostile, la vie remonte seule, la faim ne descend pas.
pub const PEACEFUL: u8 = 0;
pub const EASY: u8 = 1;
pub const NORMAL: u8 = 2;
pub const HARD: u8 = 3;

pub fn mode_name(mode: u8) -> &'static str {
    ["Survie", "Créatif", "Aventure", "Spectateur"][(mode as usize).min(3)]
}

pub fn difficulty_name(difficulty: u8) -> &'static str {
    ["Paisible", "Facile", "Normale", "Difficile"][(difficulty as usize).min(3)]
}

/// Ce mode peut-il casser et poser des blocs ?
pub fn builds(mode: u8) -> bool {
    matches!(mode, SURVIVAL | CREATIVE)
}

/// Ce mode peut-il toucher au monde (ouvrir un coffre, frapper, ramasser) ?
pub fn touches(mode: u8) -> bool {
    mode != SPECTATOR
}

/// Ce mode ne craint ni les coups, ni la faim, ni la noyade.
pub fn invulnerable(mode: u8) -> bool {
    matches!(mode, CREATIVE | SPECTATOR)
}

/// Ce mode peut-il voler ?
pub fn flies(mode: u8) -> bool {
    matches!(mode, CREATIVE | SPECTATOR)
}

/// Degats d'une creature, d'un projectile ou d'une explosion selon la difficulte :
/// moitie plus un en facile (sans jamais depasser le coup d'origine), une fois et demie en difficile.
pub fn scale_damage(dmg: f64, difficulty: u8) -> f64 {
    match difficulty {
        PEACEFUL => 0.0,
        EASY => (dmg / 2.0 + 1.0).min(dmg),
        HARD => dmg * 1.5,
        _ => dmg,
    }
}

/// Vie en dessous de laquelle la faim cesse de blesser : on meurt de faim en difficile seulement.
pub fn starve_floor(difficulty: u8) -> f64 {
    match difficulty {
        PEACEFUL => 20.0,
        EASY => 10.0,
        HARD => 0.0,
        _ => 1.0,
    }
}

/// Regles d'un monde.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rules {
    /// Mode donne aux joueurs qui arrivent pour la premiere fois.
    pub mode: u8,
    pub difficulty: u8,
    /// Mode extreme : difficulte bloquee sur difficile, et une seule vie (apres la mort, on reste spectateur).
    pub hardcore: bool,
    /// Autorise a changer de mode de jeu en cours de partie.
    pub cheats: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Rules { mode: SURVIVAL, difficulty: NORMAL, hardcore: false, cheats: false }
    }
}

impl Rules {
    /// Regles d'un monde cree avant leur existence : la survie normale, ou l'on pouvait deja s'envoler librement.
    pub fn legacy() -> Rules {
        Rules { cheats: true, ..Rules::default() }
    }

    /// Remet d'aplomb des regles lues ou recues : valeurs dans leurs bornes, mode extreme toujours difficile.
    pub fn checked(mut self) -> Rules {
        self.mode = self.mode.min(SPECTATOR);
        self.difficulty = self.difficulty.min(HARD);
        if self.hardcore {
            self.difficulty = HARD;
            self.mode = SURVIVAL;
            self.cheats = false;
        }
        self
    }

    /// Lit le fichier `regles.txt` ; les lignes inconnues sont ignorees.
    pub fn parse(text: &str) -> Rules {
        let mut r = Rules::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let flag = v == "1" || v == "oui";
            match k {
                "mode" => r.mode = v.parse().unwrap_or(r.mode),
                "difficulte" => r.difficulty = v.parse().unwrap_or(r.difficulty),
                "extreme" => r.hardcore = flag,
                "triche" => r.cheats = flag,
                _ => {}
            }
        }
        r.checked()
    }

    pub fn to_text(&self) -> String {
        format!("mode={}\ndifficulte={}\nextreme={}\ntriche={}\n", self.mode, self.difficulty, self.hardcore as u8, self.cheats as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_and_difficulties() {
        assert!(builds(SURVIVAL) && builds(CREATIVE) && !builds(ADVENTURE) && !builds(SPECTATOR));
        assert!(touches(ADVENTURE) && !touches(SPECTATOR));
        assert!(invulnerable(CREATIVE) && invulnerable(SPECTATOR) && !invulnerable(ADVENTURE));
        assert!(flies(SPECTATOR) && !flies(SURVIVAL));
        // Un coup de 6 : nul en paisible, 4 en facile, 6 en normal, 9 en difficile.
        assert_eq!([0, 1, 2, 3].map(|d| scale_damage(6.0, d)), [0.0, 4.0, 6.0, 9.0]);
        // En facile, un petit coup n'est jamais grossi.
        assert_eq!(scale_damage(1.0, EASY), 1.0);
        // La faim tue en difficile, laisse un demi-coeur en normal, cinq coeurs en facile.
        assert_eq!([1, 2, 3].map(starve_floor), [10.0, 1.0, 0.0]);
    }

    #[test]
    fn rules_file_roundtrip() {
        let r = Rules { mode: CREATIVE, difficulty: EASY, hardcore: false, cheats: true };
        assert_eq!(Rules::parse(&r.to_text()), r);
        assert_eq!(Rules::parse(""), Rules::default());
        // Le mode extreme impose la survie en difficile, sans triche.
        let h = Rules::parse("mode=1\ndifficulte=0\nextreme=1\ntriche=1\n");
        assert_eq!(h, Rules { mode: SURVIVAL, difficulty: HARD, hardcore: true, cheats: false });
        assert_eq!(Rules::parse("difficulte=9\nautre=3").difficulty, NORMAL.max(HARD));
    }
}
