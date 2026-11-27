//! Ce qu'un serveur exige des comptes des joueurs (voir `shared::account`).
use shared::account;

/// Nom du fichier de reglages, a cote du serveur dedie.
pub const FILE: &str = "comptes.txt";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Accounts {
    /// Sans compte verifie, on n'entre pas.
    pub required: bool,
    /// Cle publique du service de comptes ; sans elle, aucun compte ne peut etre verifie.
    pub key: Option<[u8; 32]>,
    /// Adresses publiques du serveur. Si la liste n'est pas vide, un joueur n'est admis que s'il
    /// a compose l'une d'elles : un autre serveur ne peut pas relayer sa connexion jusqu'ici.
    pub addresses: Vec<String>,
    /// Accepte les jetons perimes : partie locale, ou l'on doit pouvoir jouer sans reseau.
    pub lenient: bool,
}

impl Accounts {
    /// Serveur dedie sans fichier de reglages : comptes facultatifs.
    pub fn open() -> Accounts {
        Accounts { key: account::service_key(), ..Accounts::default() }
    }

    /// Partie solo : le joueur garde son compte s'il en a un, meme avec un jeton perime.
    pub fn local() -> Accounts {
        Accounts { lenient: true, ..Accounts::open() }
    }

    /// Lit `comptes.txt` ; les lignes inconnues et les commentaires (#) sont ignores.
    pub fn parse(text: &str) -> Accounts {
        let mut a = Accounts::open();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("");
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "comptes" => a.required = v.starts_with("oblig"),
                "cle" => a.key = account::parse_key(v).or(a.key),
                "adresse" if !v.is_empty() => a.addresses.push(account::normal_address(v)),
                _ => {}
            }
        }
        a
    }

    /// Fichier d'origine, ecrit au premier demarrage du serveur dedie.
    pub fn template() -> &'static str {
        "# Comptes des joueurs sur ce serveur.\n\
         #\n\
         # comptes=facultatifs : tout le monde entre ; un joueur sans compte est reconnu a son seul pseudo.\n\
         # comptes=obligatoires : seuls les joueurs connectes a leur compte Epicube Craft entrent.\n\
         comptes=facultatifs\n\
         #\n\
         # Cle publique du service de comptes (inutile si le jeu la connait deja).\n\
         # cle=\n\
         #\n\
         # Adresse publique du serveur, telle que les joueurs la composent. Une ligne par adresse.\n\
         # Renseignee, elle empeche un autre serveur de relayer la connexion d'un joueur jusqu'ici.\n\
         # adresse=jeu.exemple.fr:25600\n"
    }

    /// Pourquoi ces reglages ne peuvent pas fonctionner, s'il y a lieu.
    pub fn problem(&self) -> Option<&'static str> {
        (self.required && self.key.is_none()).then_some("les comptes sont obligatoires mais aucune clé du service de comptes n'est connue : personne ne pourrait entrer")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lecture() {
        let key = account::public_of(&[5u8; 32]);
        let text = format!("# essai\ncomptes = obligatoires\ncle={}\nadresse= Jeu.Exemple.fr # la vraie\nadresse=10.0.0.2:7000\n", account::format_key(&key));
        let a = Accounts::parse(&text);
        assert!(a.required && !a.lenient);
        assert_eq!(a.key, Some(key));
        assert_eq!(a.addresses, ["jeu.exemple.fr:25600", "10.0.0.2:7000"]);
        assert_eq!(a.problem(), None);
        // Le fichier d'origine laisse entrer tout le monde.
        assert!(!Accounts::parse(Accounts::template()).required);
        assert!(Accounts { required: true, ..Accounts::default() }.problem().is_some());
    }
}
