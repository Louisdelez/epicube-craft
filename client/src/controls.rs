//! Touches du jeu : chaque action a sa touche, que le joueur change dans Parametres -> Controles.
//!
//! Une touche est designee par sa place sur le clavier, pas par la lettre qu'elle porte : « avancer »
//! reste au meme endroit sur un clavier AZERTY (Z) comme QWERTY (W). La lettre affichee est celle que
//! la touche a donnee la derniere fois qu'on l'a enfoncee.
use winit::keyboard::KeyCode;

/// Nombre d'actions.
pub const COUNT: usize = 24;

/// Les actions, dans l'ordre de la page des controles.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Act {
    Forward,
    Back,
    Left,
    Right,
    Jump,
    Sprint,
    Down,
    Inventory,
    Drop,
    Chat,
    Voice,
    Info,
    Mode,
    Borders,
    Friends,
    Slot1,
}

/// (nom dans le fichier de reglages, nom affiche, touche d'origine)
const TABLE: [(&str, &str, KeyCode); COUNT] = [
    ("avancer", "Avancer", KeyCode::KeyW),
    ("reculer", "Reculer", KeyCode::KeyS),
    ("gauche", "Aller à gauche", KeyCode::KeyA),
    ("droite", "Aller à droite", KeyCode::KeyD),
    ("sauter", "Sauter, nager, monter en vol", KeyCode::Space),
    ("courir", "Courir", KeyCode::ShiftLeft),
    ("descendre", "Descendre en vol", KeyCode::ControlLeft),
    ("inventaire", "Inventaire", KeyCode::KeyE),
    ("jeter", "Jeter l'objet tenu", KeyCode::KeyQ),
    ("chat", "Ouvrir le chat", KeyCode::KeyT),
    ("parler", "Parler au micro", KeyCode::KeyV),
    ("informations", "Informations à l'écran", KeyCode::F3),
    ("mode", "Changer de mode de jeu", KeyCode::F4),
    ("bordures", "Bordures des chunks", KeyCode::F6),
    ("amis", "Barre des amis", KeyCode::KeyO),
    ("case1", "Barre rapide : case 1", KeyCode::Digit1),
    ("case2", "Barre rapide : case 2", KeyCode::Digit2),
    ("case3", "Barre rapide : case 3", KeyCode::Digit3),
    ("case4", "Barre rapide : case 4", KeyCode::Digit4),
    ("case5", "Barre rapide : case 5", KeyCode::Digit5),
    ("case6", "Barre rapide : case 6", KeyCode::Digit6),
    ("case7", "Barre rapide : case 7", KeyCode::Digit7),
    ("case8", "Barre rapide : case 8", KeyCode::Digit8),
    ("case9", "Barre rapide : case 9", KeyCode::Digit9),
];

#[derive(Clone, PartialEq)]
pub struct Controls {
    keys: [KeyCode; COUNT],
    /// Lettre que porte la touche sur le clavier du joueur, quand on la connait.
    labels: [String; COUNT],
}

impl Default for Controls {
    fn default() -> Self {
        Controls { keys: TABLE.map(|t| t.2), labels: std::array::from_fn(|_| String::new()) }
    }
}

/// Touches que l'on peut donner a une action.
fn candidates() -> Vec<KeyCode> {
    use KeyCode::*;
    vec![
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
        Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
        F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
        Space, Tab, CapsLock, ShiftLeft, ShiftRight, ControlLeft, ControlRight, AltLeft, AltRight,
        ArrowUp, ArrowDown, ArrowLeft, ArrowRight, Insert, Delete, Home, End, PageUp, PageDown,
        Backquote, Minus, Equal, BracketLeft, BracketRight, Semicolon, Quote, Comma, Period, Slash, Backslash, IntlBackslash,
        Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9, NumpadAdd, NumpadSubtract, NumpadMultiply, NumpadDivide, NumpadDecimal,
    ]
}

/// Une touche peut-elle servir a une action ? (Echap et Entree gardent leur role dans les menus.)
pub fn bindable(key: KeyCode) -> bool {
    candidates().contains(&key)
}

/// Nom d'une touche d'apres sa place sur le clavier.
fn place_name(key: KeyCode) -> String {
    use KeyCode::*;
    let fixed = match key {
        Space => "Espace",
        Tab => "Tab",
        CapsLock => "Verr. maj",
        ShiftLeft => "Maj gauche",
        ShiftRight => "Maj droite",
        ControlLeft => "Ctrl gauche",
        ControlRight => "Ctrl droite",
        AltLeft => "Alt gauche",
        AltRight => "Alt droite",
        ArrowUp => "Flèche haut",
        ArrowDown => "Flèche bas",
        ArrowLeft => "Flèche gauche",
        ArrowRight => "Flèche droite",
        Insert => "Inser",
        Delete => "Suppr",
        Home => "Début",
        End => "Fin",
        PageUp => "Page haut",
        PageDown => "Page bas",
        _ => "",
    };
    if !fixed.is_empty() {
        return fixed.to_string();
    }
    let raw = format!("{key:?}");
    for (prefix, shown) in [("Key", ""), ("Digit", ""), ("Numpad", "Pavé ")] {
        if let Some(rest) = raw.strip_prefix(prefix) {
            return format!("{shown}{rest}");
        }
    }
    raw
}

