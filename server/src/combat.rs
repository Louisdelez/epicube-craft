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
                // Infinite : la fleche n'est pas depensee (et ne se ramasse pas). Puissance : un quart de plus par niveau, et un.
                let endless = st.ench.level(shared::ench::INFINITY) > 0;
                if !endless {
                    items::remove(&mut c.inv, items::ARROW, 1);
                }
                let strength = match st.ench.level(shared::ench::POWER) {
                    0 => 1.0,
                    l => 1.0 + 0.25 * (l as f64 + 1.0),
                };
                // De 2 a 9 points selon la tension ; une fleche tiree a fond peut porter plus fort encore.
                let bonus = if power >= 0.99 && lucky { 2.0 } else { 0.0 };
                (dir * (10.0 + (ARROW_SPEED - 10.0) * power), 0, (2.0 + 7.0 * power + bonus) * strength, (!endless).then(|| Stack::new(items::ARROW, 1)))
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
    /// Renvoie vrai si le coup est pare ; le bouclier s'use d'autant. Il ne pare qu'un quart de seconde
    /// apres avoir ete leve, et une hache (`axe`) l'abat : il reste baisse cinq secondes.
    pub(crate) fn shield_blocks(&mut self, id: u32, dmg: f64, knock: Vec3, axe: bool) -> bool {
        let Some(c) = self.clients.get_mut(&id) else { return false };
        if !c.blocking || c.held() != Some(items::SHIELD) || c.block_since.elapsed().as_secs_f64() < 0.25 {
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
        c.last_dmg = dmg;
        if axe {
            c.blocking = false;
            c.shield_off = 5.0;
            c.send(ServerMsg::ShieldDown { secs: 5.0 });
            self.act(id, shared::proto::act::GUARD_DOWN);
        }
        let Some(c) = self.clients.get(&id) else { return true };
        let at = [c.pos[0].floor() as i32, c.pos[1].floor() as i32 + 1, c.pos[2].floor() as i32];
        self.sound(at, if axe { shared::proto::sound::SHIELD_DOWN } else { shared::proto::sound::PARRY }, 0);
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

    /// Un second joueur, `dz` blocs devant le premier (et `dx` sur le cote), tourne vers lui.
    fn rival(srv: &mut Server, c: [i32; 3], id: u32, dx: f64, dz: f64, held: Option<Stack>) -> Receiver<ServerMsg> {
        let (tx, rx) = mpsc::channel();
        let mut cl = Client::new(tx, [c[0] as f64 + 0.5 + dx, c[1] as f64, c[2] as f64 + 0.5 + dz]);
        cl.ready = true;
        cl.on_ground = true;
        cl.inv[0] = held;
        cl.yaw = std::f64::consts::PI;
        srv.clients.insert(id, cl);
        rx
    }

    /// Deux joueurs face a face, le premier arme d'un katana revenu en garde.
    fn duel() -> (Server, [i32; 3], Receiver<ServerMsg>, Receiver<ServerMsg>) {
        let (mut srv, c) = lab();
        srv.pvp = true;
        srv.rules.difficulty = shared::rules::NORMAL;
        let rx1 = player(&mut srv, c, Stack::new(items::tool_id(items::SWORD, 2), 1));
        let rx2 = rival(&mut srv, c, 2, 0.0, 2.0, None);
        rest(&mut srv, 5.0);
        (srv, c, rx1, rx2)
    }

    /// Remet tout le monde en etat de frapper et d'encaisser.
    fn again(srv: &mut Server) {
        rest(srv, 5.0);
        for c in srv.clients.values_mut() {
            (c.hp, c.inv_t, c.last_dmg) = (20.0, 0.0, 0.0);
        }
    }

    fn knock(rx: &Receiver<ServerMsg>) -> Option<Vec3> {
        rx.try_iter().filter_map(|m| if let ServerMsg::Knock { vel } = m { Some(Vec3::from(vel)) } else { None }).last()
    }

    #[test]
    fn pvp_sprint_hit_knocks_further_and_is_never_critical() {
        let (mut srv, _c, _rx1, rx2) = duel();
        srv.hit_player(1, 2, false);
        assert_eq!(srv.clients[&2].hp, 14.0);
        let plain = knock(&rx2).expect("recul");
        again(&mut srv);
        srv.hit_player(1, 2, true);
        let sprint = knock(&rx2).expect("recul");
        assert!(sprint.z > plain.z * 1.5, "un coup porte en courant repousse plus loin : {sprint} contre {plain}");
        // En retombant : critique a l'arret, pas en courant.
        again(&mut srv);
        let fall = |srv: &mut Server| {
            let cl = srv.clients.get_mut(&1).unwrap();
            cl.on_ground = false;
            cl.fall_peak = cl.pos[1] + 1.0;
        };
        fall(&mut srv);
        srv.hit_player(1, 2, false);
        assert_eq!(srv.clients[&2].hp, 11.0, "critique : 6 x 1,5");
        assert!(rx2.try_iter().any(|m| matches!(m, ServerMsg::PlayerHurt { id: 2, kind: shared::proto::blow::CRIT })));
        again(&mut srv);
        fall(&mut srv);
        srv.hit_player(1, 2, true);
        assert_eq!(srv.clients[&2].hp, 14.0);
    }

    #[test]
    fn pvp_knockback_lifts_only_from_the_ground() {
        let (mut srv, _c, _rx1, rx2) = duel();
        srv.hit_player(1, 2, false);
        let grounded = knock(&rx2).unwrap();
        again(&mut srv);
        srv.clients.get_mut(&2).unwrap().on_ground = false;
        srv.hit_player(1, 2, false);
        let airborne = knock(&rx2).unwrap();
        assert!(airborne.y < grounded.y * 0.5 && (airborne.z - grounded.z).abs() < 1e-9);
    }

    #[test]
    fn pvp_sweep_grazes_the_neighbours() {
        let (mut srv, c, _rx1, _rx2) = duel();
        let _rx3 = rival(&mut srv, c, 3, 1.0, 2.0, None);
        srv.hit_player(1, 2, false);
        assert_eq!((srv.clients[&2].hp, srv.clients[&3].hp), (14.0, 19.0));
        // En courant, la lame ne fauche pas.
        again(&mut srv);
        srv.hit_player(1, 2, true);
        assert_eq!(srv.clients[&3].hp, 20.0);
    }

    #[test]
    fn invulnerability_lets_only_a_stronger_blow_through() {
        let (mut srv, _c, _rx1, _rx2) = duel();
        assert_eq!(srv.strike(2, 4.0, Vec3::Z, false, 0), Struck::Hurt);
        assert_eq!(srv.strike(2, 3.0, Vec3::Z, false, 0), Struck::Missed);
        assert_eq!(srv.clients[&2].hp, 16.0);
        // Un coup plus fort passe pour la difference.
        assert_eq!(srv.strike(2, 6.0, Vec3::Z, false, 0), Struck::Hurt);
        assert_eq!(srv.clients[&2].hp, 14.0);
    }

    #[test]
    fn an_axe_knocks_the_shield_down() {
        let (mut srv, c, rx1, _rx2) = duel();
        let rx2 = rival(&mut srv, c, 2, 0.0, 2.0, Some(Stack::new(items::SHIELD, 1)));
        srv.on_msg(2, ClientMsg::Block(true));
        // A peine leve, le bouclier ne pare pas encore.
        srv.hit_player(1, 2, false);
        assert_eq!(srv.clients[&2].hp, 14.0);
        // Leve depuis un moment : il pare le katana, et renvoie un peu l'attaquant.
        again(&mut srv);
        srv.clients.get_mut(&2).unwrap().block_since = Instant::now() - Duration::from_secs(1);
        let _ = knock(&rx1);
        srv.hit_player(1, 2, false);
        assert_eq!(srv.clients[&2].hp, 20.0);
        assert!(srv.clients[&2].blocking);
        assert!(knock(&rx1).is_some_and(|k| k.z < 0.0));
        // Une hache l'abat : cinq secondes sans pouvoir le relever.
        again(&mut srv);
        srv.clients.get_mut(&1).unwrap().inv[0] = Some(Stack::new(items::tool_id(items::AXE, 2), 1));
        srv.hit_player(1, 2, false);
        assert_eq!(srv.clients[&2].hp, 20.0);
        assert!(!srv.clients[&2].blocking && srv.clients[&2].shield_off > 4.0);
        assert!(rx2.try_iter().any(|m| matches!(m, ServerMsg::ShieldDown { .. })));
        srv.on_msg(2, ClientMsg::Block(true));
        assert!(!srv.clients[&2].blocking);
    }

    #[test]
    fn a_swing_or_a_new_weapon_resets_the_guard() {
        let (mut srv, _c, _rx1, _rx2) = duel();
        srv.on_msg(1, ClientMsg::Swing { miss: true });
        srv.on_msg(1, ClientMsg::Hit { id: 2, sprint: false });
        assert_eq!(srv.clients[&2].hp, 20.0, "frapper aussitot apres un coup dans le vide ne porte pas");
        rest(&mut srv, 5.0);
        srv.clients.get_mut(&1).unwrap().inv[1] = Some(Stack::new(items::tool_id(items::AXE, 2), 1));
        srv.on_msg(1, ClientMsg::Select(1));
        srv.on_msg(1, ClientMsg::Hit { id: 2, sprint: false });
        assert_eq!(srv.clients[&2].hp, 20.0);
    }

    #[test]
    fn enchantments_change_the_fight() {
        use shared::ench as e;
        let (mut srv, _c, _rx1, rx2) = duel();
        let katana = srv.clients[&1].inv[0].unwrap();
        // Tranchant III : 2 points de plus. Recul II : la cible part plus loin.
        srv.hit_player(1, 2, false);
        let plain = knock(&rx2).unwrap();
        again(&mut srv);
        srv.clients.get_mut(&1).unwrap().inv[0] = Some(Stack { ench: e::Ench::NONE.with(e::SHARPNESS, 3).with(e::KNOCKBACK, 2), ..katana });
        srv.hit_player(1, 2, false);
        assert_eq!(srv.clients[&2].hp, 12.0);
        assert!(knock(&rx2).unwrap().z > plain.z * 2.0);
        // Protection IV sur les quatre pieces : 64 % de moins, en plus de l'armure.
        again(&mut srv);
        let bare = {
            let v = srv.clients.get_mut(&2).unwrap();
            for slot in 0..4u8 {
                v.armor[slot as usize] = Some(Stack::new(items::armor_id(slot, 2), 1));
            }
            srv.strike(2, 10.0, Vec3::Z, false, 0);
            20.0 - srv.clients[&2].hp
        };
        again(&mut srv);
        for st in srv.clients.get_mut(&2).unwrap().armor.iter_mut().flatten() {
            st.ench = e::Ench::NONE.with(e::PROTECTION, 4);
        }
        srv.strike(2, 10.0, Vec3::Z, false, 0);
        let guarded = 20.0 - srv.clients[&2].hp;
        assert!((guarded - bare * 0.36).abs() < 1e-9, "{guarded} contre {bare}");
        // Solidite III : l'arme ne s'use qu'un coup sur quatre environ.
        let tough = Stack { ench: e::Ench::NONE.with(e::UNBREAKING, 3), ..katana };
        srv.clients.get_mut(&1).unwrap().inv[0] = Some(tough);
        for _ in 0..200 {
            let roll = srv.rng.d();
            srv.clients.get_mut(&1).unwrap().wear(1, roll);
        }
        let used = tough.dur - srv.clients[&1].inv[0].unwrap().dur;
        assert!((25..=80).contains(&used), "usure : {used}");
    }

    #[test]
    fn experience_altar_and_anvil() {
        use shared::ench as e;
        let (mut srv, c) = lab();
        let rx = player(&mut srv, c, Stack::new(items::tool_id(items::PICK, 2), 1));
        // L'experience monte, et le Raccommodage en prend sa part pour reparer.
        srv.give_xp(1, e::points_for(30));
        assert_eq!(e::level_of(srv.clients[&1].xp).0, 30);
        // Un autel entoure de quinze bibliotheques propose jusqu'au niveau 30.
        let altar = [c[0], c[1], c[2] + 1];
        srv.world.set(altar, ENCHANT_TABLE, 0);
        let mut ring = 0;
        for dz in -2..=2i32 {
            for dx in -2..=2i32 {
                if (dx.abs() == 2 || dz.abs() == 2) && ring < 15 {
                    srv.world.set([altar[0] + dx, altar[1], altar[2] + dz], BOOKSHELF, 0);
                    ring += 1;
                }
            }
        }
        assert_eq!(srv.shelves(altar), 15);
        srv.open_window(1, Win::Enchant(altar));
        {
            let cl = srv.clients.get_mut(&1).unwrap();
            cl.craft[0] = cl.inv[0].take();
            cl.craft[1] = Some(Stack::new(CRYSTAL as u16, 3));
            cl.inv_dirty = true;
        }
        srv.flush();
        let offers = rx.try_iter().filter_map(|m| if let ServerMsg::Offers { offers, shelves: 15 } = m { Some(offers) } else { None }).last().expect("offres de l'autel");
        assert_eq!((offers.len(), offers[2].cost), (3, 30));
        srv.on_msg(1, ClientMsg::Enchant { index: 2 });
        let got = srv.clients[&1].craft[0].unwrap();
        assert_eq!(got.ench, offers[2].ench);
        assert!(srv.clients[&1].craft[1].is_none(), "trois cristaux payes");
        assert_eq!(e::level_of(srv.clients[&1].xp).0, 27, "trois niveaux payes");
        // Deja enchante : l'autel ne propose plus rien.
        srv.on_msg(1, ClientMsg::Enchant { index: 0 });
        assert_eq!(srv.clients[&1].craft[0].unwrap().ench, got.ench);
        // Enclume : deux pioches se reunissent, leurs enchantements aussi.
        srv.close_window(1);
        srv.open_window(1, Win::Anvil);
        let pick = Stack::new(items::tool_id(items::PICK, 2), 1);
        {
            let cl = srv.clients.get_mut(&1).unwrap();
            cl.craft[0] = Some(Stack { dur: 10, ench: e::Ench::NONE.with(e::EFFICIENCY, 2), ..pick });
            cl.craft[1] = Some(Stack { dur: 20, ench: e::Ench::NONE.with(e::EFFICIENCY, 2).with(e::UNBREAKING, 1), ..pick });
        }
        srv.on_msg(1, ClientMsg::Forge);
        let out = srv.clients[&1].craft[0].unwrap();
        assert_eq!(out.ench.list(), vec![(e::EFFICIENCY, 3), (e::UNBREAKING, 1)]);
        assert_eq!(out.dur, 30 + items::max_dur(pick.id) * 12 / 100);
        assert!(srv.clients[&1].craft[1].is_none());
        assert_eq!(e::level_of(srv.clients[&1].xp).0, 27 - 5);
        // Un objet enchante survit a la sauvegarde ; un objet ordinaire s'ecrit comme avant.
        let bytes = bincode::serialize(&Some(out)).unwrap();
        assert_eq!(bincode::deserialize::<Slot>(&bytes).unwrap(), Some(out));
        assert_eq!(bincode::serialize(&pick).unwrap(), bincode::serialize(&(pick.id, pick.n, pick.dur)).unwrap());
        // La mort emporte l'experience.
        srv.kill(1);
        assert_eq!(srv.clients[&1].xp, 0);
    }

    #[test]
    fn enchanted_tools_mine_faster_and_richer() {
        use shared::ench as e;
        let pick = Some(items::tool_id(items::PICK, 2));
        let (slow, fast) = (items::break_time_with(STONE, pick, 0).unwrap(), items::break_time_with(STONE, pick, 5).unwrap());
        assert!(fast < slow / 4.0, "{fast} contre {slow}");
        assert_eq!(items::break_time_with(STONE, None, 5), items::break_time(STONE, None), "sans le bon outil, l'efficacite ne sert pas");
        let mut rng = shared::util::Rng::new(3);
        assert_eq!(items::drops_with(DIAMOND_ORE, 0, &mut rng, e::Ench::NONE.with(e::SILK_TOUCH, 1)), vec![Stack::new(DIAMOND_ORE as u16, 1)]);
        let rich: u32 = (0..200).map(|_| items::drops_with(DIAMOND_ORE, 0, &mut rng, e::Ench::NONE.with(e::FORTUNE, 3)).iter().map(|s| s.n as u32).sum::<u32>()).sum();
        let poor: u32 = (0..200).map(|_| items::drops(DIAMOND_ORE, 0, &mut rng).iter().map(|s| s.n as u32).sum::<u32>()).sum();
        assert!(rich as f64 > poor as f64 * 1.7, "{rich} contre {poor}");
    }

    #[test]
    fn skins_reach_the_other_players() {
        let (mut srv, c) = lab();
        let rx1 = player(&mut srv, c, Stack::new(items::STICK, 1));
        let rx2 = rival(&mut srv, c, 2, 0.0, 2.0, None);
        let png = [b"\x89PNG\r\n\x1a\n".as_slice(), &[7u8; 200]].concat();
        srv.on_msg(1, ClientMsg::Skin { data: png.clone() });
        // L'autre joueur le recoit ; celui qui l'envoie, non.
        assert!(rx2.try_iter().any(|m| matches!(m, ServerMsg::PlayerSkin { id: 1, data } if data == png)));
        assert!(!rx1.try_iter().any(|m| matches!(m, ServerMsg::PlayerSkin { .. })));
        assert_eq!(srv.clients[&1].skin.as_deref(), Some(png.as_slice()));
        // Ce qui n'est pas une image, ou qui est trop gros, est ignore.
        srv.on_msg(2, ClientMsg::Skin { data: b"pas une image".to_vec() });
        srv.on_msg(2, ClientMsg::Skin { data: [b"\x89PNG".as_slice(), &vec![0u8; shared::proto::MAX_SKIN]].concat() });
        assert!(srv.clients[&2].skin.is_none());
        assert!(!rx1.try_iter().any(|m| matches!(m, ServerMsg::PlayerSkin { .. })));
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
