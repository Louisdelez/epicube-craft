//! Modeles des creatures et du joueur : des boites texturees, eventuellement inclinees.
//! Le bestiaire s'inspire du folklore japonais : animaux des campagnes, yokai et demons.
//! Repere local : x a droite, y vers le haut, z vers l'avant, origine aux pieds, en blocs.
//! Chaque modele dispose d'une case de 256 x 256 pixels dans l'atlas, ou ses faces sont rangees.
use crate::atlas::{Img, AT};
use glam::Vec3;
use shared::mobs::*;
use shared::util::hash;

/// Ligne de pixels de l'atlas ou commencent les textures des modeles.
pub const MODEL_Y: usize = 512;
pub const CELL: usize = 256;
/// Indice du modele du joueur (les creatures utilisent leur genre).
pub const PLAYER: usize = 11;
/// Noms des fichiers de modeles, dans l'ordre des genres de creatures.
pub const NAMES: [&str; 14] = [
    "cerf_sika",
    "sanglier",
    "tanuki",
    "grue",
    "oni",
    "samourai_squelette",
    "tsuchigumo",
    "lanterne_hantee",
    "yurei",
    "wanyudo",
    "ryujin",
    "joueur",
    "chariot_daihachi",
    "villageois",
];

#[derive(Clone)]
pub struct Part {
    pub c: Vec3,
    pub h: Vec3,
    /// Inclinaison en degres autour de x, y, z, appliquee autour de `pivot`.
    pub rot: Vec3,
    pub pivot: Vec3,
    /// 0 = fixe ; +1 / -1 = membre qui se balance en marchant, autour de `pivot`.
    pub leg: i8,
    /// Rectangle (u0, v0, u1, v1) de chaque face dans l'atlas. Faces : +X, -X, +Y, -Y, +Z, -Z.
    pub uv: [[f32; 4]; 6],
}

#[derive(Clone, Default)]
pub struct Model {
    pub parts: Vec<Part>,
}

/// Boite decrite par le code, avant de recevoir sa texture.
pub struct Raw {
    pub c: Vec3,
    pub h: Vec3,
    pub color: [u8; 3],
    pub rot: Vec3,
    pub pivot: Vec3,
    pub leg: i8,
}

type Col = [u8; 3];
const WHITE: Col = [255, 255, 255];
const BLACK: Col = [28, 24, 36];

/// Boite droite.
fn b(v: &mut Vec<Raw>, c: [f32; 3], h: [f32; 3], color: Col) {
    v.push(Raw { c: Vec3::from(c), h: Vec3::from(h), color, rot: Vec3::ZERO, pivot: Vec3::from(c), leg: 0 });
}

/// Boite inclinee de `rot` degres autour de son centre.
fn r(v: &mut Vec<Raw>, c: [f32; 3], h: [f32; 3], color: Col, rot: [f32; 3]) {
    v.push(Raw { c: Vec3::from(c), h: Vec3::from(h), color, rot: Vec3::from(rot), pivot: Vec3::from(c), leg: 0 });
}

/// La meme boite de chaque cote (x et -x).
fn pair(v: &mut Vec<Raw>, c: [f32; 3], h: [f32; 3], color: Col) {
    b(v, c, h, color);
    b(v, [-c[0], c[1], c[2]], h, color);
}

/// Deux boites inclinees, en miroir de part et d'autre.
fn pair_r(v: &mut Vec<Raw>, c: [f32; 3], h: [f32; 3], color: Col, rot: [f32; 3]) {
    r(v, c, h, color, rot);
    r(v, [-c[0], c[1], c[2]], h, color, [rot[0], -rot[1], -rot[2]]);
}

/// Deux membres opposes qui se balancent autour de leur sommet ; `phase` inverse le pas.
fn legs(v: &mut Vec<Raw>, c: [f32; 3], h: [f32; 3], color: Col, phase: i8) {
    for (x, leg) in [(c[0], phase), (-c[0], -phase)] {
        let top = Vec3::new(x, c[1] + h[1], c[2]);
        v.push(Raw { c: Vec3::new(x, c[1], c[2]), h: Vec3::from(h), color, rot: Vec3::ZERO, pivot: top, leg });
    }
}

/// Comme `legs`, pour une piece attachee plus bas (sabot, chaussure) : `hip` est la hauteur de la hanche.
fn feet(v: &mut Vec<Raw>, c: [f32; 3], h: [f32; 3], color: Col, phase: i8, hip: f32) {
    for (x, leg) in [(c[0], phase), (-c[0], -phase)] {
        let pivot = Vec3::new(x, hip, c[2]);
        v.push(Raw { c: Vec3::new(x, c[1], c[2]), h: Vec3::from(h), color, rot: Vec3::ZERO, pivot, leg });
    }
}

