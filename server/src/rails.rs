//! Rails et chariot daihachi.
//!
//! Le chariot suit la ligne des rails : il avance droit jusqu'au centre de chaque case, ou le rail
//! decide de sa sortie (tout droit, ou l'autre bras d'un virage). Un rail de reiseki alimente le lance,
//! eteint il le freine ; un passager peut aussi le pousser doucement dans la direction ou il regarde.
use super::*;
use shared::blocks::*;
use shared::signal::{self as sg, add, DIRS4};

/// Vitesse maximale sur rail de reiseki, en blocs par seconde.
const TOP_SPEED: f64 = 8.0;
/// Vitesse qu'un passager atteint en poussant du pied.
const PUSH_SPEED: f64 = 3.0;

fn v3(d: [i32; 3]) -> Vec3 {
    Vec3::new(d[0] as f64, d[1] as f64, d[2] as f64)
}

/// Direction cardinale la plus proche d'un vecteur horizontal.
fn cardinal(v: Vec3) -> Vec3 {
    if v.x.abs() >= v.z.abs() {
        Vec3::new(v.x.signum(), 0.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, v.z.signum())
    }
}

/// Avance un chariot d'un tick. `look` : regard du passager, s'il y en a un.
pub(crate) fn cart_step(world: &World, pos: &mut Vec3, vel: &mut Vec3, yaw: &mut f64, look: Option<Vec3>) {
    let c = [pos.x.floor() as i32, (pos.y + 0.1).floor() as i32, pos.z.floor() as i32];
    let Some((b, m)) = world.get(c) else { return };
    if !sg::is_rail(b) {
        *vel = Vec3::ZERO;
        return;
    }
    let arms = sg::rail_dirs(m).map(v3);
    let centre = Vec3::new(c[0] as f64 + 0.5, c[1] as f64, c[2] as f64 + 0.5);
    let mut speed = vel.length();
    let mut dir = (speed > 0.01).then(|| cardinal(*vel));
    // Bras du rail vers lequel le passager regarde.
    let want = look.and_then(|l| arms.into_iter().map(|a| (a.dot(l), a)).filter(|(k, _)| *k > 0.3).max_by(|x, y| x.0.total_cmp(&y.0)).map(|x| x.1));
    if b == POWERED_RAIL {
        if m & 8 != 0 {
            if dir.is_none() {
                dir = Some(want.unwrap_or(arms[0]));
            }
            speed = (speed + 8.0 * DT).min(TOP_SPEED);
        } else {
            speed = (speed - 12.0 * DT).max(0.0);
        }
    } else if let Some(w) = want {
        if speed < PUSH_SPEED && dir.is_none_or(|d| d == w) {
            dir = Some(w);
            speed += 3.0 * DT;
        }
    }
    speed *= 0.997;
    let Some(mut d) = dir.filter(|_| speed >= 0.02) else {
        *vel = Vec3::ZERO;
        return;
    };
    let mut t = (*pos - centre).dot(d);
    let mut step = speed * DT;
    if t < 0.0 && t + step >= 0.0 {
        // Passage au centre : le rail choisit la sortie.
        let from = -d;
        let exit = if arms[0] == from {
            Some(arms[1])
        } else if arms[1] == from {
            Some(arms[0])
        } else if arms.contains(&d) {
            Some(d)
        } else {
            None
        };
        step += t;
        t = 0.0;
        match exit {
            Some(e) => d = e,
            None => {
                speed = 0.0;
                step = 0.0;
            }
        }
    }
    t += step;
    if t > 0.5 {
        // Sortie de la case : il faut un rail en face, tourne de facon a nous accueillir.
        let next = [c[0] + d.x as i32, c[1], c[2] + d.z as i32];
        let ok = world.get(next).is_some_and(|(nb, nm)| {
            let a = sg::rail_dirs(nm).map(v3);
            sg::is_rail(nb) && (a.contains(&-d) || a.contains(&d))
        });
        if !ok {
            t = 0.45;
            speed = 0.0;
        }
    }
    *pos = centre + d * t;
    *vel = d * speed;
    *yaw = d.x.atan2(d.z);
}

impl Server {
    /// Redonne sa forme au rail en `p` d'apres ses voisins.
    fn fix_rail(&mut self, p: [i32; 3]) {
        let Some((b, m)) = self.world.get(p) else { return };
        if sg::is_rail(b) {
            let get = |q: [i32; 3]| self.world.get(q).unwrap_or((AIR, 0));
            let shape = sg::rail_shape(&get, p);
            self.world.set(p, b, (m & 8) | shape);
        }
    }

    /// A appeler quand un rail est pose ou retire en `p` : lui et ses voisins se raccordent.
    pub(crate) fn fix_rails_around(&mut self, p: [i32; 3]) {
        self.fix_rail(p);
        for d in DIRS4 {
            self.fix_rail(add(p, d));
        }
    }

