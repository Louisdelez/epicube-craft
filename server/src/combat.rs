//! Armes : tir a l'arc, armes de jet, bouclier, et tests du combat.
//!
//! Le corps a corps suit trois regles : une arme doit revenir en garde pour frapper fort
//! (un coup porte trop tot ne fait qu'un cinquieme des degats) ; un coup porte en retombant
//! d'un saut est critique (x 1,5) ; un katana a pleine charge fauche aussi les voisins de la cible.
use super::*;

/// Vitesse d'une fleche tiree a pleine tension, en blocs par seconde.
const ARROW_SPEED: f64 = 42.0;

impl Server {
    /// Tire avec l'arc tenu, ou jette la lance ou le kunai tenu.
    pub(crate) fn shoot(&mut self, id: u32, charge: f64) {
        let now = Instant::now();
        let lucky = self.rng.chance(0.25);
        let Some(c) = self.clients.get_mut(&id) else { return };
        let Some(st) = c.inv[c.sel] else { return };
        if c.dead || !charge.is_finite() || now.duration_since(c.last_hit).as_secs_f64() < 0.25 {
            return;
        }
        let dir = Vec3::new(c.yaw.sin() * c.pitch.cos(), c.pitch.sin(), c.yaw.cos() * c.pitch.cos());
        let from = c.eye() + dir * 0.5 - Vec3::Y * 0.1;
        let (vel, kind, dmg, stack) = match st.id {
            items::YUMI => {
                let power = items::draw_power(charge);
                if power < 0.1 || items::count(&c.inv, items::ARROW) == 0 {
                    return;
                }
                items::remove(&mut c.inv, items::ARROW, 1);
                // De 2 a 9 points selon la tension ; une fleche tiree a fond peut porter plus fort encore.
                let bonus = if power >= 0.99 && lucky { 2.0 } else { 0.0 };
                (dir * (10.0 + (ARROW_SPEED - 10.0) * power), 0, 2.0 + 7.0 * power + bonus, Some(Stack::new(items::ARROW, 1)))
            }
            items::YARI => {
                // La lance part avec son usure : on la retrouve plantee la ou elle tombe.
                c.inv[c.sel] = None;
                let left = st.dur.saturating_sub(1);
                (dir * 26.0, 2, 8.0, (left > 0).then_some(Stack { dur: left, ..st }))
            }
            items::KUNAI => {
                c.inv[c.sel] = if st.n > 1 { Some(Stack { n: st.n - 1, ..st }) } else { None };
                (dir * 32.0, 3, 4.0, Some(Stack::new(items::KUNAI, 1)))
            }
            _ => return,
        };
        c.last_hit = now;
        c.inv_dirty = true;
        c.exhaustion += 0.05;
        if st.id == items::YUMI {
            self.wear_held(id);
        }
        self.spawn_shot(from, vel, kind, dmg, Some(id), stack);
    }