/// Deux yeux de face : blanc, iris, pupille et reflet.
fn eyes(v: &mut Vec<Raw>, x: f32, y: f32, z: f32, size: f32, iris: Col) {
    pair(v, [x, y, z], [size, size * 1.1, 0.004], WHITE);
    pair(v, [x, y - size * 0.1, z + 0.004], [size * 0.68, size * 0.9, 0.004], iris);
    pair(v, [x, y - size * 0.2, z + 0.008], [size * 0.36, size * 0.56, 0.004], BLACK);
    for sx in [x + size * 0.3, -x + size * 0.3] {
        b(v, [sx, y + size * 0.42, z + 0.012], [size * 0.22, size * 0.22, 0.003], WHITE);
    }
}

fn deer(v: &mut Vec<Raw>) {
    // Cerf sika : robe fauve tachetee de blanc, bois clairs.
    let (coat, cream, antler) = ([204, 132, 74], [246, 226, 198], [240, 226, 194]);
    b(v, [0.0, 0.98, 0.0], [0.28, 0.26, 0.55], coat);
    b(v, [0.0, 0.75, 0.0], [0.24, 0.05, 0.5], cream);
    for (x, z) in [(0.12, 0.25), (-0.1, 0.05), (0.14, -0.2), (-0.13, -0.32), (0.0, 0.42)] {
        b(v, [x, 1.243, z], [0.045, 0.004, 0.045], WHITE);
    }
    r(v, [0.0, 1.32, 0.5], [0.13, 0.3, 0.13], coat, [28.0, 0.0, 0.0]);
    b(v, [0.0, 1.66, 0.74], [0.15, 0.14, 0.2], coat);
    b(v, [0.0, 1.6, 0.98], [0.09, 0.08, 0.07], cream);
    b(v, [0.0, 1.64, 1.052], [0.045, 0.035, 0.012], BLACK);
    pair(v, [0.153, 1.7, 0.8], [0.005, 0.045, 0.045], BLACK);
    pair(v, [0.158, 1.72, 0.82], [0.004, 0.015, 0.015], WHITE);
    pair_r(v, [0.2, 1.84, 0.62], [0.035, 0.09, 0.06], coat, [0.0, 0.0, -35.0]);
    pair_r(v, [0.1, 2.02, 0.64], [0.025, 0.22, 0.025], antler, [-12.0, 0.0, -18.0]);
    pair_r(v, [0.22, 2.12, 0.7], [0.02, 0.1, 0.02], antler, [35.0, 0.0, -45.0]);
    pair_r(v, [0.2, 2.26, 0.56], [0.02, 0.09, 0.02], antler, [-35.0, 0.0, -40.0]);
    b(v, [0.0, 1.1, -0.59], [0.07, 0.09, 0.05], WHITE);
    for (z, phase) in [(0.4, 1), (-0.4, -1)] {
        legs(v, [0.19, 0.4, z], [0.065, 0.34, 0.065], coat, phase);
        feet(v, [0.19, 0.04, z], [0.07, 0.04, 0.07], BLACK, phase, 0.74);
    }
}

fn boar(v: &mut Vec<Raw>) {
    // Sanglier : trapu, criniere sombre et defenses.
    let (fur, dark, snout) = ([122, 86, 62], [70, 48, 40], [198, 138, 126]);
    b(v, [0.0, 0.58, 0.0], [0.34, 0.3, 0.52], fur);
    b(v, [0.0, 0.92, -0.05], [0.09, 0.06, 0.46], dark);
    b(v, [0.0, 0.56, 0.64], [0.26, 0.24, 0.2], fur);
    b(v, [0.0, 0.48, 0.88], [0.13, 0.1, 0.07], snout);
    pair(v, [0.05, 0.49, 0.952], [0.025, 0.03, 0.004], dark);
    pair_r(v, [0.15, 0.52, 0.9], [0.025, 0.085, 0.025], WHITE, [-10.0, 0.0, -22.0]);
    pair_r(v, [0.2, 0.86, 0.56], [0.06, 0.08, 0.03], dark, [0.0, 0.0, -28.0]);
    eyes(v, 0.14, 0.68, 0.842, 0.045, [120, 70, 40]);
    r(v, [0.0, 0.7, -0.56], [0.03, 0.03, 0.08], dark, [-40.0, 0.0, 0.0]);
    for (z, phase) in [(0.34, 1), (-0.34, -1)] {
        legs(v, [0.22, 0.15, z], [0.08, 0.15, 0.08], dark, phase);
    }
}

