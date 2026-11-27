//! Regles des circuits de reiseki, communes au serveur (qui les simule) et au client (qui les dessine).
//!
//! Le reiseki est une pierre d'esprit dont la poudre transporte le ki. Le modele :
//! - un signal a une force de 0 a 15 ; un fil de poudre perd 1 par bloc ;
//! - un bloc plein opaque est un « conducteur » : il peut etre charge ;
//! - une charge *forte* (lanterne hitodama dessous, relais ou balance en face, levier accroche)
//!   alimente les fils voisins ; une charge *faible* (fil pose dessus ou pointe vers lui)
//!   ne fait qu'activer les mecanismes, relais et lanternes qui le touchent ;
//! - un fil ne donne son ki qu'au bloc qu'il recouvre et a ceux vers lesquels il pointe.
use crate::blocks::*;

pub type P3 = [i32; 3];
/// Lecture d'un bloc : (identifiant, metadonnee) ; (AIR, 0) hors du monde charge.
pub type Get<'a> = &'a dyn Fn(P3) -> (B, u8);

pub const DIRS4: [P3; 4] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
/// Meme ordre que les faces des cubes : +X, -X, +Y, -Y, +Z, -Z.
pub const DIRS6: [P3; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
/// Directions d'accroche (du bloc vers son support) : sol, quatre murs, plafond.
pub const ATTACH: [P3; 6] = [[0, -1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0]];
const UP: P3 = [0, 1, 0];
const DOWN: P3 = [0, -1, 0];

pub fn add(p: P3, d: P3) -> P3 {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

pub fn neg(d: P3) -> P3 {
    [-d[0], -d[1], -d[2]]
}

/// Direction du support d'un levier, d'un sceau ou d'une lanterne hitodama (bits 1 a 3).
pub fn attach_of(m: u8) -> P3 {
    ATTACH[(((m >> 1) & 7) as usize).min(5)]
}

pub fn attach_index(d: P3) -> u8 {
    ATTACH.iter().position(|a| *a == d).unwrap_or(0) as u8
}

/// Sens de sortie d'un relais ou d'une balance (bits 0 et 1).
pub fn facing4(m: u8) -> P3 {
    DIRS4[(m & 3) as usize]
}

/// Orientation d'un poussoir, d'un daruma ou d'un distributeur (bits 0 a 2).
pub fn facing6(m: u8) -> P3 {
    DIRS6[((m & 7) as usize).min(5)]
}

/// Indice dans DIRS4 de la direction horizontale dominante d'un vecteur.
pub fn index4(v: [f64; 3]) -> u8 {
    if v[0].abs() >= v[2].abs() {
        (v[0] < 0.0) as u8
    } else {
        2 + (v[2] < 0.0) as u8
    }
}

/// Indice dans DIRS6 de la direction dominante d'un vecteur.
pub fn index6(v: [f64; 3]) -> u8 {
    let (ax, ay, az) = (v[0].abs(), v[1].abs(), v[2].abs());
    if ay > ax && ay > az {
        2 + (v[1] < 0.0) as u8
    } else if ax >= az {
        (v[0] < 0.0) as u8
    } else {
        4 + (v[2] < 0.0) as u8
    }
}

/// Bloc qui peut etre charge et transmettre sa charge.
pub fn conductor(b: B) -> bool {
    opaque(b) && !matches!(b, VOLT_BLOCK | PISTON | STICKY_PISTON | OBSERVER | CHEST | EXPLOSIVE | ICE | HOPPER | TARGET)
}

/// Liens d'un fil vers ses quatre cotes : 0 aucun, 1 a plat, 2 en montant sur le bloc voisin.
pub fn links(get: Get, p: P3) -> [u8; 4] {
    let mut out = [0u8; 4];
    let roof = conductor(get(add(p, UP)).0);
    for (k, d) in DIRS4.iter().enumerate() {
        let q = add(p, *d);
        let (b, m) = get(q);
        let side = match b {
            WIRE | LEVER | BUTTON | SIGTORCH | VOLT_BLOCK | PLATE | SUNDIAL | COMPARATOR | HOOK | TARGET | DETECTOR_RAIL => true,
            REPEATER => facing4(m) == *d || facing4(m) == neg(*d),
            // Le daruma ne se relie que par son dos, d'ou sort le signal.
            OBSERVER => facing6(m) == *d,
            _ => false,
        };
        if side || (!conductor(b) && get(add(q, DOWN)).0 == WIRE) {
            out[k] = 1;
        }
        if !roof && get(add(q, UP)).0 == WIRE {
            out[k] = 2;
        }
    }
    out
}

/// Cotes vers lesquels le fil pointe : seul, il forme une croix ; avec un seul lien, une ligne droite.
pub fn pointed(l: [u8; 4]) -> [bool; 4] {
    let mut on = [l[0] > 0, l[1] > 0, l[2] > 0, l[3] > 0];
    match on.iter().filter(|x| **x).count() {
        0 => on = [true; 4],
        1 => {
            let k = on.iter().position(|x| *x).unwrap_or(0);
            on[k ^ 1] = true;
        }
        _ => {}
    }
    on
}

/// Fils relies a celui-ci : a plat, une marche plus haut ou une marche plus bas.
pub fn wire_neighbors(get: Get, p: P3) -> Vec<P3> {
    let mut out = Vec::with_capacity(4);
    let roof = conductor(get(add(p, UP)).0);
    for d in DIRS4 {
        let q = add(p, d);
        let b = get(q).0;
        if b == WIRE {
            out.push(q);
            continue;
        }
        if !conductor(b) && get(add(q, DOWN)).0 == WIRE {
            out.push(add(q, DOWN));
        }
        if !roof && get(add(q, UP)).0 == WIRE {
            out.push(add(q, UP));
        }
    }
    out
}

/// Force emise par le composant en `q` vers la direction `d` (les fils sont traites a part).
pub fn emit_to(get: Get, q: P3, d: P3) -> u8 {
    let (b, m) = get(q);
    match b {
        LEVER | BUTTON | PLATE | HOOK if m & 1 == 1 => 15,
        DETECTOR_RAIL if m & 8 != 0 => 15,
        TARGET => m & 15,
        // La lanterne ne charge jamais le bloc qui la porte.
        SIGTORCH if m & 1 == 1 && attach_of(m) != d => 15,
        VOLT_BLOCK => 15,
        SUNDIAL => m & 15,
        REPEATER if m & 16 != 0 && facing4(m) == d => 15,
        COMPARATOR if facing4(m) == d => m >> 4,
        OBSERVER if m & 8 != 0 && facing6(m) == neg(d) => 15,
        _ => 0,
    }
}

/// Cette emission charge-t-elle *fortement* un conducteur place en `q + d` ?
pub fn strong_to(get: Get, q: P3, d: P3) -> bool {
    let (b, m) = get(q);
    match b {
        LEVER | BUTTON | HOOK => attach_of(m) == d,
        PLATE | DETECTOR_RAIL => d == DOWN,
        SIGTORCH => d == UP,
        REPEATER | COMPARATOR | OBSERVER => true,
        _ => false,
    }
}

/// Ki qu'un fil en `q` donne vers `d` : au bloc qu'il recouvre et a ceux vers lesquels il pointe.
pub fn wire_to(get: Get, q: P3, d: P3) -> u8 {
    let m = get(q).1 & 15;
    if m == 0 || d == UP {
        return 0;
    }
    if d == DOWN {
        return m;
    }
    let on = pointed(links(get, q));
    match DIRS4.iter().position(|x| *x == d) {
        Some(k) if on[k] => m,
        _ => 0,
    }
}

/// Charge (forte, faible) du conducteur en `c`.
pub fn level(get: Get, c: P3) -> (u8, u8) {
    let (mut strong, mut weak) = (0u8, 0u8);
    for e in DIRS6 {
        let q = add(c, e);
        let d = neg(e);
        if get(q).0 == WIRE {
            weak = weak.max(wire_to(get, q, d));
        } else if strong_to(get, q, d) {
            strong = strong.max(emit_to(get, q, d));
        }
    }
    (strong, weak)
}

/// Force qui arrive en `p` depuis son voisin dans la direction `e`.
pub fn input(get: Get, p: P3, e: P3) -> u8 {
    let q = add(p, e);
    let b = get(q).0;
    if b == WIRE {
        wire_to(get, q, neg(e))
    } else if conductor(b) {
        let (s, w) = level(get, q);
        s.max(w)
    } else {
        emit_to(get, q, neg(e))
    }
}

/// Force maximale recue par un mecanisme, toutes faces confondues.
pub fn powered(get: Get, p: P3) -> u8 {
    DIRS6.iter().map(|e| input(get, p, *e)).max().unwrap_or(0)
}

/// Force qu'un fil recoit de ce qui n'est pas un fil : composants voisins et blocs fortement charges.
pub fn wire_source(get: Get, p: P3) -> u8 {
    let mut best = 0u8;
    for e in DIRS6 {
        let q = add(p, e);
        let b = get(q).0;
        let s = if b == WIRE {
            0
        } else if conductor(b) {
            level(get, q).0
        } else {
            emit_to(get, q, neg(e))
        };
        best = best.max(s);
    }
    best
}

/// Ce que devient un bloc quand un poussoir karakuri le rencontre.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Push {
    /// Rien a pousser : la course s'arrete la.
    Empty,
    Move,
    /// Se detache et tombe en objet.
    Break,
    /// Bloque le poussoir.
    Fixed,
}

pub fn push_kind(b: B, m: u8) -> Push {
    match b {
        AIR | WATER | LAVA => Push::Empty,
        BEDROCK | OBSIDIAN | CHEST | FURNACE | DROPPER | HOPPER | SUNDIAL | SPAWNER | GATEWAY | ABYSS_PORTAL | ZENITH_PORTAL | PISTON_HEAD => Push::Fixed,
        PISTON | STICKY_PISTON if m & 8 != 0 => Push::Fixed,
        // Feux d'artifice deja amorces.
        EXPLOSIVE if m != 0 => Push::Fixed,
        DOOR | BED | CACTUS => Push::Break,
        _ if shape(b) != Shape::Cube => Push::Break,
        _ => Push::Move,
    }
}

/// Force lue par une balance onmyo sur un conteneur : 0 s'il est vide, sinon de 1 a 15 selon son remplissage.
pub fn fullness(slots: &[crate::items::Slot]) -> u8 {
    if slots.is_empty() {
        return 0;
    }
    let sum: f32 = slots.iter().flatten().map(|s| s.n as f32 / crate::items::max_stack(s.id) as f32).sum();
    if sum <= 0.0 {
        0
    } else {
        (1.0 + sum / slots.len() as f32 * 14.0).floor() as u8
    }
}

/// Les trois sortes de rails.
pub fn is_rail(b: B) -> bool {
    matches!(b, RAIL | POWERED_RAIL | DETECTOR_RAIL)
}

/// Les deux bouts d'un rail selon sa forme (bits 0 a 2) : droit en X, droit en Z, puis les quatre virages.
pub fn rail_dirs(m: u8) -> [P3; 2] {
    match m & 7 {
        0 => [[1, 0, 0], [-1, 0, 0]],
        1 => [[0, 0, 1], [0, 0, -1]],
        2 => [[1, 0, 0], [0, 0, 1]],
        3 => [[1, 0, 0], [0, 0, -1]],
        4 => [[-1, 0, 0], [0, 0, 1]],
        _ => [[-1, 0, 0], [0, 0, -1]],
    }
}

/// Forme que prend un rail d'apres ses voisins ; seul le rail simple sait tourner.
pub fn rail_shape(get: Get, p: P3) -> u8 {
    let (b, m) = get(p);
    let n: Vec<bool> = DIRS4.iter().map(|d| is_rail(get(add(p, *d)).0)).collect();
    let (xs, zs) = (n[0] || n[1], n[2] || n[3]);
    let cur = if m & 7 < 2 { m & 7 } else { 0 };
    if b == RAIL && xs && zs && !(n[0] && n[1]) && !(n[2] && n[3]) {
        match (n[0], n[2]) {
            (true, true) => 2,
            (true, false) => 3,
            (false, true) => 4,
            (false, false) => 5,
        }
    } else if xs && !zs {
        0
    } else if zs && !xs {
        1
    } else if b == RAIL && !xs && !zs && m & 7 >= 2 {
        m & 7
    } else {
        cur
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn world(cells: &[(P3, B, u8)]) -> HashMap<P3, (B, u8)> {
        cells.iter().map(|(p, b, m)| (*p, (*b, *m))).collect()
    }

    #[test]
    fn wire_shapes() {
        // Un fil seul forme une croix ; avec un voisin, une ligne.
        let w = world(&[([0, 1, 0], WIRE, 0)]);
        let get = |p: P3| w.get(&p).copied().unwrap_or((AIR, 0));
        assert_eq!(pointed(links(&get, [0, 1, 0])), [true; 4]);
        let w = world(&[([0, 1, 0], WIRE, 0), ([1, 1, 0], WIRE, 0)]);
        let get = |p: P3| w.get(&p).copied().unwrap_or((AIR, 0));
        assert_eq!(pointed(links(&get, [0, 1, 0])), [true, true, false, false]);
        // Un fil une marche plus haut se relie, sauf si un conducteur couvre le fil du bas.
        let mut w = world(&[([0, 1, 0], WIRE, 0), ([1, 1, 0], STONE, 0), ([1, 2, 0], WIRE, 0)]);
        {
            let get = |p: P3| w.get(&p).copied().unwrap_or((AIR, 0));
            assert_eq!(links(&get, [0, 1, 0])[0], 2);
            assert_eq!(wire_neighbors(&get, [0, 1, 0]), vec![[1, 2, 0]]);
            assert_eq!(wire_neighbors(&get, [1, 2, 0]), vec![[0, 1, 0]]);
        }
        w.insert([0, 2, 0], (STONE, 0));
        let get = |p: P3| w.get(&p).copied().unwrap_or((AIR, 0));
        assert!(wire_neighbors(&get, [0, 1, 0]).is_empty());
    }

    #[test]
    fn strong_and_weak() {
        // Fil charge pose sur une pierre : charge faible. Un fil voisin n'en recoit rien, une lampe si.
        let w = world(&[([0, 1, 0], STONE, 0), ([0, 2, 0], WIRE, 9), ([1, 1, 0], LAMP, 0), ([0, 1, 1], WIRE, 0)]);
        let get = |p: P3| w.get(&p).copied().unwrap_or((AIR, 0));
        assert_eq!(level(&get, [0, 1, 0]), (0, 9));
        assert_eq!(powered(&get, [1, 1, 0]), 9);
        assert_eq!(wire_source(&get, [0, 1, 1]), 0);
        // Relais allume face a la pierre : charge forte, le fil voisin la recoit.
        let w = world(&[([0, 1, 0], STONE, 0), ([-1, 1, 0], REPEATER, 16), ([0, 1, 1], WIRE, 0)]);
        let get = |p: P3| w.get(&p).copied().unwrap_or((AIR, 0));
        assert_eq!(level(&get, [0, 1, 0]), (15, 0));
        assert_eq!(wire_source(&get, [0, 1, 1]), 15);
    }

    #[test]
    fn container_reading() {
        use crate::items::Stack;
        let mut slots = vec![None; 27];
        assert_eq!(fullness(&slots), 0);
        slots[0] = Some(Stack::new(DIRT as u16, 1));
        assert_eq!(fullness(&slots), 1);
        for s in slots.iter_mut() {
            *s = Some(Stack::new(DIRT as u16, 64));
        }
        assert_eq!(fullness(&slots), 15);
    }
}
