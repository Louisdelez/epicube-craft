//! Graines de monde. Comme dans Minecraft : une graine est un entier de 64 bits ; on peut la saisir
//! en chiffres, ou en toutes lettres (le texte est alors reduit a un nombre, toujours le meme pour
//! un meme texte) ; laissee vide, elle est tiree au hasard. A graine egale, monde identique.

/// Lit une graine saisie. None si le champ est vide : il faudra en tirer une au hasard.
pub fn parse(text: &str) -> Option<i64> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if let Ok(n) = t.parse::<i64>() {
        return Some(n);
    }
    // Texte : la meme reduction que Java (somme ponderee par 31 des unites UTF-16, sur 32 bits).
    let mut h: i32 = 0;
    for unit in t.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(unit as i32);
    }
    Some(h as i64)
}

/// Graine au hasard, tiree de l'horloge.
pub fn random() -> i64 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0x9E37_79B9);
    // Un compteur garantit deux graines differentes meme tirees dans le meme instant.
    static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let turn = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut z = now ^ turn.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(21);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) as i64
}

/// Reduit la graine aux 32 bits dont se nourrit la generation. Les petites graines positives
/// passent telles quelles (les mondes d'avant gardent leur graine) ; les autres sont brassees.
pub fn fold(seed: i64) -> u32 {
    if (0..=u32::MAX as i64).contains(&seed) {
        return seed as u32;
    }
    let mut z = seed as u64;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z as u32) ^ ((z >> 32) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_text_and_empty() {
        assert_eq!(parse("  42 "), Some(42));
        assert_eq!(parse("-9000000000000000000"), Some(-9_000_000_000_000_000_000));
        assert_eq!(parse(""), None);
        // Meme valeur que String.hashCode() de Java.
        assert_eq!(parse("Voxterra"), Some("Voxterra".chars().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32)) as i64));
        assert_eq!(parse("abc"), Some(96354));
        assert_eq!(parse("sakura"), parse("sakura"));
        assert_ne!(parse("sakura"), parse("Sakura"));
        // Les petites graines sont inchangees ; les grandes et les negatives se repartissent.
        assert_eq!(fold(1337), 1337);
        assert_ne!(fold(-1), fold(-2));
        assert_ne!(fold(1 << 40), fold(1 << 41));
        assert_ne!(random(), random());
    }
}