    /// Pose un chariot sur le rail vise. Renvoie vrai si c'est fait.
    pub(crate) fn place_cart(&mut self, id: u32, pos: [i32; 3]) -> bool {
        if !self.world.get(pos).is_some_and(|(b, _)| sg::is_rail(b)) {
            return false;
        }
        let Some(c) = self.clients.get_mut(&id) else { return false };
        if !c.inv[c.sel].is_some_and(|s| s.id == items::CART) {
            return false;
        }
        c.inv[c.sel] = None;
        c.inv_dirty = true;
        self.spawn_mob(shared::mobs::CART, Vec3::new(pos[0] as f64 + 0.5, pos[1] as f64, pos[2] as f64 + 0.5), 0.0, 1.0);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lab() -> (Server, [i32; 3]) {
        let mut srv = Server::new(7, None);
        srv.world.ensure((0, 0));
        for dz in -6..=6 {
            for dx in -6..=6 {
                srv.world.set([8 + dx, 300, 8 + dz], STONE, 0);
                for dy in 1..=4 {
                    srv.world.set([8 + dx, 300 + dy, 8 + dz], AIR, 0);
                }
            }
        }
        (srv, [8, 301, 8])
    }

    fn lay(srv: &mut Server, p: [i32; 3], b: B) {
        srv.world.set(p, b, 0);
        srv.fix_rails_around(p);
    }

    #[test]
    fn rails_connect_and_cart_loops() {
        let (mut srv, c) = lab();
        // Circuit ferme de 4 x 4 : seuls les coins tournent.
        let mut track = Vec::new();
        for i in 0..4 {
            track.push([c[0] + i, c[1], c[2]]);
            track.push([c[0] + i, c[1], c[2] + 3]);
            track.push([c[0], c[1], c[2] + i]);
            track.push([c[0] + 3, c[1], c[2] + i]);
        }
        for p in &track {
            lay(&mut srv, *p, RAIL);
        }
        let meta = |srv: &Server, x: i32, z: i32| srv.world.get([c[0] + x, c[1], c[2] + z]).map(|v| v.1 & 7);
        assert_eq!(meta(&srv, 1, 0), Some(0), "droit en X");
        assert_eq!(meta(&srv, 0, 1), Some(1), "droit en Z");
        assert_eq!(meta(&srv, 0, 0), Some(2), "coin : +X et +Z");
        assert_eq!(meta(&srv, 3, 0), Some(4), "coin : -X et +Z");
        assert_eq!(meta(&srv, 0, 3), Some(3), "coin : +X et -Z");
        assert_eq!(meta(&srv, 3, 3), Some(5), "coin : -X et -Z");

        // Un chariot lance fait le tour et repasse par toutes les cases.
        let mut pos = Vec3::new(c[0] as f64 + 1.5, c[1] as f64, c[2] as f64 + 0.5);
        let mut vel = Vec3::new(6.0, 0.0, 0.0);
        let mut yaw = 0.0;
        let mut seen = HashSet::new();
        for _ in 0..200 {
            cart_step(&srv.world, &mut pos, &mut vel, &mut yaw, None);
            seen.insert([pos.x.floor() as i32, pos.z.floor() as i32]);
            assert!(sg::is_rail(srv.world.get([pos.x.floor() as i32, c[1], pos.z.floor() as i32]).unwrap().0), "reste sur les rails");
        }
        assert_eq!(seen.len(), 12, "les douze cases du circuit sont parcourues");
        assert!(vel.length() > 2.0, "le frottement est faible");
    }

    #[test]
    fn powered_rail_launches_and_brakes() {
        let (mut srv, c) = lab();
        for i in -5..=5 {
            lay(&mut srv, [c[0] + i, c[1], c[2]], if i <= -3 { POWERED_RAIL } else { RAIL });
        }
        // Rail de reiseki alimente par un levier : le chariot demarre seul.
        let lever = [c[0] - 5, c[1], c[2] + 1];
        srv.world.set(lever, LEVER, 1);
        for _ in 0..4 {
            srv.tick_signal();
        }
        for i in -5..=-3 {
            assert_eq!(srv.world.get([c[0] + i, c[1], c[2]]).unwrap().1 & 8, 8, "le ki se propage de rail en rail");
        }
        let mut pos = Vec3::new(c[0] as f64 - 4.5, c[1] as f64, c[2] as f64 + 0.5);
        let (mut vel, mut yaw) = (Vec3::ZERO, 0.0);
        for _ in 0..40 {
            cart_step(&srv.world, &mut pos, &mut vel, &mut yaw, None);
        }
        assert!(pos.x > c[0] as f64, "lance vers +X : x = {}", pos.x);
        // Au bout de la voie, il s'arrete sans derailler.
        for _ in 0..200 {
            cart_step(&srv.world, &mut pos, &mut vel, &mut yaw, None);
        }
        assert_eq!(pos.x.floor() as i32, c[0] + 5);
        assert_eq!(vel, Vec3::ZERO);

        // Rail de reiseki eteint : il freine un chariot lance.
        srv.world.set(lever, LEVER, 0);
        for _ in 0..4 {
            srv.tick_signal();
        }
        let mut pos = Vec3::new(c[0] as f64 + 2.5, c[1] as f64, c[2] as f64 + 0.5);
        let mut vel = Vec3::new(-6.0, 0.0, 0.0);
        for _ in 0..200 {
            cart_step(&srv.world, &mut pos, &mut vel, &mut yaw, None);
        }
        assert_eq!(vel, Vec3::ZERO);
        assert!(pos.x < c[0] as f64 - 2.0 && pos.x > c[0] as f64 - 5.0, "arrete sur les rails eteints : x = {}", pos.x);
    }
}