fn tanuki(v: &mut Vec<Raw>) {
    // Tanuki : masque sombre, gros ventre clair, queue touffue et feuille sur la tete.
    let (fur, dark, cream) = ([160, 124, 92], [64, 50, 48], [240, 218, 184]);
    b(v, [0.0, 0.44, 0.0], [0.26, 0.24, 0.32], fur);
    b(v, [0.0, 0.38, 0.2], [0.2, 0.17, 0.14], cream);
    b(v, [0.0, 0.8, 0.28], [0.24, 0.2, 0.2], fur);
    pair(v, [0.11, 0.82, 0.482], [0.09, 0.075, 0.004], dark);
    eyes(v, 0.11, 0.82, 0.488, 0.045, [90, 60, 40]);
    b(v, [0.0, 0.71, 0.5], [0.08, 0.06, 0.04], cream);
    b(v, [0.0, 0.74, 0.545], [0.03, 0.025, 0.01], BLACK);
    pair(v, [0.17, 1.04, 0.26], [0.06, 0.06, 0.04], fur);
    pair(v, [0.17, 1.08, 0.262], [0.035, 0.03, 0.04], dark);
    r(v, [0.02, 1.02, 0.3], [0.09, 0.012, 0.06], [96, 196, 96], [0.0, 30.0, 12.0]);
    r(v, [0.0, 0.5, -0.5], [0.15, 0.15, 0.24], fur, [-32.0, 0.0, 0.0]);
    r(v, [0.0, 0.62, -0.66], [0.155, 0.155, 0.06], dark, [-32.0, 0.0, 0.0]);
    for (z, phase) in [(0.2, 1), (-0.2, -1)] {
        legs(v, [0.16, 0.11, z], [0.07, 0.11, 0.07], dark, phase);
    }
}

fn crane(v: &mut Vec<Raw>) {
    // Grue du Japon : plumage blanc, cou et queue noirs, calotte rouge.
    let (plume, grey, red) = ([250, 250, 255], [150, 154, 170], [226, 50, 60]);
    b(v, [0.0, 0.76, 0.0], [0.16, 0.14, 0.26], plume);
    r(v, [0.0, 0.84, -0.32], [0.12, 0.05, 0.14], BLACK, [-28.0, 0.0, 0.0]);
    pair(v, [0.17, 0.78, -0.02], [0.02, 0.1, 0.22], [232, 234, 244]);
    r(v, [0.0, 1.04, 0.24], [0.045, 0.22, 0.045], BLACK, [18.0, 0.0, 0.0]);
    b(v, [0.0, 1.3, 0.32], [0.07, 0.07, 0.09], plume);
    b(v, [0.0, 1.375, 0.31], [0.05, 0.012, 0.06], red);
    b(v, [0.0, 1.28, 0.48], [0.02, 0.02, 0.1], [236, 204, 96]);
    pair(v, [0.073, 1.31, 0.35], [0.004, 0.022, 0.022], BLACK);
    legs(v, [0.07, 0.32, 0.0], [0.02, 0.3, 0.02], grey, 1);
    feet(v, [0.07, 0.015, 0.04], [0.035, 0.015, 0.07], grey, 1, 0.62);
}

fn oni(v: &mut Vec<Raw>) {
    // Oni : demon a la peau rouge, cornes, pagne en peau de tigre et massue cloutee.
    let (skin, horn, tiger, wood) = ([220, 62, 56], [246, 234, 204], [244, 190, 60], [112, 78, 56]);
    legs(v, [0.15, 0.36, 0.0], [0.13, 0.36, 0.14], skin, 1);
    b(v, [0.0, 0.74, 0.0], [0.3, 0.14, 0.17], tiger);
    for x in [-0.18, 0.0, 0.18] {
        r(v, [x, 0.74, 0.172], [0.025, 0.12, 0.004], BLACK, [0.0, 0.0, 14.0]);
    }
    b(v, [0.0, 1.22, 0.0], [0.32, 0.36, 0.18], skin);
    b(v, [0.0, 1.82, 0.0], [0.26, 0.24, 0.24], skin);
    b(v, [0.0, 2.05, -0.03], [0.28, 0.06, 0.27], BLACK);
    b(v, [0.0, 1.84, -0.25], [0.28, 0.24, 0.05], BLACK);
    pair(v, [0.27, 1.9, -0.06], [0.03, 0.16, 0.2], BLACK);
    pair_r(v, [0.14, 2.2, 0.1], [0.045, 0.11, 0.045], horn, [10.0, 0.0, -14.0]);
    eyes(v, 0.11, 1.86, 0.242, 0.058, [255, 220, 70]);
    pair_r(v, [0.11, 1.95, 0.246], [0.08, 0.02, 0.006], BLACK, [0.0, 0.0, 22.0]);
    b(v, [0.0, 1.68, 0.243], [0.13, 0.035, 0.005], [96, 20, 28]);
    pair(v, [0.085, 1.705, 0.249], [0.022, 0.04, 0.005], WHITE);
    pair(v, [0.43, 1.2, 0.0], [0.11, 0.36, 0.12], skin);
    // La massue, tenue vers l'avant.
    r(v, [0.43, 0.98, 0.42], [0.07, 0.07, 0.42], wood, [-18.0, 0.0, 0.0]);
    for z in [0.5, 0.68, 0.84] {
        r(v, [0.43, 0.98 + (z - 0.42) * 0.32 + 0.075, z], [0.025, 0.02, 0.025], [200, 204, 216], [-18.0, 0.0, 0.0]);
    }
}

