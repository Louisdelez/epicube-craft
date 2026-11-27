//! Les objets, concus piece par piece.
//!
//! Unite : le seizieme de bloc. x a droite, y vers le haut (0 = la base), z vers celui qui regarde.
//! Un outil se construit debout, manche en bas ; un objet pose se construit sur son socle.
use crate::items3d::{m, ma, mp, Kit, Mat, Pat, Pose};
use shared::items::*;

// Matieres communes.
const WOOD: Mat = mp(0xA87448, Pat::Grain);
const DARKWOOD: Mat = mp(0x6E4630, Pat::Grain);
const HINOKI: Mat = mp(0xD8B27A, Pat::Grain);
const BAMBOO: Mat = ma(0xB4BC62, 0x6F7A34, Pat::Bands);
const STRAW: Mat = ma(0xDDBD6C, 0xA8843E, Pat::Bands);
const STEEL: Mat = mp(0xD2D9E2, Pat::Metal);
const IRON: Mat = mp(0x565C6A, Pat::Speck);
const GOLD: Mat = mp(0xE8B93A, Pat::Metal);
const COPPER: Mat = mp(0xC47A4A, Pat::Metal);
const RED: Mat = m(0xC0362C);
const LACQUER: Mat = mp(0x8E2A26, Pat::Metal);
const BLACK: Mat = mp(0x2C2A36, Pat::Metal);
const PAPER: Mat = m(0xF4EBD6);
const WHITE: Mat = m(0xF6F4EE);
const IVORY: Mat = mp(0xEDE6D2, Pat::Speck);
const RICE: Mat = mp(0xF7F4EA, Pat::Speck);
const SEAWEED: Mat = ma(0x24382C, 0x3E5A44, Pat::Dots);
const LEAF: Mat = mp(0x4E9A3E, Pat::Grain);
const INK: Mat = m(0x1E1C28);

/// Lame ou tete de l'outil selon le materiau : bois, pierre, fer, diamant, tamahagane.
fn head(tier: u8) -> Mat {
    [mp(0xC49A62, Pat::Grain), mp(0x8E9098, Pat::Speck), mp(0xD5DBE3, Pat::Metal), mp(0x6FE3E0, Pat::Metal), mp(0x4C62A0, Pat::Metal)][tier as usize]
}

/// Dos de la lame, plus sombre que le tranchant.
fn spine(tier: u8) -> Mat {
    [mp(0x9A7446, Pat::Grain), mp(0x6C6E78, Pat::Speck), mp(0x8E96A6, Pat::Metal), mp(0x3FA8AE, Pat::Metal), mp(0x2A3460, Pat::Metal)][tier as usize]
}

/// Ligature de l'outil : chanvre, cuir, cordon rouge, or, soie violette.
fn binding(tier: u8) -> Mat {
    [m(0xE4D8B4), m(0x5E4A3A), m(0xC0362C), mp(0xE8B93A, Pat::Metal), m(0x9A6AE8)][tier as usize]
}

/// Facon de presenter l'objet.
pub fn pose(id: Id) -> Pose {
    if tool(id).is_some() {
        return Pose::Tool;
    }
    if let Some((slot, _)) = armor(id) {
        return if slot == FEET { Pose::Thing } else { Pose::Flat };
    }
    match id {
        YUMI => Pose::Roll(-28.0),
        YARI | NAGINATA | TANTO | TETSUBO | KUNAI | ARROW | YAKITORI | SHEARS => Pose::Tool,
        FISHING_ROD => Pose::Roll(-24.0),
        STICK | FEATHER | BONE => Pose::Roll(-38.0),
        CARROT_ITEM => Pose::Roll(-34.0),
        CART | FUGU | DIAMOND | RAW_MEAT | COOKED_MEAT | ROTTEN | SHIELD | KOBAN | HAGOROMO | LEATHER | STRING | WHEAT_ITEM | TEA_LEAF | RAW_FISH | COOKED_FISH | EDAMAME | KABOCHA_SLICE | ROAST_KABOCHA | LIGHTER => Pose::Flat,
        _ => Pose::Thing,
    }
}

/// Part de la case que l'icone remplit : les petits objets restent petits.
pub fn fill(id: Id) -> f32 {
    match id {
        TANTO | KUNAI => 0.8,
        EGG | SEEDS | SOY_BEANS | KABOCHA_SEEDS | FLINT | TAMA_SHARD | APPLE | PEACH | PEARL | TOFU | GREEN_TEA => 0.84,
        _ => 1.0,
    }
}

/// Manche droit, avec son talon.
fn shaft(k: &mut Kit, len: f32) {
    k.b([0.0, len / 2.0 + 0.4, 0.0], [1.4, len - 0.8, 1.4], WOOD);
    k.b([0.0, 0.5, 0.0], [1.8, 1.0, 1.8], DARKWOOD);
}

/// Lame courbe a un seul tranchant, passant par `pts` : dos sombre, tranchant clair, pointe.
fn blade(k: &mut Kit, pts: &[[f32; 2]], w: f32, tier: u8) {
    let n = pts.len() - 1;
    for (i, p) in pts.windows(2).enumerate() {
        let (dx, dy) = (p[1][0] - p[0][0], p[1][1] - p[0][1]);
        let l = (dx * dx + dy * dy).sqrt().max(0.01);
        // Normale vers le tranchant (a droite d'une lame qui monte).
        let (nx, ny) = (dy / l, -dx / l);
        let (e, last) = (0.12, i + 1 == n);
        let (a, b) = ([p[0][0] - dx * e, p[0][1] - dy * e], [p[1][0] + dx * e, p[1][1] + dy * e]);
        let taper = if last { 0.62 } else { 1.0 };
        let (wm, we) = (w * 0.42 * taper, w * 0.58 * taper);
        let off = |q: [f32; 2], o: f32| [q[0] + nx * o, q[1] + ny * o];
        k.seg(off(a, -we / 2.0), off(b, -we / 2.0), wm, 0.6, spine(tier));
        k.seg(off(a, wm / 2.0), off(b, wm / 2.0), we, 0.4, head(tier));
        if last {
            // Pointe.
            let tip = [p[1][0] + dx / l * 0.7 - nx * 0.25, p[1][1] + dy / l * 0.7 - ny * 0.25];
            k.seg(off(p[1], -0.2), tip, w * 0.42, 0.4, head(tier));
        }
    }
}

/// Monture de sabre : pommeau, poignee tressee longue de `len`, garde, collet. Renvoie la hauteur ou part la lame.
fn hilt(k: &mut Kit, len: f32, guard: f32, tier: u8) -> f32 {
    let wrap = [0x3A2A22, 0x4A5060, 0x8E2A26, 0x1F6A6A, 0x5A3A8E][tier as usize];
    k.b([0.0, 0.4, 0.0], [1.7, 0.8, 1.3], GOLD);
    k.b([0.0, 0.8 + len / 2.0, 0.0], [1.4, len, 1.0], ma(wrap, 0xECE2C6, Pat::Wrap));
    k.b([0.0, 1.0 + len, 0.0], [1.7, 0.5, 1.3], GOLD);
    let tsuba = if tier == 3 { GOLD } else if tier == 4 { ma(0x2C2A36, 0xE8B93A, Pat::Dots) } else { IRON };
    k.b([0.0, 1.5 + len, 0.0], [guard, 0.6, guard * 0.78], tsuba);
    k.b([0.0, 2.2 + len, 0.0], [1.25, 0.9, 0.8], COPPER);
    2.6 + len
}