    /// Bouclier leve : pare ce qui vient de face. `knock` indique d'ou vient le coup.
    /// Renvoie vrai si le coup est pare ; le bouclier s'use d'autant.
    pub(crate) fn shield_blocks(&mut self, id: u32, dmg: f64, knock: Vec3) -> bool {
        let Some(c) = self.clients.get_mut(&id) else { return false };
        if !c.blocking || c.held() != Some(items::SHIELD) {
            return false;
        }
        let push = Vec3::new(knock.x, 0.0, knock.z);
        let look = Vec3::new(c.yaw.sin(), 0.0, c.yaw.cos());
        // Le recul pousse dans le dos de qui fait face a l'attaquant.
        if push.length_squared() < 0.01 || look.dot(push.normalize()) > -0.2 {
            return false;
        }
        if let Some(st) = c.inv[c.sel].as_mut() {
            st.dur = st.dur.saturating_sub(dmg.ceil().max(1.0) as u16);
            if st.dur == 0 {
                c.inv[c.sel] = None;
                c.blocking = false;
            }
            c.inv_dirty = true;
        }
        c.inv_t = 0.3;
        let at = [c.pos[0].floor() as i32, c.pos[1].floor() as i32 + 1, c.pos[2].floor() as i32];
        self.sound(at, shared::proto::sound::PARRY, 0);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::blocks::*;
    use shared::mobs as mk;

    fn lab() -> (Server, [i32; 3]) {
        let mut srv = Server::new(7, None);
        srv.world.ensure((0, 0));
        for dz in -7..=7 {
            for dx in -7..=7 {
                srv.world.set([8 + dx, 300, 8 + dz], STONE, 0);
                for dy in 1..=6 {
                    srv.world.set([8 + dx, 300 + dy, 8 + dz], AIR, 0);
                }
            }
        }
        (srv, [8, 301, 8])
    }

    fn player(srv: &mut Server, c: [i32; 3], held: Stack) -> Receiver<ServerMsg> {
        let (tx, rx) = mpsc::channel();
        let mut cl = Client::new(tx, [c[0] as f64 + 0.5, c[1] as f64, c[2] as f64 + 0.5]);
        cl.ready = true;
        cl.on_ground = true;
        cl.inv[0] = Some(held);
        // Regarde vers +Z.
        cl.yaw = 0.0;
        srv.clients.insert(1, cl);
        rx
    }

    /// Fait apparaitre une creature a `dz` blocs devant le joueur et renvoie son identifiant.
    fn target(srv: &mut Server, c: [i32; 3], dx: f64, dz: f64, kind: u8) -> u32 {
        srv.spawn_mob(kind, Vec3::new(c[0] as f64 + 0.5 + dx, c[1] as f64, c[2] as f64 + 0.5 + dz), 0.0, 100.0);
        srv.mobs.last().unwrap().id
    }

    fn hp(srv: &Server, id: u32) -> f64 {
        srv.mobs.iter().find(|m| m.id == id).map(|m| m.hp).unwrap_or(0.0)
    }

    /// Laisse l'arme revenir en garde et la creature redevenir vulnerable.
    fn rest(srv: &mut Server, secs: f64) {
        if let Some(c) = srv.clients.get_mut(&1) {
            c.last_hit = Instant::now() - Duration::from_secs_f64(secs);
        }
        for m in srv.mobs.iter_mut() {
            m.hurt_cd = 0.0;
        }
    }

    #[test]
    fn cooldown_crit_and_sweep() {
        let (mut srv, c) = lab();
        let katana = items::tool_id(items::SWORD, 2);
        let _rx = player(&mut srv, c, Stack::new(katana, 1));
        let a = target(&mut srv, c, 0.0, 2.0, mk::ZOMBIE);
        let b = target(&mut srv, c, 1.0, 2.5, mk::ZOMBIE);

        // A pleine charge : 6 points, et le voisin est fauche (1 point).
        rest(&mut srv, 5.0);
        srv.attack(1, a);
        assert_eq!(hp(&srv, a), 94.0);
        assert_eq!(hp(&srv, b), 99.0, "le katana fauche le voisin");
        assert_eq!(srv.clients[&1].inv[0].map(|s| s.dur), Some(items::max_dur(katana) - 1));

        // Coup porte trop tot : une fraction des degats seulement, et pas de fauchage.
        rest(&mut srv, 0.15);
        srv.attack(1, a);
        let weak = 94.0 - hp(&srv, a);
        assert!(weak > 1.0 && weak < 2.0, "coup precipite : {weak}");
        assert_eq!(hp(&srv, b), 99.0);

        // En retombant d'un saut : coup critique, moitie de degats en plus.
        rest(&mut srv, 5.0);
        {
            let cl = srv.clients.get_mut(&1).unwrap();
            cl.on_ground = false;
            cl.fall_peak = cl.pos[1] + 1.0;
        }
        let before = hp(&srv, a);
        srv.attack(1, a);
        assert!((before - hp(&srv, a) - 9.0).abs() < 0.01, "critique : {}", before - hp(&srv, a));

        // Une hache frappe plus fort, mais s'use de deux crans par coup.
        let axe = items::tool_id(items::AXE, 2);
        srv.clients.get_mut(&1).unwrap().inv[0] = Some(Stack::new(axe, 1));
        srv.clients.get_mut(&1).unwrap().on_ground = true;
        rest(&mut srv, 5.0);
        let before = hp(&srv, a);
        srv.attack(1, a);
        assert_eq!(before - hp(&srv, a), 9.0);
        assert_eq!(srv.clients[&1].inv[0].map(|s| s.dur), Some(items::max_dur(axe) - 2));
    }

    #[test]
    fn reach_and_heavy_weapons() {
        let (mut srv, c) = lab();
        let _rx = player(&mut srv, c, Stack::new(items::tool_id(items::SWORD, 2), 1));
        // A 6,5 blocs : hors de portee du katana, a portee de la naginata.
        let far = target(&mut srv, c, 0.0, 6.5, mk::ZOMBIE);
        rest(&mut srv, 5.0);
        srv.attack(1, far);
        assert_eq!(hp(&srv, far), 100.0);
        srv.clients.get_mut(&1).unwrap().inv[0] = Some(Stack::new(items::NAGINATA, 1));
        rest(&mut srv, 5.0);
        srv.attack(1, far);
        assert_eq!(hp(&srv, far), 94.0);

        // Massue tetsubo : frappee en tombant de haut, elle ecrase.
        let near = target(&mut srv, c, 0.0, 2.0, mk::ZOMBIE);
        {
            let cl = srv.clients.get_mut(&1).unwrap();
            cl.inv[0] = Some(Stack::new(items::TETSUBO, 1));
            cl.on_ground = false;
            cl.fall_peak = cl.pos[1] + 4.5;
        }
        rest(&mut srv, 5.0);
        srv.attack(1, near);
        // 9 x 1,5 (critique) + 3 par bloc de chute au-dela de 1,5.
        assert!((100.0 - hp(&srv, near) - (13.5 + 9.0)).abs() < 0.01, "ecrasement : {}", 100.0 - hp(&srv, near));
        assert_eq!(srv.clients[&1].fall_peak, srv.clients[&1].pos[1], "le coup amortit la chute");
    }

    #[test]
    fn bow_spear_and_kunai() {
        let (mut srv, c) = lab();
        let _rx = player(&mut srv, c, Stack::new(items::YUMI, 1));
        let foe = target(&mut srv, c, 0.0, 5.0, mk::ZOMBIE);
        // Sans fleche, rien ne part.
        rest(&mut srv, 5.0);
        srv.shoot(1, 1.0);
        assert!(srv.projs.is_empty());
        srv.clients.get_mut(&1).unwrap().inv[5] = Some(Stack::new(items::ARROW, 2));
        // Vise un peu vers le bas, le torse de la cible.
        srv.clients.get_mut(&1).unwrap().pitch = -0.12;
        srv.shoot(1, 1.0);
        assert_eq!(srv.projs.len(), 1);
        assert_eq!(items::count(&srv.clients[&1].inv, items::ARROW), 1);
        assert_eq!(srv.clients[&1].inv[0].map(|s| s.dur), Some(items::max_dur(items::YUMI) - 1));
        for _ in 0..20 {
            srv.tick_projs();
        }
        assert!(srv.projs.is_empty());
        let full = 100.0 - hp(&srv, foe);
        assert!((9.0..=11.0).contains(&full), "fleche a pleine tension : {full}");

        // Corde a peine tendue : la fleche est molle.
        rest(&mut srv, 5.0);
        srv.shoot(1, 0.3);
        for _ in 0..40 {
            srv.tick_projs();
        }
        let weak = 100.0 - full - hp(&srv, foe);
        assert!(weak < full * 0.5, "fleche molle : {weak}");

        // La lance quitte la main, blesse, et se retrouve au sol, un peu plus usee.
        srv.clients.get_mut(&1).unwrap().inv[0] = Some(Stack::new(items::YARI, 1));
        rest(&mut srv, 5.0);
        let before = hp(&srv, foe);
        srv.shoot(1, 1.0);
        assert!(srv.clients[&1].inv[0].is_none());
        for _ in 0..20 {
            srv.tick_projs();
        }
        assert_eq!(before - hp(&srv, foe), 8.0);
        let spear = srv.drops.iter().find(|d| d.stack.id == items::YARI).map(|d| d.stack.dur);
        assert_eq!(spear, Some(items::max_dur(items::YARI) - 1));

        // Kunai : un de moins dans la pile ; s'il manque sa cible, on le ramasse.
        srv.clients.get_mut(&1).unwrap().inv[0] = Some(Stack::new(items::KUNAI, 3));
        srv.clients.get_mut(&1).unwrap().yaw = std::f64::consts::PI;
        rest(&mut srv, 5.0);
        srv.shoot(1, 1.0);
        assert_eq!(srv.clients[&1].inv[0].map(|s| s.n), Some(2));
        for _ in 0..60 {
            srv.tick_projs();
        }
        assert!(srv.drops.iter().any(|d| d.stack.id == items::KUNAI));
    }

    #[test]
    fn shield_blocks_the_front_only() {
        let (mut srv, c) = lab();
        let _rx = player(&mut srv, c, Stack::new(items::SHIELD, 1));
        srv.clients.get_mut(&1).unwrap().blocking = true;
        // Coup venu de face (le joueur regarde +Z, le recul le pousse vers -Z).
        srv.hurt_player(1, 5.0, Vec3::new(0.0, 3.0, -6.0));
        assert_eq!(srv.clients[&1].hp, 20.0);
        assert_eq!(srv.clients[&1].inv[0].map(|s| s.dur), Some(items::max_dur(items::SHIELD) - 5));
        // Coup dans le dos : le bouclier n'y peut rien.
        srv.clients.get_mut(&1).unwrap().inv_t = 0.0;
        srv.hurt_player(1, 5.0, Vec3::new(0.0, 3.0, 6.0));
        assert_eq!(srv.clients[&1].hp, 15.0);
        // Bouclier baisse : il ne pare plus.
        srv.clients.get_mut(&1).unwrap().inv_t = 0.0;
        srv.clients.get_mut(&1).unwrap().blocking = false;
        srv.hurt_player(1, 5.0, Vec3::new(0.0, 3.0, -6.0));
        assert_eq!(srv.clients[&1].hp, 10.0);
    }

    #[test]
    fn mining_wear_and_repair() {
        let (mut srv, c) = lab();
        let katana = items::tool_id(items::SWORD, 2);
        let _rx = player(&mut srv, c, Stack::new(katana, 1));
        // Miner avec une arme l'abime deux fois plus qu'avec le bon outil.
        let block = [c[0], c[1] - 1, c[2] + 1];
        srv.clients.get_mut(&1).unwrap().last_dig = Instant::now() - Duration::from_secs(60);
        srv.dig(1, block);
        assert_eq!(srv.world.get(block).map(|x| x.0), Some(AIR));
        assert_eq!(srv.clients[&1].inv[0].map(|s| s.dur), Some(items::max_dur(katana) - 2));
        let pick = items::tool_id(items::PICK, 2);
        srv.clients.get_mut(&1).unwrap().inv[0] = Some(Stack::new(pick, 1));
        let block = [c[0] + 1, c[1] - 1, c[2]];
        srv.clients.get_mut(&1).unwrap().last_dig = Instant::now() - Duration::from_secs(60);
        srv.dig(1, block);
        assert_eq!(srv.clients[&1].inv[0].map(|s| s.dur), Some(items::max_dur(pick) - 1));
    }
}