fn samurai(v: &mut Vec<Raw>) {
    // Samourai squelette : armure laquee rouge, casque kabuto a cornes dorees, arc.
    let (bone, armor, navy, gold) = ([232, 230, 218], [180, 42, 52], [42, 48, 92], [252, 208, 72]);
    legs(v, [0.1, 0.36, 0.0], [0.06, 0.36, 0.06], bone, 1);
    b(v, [0.0, 0.8, 0.0], [0.24, 0.1, 0.13], armor);
    b(v, [0.0, 1.12, 0.0], [0.22, 0.26, 0.12], armor);
    for y in [1.0, 1.14, 1.28] {
        b(v, [0.0, y, 0.122], [0.2, 0.018, 0.004], gold);
    }
    pair(v, [0.32, 1.3, 0.0], [0.09, 0.1, 0.13], navy);
    pair(v, [0.32, 0.98, 0.0], [0.05, 0.24, 0.05], bone);
    b(v, [0.0, 1.66, 0.0], [0.22, 0.2, 0.22], bone);
    pair(v, [0.09, 1.69, 0.222], [0.058, 0.058, 0.005], BLACK);
    pair(v, [0.09, 1.69, 0.228], [0.022, 0.022, 0.004], [255, 60, 60]);
    b(v, [0.0, 1.6, 0.222], [0.025, 0.03, 0.005], BLACK);
    b(v, [0.0, 1.52, 0.222], [0.11, 0.03, 0.005], [196, 194, 184]);
    for x in [-0.055, 0.0, 0.055] {
        b(v, [x, 1.52, 0.227], [0.005, 0.03, 0.003], BLACK);
    }
    b(v, [0.0, 1.9, 0.0], [0.25, 0.08, 0.25], navy);
    r(v, [0.0, 1.85, 0.28], [0.24, 0.015, 0.08], navy, [22.0, 0.0, 0.0]);
    r(v, [0.0, 1.72, -0.27], [0.26, 0.13, 0.03], navy, [-16.0, 0.0, 0.0]);
    pair_r(v, [0.28, 1.76, -0.02], [0.03, 0.1, 0.2], navy, [0.0, 0.0, -14.0]);
    pair_r(v, [0.13, 2.1, 0.2], [0.03, 0.15, 0.015], gold, [0.0, 0.0, -26.0]);
    b(v, [0.0, 1.99, 0.24], [0.04, 0.04, 0.015], gold);
    // Arc.
    b(v, [0.34, 1.02, 0.2], [0.02, 0.42, 0.02], [140, 96, 60]);
    b(v, [0.34, 1.02, 0.14], [0.005, 0.4, 0.005], WHITE);
}

fn tsuchigumo(v: &mut Vec<Raw>) {
    // Tsuchigumo : araignee yokai a l'abdomen raye et au visage de demon.
    let (dark, head, stripe) = ([64, 46, 60], [96, 68, 84], [244, 150, 50]);
    b(v, [0.0, 0.46, -0.3], [0.42, 0.3, 0.45], dark);
    for z in [-0.58, -0.36, -0.14] {
        b(v, [0.0, 0.763, z], [0.42, 0.004, 0.06], stripe);
    }
    b(v, [0.0, 0.4, 0.36], [0.28, 0.22, 0.22], head);
    eyes(v, 0.12, 0.47, 0.582, 0.06, [255, 60, 60]);
    pair(v, [0.22, 0.34, 0.582], [0.03, 0.03, 0.005], [255, 60, 60]);
    pair_r(v, [0.12, 0.57, 0.586], [0.08, 0.018, 0.006], BLACK, [0.0, 0.0, 20.0]);
    pair(v, [0.08, 0.23, 0.57], [0.03, 0.07, 0.03], WHITE);
    for (k, z) in [-0.32, -0.12, 0.08, 0.28].into_iter().enumerate() {
        let phase = if k % 2 == 0 { 1 } else { -1 };
        // Chaque patte : une cuisse qui monte, un tibia qui redescend.
        for (x, leg) in [(1.0, phase), (-1.0, -phase)] {
            let hip = Vec3::new(0.38 * x, 0.42, z);
            v.push(Raw { c: Vec3::new(0.62 * x, 0.52, z), h: Vec3::new(0.26, 0.04, 0.04), color: BLACK, rot: Vec3::new(0.0, 0.0, 24.0 * x), pivot: hip, leg });
            v.push(Raw { c: Vec3::new(0.9 * x, 0.3, z), h: Vec3::new(0.04, 0.3, 0.04), color: BLACK, rot: Vec3::new(0.0, 0.0, -12.0 * x), pivot: hip, leg });
        }
    }
}