fn tool_model(k: &mut Kit, kind: u8, tier: u8) {
    let (h, s, bind) = (head(tier), spine(tier), binding(tier));
    match kind {
        PICK => {
            shaft(k, 13.6);
            k.b([0.0, 13.7, 0.0], [2.4, 2.2, 2.2], s);
            for sx in [-1.0f32, 1.0] {
                k.r([sx * 3.0, 13.4, 0.0], [4.4, 1.7, 1.5], h, [0.0, 0.0, -sx * 16.0]);
                k.r([sx * 6.1, 12.0, 0.0], [3.0, 1.2, 1.1], h, [0.0, 0.0, -sx * 34.0]);
            }
            k.b([0.0, 12.1, 0.0], [1.9, 0.8, 1.9], bind);
        }
        AXE => {
            shaft(k, 14.4);
            k.b([0.0, 12.4, 0.0], [2.3, 3.4, 2.1], s);
            k.b([-1.6, 12.4, 0.0], [1.0, 2.2, 1.7], s);
            k.b([2.7, 12.5, 0.0], [3.2, 4.2, 1.0], h);
            k.b([3.4, 10.0, 0.0], [1.8, 1.4, 0.9], h);
            k.b([4.8, 12.0, 0.0], [1.2, 6.2, 0.6], h);
            k.b([0.0, 10.2, 0.0], [1.9, 0.8, 1.9], bind);
        }
        SHOVEL => {
            k.b([0.0, 5.6, 0.0], [1.4, 9.6, 1.4], WOOD);
            k.b([0.0, 0.6, 0.0], [3.6, 1.2, 1.3], DARKWOOD);
            k.b([0.0, 10.6, 0.0], [1.9, 1.6, 1.7], s);
            k.b([0.0, 12.9, 0.0], [4.6, 3.6, 0.8], h);
            k.b([0.0, 15.0, 0.0], [3.4, 1.0, 0.8], h);
            k.b([0.0, 15.8, 0.0], [1.8, 0.7, 0.7], h);
            k.b([0.0, 12.6, 0.3], [0.8, 3.2, 0.6], s);
            k.b([0.0, 9.6, 0.0], [1.8, 0.7, 1.8], bind);
        }
        HOE => {
            shaft(k, 14.6);
            k.b([0.0, 13.7, 0.0], [2.1, 1.9, 2.1], s);
            k.b([1.9, 13.7, 0.0], [2.2, 1.1, 1.5], s);
            k.r([3.5, 11.3, 0.0], [0.9, 5.4, 3.8], h, [0.0, 0.0, -10.0]);
            k.b([0.0, 12.3, 0.0], [1.9, 0.8, 1.9], bind);
        }
        _ => {
            if tier == 0 {
                // Bokken : un sabre d'entrainement, tout en bois.
                k.b([0.0, 2.8, 0.0], [1.4, 5.2, 1.1], DARKWOOD);
                k.b([0.0, 0.4, 0.0], [1.6, 0.8, 1.2], DARKWOOD);
                k.b([0.0, 5.6, 0.0], [1.8, 0.5, 1.4], m(0xE4D8B4));
                k.line(&[[0.0, 5.6], [-0.1, 9.0], [-0.5, 12.4], [-1.4, 15.4]], 1.3, 0.8, h);
                k.seg([-1.4, 15.4], [-1.9, 16.2], 0.9, 0.7, h);
            } else {
                let y = hilt(k, 4.2, 3.6, tier);
                blade(k, &[[0.0, y], [-0.1, y + 3.2], [-0.6, y + 6.3], [-1.6, y + 9.0]], 1.5, tier);
            }
        }
    }
}

/// Poudre en petit tas sur une feuille de papier plie.
fn powder(k: &mut Kit, mat: Mat) {
    k.r([0.0, 0.25, 0.0], [11.0, 0.5, 9.0], PAPER, [0.0, 14.0, 0.0]);
    k.b([0.0, 1.0, 0.0], [7.0, 1.0, 5.6], mat);
    k.b([0.0, 1.9, 0.0], [5.0, 0.9, 4.0], mat);
    k.b([0.0, 2.7, 0.0], [3.0, 0.8, 2.4], mat);
    k.b([0.2, 3.3, 0.0], [1.4, 0.6, 1.2], mat);
    for (x, z) in [(-4.2, 2.6), (4.0, -2.8), (3.4, 3.0), (-3.6, -3.2)] {
        k.b([x, 0.8, z], [0.7, 0.6, 0.7], mat);
    }
}

/// Petit tas de graines de taille `s`.
fn grains(k: &mut Kit, s: [f32; 3], mat: Mat) {
    let spots: [(f32, f32, f32, f32); 9] = [
        (-2.6, 0.0, -1.4, 20.0),
        (0.2, 0.0, -2.4, -35.0),
        (2.8, 0.0, -0.8, 60.0),
        (-1.6, 0.0, 1.8, -15.0),
        (1.4, 0.0, 1.6, 40.0),
        (3.6, 0.0, 2.6, -60.0),
        (-0.8, 1.0, -0.4, 75.0),
        (1.2, 1.0, 0.2, -25.0),
        (0.2, 2.0, 0.0, 30.0),
    ];
    for (x, layer, z, yaw) in spots {
        k.r([x, s[1] / 2.0 + layer * s[1] * 0.9, z], s, mat, [0.0, yaw, 0.0]);
    }
}

/// Lingot : une barre a pans, frappee d'un sceau.
fn ingot(k: &mut Kit, mat: Mat, seal: Mat) {
    k.b([0.0, 1.1, 0.0], [10.0, 2.2, 5.0], mat);
    k.b([0.0, 2.9, 0.0], [8.4, 1.4, 3.6], mat);
    k.b([0.0, 3.65, 0.0], [2.6, 0.2, 1.6], seal);
}

/// Cristaux dresses sur leur gangue.
fn crystals(k: &mut Kit, mat: Mat, rock: Mat) {
    k.b([0.0, 0.7, 0.0], [7.0, 1.4, 5.6], rock);
    k.r([0.0, 5.4, 0.0], [2.6, 9.4, 2.6], mat, [0.0, 30.0, 0.0]);
    k.r([0.0, 10.6, 0.0], [1.5, 1.4, 1.5], mat, [0.0, 30.0, 0.0]);
    k.r([-2.7, 3.6, 0.6], [2.0, 6.4, 2.0], mat, [0.0, 10.0, 22.0]);
    k.r([2.7, 3.2, -0.4], [2.0, 5.8, 2.0], mat, [8.0, -20.0, -26.0]);
    k.r([1.0, 2.6, 2.2], [1.6, 4.4, 1.6], mat, [30.0, 0.0, -8.0]);
}

/// Eclats anguleux.
fn shards(k: &mut Kit, mat: Mat) {
    k.r([-0.6, 4.6, 0.0], [3.4, 8.4, 2.4], mat, [0.0, 20.0, -24.0]);
    k.r([2.4, 3.2, 0.6], [2.6, 5.6, 2.0], mat, [0.0, -12.0, 30.0]);
    k.r([-3.0, 1.8, 1.0], [2.2, 3.0, 1.6], mat, [0.0, 0.0, 52.0]);
    k.r([0.6, 1.2, -1.2], [3.6, 2.2, 2.4], mat, [0.0, 30.0, 6.0]);
}

/// Piece de viande en tranche : chair, bord de gras et os rond.
fn meat(k: &mut Kit, flesh: Mat, fat: Mat) {
    k.b([0.0, 4.6, 0.0], [10.4, 5.0, 3.0], flesh);
    k.b([0.0, 4.6, 0.0], [8.2, 7.6, 3.0], flesh);
    k.b([0.0, 4.6, 0.0], [9.4, 6.4, 3.1], flesh);
    k.b([-0.6, 8.5, 0.0], [6.4, 0.9, 3.2], fat);
    k.b([-4.3, 7.5, 0.0], [1.6, 1.5, 3.2], fat);
    k.b([-5.3, 5.0, 0.0], [0.9, 4.0, 3.2], fat);
    k.b([1.2, 4.4, 0.0], [2.4, 2.4, 3.4], IVORY);
    k.b([1.2, 4.4, 0.0], [1.0, 1.0, 3.6], m(0xC8A890));
    k.turn(0, [0.0, 0.0, -12.0], [0.0, 4.6, 0.0]);
}

/// Seau japonais (oke) : douves, cercles de bambou et anse en bois.
fn pail(k: &mut Kit, content: Option<Mat>) {
    k.disc([0.0, 0.7, 0.0], 8.2, 1.0, HINOKI);
    k.hoop([0.0, 4.5, 0.0], 9.4, 8.0, 0.9, HINOKI);
    for y in [2.4, 6.6] {
        k.hoop([0.0, y, 0.0], 10.2, 0.9, 0.5, ma(0x6E7A34, 0x4E5420, Pat::Bands));
    }
    for sx in [-1.0, 1.0] {
        k.b([sx * 3.5, 10.0, 0.0], [0.9, 3.4, 2.2], HINOKI);
    }
    k.b([0.0, 11.2, 0.0], [7.8, 1.0, 1.2], WOOD);
    if let Some(c) = content {
        k.disc([0.0, 7.6, 0.0], 7.8, 0.6, c);
    }
}

