//! Commandes du chat : leur table, l'analyse d'une ligne et la completion.
//!
//! Le client s'en sert pour proposer la suite de ce que l'on tape (noms de commandes, puis valeurs
//! possibles de chaque argument) ; le serveur, pour executer. Une ligne qui commence par « / » est
//! une commande, le reste est un message pour tout le monde.
use crate::items::{self, Id};
use crate::mobs as mk;

/// Nature d'un argument : elle dit quelles valeurs proposer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arg {
    Mode,
    Difficulty,
    /// Moment de la journee, ou une heure de 0 a 24.
    Time,
    Player,
    Item,
    Count,
    Mob,
    /// Trois nombres, ou un joueur.
    Place,
    Command,
    /// Texte libre jusqu'au bout de la ligne.
    Text,
}

pub struct Spec {
    pub name: &'static str,
    /// Nom anglais, accepte lui aussi.
    pub alias: &'static str,
    /// Arguments, avec « obligatoire ? ».
    pub args: &'static [(Arg, bool)],
    pub help: &'static str,
    /// Reservee aux mondes qui autorisent la triche.
    pub cheat: bool,
}

pub const COMMANDS: &[Spec] = &[
    Spec { name: "aide", alias: "help", args: &[(Arg::Command, false)], help: "Liste les commandes, ou explique celle que l'on nomme", cheat: false },
    Spec { name: "mode", alias: "gamemode", args: &[(Arg::Mode, true), (Arg::Player, false)], help: "Change de mode de jeu", cheat: true },
    Spec { name: "difficulte", alias: "difficulty", args: &[(Arg::Difficulty, true)], help: "Change la difficulté du monde", cheat: true },
    Spec { name: "heure", alias: "time", args: &[(Arg::Time, true)], help: "Règle l'heure du jour", cheat: true },
    Spec { name: "tp", alias: "teleport", args: &[(Arg::Place, true)], help: "Téléporte vers des coordonnées ou vers un joueur", cheat: true },
    Spec { name: "donner", alias: "give", args: &[(Arg::Item, true), (Arg::Count, false), (Arg::Player, false)], help: "Donne des objets", cheat: true },
    Spec { name: "vider", alias: "clear", args: &[(Arg::Player, false)], help: "Vide l'inventaire", cheat: true },
    Spec { name: "tuer", alias: "kill", args: &[(Arg::Player, false)], help: "Tue un joueur (soi-même par défaut)", cheat: true },
    Spec { name: "invoquer", alias: "summon", args: &[(Arg::Mob, true)], help: "Fait apparaître une créature devant soi", cheat: true },
    Spec { name: "apparition", alias: "spawnpoint", args: &[], help: "Fixe son point de réapparition ici", cheat: true },
    Spec { name: "graine", alias: "seed", args: &[], help: "Affiche la graine du monde", cheat: false },
    Spec { name: "liste", alias: "list", args: &[], help: "Liste les joueurs connectés", cheat: false },
    Spec { name: "dire", alias: "say", args: &[(Arg::Text, true)], help: "Annonce un message à tous", cheat: false },
    Spec { name: "moi", alias: "me", args: &[(Arg::Text, true)], help: "Décrit une action : « * Joueur ... »", cheat: false },
    Spec { name: "msg", alias: "tell", args: &[(Arg::Player, true), (Arg::Text, true)], help: "Message privé à un joueur", cheat: false },
];

pub const MODES: [&str; 4] = ["survie", "creatif", "aventure", "spectateur"];
pub const DIFFICULTIES: [&str; 4] = ["paisible", "facile", "normale", "difficile"];
/// Moments de la journee et l'heure correspondante.
pub const TIMES: [(&str, f64); 6] = [("aube", 6.0), ("jour", 8.0), ("midi", 12.0), ("crepuscule", 18.0), ("nuit", 20.0), ("minuit", 0.0)];
/// Creatures que l'on peut invoquer.
pub const MOBS: [(&str, u8); 12] = [
    ("cerf", mk::COW),
    ("sanglier", mk::PIG),
    ("tanuki", mk::SHEEP),
    ("grue", mk::CHICKEN),
    ("oni", mk::ZOMBIE),
    ("squelette", mk::SKELETON),
    ("araignee", mk::SPIDER),
    ("lanterne", mk::BOMBYX),
    ("yurei", mk::SHADE),
    ("wanyudo", mk::BRAISIER),
    ("ryujin", mk::WYRM),
    ("villageois", mk::VILLAGER),
];

/// Longueur maximale d'un message.
pub const MAX_LEN: usize = 120;

pub fn find(name: &str) -> Option<&'static Spec> {
    let name = plain(name);
    COMMANDS.iter().find(|c| c.name == name || c.alias == name)
}

