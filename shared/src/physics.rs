//! Collision d'une boite alignee sur les axes contre la grille de blocs.
use glam::Vec3;

pub trait BlockSource {
    /// Vrai si le bloc arrete les entites (les zones non chargees doivent etre solides).
    fn solid_at(&self, x: i32, y: i32, z: i32) -> bool;
}

const EPS: f32 = 1e-3;

/// `pos` = centre des pieds, `hw` = demi-largeur, `h` = hauteur.
pub fn overlaps(w: &impl BlockSource, pos: Vec3, hw: f32, h: f32) -> bool {
    let x0 = (pos.x - hw + EPS).floor() as i32;
    let x1 = (pos.x + hw - EPS).floor() as i32;
    let y0 = (pos.y + EPS).floor() as i32;
    let y1 = (pos.y + h - EPS).floor() as i32;
    let z0 = (pos.z - hw + EPS).floor() as i32;
    let z1 = (pos.z + hw - EPS).floor() as i32;
    for y in y0..=y1 {
        for z in z0..=z1 {
            for x in x0..=x1 {
                if w.solid_at(x, y, z) {
                    return true;
                }
            }
        }
    }
    false
}

pub struct MoveResult {
    pub on_ground: bool,
    pub hit_wall: bool,
}

/// Deplace le corps de `vel * dt`, axe par axe, en s'arretant contre les blocs.
pub fn move_body(w: &impl BlockSource, pos: &mut Vec3, vel: &mut Vec3, hw: f32, h: f32, dt: f32) -> MoveResult {
    let mut res = MoveResult { on_ground: false, hit_wall: false };
    let dist = vel.abs().max_element() * dt;
    let steps = (dist / 0.4).ceil().max(1.0) as i32;
    let sdt = dt / steps as f32;
    for _ in 0..steps {
        pos.y += vel.y * sdt;
        if overlaps(w, *pos, hw, h) {
            if vel.y < 0.0 {
                pos.y = pos.y.floor() + 1.0;
                res.on_ground = true;
            } else {
                pos.y = (pos.y + h).floor() - h - EPS;
            }
            vel.y = 0.0;
        }
        pos.x += vel.x * sdt;
        if overlaps(w, *pos, hw, h) {
            if vel.x > 0.0 {
                pos.x = (pos.x + hw).floor() - hw - EPS;
            } else {
                pos.x = (pos.x - hw).floor() + 1.0 + hw + EPS;
            }
            vel.x = 0.0;
            res.hit_wall = true;
        }
        pos.z += vel.z * sdt;
        if overlaps(w, *pos, hw, h) {
            if vel.z > 0.0 {
                pos.z = (pos.z + hw).floor() - hw - EPS;
            } else {
                pos.z = (pos.z - hw).floor() + 1.0 + hw + EPS;
            }
            vel.z = 0.0;
            res.hit_wall = true;
        }
    }
    res
}

/// Un pas de simulation d'un objet au sol ; identique sur le client et le serveur.
pub fn step_drop(w: &impl BlockSource, pos: &mut Vec3, vel: &mut Vec3, dt: f32) {
    vel.y = (vel.y - 22.0 * dt).max(-40.0);
    let r = move_body(w, pos, vel, 0.125, 0.25, dt);
    let k = if r.on_ground { (1.0 - 8.0 * dt).max(0.0) } else { (1.0 - 0.5 * dt).max(0.0) };
    vel.x *= k;
    vel.z *= k;
}

/// Les memes collisions en double precision, pour le serveur : a des dizaines de millions de blocs
/// de l'origine, un flottant simple ne sait plus placer une creature a moins de deux blocs pres.
pub mod d {
    use super::BlockSource;
    use glam::DVec3;

    const EPS: f64 = 1e-3;

