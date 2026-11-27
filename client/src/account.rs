//! Compte du joueur, tel que le launcher le remet au jeu (voir `shared::account`).
use shared::account;

pub struct Account {
    /// Jeton signe par le service de comptes, presente a chaque serveur.
    pub token: String,
    pub name: String,
    pub id: account::Id,
    /// Cle privee de session : elle ne quitte jamais le jeu, qui signe avec elle les defis des serveurs.
    pub secret: [u8; 32],
    /// Le jeton a passe sa date : les serveurs en ligne le refuseront, la partie solo l'accepte.
    pub expired: bool,
}

/// Lit le compte dans l'environnement. Ok(None) : le jeu a ete lance sans launcher, on joue hors ligne.
pub fn from_env() -> Result<Option<Account>, String> {
    let Ok(token) = std::env::var(account::TOKEN_ENV) else { return Ok(None) };
    let secret = std::env::var(account::SESSION_ENV).ok().and_then(|k| account::parse_key(&k)).ok_or("clé de session absente ou illisible")?;
    let claims = account::peek(&token).ok_or("jeton illisible")?;
    if account::public_of(&secret) != claims.key {
        return Err("la clé de session ne correspond pas au jeton".into());
    }
    Ok(Some(Account { token, name: claims.name, id: claims.id, secret, expired: account::now() > claims.expires }))
}