fn lantern(v: &mut Vec<Raw>) {
    // Lanterne hantee (chochin-obake) : papier dechire, un oeil unique et une longue langue.
    let (paper, rib, cap) = ([255, 238, 194], [160, 52, 44], [44, 38, 48]);
    b(v, [0.0, 0.78, 0.0], [0.3, 0.36, 0.3], paper);
    for y in [0.56, 0.72, 0.9, 1.04] {
        b(v, [0.0, y, 0.0], [0.306, 0.012, 0.306], rib);
    }
    b(v, [0.0, 1.18, 0.0], [0.2, 0.05, 0.2], cap);
    b(v, [0.0, 0.38, 0.0], [0.2, 0.05, 0.2], cap);
    b(v, [0.0, 1.3, 0.0], [0.03, 0.08, 0.03], cap);
    // L'oeil.
    b(v, [0.0, 0.94, 0.302], [0.13, 0.11, 0.005], WHITE);
    b(v, [0.0, 0.93, 0.308], [0.075, 0.085, 0.004], [255, 196, 60]);
    b(v, [0.0, 0.92, 0.313], [0.035, 0.065, 0.004], BLACK);
    b(v, [0.04, 0.98, 0.318], [0.025, 0.025, 0.003], WHITE);
    // La gueule et la langue.
    b(v, [0.0, 0.62, 0.302], [0.21, 0.07, 0.005], [70, 16, 24]);
    for x in [-0.14, -0.05, 0.05, 0.14] {
        b(v, [x, 0.665, 0.308], [0.03, 0.025, 0.004], WHITE);
    }
    r(v, [0.0, 0.5, 0.38], [0.07, 0.02, 0.15], [244, 84, 96], [28.0, 0.0, 0.0]);
}

fn yurei(v: &mut Vec<Raw>) {
    // Yurei : fantome en kimono blanc, longs cheveux noirs, sans jambes.
    let (robe, mist, pale, obi) = ([244, 246, 255], [204, 216, 250], [228, 234, 246], [150, 160, 214]);
    b(v, [0.0, 1.5, 0.0], [0.2, 0.4, 0.12], robe);
    b(v, [0.0, 1.22, 0.0], [0.206, 0.05, 0.126], obi);
    b(v, [0.0, 0.9, 0.0], [0.17, 0.22, 0.1], robe);
    b(v, [0.0, 0.56, -0.03], [0.12, 0.15, 0.08], mist);
    r(v, [0.0, 0.3, -0.1], [0.07, 0.13, 0.05], mist, [-28.0, 0.0, 0.0]);
    // Bras tendus, mains pendantes.
    pair_r(v, [0.27, 1.56, 0.16], [0.06, 0.07, 0.24], robe, [14.0, 0.0, 0.0]);
    pair(v, [0.27, 1.36, 0.4], [0.045, 0.09, 0.04], pale);
    b(v, [0.0, 2.14, 0.0], [0.2, 0.21, 0.2], pale);
    b(v, [0.0, 2.37, -0.01], [0.22, 0.035, 0.22], BLACK);
    b(v, [0.0, 1.82, -0.2], [0.22, 0.52, 0.05], BLACK);
    pair(v, [0.21, 1.94, 0.02], [0.03, 0.4, 0.16], BLACK);
    // Une meche cache un oeil ; l'autre fixe le joueur.
    b(v, [0.09, 2.2, 0.206], [0.12, 0.13, 0.01], BLACK);
    b(v, [-0.1, 2.15, 0.203], [0.055, 0.045, 0.004], WHITE);
    b(v, [-0.1, 2.14, 0.208], [0.025, 0.035, 0.004], [200, 40, 60]);
    b(v, [0.0, 2.0, 0.203], [0.04, 0.012, 0.004], [120, 70, 110]);
    // Bandeau triangulaire des defunts.
    b(v, [0.0, 2.33, 0.21], [0.07, 0.04, 0.012], WHITE);
}

fn wanyudo(v: &mut Vec<Raw>) {
    // Wanyudo : roue de char enflammee portant un visage furieux en son moyeu.
    let (wood, face, fire, glow) = ([76, 54, 48], [226, 150, 118], [255, 140, 40], [255, 232, 110]);
    let (cy, radius) = (0.75, 0.52);
    for k in 0..8 {
        let a = k as f32 * 45.0;
        let (s, c) = a.to_radians().sin_cos();
        r(v, [c * radius, cy + s * radius, 0.0], [0.07, 0.22, 0.09], wood, [0.0, 0.0, a]);
        // Une flamme entre deux jantes.
        let (s2, c2) = (a + 22.5).to_radians().sin_cos();
        r(v, [c2 * 0.68, cy + s2 * 0.68, 0.0], [0.08, 0.13, 0.06], fire, [0.0, 0.0, a + 22.5 - 90.0]);
        r(v, [c2 * 0.8, cy + s2 * 0.8, 0.0], [0.045, 0.07, 0.04], glow, [0.0, 0.0, a + 22.5 - 90.0]);
    }
    for k in 0..4 {
        r(v, [0.0, cy, 0.0], [0.03, 0.46, 0.03], wood, [0.0, 0.0, k as f32 * 45.0]);
    }
    b(v, [0.0, cy, 0.03], [0.22, 0.22, 0.1], face);
    eyes(v, 0.09, cy + 0.05, 0.132, 0.05, [255, 220, 70]);
    pair_r(v, [0.09, cy + 0.13, 0.137], [0.07, 0.018, 0.006], BLACK, [0.0, 0.0, 24.0]);
    b(v, [0.0, cy - 0.1, 0.132], [0.11, 0.045, 0.005], [80, 16, 24]);
    pair(v, [0.07, cy - 0.075, 0.138], [0.02, 0.03, 0.005], WHITE);
    b(v, [0.0, cy - 0.01, 0.14], [0.03, 0.035, 0.012], [200, 120, 96]);
}