    /// `pos` = centre des pieds, `hw` = demi-largeur, `h` = hauteur.
    pub fn overlaps(w: &impl BlockSource, pos: DVec3, hw: f64, h: f64) -> bool {
        let x0 = (pos.x - hw + EPS).floor() as i32;
        let x1 = (pos.x + hw - EPS).floor() as i32;
        let y0 = (pos.y + EPS).floor() as i32;
        let y1 = (pos.y + h - EPS).floor() as i32;
        let z0 = (pos.z - hw + EPS).floor() as i32;
        let z1 = (pos.z + hw - EPS).floor() as i32;
        for y in y0..=y1 {
            for z in z0..=z1 {
                for x in x0..=x1 {
                    if w.solid_at(x, y, z) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub struct MoveResult {
        pub on_ground: bool,
        pub hit_wall: bool,
    }

    /// Deplace le corps de `vel * dt`, axe par axe, en s'arretant contre les blocs.
    pub fn move_body(w: &impl BlockSource, pos: &mut DVec3, vel: &mut DVec3, hw: f64, h: f64, dt: f64) -> MoveResult {
        let mut res = MoveResult { on_ground: false, hit_wall: false };
        let dist = vel.abs().max_element() * dt;
        let steps = (dist / 0.4).ceil().max(1.0) as i32;
        let sdt = dt / steps as f64;
        for _ in 0..steps {
            pos.y += vel.y * sdt;
            if overlaps(w, *pos, hw, h) {
                if vel.y < 0.0 {
                    pos.y = pos.y.floor() + 1.0;
                    res.on_ground = true;
                } else {
                    pos.y = (pos.y + h).floor() - h - EPS;
                }
                vel.y = 0.0;
            }
            pos.x += vel.x * sdt;
            if overlaps(w, *pos, hw, h) {
                if vel.x > 0.0 {
                    pos.x = (pos.x + hw).floor() - hw - EPS;
                } else {
                    pos.x = (pos.x - hw).floor() + 1.0 + hw + EPS;
                }
                vel.x = 0.0;
                res.hit_wall = true;
            }
            pos.z += vel.z * sdt;
            if overlaps(w, *pos, hw, h) {
                if vel.z > 0.0 {
                    pos.z = (pos.z + hw).floor() - hw - EPS;
                } else {
                    pos.z = (pos.z - hw).floor() + 1.0 + hw + EPS;
                }
                vel.z = 0.0;
                res.hit_wall = true;
            }
        }
        res
    }

    /// Un pas de simulation d'un objet au sol ; identique sur le client et le serveur.
    pub fn step_drop(w: &impl BlockSource, pos: &mut DVec3, vel: &mut DVec3, dt: f64) {
        vel.y = (vel.y - 22.0 * dt).max(-40.0);
        let r = move_body(w, pos, vel, 0.125, 0.25, dt);
        let k = if r.on_ground { (1.0 - 8.0 * dt).max(0.0) } else { (1.0 - 0.5 * dt).max(0.0) };
        vel.x *= k;
        vel.z *= k;
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bassin : sol plein sous y = 0, berge pleine pour x >= 1 jusqu'a y = 1.
    struct Pool;
    impl BlockSource for Pool {
        fn solid_at(&self, x: i32, y: i32, _z: i32) -> bool {
            y < 0 || (x >= 1 && y < 1)
        }
    }

    /// Reproduit la nage du client : on maintient "avancer" et "sauter" contre la berge.
    #[test]
    fn swimmer_climbs_out_onto_the_bank() {
        let surface = 0.875;
        let (mut pos, mut vel) = (Vec3::new(0.5, 0.0, 0.5), Vec3::ZERO);
        let mut hit_wall = false;
        let dt: f32 = 1.0 / 60.0;
        for _ in 0..240 {
            let in_water = pos.x < 1.0 && (pos.y + 0.1 < surface || pos.y + 1.2 < surface);
            vel.x += (4.3 * 0.55 - vel.x) * (14.0 * dt).min(1.0);
            if in_water {
                vel.y = (vel.y - 8.0 * dt).max(-3.0);
                vel.y = (vel.y + 30.0 * dt).min(4.0);
                if hit_wall {
                    vel.y = 7.0;
                }
            } else {
                vel.y = (vel.y - 28.0 * dt).max(-60.0);
            }
            let r = move_body(&Pool, &mut pos, &mut vel, 0.3, 1.8, dt);
            hit_wall = r.hit_wall;
        }
        assert!(pos.x > 1.3 && pos.y >= 0.999, "le nageur est reste dans l'eau : {pos:?}");
    }
}
