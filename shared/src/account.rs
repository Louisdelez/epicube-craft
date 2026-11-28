//! Comptes des joueurs : identifiant fixe, jeton signe par le service de comptes, preuve de possession.
//!
//! Le jeu ne voit jamais de mot de passe. Le launcher se connecte au service de comptes, qui lui remet
//! un jeton signe (un JWT, algorithme EdDSA) ; le serveur de jeu le verifie avec la seule cle publique
//! du service, sans le contacter. Le jeton nomme une cle de session dont seul le joueur detient la
//! partie privee : il signe avec elle le defi que lui lance le serveur, si bien qu'un jeton vu passer
//! ne suffit pas a se faire passer pour lui. Le format complet est decrit dans `docs/comptes.md`.
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Identifiant d'un compte : un UUID, qui ne change jamais meme si le joueur change de pseudo.
pub type Id = [u8; 16];

/// Cle publique du service de comptes, connue de tous les serveurs : celle d'Epicube Hub, qui
/// signe les jetons sur https://epicube.ch (en base64url : THh5h9HFJF1KewRbIEqRmSEkOY3yse4-EOk-eHwne0o,
/// lisible sur https://epicube.ch/api/auth/service-key).
/// `comptes.txt` ou la variable EPICUBE_CLE_COMPTES permettent d'en utiliser une autre.
#[rustfmt::skip]
pub const SERVICE_KEY: Option<[u8; 32]> = Some([
    0x4c, 0x78, 0x79, 0x87, 0xd1, 0xc5, 0x24, 0x5d,
    0x4a, 0x7b, 0x04, 0x5b, 0x20, 0x4a, 0x91, 0x99,
    0x21, 0x24, 0x39, 0x8d, 0xf2, 0xb1, 0xee, 0x3e,
    0x10, 0xe9, 0x3e, 0x78, 0x7c, 0x27, 0x7b, 0x4a,
]);
/// Variable d'environnement qui remplace `SERVICE_KEY` (cle publique en base64url ou en hexadecimal).
pub const KEY_ENV: &str = "EPICUBE_CLE_COMPTES";
/// Variables par lesquelles le launcher remet au jeu le jeton et la cle privee de session.
pub const TOKEN_ENV: &str = "EPICUBE_JETON";
pub const SESSION_ENV: &str = "EPICUBE_CLE_SESSION";

/// Longueur maximale d'un pseudo.
pub const NAME_MAX: usize = 16;
/// Ecart d'horloge tolere entre le service et le serveur, en secondes.
const SKEW: u64 = 120;
/// En-tete du message que le joueur signe pour prouver qu'il detient la cle de session.
const PROOF_TAG: &[u8] = b"EpicubeCraft preuve 1\n";

/// Ecrit un identifiant sous la forme habituelle : 8-4-4-4-12 chiffres hexadecimaux.
pub fn format_id(id: &Id) -> String {
    let h: String = id.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// Lit un identifiant, avec ou sans tirets.
pub fn parse_id(text: &str) -> Option<Id> {
    let hex: Vec<u8> = text.bytes().filter(|b| *b != b'-').collect();
    if hex.len() != 32 {
        return None;
    }
    let mut id = [0u8; 16];
    for (i, pair) in hex.chunks(2).enumerate() {
        id[i] = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(id)
}

/// Identifiant d'un joueur sans compte, tire de son pseudo. Il porte le numero de version 3, que le
/// service de comptes n'attribue jamais : un compte ne peut donc pas tomber sur un identifiant hors ligne.
pub fn offline_id(name: &str) -> Id {
    let hash = Sha256::digest(format!("EpicubeCraft hors ligne:{name}").as_bytes());
    let mut id = [0u8; 16];
    id.copy_from_slice(&hash[..16]);
    id[6] = (id[6] & 0x0f) | 0x30;
    id[8] = (id[8] & 0x3f) | 0x80;
    id
}

pub fn is_offline(id: &Id) -> bool {
    id[6] >> 4 == 3
}

/// Ne garde d'un pseudo que ce qui s'affiche, dans la limite de longueur.
pub fn clean_name(name: &str) -> String {
    name.chars().filter(|c| !c.is_control()).take(NAME_MAX).collect()
}

/// Ce que le jeton affirme, une fois sa signature verifiee.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Claims {
    pub id: Id,
    pub name: String,
    /// Cle publique de session : le joueur doit prouver qu'il en detient la partie privee.
    pub key: [u8; 32],
    /// Fin de validite, en secondes depuis 1970.
    pub expires: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Reject {
    /// Le texte n'a pas la forme d'un jeton.
    Malformed,
    /// Signe par une autre cle que celle du service de comptes.
    BadSignature,
    Expired,
    /// Pseudo ou identifiant inacceptable.
    BadClaims,
}

impl std::fmt::Display for Reject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Reject::Malformed => "jeton de compte illisible",
            Reject::BadSignature => "jeton de compte non reconnu par ce serveur",
            Reject::Expired => "session expirée : relancez le jeu depuis le launcher",
            Reject::BadClaims => "compte invalide",
        })
    }
}