/// Bol rond, evase, sur son pied. Renvoie la hauteur du contenu.
fn bowl(k: &mut Kit, mat: Mat, inner: Mat) -> f32 {
    k.disc([0.0, 0.5, 0.0], 4.2, 1.0, mat);
    k.disc([0.0, 1.4, 0.0], 6.8, 0.9, mat);
    k.hoop([0.0, 2.8, 0.0], 8.8, 2.0, 0.9, mat);
    k.hoop([0.0, 4.5, 0.0], 10.6, 1.6, 0.9, mat);
    k.disc([0.0, 2.0, 0.0], 6.6, 0.4, inner);
    4.4
}

/// Poisson couche, tete a gauche : corps fusele, ventre clair, queue fourchue.
fn fish(k: &mut Kit, back: Mat, belly: Mat, fin: Mat) {
    k.b([-5.0, 5.0, 0.0], [3.0, 4.2, 2.2], back);
    k.b([-6.9, 4.8, 0.0], [1.2, 2.6, 1.8], back);
    k.b([-1.4, 5.1, 0.0], [4.6, 5.0, 2.4], back);
    k.b([2.3, 5.0, 0.0], [3.4, 3.8, 2.0], back);
    k.b([4.7, 5.0, 0.0], [1.8, 2.4, 1.5], back);
    k.b([-2.4, 3.2, 0.0], [7.6, 1.5, 2.5], belly);
    k.b([2.4, 3.6, 0.0], [3.0, 1.1, 2.1], belly);
    k.r([6.6, 6.4, 0.0], [2.8, 1.8, 0.6], fin, [0.0, 0.0, 34.0]);
    k.r([6.6, 3.6, 0.0], [2.8, 1.8, 0.6], fin, [0.0, 0.0, -34.0]);
    k.b([-0.8, 8.0, 0.0], [4.4, 1.2, 0.5], fin);
    k.b([-1.2, 2.1, 0.0], [2.0, 1.0, 0.5], fin);
    for sz in [-1.0, 1.0] {
        k.b([-5.6, 5.6, sz * 1.05], [1.4, 1.4, 0.3], WHITE);
        k.b([-5.7, 5.6, sz * 1.15], [0.7, 0.7, 0.3], INK);
        k.b([-3.5, 4.9, sz * 1.2], [0.4, 3.0, 0.2], fin);
    }
}

/// Part de kabocha : croissant d'ecorce et de chair.
fn wedge(k: &mut Kit, rind: Mat, flesh: Mat) {
    k.line(&[[-6.0, 8.6], [-4.8, 4.2], [-2.0, 1.8], [2.0, 1.8], [4.8, 4.2], [6.0, 8.6]], 1.2, 3.2, rind);
    k.line(&[[-4.5, 8.6], [-3.5, 5.2], [-1.4, 3.5], [1.4, 3.5], [3.5, 5.2], [4.5, 8.6]], 2.0, 2.9, flesh);
    k.line(&[[-2.6, 8.4], [-1.8, 6.4], [0.0, 5.6], [1.8, 6.4], [2.6, 8.4]], 1.1, 2.4, ma(0xF4D488, 0xFFF0C0, Pat::Dots));
}

/// Fruit rond avec sa queue.
fn fruit(k: &mut Kit, mat: Mat, r: f32) {
    k.ball([0.0, r, 0.0], r, mat);
    k.seg([0.0, 2.0 * r - 0.4], [0.8, 2.0 * r + 1.8], 0.7, 0.7, DARKWOOD);
}

/// Matieres d'une armure : (plaques lacees, garniture, couleur du lacage).
fn armor_mats(tier: u8) -> (Mat, Mat, Mat) {
    match tier {
        0 => (mp(0xA06A3E, Pat::Speck), m(0x6E4630), m(0xE4D8B4)),
        1 => (ma(0xB8BE6A, 0x3A2A22, Pat::Bands), m(0x6F7A34), m(0x3A2A22)),
        2 => (ma(0x545A6A, 0xC0362C, Pat::Bands), STEEL, RED),
        3 => (ma(0xE8B93A, 0x8E2A26, Pat::Bands), m(0xFFE38A), m(0x8E2A26)),
        4 => (ma(0x5FDDD8, 0x2B6E9A, Pat::Bands), m(0xC8FAF4), m(0x2B6E9A)),
        _ => (ma(0x3A4878, 0xE8B93A, Pat::Bands), GOLD, GOLD),
    }
}

/// Piece d'armure. La tenue de cuir (matiere 0) est celle du voyageur : chapeau de paille, veste, hakama, sandales.
fn armor_model(k: &mut Kit, slot: u8, tier: u8) {
    let (plate, trim, lace) = armor_mats(tier);
    match (slot, tier) {
        (HEAD, 0) => {
            // Kasa : large chapeau conique en paille tressee, et sa jugulaire.
            for (y, d, h) in [(0.5, 16.0, 1.0), (1.6, 12.6, 1.2), (2.8, 9.0, 1.2), (4.0, 5.6, 1.2)] {
                k.disc([0.0, y, 0.0], d, h, STRAW);
            }
            k.b([0.0, 5.0, 0.0], [1.6, 0.9, 1.6], plate);
            k.disc([0.0, 0.2, 0.0], 16.4, 0.4, m(0xA8843E));
            for sx in [-1.0, 1.0] {
                k.b([sx * 3.0, -1.6, 0.0], [0.5, 3.6, 0.5], RED);
            }
        }
        (HEAD, _) => {
            // Kabuto : bol lisse, visiere, couvre-nuque a lames lacees, cornes du cimier.
            let bowl = m(plate.col);
            k.disc([0.0, 6.4, 0.0], 10.4, 3.4, bowl);
            k.disc([0.0, 9.0, 0.0], 9.0, 2.0, bowl);
            k.disc([0.0, 10.5, 0.0], 6.2, 1.2, bowl);
            k.b([0.0, 11.4, 0.0], [1.6, 0.8, 1.6], trim);
            k.b([0.0, 5.0, 4.4], [7.6, 0.8, 2.6], trim);
            for i in 0..3 {
                let f = i as f32;
                k.b([0.0, 4.0 - f * 1.4, -4.2 - f * 0.6], [10.0 + f * 2.4, 1.4, 1.4], plate);
                for sx in [-1.0, 1.0] {
                    k.b([sx * (5.2 + f * 1.2), 4.0 - f * 1.4, -0.6], [1.4, 1.4, 7.4], plate);
                }
            }
            for sx in [-1.0f32, 1.0] {
                k.segz([sx * 1.2, 7.4], [sx * 3.2, 11.4], 4.9, 1.2, 0.6, trim);
                k.segz([sx * 3.2, 11.0], [sx * 3.8, 14.6], 4.9, 1.0, 0.6, trim);
                k.b([sx * 4.4, 2.0, 2.8], [0.5, 3.4, 0.5], lace);
            }
            k.b([0.0, 7.8, 4.9], [2.2, 2.2, 0.6], trim);
        }
        (BODY, 0) => {
            // Veste de cuir croisee, ceinture de corde.
            k.b([0.0, 6.4, 0.0], [9.0, 9.6, 4.6], plate);
            for sx in [-1.0f32, 1.0] {
                k.b([sx * 6.2, 9.4, 0.0], [3.6, 3.8, 4.2], plate);
                k.b([sx * 3.0, 11.8, 0.0], [2.4, 1.4, 4.8], plate);
                k.segz([sx * 2.6, 11.6], [-sx * 1.0, 3.6], 2.4, 1.0, 0.4, lace);
            }
            k.b([0.0, 3.2, 0.0], [9.4, 1.2, 5.0], trim);
            k.b([1.6, 2.2, 2.6], [0.8, 2.2, 0.5], trim);
        }
        (BODY, _) => {
            // Cuirasse do : plastron lace, bretelles, epaulieres sode et jupe de plaques kusazuri.
            k.b([0.0, 7.4, 0.0], [9.0, 8.0, 5.0], plate);
            k.b([0.0, 11.8, 0.0], [5.0, 1.0, 5.2], trim);
            for sx in [-1.0, 1.0] {
                k.b([sx * 3.2, 12.4, 0.0], [2.0, 2.0, 5.4], lace);
                k.b([sx * 6.5, 9.6, 0.0], [3.4, 5.2, 5.6], plate);
                k.b([sx * 6.5, 12.4, 0.0], [3.6, 0.6, 5.8], trim);
            }
            k.b([0.0, 3.0, 0.0], [9.4, 1.0, 5.4], lace);
            for x in [-3.2, 0.0, 3.2] {
                k.b([x, 1.2, 2.3], [2.8, 2.8, 0.9], plate);
                k.b([x, 1.2, -2.3], [2.8, 2.8, 0.9], plate);
            }
            k.b([0.0, 8.4, 2.6], [2.0, 2.0, 0.4], trim);
        }
        (LEGS, 0) => {
            // Hakama : pantalon large a plis.
            let cloth = mp(0x4A4466, Pat::Grain);
            k.b([0.0, 12.4, 0.0], [9.0, 1.6, 4.4], trim);
            for sx in [-1.0, 1.0] {
                k.b([sx * 2.4, 8.6, 0.0], [4.4, 6.4, 4.2], cloth);
                k.b([sx * 2.8, 3.0, 0.0], [5.4, 5.2, 4.8], cloth);
            }
            k.b([0.0, 13.4, 2.3], [2.4, 1.0, 0.4], lace);
        }
        (LEGS, _) => {
            // Cuissards haidate : ceinture et deux tabliers de lames.
            k.b([0.0, 12.6, 0.0], [9.4, 1.6, 4.6], lace);
            for sx in [-1.0, 1.0] {
                k.b([sx * 2.5, 7.6, 0.0], [4.4, 8.6, 4.2], plate);
                k.b([sx * 2.5, 3.0, 0.0], [4.6, 0.8, 4.4], trim);
                k.b([sx * 2.5, 1.4, 0.0], [3.4, 2.6, 3.6], m(0x3A3648));
            }
        }
        (_, 0) => {
            // Waraji : sandales de paille tressee, lanieres nouees.
            for sx in [-1.0f32, 1.0] {
                k.b([sx * 2.6, 0.5, 0.0], [3.6, 1.0, 8.0], STRAW);
                k.b([sx * 2.6, 1.3, 2.2], [3.8, 0.6, 0.7], lace);
                k.b([sx * 2.6, 1.3, -0.4], [3.8, 0.6, 0.7], lace);
                k.b([sx * 2.6, 2.2, -2.8], [0.7, 2.6, 0.7], lace);
                k.b([sx * 2.6, 3.4, -1.6], [2.6, 0.6, 3.0], lace);
            }
        }
        _ => {
            // Jambieres suneate : eclisses, genouillere, sandale.
            for sx in [-1.0, 1.0] {
                k.b([sx * 2.7, 5.6, 0.0], [3.6, 8.0, 3.6], plate);
                k.b([sx * 2.7, 10.2, 0.2], [4.0, 1.6, 4.0], trim);
                k.b([sx * 2.7, 2.2, 0.0], [3.9, 0.8, 3.9], lace);
                k.b([sx * 2.7, 0.5, 0.9], [3.8, 1.0, 6.0], m(0x3A3648));
            }
        }
    }
}