/// Mode d'emploi d'une commande : « /mode <mode> [joueur] ».
pub fn usage(spec: &Spec) -> String {
    let mut out = format!("/{}", spec.name);
    for (arg, required) in spec.args {
        let word = match arg {
            Arg::Mode => "mode",
            Arg::Difficulty => "difficulté",
            Arg::Time => "moment ou heure",
            Arg::Player => "joueur",
            Arg::Item => "objet",
            Arg::Count => "quantité",
            Arg::Mob => "créature",
            Arg::Place => "x y z ou joueur",
            Arg::Command => "commande",
            Arg::Text => "texte",
        };
        out += &if *required { format!(" <{word}>") } else { format!(" [{word}]") };
    }
    out
}

/// Minuscules sans accents : « Créatif » et « creatif » se valent.
pub fn plain(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

/// Nom d'un objet dans une commande : son nom affiche, en minuscules, les espaces en tirets bas.
pub fn item_word(id: Id) -> String {
    let mut out = String::new();
    for c in plain(&items::name(id)).chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    out.trim_end_matches('_').to_string()
}

/// Tous les objets que l'on peut tenir : blocs posables, puis objets, outils, armes et armures.
pub fn all_items() -> Vec<Id> {
    (1..256u16)
        .filter(|id| items::place_block(*id) == Some(*id as u8))
        .chain(256..items::ITEM_END)
        .chain(items::TOOL_BASE..items::TOOL_BASE + 20)
        .chain(items::COOKED_RICE..items::MORE_END)
        .chain(items::TOOL4_BASE..=items::SHIELD)
        .chain(items::TAMA_SHARD..items::REALM_END)
        .chain(items::ARMOR_BASE..items::ARMOR_END)
        .chain(crate::mods::content().items.iter().map(|d| d.id))
        .collect()
}

/// Objet designe par ce mot (son nom, ou son numero).
pub fn item_from(word: &str) -> Option<Id> {
    let word = plain(word);
    let all = all_items();
    if let Ok(n) = word.parse::<Id>() {
        return all.contains(&n).then_some(n);
    }
    all.into_iter().find(|id| item_word(*id) == word)
}

pub fn index_of(list: &[&str], word: &str) -> Option<u8> {
    let word = plain(word);
    // Le numero est accepte aussi, comme le debut du mot s'il n'y a pas d'ambiguite.
    if let Ok(n) = word.parse::<usize>() {
        return (n < list.len()).then_some(n as u8);
    }
    let hits: Vec<usize> = (0..list.len()).filter(|i| list[*i].starts_with(&word)).collect();
    list.iter().position(|w| *w == word).or((hits.len() == 1 && !word.is_empty()).then(|| hits[0])).map(|i| i as u8)
}

/// Heure du jour (0 a 24) designee par ce mot.
pub fn hour_from(word: &str) -> Option<f64> {
    let word = plain(word);
    TIMES.iter().find(|t| t.0 == word).map(|t| t.1).or_else(|| word.parse::<f64>().ok().filter(|h| (0.0..=24.0).contains(h)))
}

pub fn mob_from(word: &str) -> Option<u8> {
    let word = plain(word);
    MOBS.iter().find(|m| m.0 == word).map(|m| m.1)
}

/// Valeurs possibles d'un argument.
fn options(arg: Arg, players: &[String]) -> Vec<String> {
    let own = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match arg {
        Arg::Mode => own(&MODES),
        Arg::Difficulty => own(&DIFFICULTIES),
        Arg::Time => TIMES.iter().map(|t| t.0.to_string()).collect(),
        Arg::Player => players.to_vec(),
        Arg::Item => all_items().into_iter().map(item_word).collect(),
        Arg::Mob => MOBS.iter().map(|m| m.0.to_string()).collect(),
        Arg::Place => players.to_vec(),
        Arg::Command => COMMANDS.iter().map(|c| c.name.to_string()).collect(),
        Arg::Count | Arg::Text => Vec::new(),
    }
}

/// Ce que l'on peut taper a la place du dernier mot de `input` : noms de commandes apres le « / »,
/// puis valeurs de l'argument en cours. Vide si la ligne n'est pas une commande ou si rien ne convient.
pub fn suggest(input: &str, players: &[String], cheats: bool) -> Vec<String> {
    let Some(rest) = input.strip_prefix('/') else { return Vec::new() };
    let words: Vec<&str> = rest.split(' ').collect();
    let last = plain(words.last().copied().unwrap_or(""));
    let pool = if words.len() <= 1 {
        // Les commandes de triche ne sont proposees que si le monde les permet.
        let mods = crate::mods::content();
        COMMANDS.iter().filter(|c| cheats || !c.cheat).map(|c| c.name.to_string()).chain(mods.commands.iter().map(|c| c.0.clone())).collect()
    } else {
        let Some(spec) = find(words[0]) else { return Vec::new() };
        match spec.args.get(words.len() - 2) {
            Some((arg, _)) => options(*arg, players),
            None => Vec::new(),
        }
    };
    // D'abord ce qui commence par ce qui est tape, puis ce qui le contient.
    let mut out: Vec<String> = pool.iter().filter(|w| plain(w).starts_with(&last)).cloned().collect();
    if !last.is_empty() {
        out.extend(pool.iter().filter(|w| !plain(w).starts_with(&last) && plain(w).contains(&last)).cloned());
    }
    out.dedup();
    // Un seul choix, deja tape en entier : il n'y a plus rien a proposer.
    if out.len() == 1 && plain(&out[0]) == last {
        out.clear();
    }
    out
}

/// Remplace le dernier mot de `input` par `choice`.
pub fn complete(input: &str, choice: &str) -> String {
    let cut = input.rfind(' ').map_or(1.min(input.len()), |i| i + 1);
    format!("{}{} ", &input[..cut], choice)
}

/// Mode d'emploi de la commande en cours de frappe, s'il y en a une de reconnue.
pub fn hint(input: &str) -> Option<String> {
    let rest = input.strip_prefix('/')?;
    let name = rest.split(' ').next()?;
    if !rest.contains(' ') {
        return None;
    }
    find(name).map(|s| format!("{} - {}", usage(s), s.help)).or_else(|| {
        // Une commande de mod : son nom et l'aide que le mod a donnee.
        let (mods, name) = (crate::mods::content(), plain(name));
        mods.commands.iter().find(|c| c.0 == name).map(|c| if c.1.is_empty() { format!("/{}", c.0) } else { format!("/{} - {}", c.0, c.1) })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_are_found_by_either_name() {
        assert_eq!(find("mode").map(|c| c.name), Some("mode"));
        assert_eq!(find("GAMEMODE").map(|c| c.name), Some("mode"));
        assert_eq!(find("Difficulté").map(|c| c.name), Some("difficulte"));
        assert!(find("abracadabra").is_none());
        assert_eq!(usage(find("donner").unwrap()), "/donner <objet> [quantité] [joueur]");
        // Pas de doublon entre noms et alias.
        let mut seen = std::collections::HashSet::new();
        for c in COMMANDS {
            assert!(seen.insert(c.name) && seen.insert(c.alias), "nom en double : {}", c.name);
        }
    }

    #[test]
    fn words_name_modes_items_and_times() {
        assert_eq!(index_of(&MODES, "créatif"), Some(1));
        assert_eq!(index_of(&MODES, "spec"), Some(3), "un debut sans ambiguite suffit");
        assert_eq!(index_of(&MODES, "s"), None, "survie ou spectateur ?");
        assert_eq!(index_of(&MODES, "2"), Some(2));
        assert_eq!(index_of(&DIFFICULTIES, "paisible"), Some(0));
        assert_eq!(hour_from("midi"), Some(12.0));
        assert_eq!(hour_from("17.5"), Some(17.5));
        assert_eq!(hour_from("99"), None);
        assert_eq!(mob_from("Oni"), Some(mk::ZOMBIE));
        // Chaque objet a un mot qui le retrouve.
        assert_eq!(item_word(items::DIAMOND), "diamant");
        assert_eq!(item_from("diamant"), Some(items::DIAMOND));
        assert_eq!(item_from("lingot_d_or"), Some(items::GOLD_INGOT));
        assert_eq!(item_from(&items::DIAMOND.to_string()), Some(items::DIAMOND));
        assert_eq!(item_from("licorne"), None);
        for id in all_items() {
            let word = item_word(id);
            assert!(!word.is_empty() && !word.contains(' '), "mot invalide pour l'objet {id}");
            assert!(item_from(&word).is_some());
        }
    }

    #[test]
    fn suggestions_follow_what_is_typed() {
        let players = vec!["Aiko".to_string(), "Ren".to_string()];
        // Pas une commande : rien.
        assert!(suggest("bonjour", &players, true).is_empty());
        // Juste « / » : toutes les commandes ; sans triche, seulement les autres.
        assert_eq!(suggest("/", &players, true).len(), COMMANDS.len());
        let free = suggest("/", &players, false);
        assert!(free.contains(&"aide".to_string()) && !free.contains(&"donner".to_string()));
        assert_eq!(suggest("/di", &players, true), ["difficulte", "dire"]);
        // Apres la commande : les valeurs de l'argument.
        assert_eq!(suggest("/mode ", &players, true), MODES);
        assert_eq!(suggest("/mode cr", &players, true), ["creatif"]);
        assert_eq!(suggest("/mode creatif ", &players, true), players);
        assert_eq!(suggest("/mode creatif a", &players, true), ["Aiko"]);
        assert!(suggest("/mode creatif", &players, true).is_empty(), "deja complet");
        assert!(suggest("/donner katana", &players, true).iter().all(|w| w.contains("katana")));
        assert!(suggest("/donner katana", &players, true).len() >= 4);
        assert!(suggest("/graine ", &players, true).is_empty());
        // La completion remplace le mot en cours.
        assert_eq!(complete("/mo", "mode"), "/mode ");
        assert_eq!(complete("/mode cr", "creatif"), "/mode creatif ");
        assert_eq!(complete("/mode ", "survie"), "/mode survie ");
        assert!(hint("/mode ").unwrap().starts_with("/mode <mode> [joueur]"));
        assert!(hint("/mo").is_none());
    }
}