fn ryujin(v: &mut Vec<Raw>) {
    // Ryujin : tete de dragon japonais, ecailles de jade, bois de cerf, criniere et moustaches.
    let (jade, deep, gold, antler, mane) = ([64, 176, 126], [40, 128, 96], [244, 204, 92], [250, 238, 204], [232, 72, 62]);
    b(v, [0.0, 1.0, 0.5], [0.8, 0.6, 1.0], jade);
    b(v, [0.0, 0.9, 1.9], [0.58, 0.4, 0.6], jade);
    b(v, [0.0, 0.42, 1.6], [0.54, 0.15, 0.85], gold);
    pair(v, [0.22, 1.2, 2.502], [0.07, 0.05, 0.005], BLACK);
    for (x, z) in [(0.42, 2.3), (0.42, 1.9), (0.2, 2.42)] {
        pair(v, [x, 0.6, z], [0.06, 0.13, 0.06], WHITE);
    }
    pair(v, [0.805, 1.25, 1.1], [0.012, 0.17, 0.24], [255, 226, 70]);
    pair(v, [0.82, 1.25, 1.1], [0.008, 0.15, 0.05], BLACK);
    pair_r(v, [0.62, 1.58, 1.1], [0.3, 0.08, 0.36], deep, [0.0, 0.0, -12.0]);
    pair_r(v, [0.5, 2.15, 0.1], [0.08, 0.6, 0.08], antler, [-24.0, 0.0, -18.0]);
    pair_r(v, [0.78, 2.5, -0.1], [0.06, 0.3, 0.06], antler, [-50.0, 0.0, -42.0]);
    pair_r(v, [0.46, 2.4, 0.42], [0.05, 0.24, 0.05], antler, [35.0, 0.0, -10.0]);
    b(v, [0.0, 1.76, -0.2], [0.26, 0.26, 0.9], mane);
    pair(v, [0.86, 1.0, -0.3], [0.1, 0.36, 0.5], mane);
    // Longues moustaches dorees.
    pair_r(v, [0.9, 0.82, 2.3], [0.04, 0.04, 0.8], gold, [-10.0, 38.0, 0.0]);
    b(v, [0.0, 0.3, 1.2], [0.2, 0.12, 0.3], mane);
}

fn player(v: &mut Vec<Raw>) {
    // Joueur : proportions normales, traits de manga (cheveux en meches, grands yeux a reflet).
    let (skin, hair, jacket, red) = ([255, 214, 180], [44, 48, 96], [255, 150, 60], [232, 72, 84]);
    legs(v, [0.12, 0.42, 0.0], [0.11, 0.34, 0.12], [50, 58, 104], 1);
    feet(v, [0.12, 0.06, 0.03], [0.12, 0.06, 0.16], red, 1, 0.76);
    b(v, [0.0, 1.1, 0.0], [0.25, 0.33, 0.14], jacket);
    b(v, [0.0, 1.12, 0.142], [0.07, 0.3, 0.005], WHITE);
    b(v, [0.0, 0.8, 0.0], [0.255, 0.04, 0.145], [60, 50, 76]);
    b(v, [0.0, 1.45, 0.0], [0.2, 0.05, 0.17], red);
    // Les bras se balancent a l'inverse des jambes.
    legs(v, [0.36, 1.14, 0.0], [0.09, 0.3, 0.1], jacket, -1);
    feet(v, [0.36, 0.78, 0.0], [0.08, 0.06, 0.09], skin, -1, 1.44);
    // Tete et visage.
    b(v, [0.0, 1.72, 0.0], [0.22, 0.23, 0.22], skin);
    eyes(v, 0.1, 1.72, 0.222, 0.062, [60, 140, 232]);
    pair(v, [0.1, 1.815, 0.224], [0.062, 0.012, 0.004], hair);
    b(v, [0.0, 1.585, 0.222], [0.035, 0.01, 0.004], [196, 92, 96]);
    // Cheveux : calotte, nuque, cotes, frange et meches dressees.
    b(v, [0.0, 1.97, -0.01], [0.25, 0.07, 0.25], hair);
    b(v, [0.0, 1.74, -0.2], [0.25, 0.21, 0.07], hair);
    pair(v, [0.235, 1.79, -0.02], [0.03, 0.17, 0.2], hair);
    b(v, [0.0, 1.9, 0.215], [0.23, 0.055, 0.04], hair);
    pair_r(v, [0.16, 1.84, 0.228], [0.06, 0.075, 0.02], hair, [0.0, 0.0, 12.0]);
    r(v, [0.1, 2.07, 0.02], [0.07, 0.06, 0.09], hair, [0.0, 0.0, -20.0]);
    r(v, [-0.12, 2.06, -0.08], [0.08, 0.06, 0.08], hair, [0.0, 0.0, 22.0]);
    r(v, [0.0, 2.05, -0.2], [0.1, 0.06, 0.07], hair, [-25.0, 0.0, 0.0]);
}