/// Modele d'un objet.
pub fn design(id: Id) -> Kit {
    let mut kit = Kit::default();
    let k = &mut kit;
    if let Some((slot, tier)) = armor(id) {
        armor_model(k, slot, tier);
        return kit;
    }
    if let Some((kind, tier)) = tool(id) {
        tool_model(k, kind, tier);
        return kit;
    }
    match id {
        STICK => {
            k.seg([0.0, 0.0], [0.0, 15.0], 1.5, 1.5, WOOD);
            k.seg([0.3, 8.6], [2.6, 11.0], 0.9, 0.9, WOOD);
            k.b([0.0, 15.2, 0.0], [1.2, 0.6, 1.2], m(0xE0C090));
            k.b([0.0, 4.6, 0.0], [1.7, 0.6, 1.7], DARKWOOD);
        }
        COAL => {
            let c = ma(0x33363E, 0x70788A, Pat::Dots);
            k.r([0.0, 3.0, 0.0], [7.0, 5.0, 6.0], c, [0.0, 20.0, 8.0]);
            k.r([-2.5, 5.6, 1.0], [4.0, 3.6, 4.0], c, [10.0, -15.0, -12.0]);
            k.r([2.6, 5.2, -0.6], [3.6, 3.0, 3.4], c, [0.0, 30.0, 15.0]);
            k.r([1.0, 1.6, 2.8], [3.0, 2.4, 2.4], c, [0.0, -30.0, 0.0]);
        }
        IRON_INGOT => ingot(k, mp(0xCDD3DC, Pat::Metal), mp(0x8E96A6, Pat::Plain)),
        GOLD_INGOT => ingot(k, GOLD, m(0xB07C1E)),
        TAMAHAGANE => ingot(k, ma(0x4C62A0, 0x8FA3D8, Pat::Bands), m(0xE8B93A)),
        DIAMOND => {
            // Pierre taillee : culasse en pointe, rondiste, couronne et table.
            let d = mp(0x5FDDD8, Pat::Metal);
            for (y, w) in [(0.6, 1.4), (1.8, 3.0), (3.0, 4.6), (4.2, 6.2), (5.4, 7.8)] {
                k.disc([0.0, y, 0.0], w, 1.22, d);
            }
            k.disc([0.0, 6.5, 0.0], 9.2, 1.0, mp(0x8FEEE8, Pat::Metal));
            k.disc([0.0, 7.4, 0.0], 7.6, 0.9, d);
            k.disc([0.0, 8.1, 0.0], 5.6, 0.6, m(0xC8FAF4));
        }
        VOLT_DUST => powder(k, ma(0x9A6AE8, 0xE0D0FF, Pat::Dots)),
        SPORE => powder(k, ma(0xF2D070, 0xFFF6D0, Pat::Dots)),
        FERTILIZER => powder(k, ma(0x8A6A4A, 0xEDE6D2, Pat::Dots)),
        DRIED_TEA => powder(k, ma(0x3E5A30, 0x6E8A4A, Pat::Dots)),
        SEEDS => {
            // Mesure de riz (masu) : une boite carree en hinoki, pleine de grains.
            k.b([0.0, 0.5, 0.0], [8.0, 1.0, 8.0], HINOKI);
            k.ring([0.0, 3.0, 0.0], 8.6, 5.0, 0.9, HINOKI);
            k.b([0.0, 4.9, 0.0], [6.8, 0.8, 6.8], RICE);
            k.b([0.0, 5.6, 0.0], [4.6, 0.8, 4.6], RICE);
            k.b([0.3, 6.2, -0.2], [2.4, 0.6, 2.4], RICE);
        }
        SOY_BEANS => grains(k, [1.7, 1.4, 1.5], mp(0xE2C98A, Pat::Plain)),
        KABOCHA_SEEDS => grains(k, [2.2, 0.6, 1.4], m(0xF2E8C8)),
        WHEAT_ITEM => {
            for i in -2i32..=2 {
                let f = i as f32;
                k.seg([f * 0.5, 0.4], [f * 1.9, 10.6], 0.7, 0.7, STRAW);
                k.seg([f * 1.9, 10.2], [f * 2.7 + 0.5, 14.8], 1.4, 1.2, ma(0xE8C24A, 0xFFF0A8, Pat::Dots));
            }
            k.b([0.0, 4.4, 0.0], [5.2, 1.1, 1.5], RED);
            k.b([1.6, 3.6, 0.6], [0.8, 2.0, 0.5], RED);
        }
        BREAD => {
            for (y, h, w, d) in [(1.25, 2.5, 9.0, 4.0), (3.5, 2.0, 7.2, 4.0), (5.4, 1.8, 5.2, 4.0), (7.0, 1.4, 3.2, 3.8), (8.0, 0.8, 1.6, 3.4)] {
                k.b([0.0, y, 0.0], [w, h, d], RICE);
            }
            k.b([0.0, 1.9, 0.0], [4.2, 3.8, 4.5], SEAWEED);
        }
        CARROT_ITEM => {
            let w = ma(0xF4F0E4, 0xD8D2C4, Pat::Bands);
            for (y, h, d) in [(9.6, 3.0, 3.4), (6.6, 3.0, 3.0), (3.9, 2.6, 2.3), (1.9, 1.6, 1.4), (0.6, 1.0, 0.7)] {
                k.b([0.0, y, 0.0], [d, h, d], w);
            }
            k.b([0.0, 11.3, 0.0], [2.3, 0.7, 2.3], m(0xB8D488));
            k.seg([0.0, 11.4], [-2.6, 15.6], 1.3, 0.6, LEAF);
            k.seg([0.0, 11.4], [0.3, 16.2], 1.3, 0.6, LEAF);
            k.seg([0.0, 11.4], [2.8, 15.2], 1.3, 0.6, LEAF);
        }
        RAW_MEAT => meat(k, ma(0xD85A66, 0xF6C0C4, Pat::Dots), m(0xF6E8DC)),
        COOKED_MEAT => meat(k, ma(0xA8643A, 0x5E3420, Pat::Bands), m(0xE0B880)),
        ROTTEN => meat(k, ma(0x8E2A4A, 0x7AB84A, Pat::Dots), m(0xB89AC0)),
        LEATHER => {
            let l = mp(0xA8643A, Pat::Speck);
            k.b([0.0, 8.0, 0.0], [9.0, 10.0, 0.7], l);
            for (x, y) in [(-5.2, 12.6), (5.2, 12.6), (-5.2, 3.4), (5.2, 3.4)] {
                k.b([x, y, 0.0], [2.6, 2.6, 0.7], l);
            }
            k.b([0.0, 13.7, 0.0], [3.0, 1.6, 0.7], l);
            k.b([0.0, 2.4, 0.0], [1.6, 1.6, 0.7], l);
            k.b([0.0, 8.0, 0.2], [0.6, 8.0, 0.6], m(0x7A4628));
        }
        FEATHER => {
            let v = ma(0xF4F4F0, 0xC4CDE2, Pat::Grain);
            k.seg([0.0, 0.0], [0.0, 15.6], 0.5, 0.6, m(0xE8DCC0));
            k.seg([0.0, 4.6], [0.0, 13.6], 3.6, 0.3, v);
            k.seg([0.0, 13.2], [0.0, 15.4], 2.2, 0.3, v);
            k.seg([0.0, 2.6], [0.0, 4.8], 2.4, 0.3, mp(0xC4CDE2, Pat::Grain));
            k.b([0.0, 14.6, 0.05], [1.2, 1.4, 0.35], m(0x56607A));
        }
        EGG => {
            let e = mp(0xF2EAD8, Pat::Speck);
            for (y, h, w) in [(0.6, 1.2, 3.0), (2.2, 2.2, 4.8), (4.3, 2.2, 5.2), (6.2, 1.8, 4.4), (7.6, 1.2, 2.8)] {
                k.disc([0.0, y, 0.0], w * 1.15, h, e);
            }
        }
        STRING => {
            // Corde sacree : torsade de paille, pompons et papiers plies en eclair.
            k.b([0.0, 11.0, 0.0], [13.6, 2.2, 2.2], STRAW);
            k.b([0.0, 10.9, 0.0], [7.0, 3.0, 3.0], STRAW);
            for x in [-4.6, 0.0, 4.6] {
                k.b([x, 7.8, 0.0], [1.2, 4.4, 1.2], mp(0xDDBD6C, Pat::Grain));
                k.b([x, 5.2, 0.0], [1.8, 1.4, 1.8], mp(0xDDBD6C, Pat::Grain));
            }
            for x in [-2.3, 2.3] {
                k.b([x, 8.6, 0.4], [1.3, 2.2, 0.3], WHITE);
                k.b([x + 0.7, 6.5, 0.4], [1.3, 2.2, 0.3], WHITE);
                k.b([x, 4.4, 0.4], [1.3, 2.2, 0.3], WHITE);
            }
        }
        BONE => {
            k.seg([0.0, 1.6], [0.0, 14.4], 1.4, 1.4, IVORY);
            for y in [1.0, 15.0] {
                k.b([-0.8, y, 0.0], [1.7, 1.9, 1.7], IVORY);
                k.b([0.8, y, 0.0], [1.7, 1.9, 1.7], IVORY);
            }
        }
        PEARL => {
            // Perle d'esprit, coiffee de sa flammeche.
            k.ball([0.0, 4.2, 0.0], 4.2, mp(0x8FA3D8, Pat::Glow));
            k.b([0.0, 9.2, 0.0], [2.2, 1.8, 2.2], m(0xC8D8FF));
            k.b([0.6, 10.7, 0.0], [1.3, 1.5, 1.3], m(0xE4ECFF));
            k.b([1.1, 11.9, 0.0], [0.7, 1.1, 0.7], WHITE);
        }
        WYRM_HEART => {
            // Perle du dragon dans sa griffe d'or.
            k.b([0.0, 0.5, 0.0], [5.0, 1.0, 5.0], GOLD);
            k.ball([0.0, 5.6, 0.0], 4.4, mp(0x3FD0C8, Pat::Glow));
            for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                k.r([sx * 3.3, 2.8, sz * 3.3], [1.1, 4.6, 1.1], GOLD, [sz * 14.0, 0.0, -sx * 14.0]);
                k.b([sx * 3.2, 5.4, sz * 3.2], [0.9, 1.2, 0.9], GOLD);
            }
        }
        EMBER_CRYSTAL => crystals(k, mp(0xF08A24, Pat::Glow), mp(0x4A2E2E, Pat::Speck)),
        BUCKET => pail(k, None),
        WATER_BUCKET => pail(k, Some(ma(0x3F86D6, 0xA8D4FF, Pat::Dots))),
        LAVA_BUCKET => pail(k, Some(ma(0xF08A24, 0xFFE070, Pat::Dots))),
        LIGHTER => {
            // Briquet a silex : la lame d'acier dans sa poignee de bois, et la pierre.
            k.b([-1.6, 9.4, 0.0], [8.6, 3.0, 1.5], DARKWOOD);
            k.b([-1.6, 11.2, 0.0], [5.6, 0.9, 1.3], DARKWOOD);
            k.b([-1.6, 7.4, 0.0], [9.6, 1.1, 1.0], STEEL);
            k.b([-1.6, 12.0, 0.0], [1.4, 1.2, 0.7], RED);
            k.r([3.6, 3.4, 0.0], [4.2, 3.2, 2.4], mp(0x4A4E5A, Pat::Metal), [0.0, 20.0, 24.0]);
            k.r([2.2, 2.2, 0.6], [2.4, 2.0, 1.8], mp(0x4A4E5A, Pat::Metal), [0.0, -10.0, -20.0]);
            for (x, y) in [(0.4, 5.6), (1.8, 6.2), (-0.8, 4.6)] {
                k.b([x, y, 0.4], [0.6, 0.6, 0.6], m(0xFFD95A));
            }
        }
        APPLE => {
            fruit(k, mp(0xD03A30, Pat::Glow), 4.2);
            k.r([2.2, 9.4, 0.0], [2.8, 0.5, 1.7], LEAF, [0.0, 0.0, 22.0]);
        }
        PEACH => {
            let p = mp(0xF7A8A0, Pat::Glow);
            k.ball([0.0, 4.6, 0.0], 4.4, p);
            k.b([0.0, 9.0, 0.0], [1.8, 1.0, 1.8], p);
            k.b([0.0, 4.8, 0.0], [0.6, 8.6, 9.0], m(0xE0786E));
            for sx in [-1.0, 1.0] {
                k.r([sx * 3.4, 0.5, 1.2], [4.2, 0.6, 2.4], LEAF, [0.0, sx * 24.0, sx * 12.0]);
            }
        }
        FLINT => shards(k, mp(0x4A4E5A, Pat::Metal)),
        TAMA_SHARD => shards(k, mp(0x5C74B4, Pat::Metal)),
        ARROW => {
            k.b([0.0, 7.4, 0.0], [0.7, 12.4, 0.7], ma(0xD8C48A, 0x8A7A44, Pat::Bands));
            k.b([0.0, 14.2, 0.0], [1.6, 1.6, 0.5], STEEL);
            k.b([0.0, 15.4, 0.0], [0.9, 1.0, 0.45], STEEL);
            k.b([0.0, 13.2, 0.0], [1.0, 0.6, 1.0], RED);
            let f = ma(0xF4F4F0, 0x56607A, Pat::Bands);
            k.b([0.0, 3.4, 0.0], [2.6, 3.6, 0.3], f);
            k.b([0.0, 3.4, 0.0], [0.3, 3.6, 2.6], f);
            k.b([0.0, 1.2, 0.0], [0.9, 0.6, 0.9], RED);
        }
        CART => {
            // Chariot a bras : plateau a ridelles, deux grandes roues a rayons, brancards.
            k.b([0.0, 6.8, 0.0], [12.0, 0.9, 6.4], HINOKI);
            k.b([-5.7, 8.0, 0.0], [0.6, 1.6, 6.4], WOOD);
            k.b([0.0, 4.4, 0.0], [0.9, 0.9, 9.0], IRON);
            for sz in [-1.0, 1.0] {
                k.b([0.0, 8.0, sz * 2.95], [12.0, 1.6, 0.5], WOOD);
                let z = sz * 4.2;
                let from = k.v.len();
                k.disc([0.0, 4.4, z], 8.8, 0.8, DARKWOOD);
                k.turn(from, [90.0, 0.0, 0.0], [0.0, 4.4, z]);
                k.b([0.0, 4.4, z + sz * 0.45], [7.0, 0.8, 0.3], HINOKI);
                k.b([0.0, 4.4, z + sz * 0.45], [0.8, 7.0, 0.3], HINOKI);
                k.b([0.0, 4.4, z + sz * 0.6], [1.7, 1.7, 0.5], IRON);
                k.b([9.0, 6.8, sz * 2.7], [6.4, 0.8, 0.8], WOOD);
            }
            k.b([12.0, 6.8, 0.0], [0.8, 0.8, 6.2], WOOD);
            k.shift(0, [-3.0, 0.0, 0.0]);
        }
        HAGOROMO => {
            let silk = ma(0xF6EEF8, 0xF0B8D0, Pat::Dots);
            let trim = ma(0xFFFFFF, 0xA8D4FF, Pat::Dots);
            k.b([0.0, 6.6, 0.0], [6.4, 11.0, 1.2], silk);
            k.b([0.0, 10.6, 0.0], [14.6, 4.4, 1.0], silk);
            for sx in [-1.0, 1.0] {
                k.b([sx * 6.1, 7.4, 0.0], [2.4, 2.4, 1.0], silk);
                k.b([sx * 7.6, 10.6, 0.0], [0.9, 4.8, 1.3], trim);
                k.segz([sx * 1.9, 12.9], [0.0, 8.4], 0.4, 0.9, 0.9, GOLD);
            }
            k.b([0.0, 6.6, 0.0], [6.8, 1.8, 1.6], m(0xD070A0));
            k.b([0.0, 6.6, 0.6], [1.4, 1.2, 0.6], GOLD);
            k.b([0.0, 1.2, 0.0], [7.4, 1.5, 1.5], trim);
        }
        KOBAN => {
            let g = ma(0xE8B93A, 0xB8861E, Pat::Bands);
            k.b([0.0, 8.0, 0.0], [5.2, 11.4, 0.8], g);
            k.b([0.0, 8.0, 0.0], [6.6, 9.0, 0.8], g);
            k.b([0.0, 8.0, 0.0], [7.4, 6.0, 0.8], g);
            for y in [11.6, 4.4] {
                k.b([0.0, y, 0.0], [2.0, 1.5, 1.0], m(0xB07C1E));
            }
            k.b([0.0, 8.0, 0.0], [1.2, 2.2, 1.0], m(0xFFE38A));
        }
        YUMI => {
            // Arc long asymetrique : la poignee est au tiers inferieur.
            let pts = [[1.6, 0.2], [-0.4, 2.4], [-1.5, 5.0], [-1.9, 8.0], [-1.4, 11.0], [-0.3, 13.6], [1.6, 15.8]];
            k.line(&pts, 0.95, 0.95, mp(0x3A2A2A, Pat::Metal));
            for i in [1, 2, 4, 5] {
                k.b([pts[i][0], pts[i][1], 0.0], [1.4, 0.7, 1.4], RED);
            }
            k.r([-1.7, 6.4, 0.0], [1.5, 2.4, 1.5], ma(0xECE2C6, 0x8A7A5A, Pat::Wrap), [0.0, 0.0, 6.0]);
            k.seg([1.5, 0.5], [1.5, 15.5], 0.5, 0.5, m(0xECE2C6));
            for p in [pts[0], pts[6]] {
                k.b([p[0], p[1], 0.0], [1.2, 0.8, 1.2], GOLD);
            }
        }
        YARI => {
            k.b([0.0, 5.8, 0.0], [1.1, 10.4, 1.1], LACQUER);
            k.b([0.0, 0.4, 0.0], [1.5, 0.8, 1.5], IRON);
            k.b([0.0, 10.2, 0.0], [1.8, 1.0, 1.8], RED);
            k.b([0.0, 11.1, 0.0], [1.6, 0.6, 1.6], GOLD);
            k.b([0.0, 13.0, 0.0], [1.6, 3.2, 0.5], STEEL);
            k.b([0.0, 13.2, 0.0], [0.5, 3.4, 0.8], mp(0x9AA2B2, Pat::Metal));
            k.b([0.0, 15.0, 0.0], [1.0, 1.0, 0.45], STEEL);
            k.b([0.0, 15.8, 0.0], [0.5, 0.6, 0.4], STEEL);
        }
        NAGINATA => {
            k.b([0.0, 5.0, 0.0], [1.1, 9.2, 1.1], BLACK);
            k.b([0.0, 0.4, 0.0], [1.5, 0.8, 1.5], GOLD);
            for y in [3.0, 6.6] {
                k.b([0.0, y, 0.0], [1.4, 0.6, 1.4], GOLD);
            }
            k.b([0.0, 9.5, 0.0], [1.5, 0.8, 1.5], RED);
            k.b([0.0, 10.1, 0.0], [2.4, 0.5, 1.9], GOLD);
            blade(k, &[[0.0, 10.3], [-0.1, 12.2], [-0.8, 14.0], [-2.0, 15.5]], 2.0, 2);
        }
        TANTO => {
            let y = hilt(k, 3.0, 2.6, 2);
            blade(k, &[[0.0, y], [-0.1, y + 2.6], [-0.5, y + 4.8]], 1.4, 2);
        }
        TETSUBO => {
            k.b([0.0, 0.4, 0.0], [2.2, 0.8, 2.2], STEEL);
            k.b([0.0, 2.9, 0.0], [1.4, 4.4, 1.4], ma(0x2C2A36, 0xC0362C, Pat::Wrap));
            k.b([0.0, 5.3, 0.0], [2.1, 0.6, 2.1], GOLD);
            let iron = mp(0x4A4E5A, Pat::Speck);
            for (y, h, w) in [(7.1, 3.2, 2.3), (10.2, 3.2, 2.9), (13.4, 3.4, 3.5)] {
                k.b([0.0, y, 0.0], [w, h, w], iron);
                // Clous : deux rangs par troncon, sur les quatre faces.
                for dy in [-0.8, 0.8] {
                    for (sx, sz) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                        k.b([sx * w / 2.0, y + dy, sz * w / 2.0], [0.8, 0.8, 0.8], STEEL);
                    }
                }
            }
            k.b([0.0, 15.4, 0.0], [2.6, 0.7, 2.6], iron);
        }
        KUNAI => {
            let iron = mp(0x565C6A, Pat::Metal);
            k.ring([0.0, 1.3, 0.0], 2.6, 0.6, 0.6, iron);
            k.turn(0, [90.0, 0.0, 0.0], [0.0, 1.3, 0.0]);
            k.b([0.0, 4.2, 0.0], [0.9, 3.4, 0.9], ma(0x2C2A36, 0x6A6E7A, Pat::Bands));
            for (y, h, w) in [(5.9, 0.7, 1.6), (6.8, 1.3, 2.7), (8.2, 1.6, 2.1), (9.6, 1.3, 1.4), (10.7, 1.0, 0.7)] {
                k.b([0.0, y, 0.0], [w, h, 0.5], iron);
            }
            k.b([0.0, 8.2, 0.0], [0.5, 4.6, 0.8], STEEL);
        }
        SHIELD => {
            // Pavois tate : planches, traverses de fer, et le mon rouge au centre.
            k.b([0.0, 7.6, 0.0], [10.0, 14.4, 1.0], WOOD);
            k.b([0.0, 15.2, 0.0], [10.8, 1.0, 1.4], DARKWOOD);
            for y in [3.6, 11.8] {
                k.b([0.0, y, 0.55], [10.0, 0.9, 0.5], IRON);
                for x in [-4.0, 0.0, 4.0] {
                    k.b([x, y, 0.8], [0.6, 0.6, 0.4], STEEL);
                }
            }
            k.b([0.0, 7.7, 0.6], [4.4, 2.8, 0.3], RED);
            k.b([0.0, 7.7, 0.6], [2.8, 4.4, 0.3], RED);
            k.b([0.0, 7.7, 0.62], [3.6, 3.6, 0.3], RED);
            k.b([0.0, 7.7, 0.8], [1.4, 1.4, 0.2], WHITE);
            k.r([0.0, 5.6, -2.6], [1.0, 11.0, 0.8], DARKWOOD, [-24.0, 0.0, 0.0]);
        }
        COOKED_RICE => {
            let y = bowl(k, m(0xECF0F4), WHITE);
            k.hoop([0.0, 3.4, 0.0], 10.0, 0.6, 0.5, m(0x3A5A9A));
            k.disc([0.0, y + 0.5, 0.0], 8.6, 1.2, RICE);
            k.disc([0.0, y + 1.5, 0.0], 6.6, 1.2, RICE);
            k.disc([0.0, y + 2.4, 0.0], 4.0, 0.9, RICE);
            k.b([0.2, y + 3.0, 0.0], [1.3, 0.6, 1.3], m(0xC0362C));
        }
        MISO_SOUP => {
            let y = bowl(k, m(0x9A2E28), m(0x2C2A36));
            k.hoop([0.0, 5.2, 0.0], 10.7, 0.3, 0.9, GOLD);
            k.disc([0.0, y, 0.0], 8.8, 0.6, ma(0xC89A56, 0xE0BC80, Pat::Dots));
            k.b([-1.4, y + 0.5, -1.0], [1.5, 0.8, 1.5], WHITE);
            k.b([1.6, y + 0.5, 0.8], [1.5, 0.8, 1.5], WHITE);
            k.b([-1.6, y + 0.4, 1.8], [1.0, 0.5, 1.0], m(0x7AB84A));
            k.b([1.2, y + 0.4, -1.8], [1.0, 0.5, 1.0], m(0x7AB84A));
            k.b([0.2, y + 0.4, -0.2], [1.6, 0.4, 0.8], m(0x24382C));
        }
        BOWL => {
            bowl(k, m(0x9A6A42), m(0x5E3A26));
        }
        NABE => {
            // Marmite de terre (donabe), fumante de legumes.
            let clay = m(0x7A4E3A);
            k.disc([0.0, 0.5, 0.0], 8.0, 1.0, clay);
            k.disc([0.0, 2.0, 0.0], 12.0, 2.0, clay);
            k.hoop([0.0, 4.2, 0.0], 13.0, 2.4, 1.1, clay);
            for sx in [-1.0, 1.0] {
                k.b([sx * 6.6, 4.6, 0.0], [1.6, 1.0, 2.8], clay);
            }
            k.disc([0.0, 4.6, 0.0], 11.0, 0.6, ma(0xD8B060, 0xF0D8A0, Pat::Dots));
            k.b([-2.2, 5.2, -1.8], [3.2, 1.0, 2.8], ma(0xD8E8A8, 0xF4F8E0, Pat::Bands));
            k.b([2.0, 5.2, -2.0], [2.4, 0.9, 2.4], mp(0x8A5A3A, Pat::Plain));
            k.b([2.0, 5.7, -2.0], [1.2, 0.3, 0.4], m(0xF2E8C8));
            k.b([2.0, 5.7, -2.0], [0.4, 0.3, 1.2], m(0xF2E8C8));
            k.b([-2.0, 5.2, 2.2], [2.2, 1.0, 2.2], WHITE);
            k.b([1.2, 5.1, 2.0], [1.6, 0.6, 1.6], m(0xF08A24));
            k.r([3.2, 5.2, 0.6], [1.0, 0.9, 3.0], ma(0xE8F0C8, 0x7AB84A, Pat::Bands), [0.0, 30.0, 0.0]);
        }
        GREEN_TEA => {
            // Tasse yunomi en celadon ; un brin de the s'y tient debout, signe de chance.
            let cup = m(0x8FB8A4);
            k.disc([0.0, 0.4, 0.0], 4.6, 0.8, m(0x6A9A86));
            k.disc([0.0, 1.2, 0.0], 5.6, 0.8, cup);
            k.hoop([0.0, 4.2, 0.0], 7.4, 6.0, 0.9, cup);
            k.hoop([0.0, 3.0, 0.0], 7.7, 0.7, 0.5, m(0x3A5A9A));
            k.disc([0.0, 6.2, 0.0], 5.8, 0.5, m(0x8ABA3E));
            k.b([0.7, 6.8, 0.4], [0.5, 1.0, 0.5], m(0x4E7A2A));
        }
        TOFU => {
            k.b([0.0, 0.25, 0.0], [10.0, 0.5, 7.6], HINOKI);
            k.b([0.0, 2.7, 0.0], [8.0, 4.4, 6.0], m(0xF7F2E0));
            k.b([0.0, 5.1, 0.0], [1.8, 0.5, 1.8], m(0x7AB84A));
            k.b([0.3, 5.5, 0.2], [0.9, 0.4, 0.9], m(0xF2C84A));
        }
        EDAMAME => {
            let pod = mp(0x6AAA3E, Pat::Speck);
            for (a, b) in [([-6.0f32, 3.0f32], [5.0f32, 6.6f32]), ([-4.6, 9.6], [6.0, 8.0])] {
                k.seg(a, b, 1.7, 1.4, pod);
                for t in [0.22, 0.5, 0.78] {
                    k.b([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, 0.0], [2.3, 2.3, 1.9], pod);
                }
                k.seg(b, [b[0] + 1.4, b[1] + 0.9], 0.6, 0.6, DARKWOOD);
            }
        }
        RAW_BIRD => {
            let p = mp(0xF4B8A8, Pat::Speck);
            k.ball([-1.6, 4.4, 0.0], 3.6, p);
            k.b([2.0, 3.8, 0.0], [3.4, 3.4, 3.2], p);
            k.b([4.4, 3.4, 0.0], [2.2, 2.0, 2.0], p);
            k.seg([5.0, 3.2], [8.4, 2.0], 1.1, 1.1, IVORY);
            k.b([8.6, 2.6, 0.0], [1.3, 1.3, 1.3], IVORY);
            k.b([8.9, 1.4, 0.0], [1.3, 1.3, 1.3], IVORY);
            k.shift(0, [-2.6, 0.0, 0.0]);
        }
        YAKITORI => {
            k.b([0.0, 8.0, 0.0], [0.6, 16.0, 0.6], m(0xD8C48A));
            let glaze = ma(0xB8682E, 0x6E3A1A, Pat::Bands);
            let negi = ma(0xE4EEC0, 0x7AB84A, Pat::Bands);
            for (y, mat, w) in [(13.6, glaze, 3.0), (11.2, negi, 2.6), (8.9, glaze, 3.0), (6.6, negi, 2.6), (4.3, glaze, 3.0)] {
                k.b([0.0, y, 0.0], [w, 2.0, w * 0.86], mat);
            }
        }
        RAW_FISH => fish(k, ma(0x3F7AB8, 0x4E8CCB, Pat::Scales), m(0xE4ECF4), m(0x2E5A8E)),
        COOKED_FISH => {
            fish(k, ma(0xB8824A, 0x5E3420, Pat::Bands), m(0xE8C890), m(0x8A5A30));
            k.b([0.0, 5.0, 0.0], [17.0, 0.5, 0.5], m(0xD8C48A));
        }
        FUGU => {
            // Poisson-globe : dos tachete, ventre blanc, piquants et petites nageoires.
            let skin = ma(0xD8B04A, 0x5E4420, Pat::Dots);
            k.ball([0.0, 5.0, 0.0], 4.4, skin);
            k.disc([0.0, 2.0, 0.0], 7.4, 1.6, m(0xF7F2E0));
            k.disc([0.0, 3.4, 0.0], 9.0, 1.3, m(0xF7F2E0));
            for (x, y, z) in [(0.0, 9.6, 0.0), (-2.6, 8.8, 1.6), (2.4, 8.9, -1.4), (1.6, 8.4, 3.0), (-1.4, 8.4, -3.0), (-0.6, 6.4, 4.6), (2.8, 6.4, 3.4)] {
                k.b([x, y, z], [0.6, 0.7, 0.6], m(0xF7F2E0));
            }
            k.r([5.4, 6.4, 0.0], [2.2, 1.6, 0.5], m(0x8A6A2A), [0.0, 0.0, 30.0]);
            k.r([5.4, 4.2, 0.0], [2.2, 1.6, 0.5], m(0x8A6A2A), [0.0, 0.0, -30.0]);
            k.b([0.4, 5.0, 4.5], [2.0, 1.4, 0.5], m(0x8A6A2A));
            k.b([-3.0, 6.6, 3.2], [1.5, 1.5, 0.5], WHITE);
            k.b([-3.1, 6.6, 3.4], [0.8, 0.8, 0.4], INK);
            k.b([-4.6, 4.6, 0.0], [0.7, 1.0, 1.6], m(0xE88A8A));
        }
        SUSHI => {
            // Deux nigiri sur leur planche (geta).
            k.b([0.0, 1.6, 0.0], [12.0, 0.8, 6.0], HINOKI);
            for sx in [-1.0, 1.0] {
                k.b([sx * 4.2, 0.6, 0.0], [1.0, 1.2, 6.0], HINOKI);
            }
            k.b([-2.9, 2.9, 0.0], [4.0, 1.8, 3.0], RICE);
            k.b([-2.9, 4.1, 0.0], [4.6, 0.9, 3.4], ma(0xF08A5A, 0xFFD9C0, Pat::Bands));
            k.b([2.9, 2.9, 0.0], [4.0, 1.8, 3.0], RICE);
            k.b([2.9, 4.1, 0.0], [4.6, 0.9, 3.4], m(0xF2C84A));
            k.b([2.9, 3.5, 0.0], [1.2, 2.5, 3.6], SEAWEED);
            k.b([5.0, 2.5, 2.0], [1.4, 1.0, 1.4], m(0xF4B8C0));
        }
        NORI => {
            k.r([0.0, 0.4, 0.0], [9.0, 0.5, 8.0], SEAWEED, [0.0, 8.0, 0.0]);
            k.r([0.6, 0.9, 0.3], [9.0, 0.5, 8.0], SEAWEED, [0.0, -6.0, 0.0]);
            k.r([-0.3, 1.4, 0.0], [9.0, 0.5, 8.0], SEAWEED, [0.0, 16.0, 0.0]);
        }
        TEA_LEAF => {
            k.seg([0.0, 0.4], [0.0, 9.0], 0.7, 0.7, m(0x6A7A34));
            k.seg([0.0, 3.0], [-4.6, 7.2], 2.6, 0.5, LEAF);
            k.seg([0.0, 5.2], [4.6, 9.6], 2.6, 0.5, LEAF);
            k.seg([0.0, 8.4], [-3.2, 13.0], 2.4, 0.5, LEAF);
            k.seg([0.0, 9.0], [0.8, 14.0], 1.7, 0.5, mp(0x8AC85A, Pat::Grain));
        }
        KABOCHA_SLICE => wedge(k, mp(0x2E5A34, Pat::Speck), mp(0xF09A2A, Pat::Speck)),
        ROAST_KABOCHA => wedge(k, mp(0x26402A, Pat::Speck), ma(0xD87A22, 0x7A4420, Pat::Dots)),
        TAMAGOYAKI => {
            let egg = ma(0xF2C84A, 0xC8922A, Pat::Bands);
            for i in 0..3 {
                let x = -3.5 + i as f32 * 3.5;
                k.b([x, 2.6, 0.0], [3.1, 4.4, 5.4], egg);
                k.b([x, 4.95, 0.0], [3.1, 0.3, 5.4], m(0xD8922A));
            }
            k.b([6.4, 1.0, 1.4], [2.4, 2.0, 2.4], WHITE);
            k.b([6.4, 2.2, 1.4], [1.2, 0.5, 1.2], WHITE);
        }
        BENTO => {
            k.b([0.0, 0.5, 0.0], [12.0, 1.0, 8.0], BLACK);
            k.ring([0.0, 2.3, 0.0], 8.0, 2.6, 0.7, BLACK);
            // La couronne est carree : on l'etire en rectangle avec deux flancs.
            for sx in [-1.0, 1.0] {
                k.b([sx * 5.65, 2.3, 0.0], [0.7, 2.6, 8.0], BLACK);
                k.b([sx * 4.6, 2.3, -3.65], [1.6, 2.6, 0.7], BLACK);
                k.b([sx * 4.6, 2.3, 3.65], [1.6, 2.6, 0.7], BLACK);
            }
            k.b([0.4, 2.3, 0.0], [0.5, 2.4, 6.8], RED);
            k.b([-2.6, 2.6, 0.0], [5.4, 1.4, 6.6], RICE);
            k.b([-2.6, 3.5, 0.0], [1.4, 0.6, 1.4], RED);
            k.b([1.9, 2.8, -1.9], [2.2, 1.6, 2.6], ma(0xF2C84A, 0xC8922A, Pat::Bands));
            k.b([4.1, 2.7, -1.9], [1.9, 1.4, 2.6], ma(0xF08A5A, 0xFFD9C0, Pat::Bands));
            k.b([3.0, 2.7, 1.7], [4.4, 1.4, 2.8], ma(0x4E9A3E, 0x2E6A2A, Pat::Dots));
            k.b([2.4, 3.5, 1.6], [1.2, 0.4, 1.2], m(0xF08A24));
            for z in [4.5, 5.2] {
                k.b([0.0, 1.3, z], [11.0, 0.45, 0.45], LACQUER);
            }
        }
        SHEARS => {
            // Ciseaux japonais d'une seule piece, en U.
            let iron = mp(0x565C6A, Pat::Metal);
            k.line(&[[-1.1, 3.0], [-1.3, 1.0], [0.0, 0.3], [1.3, 1.0], [1.1, 3.0]], 0.9, 0.8, iron);
            k.seg([-1.1, 2.6], [-1.0, 8.4], 0.9, 0.8, iron);
            k.seg([1.1, 2.6], [1.0, 8.4], 0.9, 0.8, iron);
            k.segz([-1.0, 8.0], [-0.1, 15.0], -0.25, 1.5, 0.4, STEEL);
            k.segz([1.0, 8.0], [0.1, 15.0], 0.25, 1.5, 0.4, STEEL);
        }
        FISHING_ROD => {
            k.b([0.0, 4.4, 0.0], [1.0, 8.4, 1.0], BAMBOO);
            k.b([0.0, 12.0, 0.0], [0.7, 7.4, 0.7], BAMBOO);
            k.b([0.0, 1.9, 0.0], [1.4, 3.4, 1.4], ma(0x3A2A22, 0xECE2C6, Pat::Wrap));
            k.b([0.0, 0.3, 0.0], [1.5, 0.6, 1.5], GOLD);
            k.seg([0.2, 15.5], [4.6, 8.8], 0.5, 0.5, m(0xECE2C6));
            k.r([4.9, 7.6, 0.0], [1.4, 1.6, 1.4], RED, [0.0, 0.0, -24.0]);
            k.r([4.5, 6.6, 0.0], [1.2, 0.7, 1.2], WHITE, [0.0, 0.0, -24.0]);
            k.r([4.1, 5.6, 0.0], [0.5, 1.2, 0.5], STEEL, [0.0, 0.0, -24.0]);
        }
        _ => {
            // Objet sans modele propre : un petit ballot de papier ficele.
            k.b([0.0, 3.0, 0.0], [7.0, 6.0, 7.0], PAPER);
            k.b([0.0, 3.0, 0.0], [7.3, 0.8, 7.3], RED);
            k.b([0.0, 3.0, 0.0], [0.8, 6.3, 7.3], RED);
        }
    }
    kit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_item_falls_back_to_the_parcel() {
        let parcel = design(9999).v.len();
        for id in crate::items3d::ids() {
            let kit = design(id);
            // Le ballot de secours a trois pieces de papier : aucun vrai objet ne doit lui ressembler.
            let is_parcel = kit.v.len() == parcel && kit.v[0].s == glam::Vec3::new(7.0, 6.0, 7.0);
            assert!(!is_parcel, "{} n'a pas de modele", name(id));
        }
        assert_eq!(BAMBOO.pat, Pat::Bands);
    }
}