#[derive(Serialize, Deserialize)]
struct Header {
    alg: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    typ: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Payload {
    /// Identifiant du compte.
    sub: String,
    name: String,
    exp: u64,
    /// Cle publique de session, en base64url.
    key: String,
}

fn decode32(text: &str) -> Option<[u8; 32]> {
    B64.decode(text).ok()?.try_into().ok()
}

/// Lit une cle de 32 octets ecrite en base64url (43 caracteres) ou en hexadecimal (64).
pub fn parse_key(text: &str) -> Option<[u8; 32]> {
    let text = text.trim();
    if text.len() == 64 {
        let mut key = [0u8; 32];
        for (i, pair) in text.as_bytes().chunks(2).enumerate() {
            key[i] = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
        }
        return Some(key);
    }
    decode32(text)
}

pub fn format_key(key: &[u8; 32]) -> String {
    B64.encode(key)
}

/// Cle publique du service de comptes a utiliser : celle de l'environnement, sinon celle du jeu.
pub fn service_key() -> Option<[u8; 32]> {
    std::env::var(KEY_ENV).ok().and_then(|v| parse_key(&v)).or(SERVICE_KEY)
}

/// Seconde courante depuis 1970.
pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Octets tires au hasard par le systeme.
pub fn random32() -> [u8; 32] {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("le systeme ne fournit pas de hasard");
    bytes
}

/// Lit le contenu d'un jeton sans rien verifier : pour l'afficher, jamais pour s'y fier.
pub fn peek(token: &str) -> Option<Claims> {
    let mut parts = token.trim().split('.');
    let (_, body, _) = (parts.next()?, parts.next()?, parts.next()?);
    claims_of(body).ok()
}

fn claims_of(body: &str) -> Result<Claims, Reject> {
    let payload: Payload = B64.decode(body).ok().and_then(|b| serde_json::from_slice(&b).ok()).ok_or(Reject::Malformed)?;
    let id = parse_id(&payload.sub).ok_or(Reject::BadClaims)?;
    let key = decode32(&payload.key).ok_or(Reject::BadClaims)?;
    let name = clean_name(&payload.name);
    if name.is_empty() || name != payload.name || is_offline(&id) {
        return Err(Reject::BadClaims);
    }
    Ok(Claims { id, name, key, expires: payload.exp })
}

/// Verifie un jeton avec la cle publique du service de comptes. `now` : seconde courante, ou None
/// pour ne pas regarder la date (partie locale, ou l'on doit pouvoir jouer sans reseau).
pub fn verify(token: &str, service: &[u8; 32], now: Option<u64>) -> Result<Claims, Reject> {
    let token = token.trim();
    let mut parts = token.split('.');
    let (Some(head), Some(body), Some(sig), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return Err(Reject::Malformed);
    };
    // Un seul algorithme est admis : on ne laisse pas le jeton choisir comment il est verifie.
    let header: Header = B64.decode(head).ok().and_then(|b| serde_json::from_slice(&b).ok()).ok_or(Reject::Malformed)?;
    if header.alg != "EdDSA" {
        return Err(Reject::Malformed);
    }
    let sig: [u8; 64] = B64.decode(sig).ok().and_then(|b| b.try_into().ok()).ok_or(Reject::Malformed)?;
    let key = VerifyingKey::from_bytes(service).map_err(|_| Reject::BadSignature)?;
    let signed = &token[..head.len() + 1 + body.len()];
    key.verify_strict(signed.as_bytes(), &Signature::from_bytes(&sig)).map_err(|_| Reject::BadSignature)?;
    let claims = claims_of(body)?;
    if now.is_some_and(|t| t > claims.expires.saturating_add(SKEW)) {
        return Err(Reject::Expired);
    }
    Ok(claims)
}

/// Fabrique un jeton : le travail du service de comptes. Ne sert ici qu'aux essais et a l'outil de jetons.
pub fn issue(service_secret: &[u8; 32], claims: &Claims) -> String {
    let head = B64.encode(serde_json::to_vec(&Header { alg: "EdDSA".into(), typ: Some("JWT".into()) }).unwrap_or_default());
    let payload = Payload { sub: format_id(&claims.id), name: claims.name.clone(), exp: claims.expires, key: format_key(&claims.key) };
    let body = B64.encode(serde_json::to_vec(&payload).unwrap_or_default());
    let signed = format!("{head}.{body}");
    let sig = SigningKey::from_bytes(service_secret).sign(signed.as_bytes());
    format!("{signed}.{}", B64.encode(sig.to_bytes()))
}

/// Cle publique qui correspond a une cle privee.
pub fn public_of(secret: &[u8; 32]) -> [u8; 32] {
    SigningKey::from_bytes(secret).verifying_key().to_bytes()
}

/// Met une adresse de serveur sous une seule forme : minuscules, port toujours ecrit.
pub fn normal_address(addr: &str) -> String {
    let addr = addr.trim().to_lowercase();
    // Une adresse IPv6 nue contient plusieurs « : » ; entre crochets, le port suit le crochet fermant.
    let has_port = match addr.rfind(']') {
        Some(end) => addr[end..].contains(':'),
        None => addr.matches(':').count() == 1,
    };
    if has_port {
        addr
    } else {
        format!("{addr}:{}", crate::DEFAULT_PORT)
    }
}

fn proof_message(nonce: &[u8; 32], address: &str) -> Vec<u8> {
    let mut msg = PROOF_TAG.to_vec();
    msg.extend_from_slice(nonce);
    msg.extend_from_slice(address.as_bytes());
    msg
}

/// Reponse du joueur au defi du serveur : il signe le nombre recu et l'adresse qu'il a composee.
/// Une signature obtenue par un serveur ne vaut donc rien aupres d'un autre.
pub fn prove(session_secret: &[u8; 32], nonce: &[u8; 32], address: &str) -> Vec<u8> {
    SigningKey::from_bytes(session_secret).sign(&proof_message(nonce, address)).to_bytes().to_vec()
}

/// Verifie la reponse au defi avec la cle de session inscrite dans le jeton.
pub fn check_proof(session_key: &[u8; 32], nonce: &[u8; 32], address: &str, sig: &[u8]) -> bool {
    let (Ok(key), Ok(sig)) = (VerifyingKey::from_bytes(session_key), <[u8; 64]>::try_from(sig)) else { return false };
    key.verify_strict(&proof_message(nonce, address), &Signature::from_bytes(&sig)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(secret: &[u8; 32], session: &[u8; 32], expires: u64) -> (Claims, String) {
        let claims = Claims { id: parse_id("0190a8e2-7d5c-7b1e-9f3a-5c2d8e4f6a1b").unwrap(), name: "Hanako".into(), key: public_of(session), expires };
        let token = issue(secret, &claims);
        (claims, token)
    }

    #[test]
    fn identifiants() {
        let id = offline_id("Joueur");
        assert!(is_offline(&id));
        assert_eq!(id, offline_id("Joueur"));
        assert_ne!(id, offline_id("joueur"));
        assert_eq!(parse_id(&format_id(&id)), Some(id));
        assert_eq!(parse_id("pas un identifiant"), None);
    }

    #[test]
    fn jeton_valide_puis_refuse() {
        let (service, session) = ([7u8; 32], [9u8; 32]);
        let (claims, token) = sample(&service, &session, 1000);
        let key = public_of(&service);
        assert_eq!(verify(&token, &key, Some(900)), Ok(claims.clone()));
        assert_eq!(verify(&token, &key, None), Ok(claims.clone()));
        assert_eq!(peek(&token), Some(claims));
        // Perime, signe par un autre, ou retouche apres signature.
        assert_eq!(verify(&token, &key, Some(5000)), Err(Reject::Expired));
        assert_eq!(verify(&token, &public_of(&[8u8; 32]), Some(900)), Err(Reject::BadSignature));
        let parts: Vec<&str> = token.split('.').collect();
        let forged = B64.encode(String::from_utf8(B64.decode(parts[1]).unwrap()).unwrap().replace("Hanako", "Taro"));
        assert_eq!(verify(&format!("{}.{forged}.{}", parts[0], parts[2]), &key, Some(900)), Err(Reject::BadSignature));
        assert_eq!(verify("n'importe quoi", &key, Some(900)), Err(Reject::Malformed));
    }

    #[test]
    fn jeton_sans_signature_refuse() {
        // Un jeton qui se declare « non signe » ne passe pas, meme bien forme.
        let (service, session) = ([7u8; 32], [9u8; 32]);
        let (_, token) = sample(&service, &session, 1000);
        let body = token.split('.').nth(1).unwrap();
        let head = B64.encode(br#"{"alg":"none"}"#);
        assert_eq!(verify(&format!("{head}.{body}."), &public_of(&service), Some(900)), Err(Reject::Malformed));
    }

    #[test]
    fn compte_hors_ligne_refuse_dans_un_jeton() {
        let service = [7u8; 32];
        let claims = Claims { id: offline_id("Hanako"), name: "Hanako".into(), key: public_of(&[9u8; 32]), expires: 1000 };
        assert_eq!(verify(&issue(&service, &claims), &public_of(&service), Some(900)), Err(Reject::BadClaims));
    }

    #[test]
    fn preuve() {
        let session = [9u8; 32];
        let (nonce, other) = ([3u8; 32], [4u8; 32]);
        let sig = prove(&session, &nonce, "jeu.exemple.fr:25600");
        let key = public_of(&session);
        assert!(check_proof(&key, &nonce, "jeu.exemple.fr:25600", &sig));
        // Ni pour un autre defi, ni pour un autre serveur, ni avec une autre cle.
        assert!(!check_proof(&key, &other, "jeu.exemple.fr:25600", &sig));
        assert!(!check_proof(&key, &nonce, "autre.exemple.fr:25600", &sig));
        assert!(!check_proof(&public_of(&[1u8; 32]), &nonce, "jeu.exemple.fr:25600", &sig));
        assert!(!check_proof(&key, &nonce, "jeu.exemple.fr:25600", &sig[..10]));
    }

    #[test]
    fn adresses() {
        assert_eq!(normal_address(" Jeu.Exemple.fr "), "jeu.exemple.fr:25600");
        assert_eq!(normal_address("jeu.exemple.fr:7000"), "jeu.exemple.fr:7000");
        assert_eq!(normal_address("[::1]"), "[::1]:25600");
        assert_eq!(normal_address("[::1]:7000"), "[::1]:7000");
    }

    #[test]
    fn cles() {
        let key = public_of(&[5u8; 32]);
        assert_eq!(parse_key(&format_key(&key)), Some(key));
        let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(parse_key(&hex), Some(key));
        assert_eq!(parse_key("trop court"), None);
    }
}
