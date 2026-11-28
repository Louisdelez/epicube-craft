//! Valeurs échangées entre le jeu et un mod, et leur forme en octets.

/// Une valeur : rien, un booléen, un nombre ou un texte.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Nil,
    Bool(bool),
    Num(f64),
    Str(String),
}

impl From<f64> for Val {
    fn from(v: f64) -> Val {
        Val::Num(v)
    }
}
impl From<bool> for Val {
    fn from(v: bool) -> Val {
        Val::Bool(v)
    }
}
impl From<&str> for Val {
    fn from(v: &str) -> Val {
        Val::Str(v.to_string())
    }
}
impl From<String> for Val {
    fn from(v: String) -> Val {
        Val::Str(v)
    }
}

/// Octets d'une liste de valeurs.
pub fn encode(vals: &[Val]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 * vals.len());
    for v in vals {
        match v {
            Val::Nil => out.push(0),
            Val::Bool(b) => out.extend([1, *b as u8]),
            Val::Num(n) => {
                out.push(2);
                out.extend(n.to_le_bytes());
            }
            Val::Str(s) => {
                out.push(3);
                out.extend((s.len() as u32).to_le_bytes());
                out.extend(s.as_bytes());
            }
        }
    }
    out
}

/// Relit une liste de valeurs ; None si les octets ne sont pas valides.
pub fn decode(mut data: &[u8]) -> Option<Vec<Val>> {
    let mut out = Vec::new();
    while let Some((tag, rest)) = data.split_first() {
        data = rest;
        out.push(match tag {
            0 => Val::Nil,
            1 => {
                let (b, rest) = data.split_first()?;
                data = rest;
                Val::Bool(*b != 0)
            }
            2 => {
                let (n, rest) = data.split_first_chunk::<8>()?;
                data = rest;
                Val::Num(f64::from_le_bytes(*n))
            }
            3 => {
                let (n, rest) = data.split_first_chunk::<4>()?;
                let len = u32::from_le_bytes(*n) as usize;
                if rest.len() < len {
                    return None;
                }
                let (s, rest) = rest.split_at(len);
                data = rest;
                Val::Str(String::from_utf8_lossy(s).into_owned())
            }
            _ => return None,
        });
    }
    Some(out)
}

/// Réserve `len` octets dans la mémoire du mod, pour que le jeu y écrive.
pub fn alloc(len: u32) -> u32 {
    let buf = vec![0u8; len as usize].into_boxed_slice();
    Box::into_raw(buf) as *mut u8 as usize as u32
}

/// Reprend des octets réservés par `alloc`.
///
/// # Safety
/// `ptr` et `len` doivent venir d'un appel à `alloc(len)`, et n'être repris qu'une fois.
pub unsafe fn take(ptr: u32, len: u32) -> Vec<u8> {
    let slice = std::ptr::slice_from_raw_parts_mut(ptr as usize as *mut u8, len as usize);
    Box::from_raw(slice).into_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_survive_the_trip() {
        let vals = vec![Val::Nil, Val::Bool(true), Val::Num(-12.5), Val::Str("été ✓".into()), Val::Str(String::new())];
        assert_eq!(decode(&encode(&vals)), Some(vals));
        assert_eq!(decode(&[3, 9, 0, 0, 0, b'a']), None);
        assert_eq!(decode(&[7]), None);
    }
}