impl Controls {
    pub fn key(&self, act: Act) -> KeyCode {
        self.keys[act as usize]
    }

    /// Touche de la case `n` de la barre rapide (0 a 8).
    pub fn slot(&self, n: usize) -> KeyCode {
        self.keys[Act::Slot1 as usize + n.min(8)]
    }

    /// Case de la barre rapide que cette touche choisit.
    pub fn slot_of(&self, key: KeyCode) -> Option<usize> {
        (0..9).find(|n| self.slot(*n) == key)
    }

    pub fn title(index: usize) -> &'static str {
        TABLE[index].1
    }

    /// Ce qu'on affiche pour la touche d'une action : la lettre du clavier du joueur, sinon sa place.
    pub fn shown(&self, index: usize) -> String {
        let label = &self.labels[index];
        // Une lettre ou un chiffre appris ; les touches sans caractere gardent leur nom.
        if !label.is_empty() && place_name(self.keys[index]).chars().count() <= 2 {
            return label.clone();
        }
        place_name(self.keys[index])
    }

    pub fn shown_for(&self, act: Act) -> String {
        self.shown(act as usize)
    }

    /// Donne une touche a une action. `label` : la lettre qu'elle a produite, si elle en produit une.
    pub fn set(&mut self, index: usize, key: KeyCode, label: Option<&str>) {
        self.keys[index] = key;
        self.labels[index] = label.unwrap_or("").to_string();
    }

    /// Retient la lettre que porte une touche, quand le joueur vient de l'enfoncer.
    pub fn learn(&mut self, key: KeyCode, label: &str) {
        for i in 0..COUNT {
            if self.keys[i] == key && self.labels[i] != label {
                self.labels[i] = label.to_string();
            }
        }
    }

    /// Deux actions partagent-elles cette touche ?
    pub fn clash(&self, index: usize) -> bool {
        (0..COUNT).any(|i| i != index && self.keys[i] == self.keys[index])
    }

    pub fn is_default(&self) -> bool {
        (0..COUNT).all(|i| self.keys[i] == TABLE[i].2)
    }

    pub fn reset(&mut self) {
        *self = Controls::default();
    }

    /// Lit une ligne du fichier de reglages : « touche_avancer=KeyW|Z ». Faux si ce n'en est pas une.
    pub fn parse(&mut self, name: &str, value: &str) -> bool {
        let Some(index) = name.strip_prefix("touche_").and_then(|n| TABLE.iter().position(|t| t.0 == n)) else { return false };
        let (code, label) = value.split_once('|').unwrap_or((value, ""));
        if let Some(key) = candidates().into_iter().find(|k| format!("{k:?}") == code) {
            self.keys[index] = key;
            self.labels[index] = label.chars().filter(|c| !c.is_control()).take(3).collect();
        }
        true
    }

    /// Lignes a ecrire dans le fichier de reglages.
    pub fn to_text(&self) -> String {
        (0..COUNT).map(|i| format!("touche_{}={:?}|{}\n", TABLE[i].0, self.keys[i], self.labels[i])).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_survive_the_settings_file() {
        let mut c = Controls::default();
        assert_eq!((c.key(Act::Forward), c.slot(8), c.slot_of(KeyCode::Digit3)), (KeyCode::KeyW, KeyCode::Digit9, Some(2)));
        assert_eq!((c.shown_for(Act::Forward), c.shown_for(Act::Sprint), c.shown_for(Act::Info)), ("W".to_string(), "Maj gauche".to_string(), "F3".to_string()));
        // Sur un clavier AZERTY, la touche « avancer » porte un Z : on l'apprend quand elle est enfoncee.
        c.learn(KeyCode::KeyW, "Z");
        assert_eq!(c.shown_for(Act::Forward), "Z");
        c.set(Act::Jump as usize, KeyCode::KeyF, Some("F"));
        c.set(Act::Inventory as usize, KeyCode::KeyF, Some("F"));
        assert!(c.clash(Act::Jump as usize) && !c.clash(Act::Forward as usize) && !c.is_default());
        let mut back = Controls::default();
        for line in c.to_text().lines() {
            let (k, v) = line.split_once('=').unwrap();
            assert!(back.parse(k, v));
        }
        assert!(back == c);
        assert!(!back.parse("volume", "1"));
        // Une ligne illisible laisse la touche d'origine.
        assert!(back.parse("touche_chat", "N'importe quoi"));
        assert_eq!(back.key(Act::Chat), KeyCode::KeyT);
        assert!(!bindable(KeyCode::Escape) && bindable(KeyCode::KeyG));
        back.reset();
        assert!(back.is_default());
    }
}
