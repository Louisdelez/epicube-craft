//! Un chunk : colonne de 16 x 384 x 16 blocs, avec une metadonnee par bloc.
//! Comme dans Minecraft, le monde fait 384 blocs de haut : 120 sous le niveau de la mer, 264 au-dessus.
use crate::blocks::B;

pub const H: usize = 384;
/// Hauteur des chunks des premieres versions, encore lus dans les anciennes sauvegardes.
pub const LEGACY_H: usize = 128;
/// De combien la surface a ete remontee quand le monde est passe a 384 blocs.
pub const LEGACY_SHIFT: usize = 64;
pub const VOL: usize = 16 * 16 * H;
pub const SEA: i32 = 120;

#[inline]
pub fn idx(x: usize, y: usize, z: usize) -> usize {
    (y * 16 + z) * 16 + x
}

#[derive(Clone)]
pub struct Chunk {
    pub b: Vec<u8>,
    pub m: Vec<u8>,
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunk {
    pub fn new() -> Self {
        Chunk { b: vec![0; VOL], m: vec![0; VOL] }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> B {
        self.b[idx(x, y, z)]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, b: B) {
        self.b[idx(x, y, z)] = b;
    }

    /// Bloc non vide le plus haut de la colonne, ou None.
    pub fn top(&self, x: usize, z: usize) -> Option<usize> {
        (0..H).rev().find(|&y| self.get(x, y, z) != 0)
    }

    /// Compression par plages (valeur u8, longueur u16) des blocs puis des metadonnees.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4096);
        rle(&self.b, &mut out);
        rle(&self.m, &mut out);
        out
    }

    /// Lit un chunk de 128 blocs de haut et le replace dans un chunk actuel, remonte de `shift` blocs.
    pub fn decode_legacy(data: &[u8], shift: usize) -> Option<Chunk> {
        let n = 16 * 16 * LEGACY_H;
        let (mut b, mut m) = (vec![0u8; n], vec![0u8; n]);
        let mut pos = unrle(data, 0, &mut b)?;
        pos = unrle(data, pos, &mut m)?;
        if pos != data.len() || (shift + LEGACY_H) > H {
            return None;
        }
        let mut c = Chunk::new();
        let at = shift * 256;
        c.b[at..at + n].copy_from_slice(&b);
        c.m[at..at + n].copy_from_slice(&m);
        if shift > 0 {
            // L'ancien plancher de roche-mere devient de la pierre : le nouveau est tout en bas.
            for i in at..at + 256 {
                if c.b[i] == crate::blocks::BEDROCK {
                    c.b[i] = crate::blocks::STONE;
                }
            }
            for y in 1..shift {
                c.b[y * 256..(y + 1) * 256].fill(crate::blocks::SLATE);
            }
            c.b[..256].fill(crate::blocks::BEDROCK);
        }
        Some(c)
    }

    pub fn decode(data: &[u8]) -> Option<Chunk> {
        let mut c = Chunk::new();
        let mut pos = 0;
        pos = unrle(data, pos, &mut c.b)?;
        pos = unrle(data, pos, &mut c.m)?;
        if pos == data.len() {
            Some(c)
        } else {
            None
        }
    }
}

fn rle(src: &[u8], out: &mut Vec<u8>) {
    let mut i = 0;
    while i < src.len() {
        let v = src[i];
        let mut n = 1usize;
        while i + n < src.len() && src[i + n] == v && n < 65535 {
            n += 1;
        }
        out.push(v);
        out.extend_from_slice(&(n as u16).to_le_bytes());
        i += n;
    }
}

fn unrle(data: &[u8], mut pos: usize, dst: &mut [u8]) -> Option<usize> {
    let mut filled = 0;
    while filled < dst.len() {
        if pos + 3 > data.len() {
            return None;
        }
        let v = data[pos];
        let n = u16::from_le_bytes([data[pos + 1], data[pos + 2]]) as usize;
        pos += 3;
        if n == 0 || filled + n > dst.len() {
            return None;
        }
        dst[filled..filled + n].fill(v);
        filled += n;
    }
    Some(pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let c = crate::gen::generate(0, 42, 3, -2);
        let d = Chunk::decode(&c.encode()).expect("decode");
        assert_eq!(c.b, d.b);
        assert_eq!(c.m, d.m);
    }

    #[test]
    fn old_saves_are_lifted() {
        // Un chunk de l'ancien format : de la terre au niveau 60, un coffre pose dessus.
        let n = 16 * 16 * LEGACY_H;
        let (mut b, m) = (vec![0u8; n], vec![0u8; n]);
        b[..256].fill(crate::blocks::BEDROCK);
        b[60 * 256 + 5 * 16 + 3] = crate::blocks::DIRT;
        b[61 * 256 + 5 * 16 + 3] = crate::blocks::CHEST;
        let mut data = Vec::new();
        rle(&b, &mut data);
        rle(&m, &mut data);
        assert!(Chunk::decode(&data).is_none(), "ce n'est pas le format actuel");
        let c = Chunk::decode_legacy(&data, LEGACY_SHIFT).expect("ancien format");
        assert_eq!(c.get(3, 124, 5), crate::blocks::DIRT);
        assert_eq!(c.get(3, 125, 5), crate::blocks::CHEST);
        assert_eq!(c.get(0, 0, 0), crate::blocks::BEDROCK);
        assert_eq!(c.get(0, 30, 0), crate::blocks::SLATE, "le sous-sol ajoute est plein");
        // Les autres mondes ne sont pas remontes.
        let same = Chunk::decode_legacy(&data, 0).unwrap();
        assert_eq!(same.get(3, 60, 5), crate::blocks::DIRT);
    }

    #[test]
    fn rejects_garbage() {
        assert!(Chunk::decode(&[1, 2, 3, 4]).is_none());
    }
}