fn cart(v: &mut Vec<Raw>) {
    // Chariot daihachi : caisse de bois cerclee, deux grandes roues, timon a l'avant.
    let (wood, dark, iron) = ([176, 122, 86], [110, 72, 50], [70, 66, 78]);
    b(v, [0.0, 0.3, 0.0], [0.44, 0.04, 0.58], dark);
    pair(v, [0.42, 0.52, 0.0], [0.04, 0.2, 0.58], wood);
    b(v, [0.0, 0.52, 0.56], [0.44, 0.2, 0.04], wood);
    b(v, [0.0, 0.52, -0.56], [0.44, 0.2, 0.04], wood);
    pair(v, [0.42, 0.74, 0.0], [0.05, 0.03, 0.6], iron);
    pair(v, [0.5, 0.26, 0.0], [0.04, 0.26, 0.26], dark);
    pair(v, [0.52, 0.26, 0.0], [0.03, 0.07, 0.07], iron);
    pair(v, [0.2, 0.36, 0.8], [0.03, 0.03, 0.26], wood);
    b(v, [0.0, 0.36, 1.04], [0.24, 0.03, 0.03], wood);
}

fn villager(v: &mut Vec<Raw>) {
    // Villageois : kimono indigo, ceinture obi, sandales, large chapeau de paille (kasa).
    let (skin, indigo, obi, straw, hair) = ([255, 214, 180], [52, 70, 120], [200, 80, 60], [222, 196, 130], [40, 36, 48]);
    legs(v, [0.12, 0.4, 0.0], [0.12, 0.34, 0.13], indigo, 1);
    feet(v, [0.12, 0.05, 0.03], [0.12, 0.05, 0.16], straw, 1, 0.74);
    b(v, [0.0, 1.08, 0.0], [0.26, 0.34, 0.15], indigo);
    b(v, [0.0, 0.84, 0.0], [0.265, 0.06, 0.155], obi);
    b(v, [0.0, 1.18, 0.152], [0.05, 0.24, 0.005], WHITE);
    legs(v, [0.37, 1.12, 0.0], [0.1, 0.3, 0.11], indigo, -1);
    feet(v, [0.37, 0.78, 0.0], [0.08, 0.06, 0.09], skin, -1, 1.42);
    b(v, [0.0, 1.7, 0.0], [0.21, 0.22, 0.21], skin);
    eyes(v, 0.095, 1.71, 0.212, 0.05, [60, 44, 36]);
    b(v, [0.0, 1.6, 0.212], [0.03, 0.01, 0.004], [196, 92, 96]);
    b(v, [0.0, 1.8, -0.12], [0.22, 0.14, 0.1], hair);
    // Chapeau : un large disque, une calotte.
    b(v, [0.0, 1.95, 0.0], [0.42, 0.025, 0.42], straw);
    b(v, [0.0, 2.02, 0.0], [0.24, 0.05, 0.24], straw);
    b(v, [0.0, 2.1, 0.0], [0.1, 0.04, 0.1], straw);
}

pub fn raw(index: usize) -> Vec<Raw> {
    let mut v = Vec::new();
    match index as u8 {
        COW => deer(&mut v),
        PIG => boar(&mut v),
        SHEEP => tanuki(&mut v),
        CHICKEN => crane(&mut v),
        ZOMBIE => oni(&mut v),
        SKELETON => samurai(&mut v),
        SPIDER => tsuchigumo(&mut v),
        BOMBYX => lantern(&mut v),
        SHADE => yurei(&mut v),
        BRAISIER => wanyudo(&mut v),
        WYRM => ryujin(&mut v),
        CART => cart(&mut v),
        VILLAGER => villager(&mut v),
        _ => player(&mut v),
    }
    v
}

/// Coin haut-gauche de la case d'atlas d'un modele.
pub fn cell_origin(index: usize) -> (usize, usize) {
    ((index % 8) * CELL, MODEL_Y + (index / 8) * CELL)
}

/// Marge de lecture d'une face large de `size` pixels.
pub fn margin(size: f32) -> f32 {
    (size * 0.25).min(0.5)
}

/// Largeur et hauteur d'une face, en blocs.
fn face_size(h: Vec3, f: usize) -> (f32, f32) {
    match f {
        0 | 1 => (2.0 * h.z, 2.0 * h.y),
        2 | 3 => (2.0 * h.x, 2.0 * h.z),
        _ => (2.0 * h.x, 2.0 * h.y),
    }
}

/// Range les faces d'un modele dans sa case, en rangees ; None si elles n'y tiennent pas a cette densite.
fn pack(parts: &[Raw], density: f32) -> Option<Vec<[[usize; 4]; 6]>> {
    let (mut x, mut y, mut row) = (0usize, 0usize, 0usize);
    let mut out = Vec::new();
    for p in parts {
        let mut rects = [[0usize; 4]; 6];
        for (f, rect) in rects.iter_mut().enumerate() {
            let (fw, fh) = face_size(p.h, f);
            let (w, h) = (((fw * density).ceil() as usize).max(1), ((fh * density).ceil() as usize).max(1));
            if w > CELL || h > CELL {
                return None;
            }
            if x + w > CELL {
                x = 0;
                y += row;
                row = 0;
            }
            if y + h > CELL {
                return None;
            }
            *rect = [x, y, w, h];
            x += w;
            row = row.max(h);
        }
        out.push(rects);
    }
    Some(out)
}

/// Peint la texture d'un modele dans l'atlas et renvoie ses boites avec leurs coordonnees de texture.
pub fn bake(img: &mut Img, index: usize, parts: &[Raw]) -> Model {
    let (ox, oy) = cell_origin(index);
    for y in 0..CELL {
        for x in 0..CELL {
            img.set_px(ox + x, oy + y, [0, 0, 0, 0]);
        }
    }
    // Meme finesse que les blocs (32 pixels par bloc) si le modele tient dans sa case, sinon moins.
    let rects = [32.0, 24.0, 16.0, 12.0, 8.0, 6.0, 4.0]
        .into_iter()
        .find_map(|d| pack(parts, d))
        .unwrap_or_else(|| vec![[[0, 0, 1, 1]; 6]; parts.len()]);
    let mut model = Model::default();
    for (k, (p, faces)) in parts.iter().zip(rects).enumerate() {
        let mut uv = [[0f32; 4]; 6];
        for (f, [x, y, w, h]) in faces.into_iter().enumerate() {
            for py in 0..h {
                for px in 0..w {
                    // Pixel art en quatre tons francs : reflet, teinte, ombre, ombre profonde.
                    let seed = (index * 1000 + k * 6 + f) as u32;
                    let big = w >= 8 && h >= 8;
                    let edge = (h as f32 * 0.62) as usize;
                    let mut tone = 1i32;
                    if h > 5 && f != 2 {
                        if py > edge + 1 {
                            tone = 2;
                        } else if py == edge + 1 {
                            // Passage a l'ombre en damier, comme un trame de manga.
                            tone = if (px + py) % 2 == 0 { 2 } else { 1 };
                        } else if py == edge && px % 4 == 0 {
                            tone = 2;
                        }
                    }
                    if big && f != 3 && tone == 1 && py < (h / 8).max(1) {
                        tone = 0;
                    }
                    // Meches : petits traits verticaux de trois pixels, clairs ou sombres.
                    if big {
                        match hash(px as i32, (py / 3) as i32, 0, seed) % 23 {
                            0 => tone -= 1,
                            1 => tone += 1,
                            _ => {}
                        }
                    }
                    // Lisere sombre autour de chaque face.
                    if w >= 6 && h >= 6 && (px == 0 || py == 0 || px == w - 1 || py == h - 1) {
                        tone += 1;
                    }
                    let gain = [116, 100, 82, 64][tone.clamp(0, 3) as usize];
                    let c = |v: u8| (v as i32 * gain / 100).clamp(0, 255) as u8;
                    img.set_px(ox + x + px, oy + y + py, [c(p.color[0]), c(p.color[1]), c(p.color[2]), 255]);
                }
            }
            // Marge d'un demi-pixel pour ne pas lire la face voisine ; moins sur les faces tres fines.
            let (ex, ey) = (margin(w as f32), margin(h as f32));
            let s = AT as f32;
            uv[f] = [
                ((ox + x) as f32 + ex) / s,
                ((oy + y) as f32 + ey) / s,
                ((ox + x + w) as f32 - ex) / s,
                ((oy + y + h) as f32 - ey) / s,
            ];
        }
        model.parts.push(Part { c: p.c, h: p.h, rot: p.rot, pivot: p.pivot, leg: p.leg, uv });
    }
    model
}

/// Construit tous les modeles integres et peint leurs textures dans l'atlas.
pub fn build(img: &mut Img) -> Vec<Model> {
    (0..NAMES.len()).map(|i| bake(img, i, &raw(i))).collect()
}
