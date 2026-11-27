//! Passages entre les mondes : torii de Yomi, porte celeste de Takamagahara, peche et robe de plumes.
//!
//! Le torii suit la regle des portails : un cadre d'obsidienne dresse (interieur de 2 x 3 au moins),
//! allume au briquet, se remplit d'un voile ; y rester quatre secondes emporte dans l'autre monde.
//! Un pas dans Yomi en vaut huit a la surface, et l'on ressort par le torii le plus proche, ou par
//! un torii neuf si aucun n'attend la.
use super::*;
use shared::blocks::*;
use shared::signal::add;

/// Bit de metadonnee du voile (le bit 0 donne son axe : 0 = X, 1 = Z).
const VEIL: u8 = 2;
/// Secondes a passer dans le voile avant d'etre emporte.
const VEIL_TIME: f64 = 4.0;
/// Rapport des distances entre la surface et Yomi.
pub(crate) const YOMI_SCALE: i32 = 8;

fn mul(d: [i32; 3], k: i32) -> [i32; 3] {
    [d[0] * k, d[1] * k, d[2] * k]
}

impl Server {
    fn is_block(&self, p: [i32; 3], b: B) -> bool {
        self.world.get(p).is_some_and(|x| x.0 == b)
    }

    /// Interieur du cadre d'obsidienne qui contient la case `p`, dans l'axe donne, s'il est complet.
    fn torii_interior(&self, p: [i32; 3], axis: u8) -> Option<Vec<[i32; 3]>> {
        let d = if axis == 0 { [1, 0, 0] } else { [0, 0, 1] };
        let up = [0, 1, 0];
        // Coin bas de l'interieur.
        let mut o = p;
        for _ in 0..22 {
            if !self.is_block(add(o, [0, -1, 0]), AIR) {
                break;
            }
            o = add(o, [0, -1, 0]);
        }
        for _ in 0..22 {
            if !self.is_block(add(o, mul(d, -1)), AIR) {
                break;
            }
            o = add(o, mul(d, -1));
        }
        let w = (0..22).take_while(|i| self.is_block(add(o, mul(d, *i)), AIR)).count() as i32;
        let h = (0..22).take_while(|j| self.is_block(add(o, mul(up, *j)), AIR)).count() as i32;
        if !(2..=21).contains(&w) || !(3..=21).contains(&h) {
            return None;
        }
        let mut cells = Vec::new();
        for i in 0..w {
            let col = add(o, mul(d, i));
            if !self.is_block(add(col, [0, -1, 0]), OBSIDIAN) || !self.is_block(add(col, mul(up, h)), OBSIDIAN) {
                return None;
            }
            for j in 0..h {
                let q = add(col, mul(up, j));
                if !self.is_block(q, AIR) {
                    return None;
                }
                cells.push(q);
            }
        }
        for j in 0..h {
            let row = add(o, mul(up, j));
            if !self.is_block(add(row, mul(d, -1)), OBSIDIAN) || !self.is_block(add(row, mul(d, w)), OBSIDIAN) {
                return None;
            }
        }
        Some(cells)
    }

    /// Allume le torii dont `inside` est une case interieure. Renvoie vrai si le voile s'est leve.
    pub(crate) fn light_torii(&mut self, inside: [i32; 3]) -> bool {
        // Rien ne s'allume a Takamagahara.
        if self.dim == 2 || !self.is_block(inside, AIR) {
            return false;
        }
        for axis in 0..2u8 {
            if let Some(cells) = self.torii_interior(inside, axis) {
                for c in cells {
                    self.world.set(c, ABYSS_PORTAL, VEIL | axis);
                }
                return true;
            }
        }
        false
    }

    /// Un voile ne tient que borde d'obsidienne ou d'autres voiles : cadre brise, il se dissipe.
    pub(crate) fn veil_holds(&self, p: [i32; 3], m: u8) -> bool {
        let d = if m & 1 == 0 { [1, 0, 0] } else { [0, 0, 1] };
        [[0, 1, 0], [0, -1, 0], d, mul(d, -1)].iter().all(|e| match self.world.get(add(p, *e)) {
            Some((OBSIDIAN, _)) | None => true,
            Some((ABYSS_PORTAL, qm)) => qm & VEIL != 0,
            _ => false,
        })
    }

    /// Emporte les joueurs restes assez longtemps dans un voile. A appeler une fois par tick, hors de toute dimension.
    pub(crate) fn tick_veils(&mut self) {
        let mut go: Vec<(u32, u8)> = Vec::new();
        for dim in 0..2u8 {
            self.enter(dim);
            for (id, c) in self.clients.iter_mut().filter(|(_, c)| c.ready && !c.dead && c.dim == dim) {
                let feet = [c.pos[0].floor() as i32, (c.pos[1] + 0.2).floor() as i32, c.pos[2].floor() as i32];
                let inside = [feet, add(feet, [0, 1, 0])].iter().any(|q| matches!(self.world.get(*q), Some((ABYSS_PORTAL, m)) if m & VEIL != 0));
                if !inside {
                    // Hors du voile, le compteur retombe ; apres un voyage, il faut d'abord en sortir.
                    c.veil_t = 0.0;
                } else if c.veil_t >= 0.0 {
                    c.veil_t += DT;
                    if c.veil_t >= VEIL_TIME {
                        go.push((*id, dim));
                    }
                }
            }
        }
        for (id, from) in go {
            self.enter(from);
            self.travel(id, 1 - from);
            if let Some(c) = self.clients.get_mut(&id) {
                // Negatif : arrive dans un voile, on n'est pas remporte tant qu'on n'en est pas sorti.
                c.veil_t = -1.0;
            }
        }
    }

    /// Trouve le torii le plus proche de (x, z) dans la dimension active, ou en dresse un neuf.
    /// Renvoie la position ou poser le voyageur, dans le voile.
    pub(crate) fn arrive_torii(&mut self, x: i32, z: i32) -> [f64; 3] {
        let radius = 16;
        for cz in (z - radius) >> 4..=(z + radius) >> 4 {
            for cx in (x - radius) >> 4..=(x + radius) >> 4 {
                self.world.ensure((cx, cz));
            }
        }
        // Le voile le plus proche, pris par sa case la plus basse.
        let mut best: Option<(i32, [i32; 3], u8)> = None;
        for qz in z - radius..=z + radius {
            for qx in x - radius..=x + radius {
                for qy in 2..H as i32 - 2 {
                    if let Some((ABYSS_PORTAL, m)) = self.world.get([qx, qy, qz]) {
                        let d2 = (qx - x).pow(2) + (qz - z).pow(2);
                        if best.is_none_or(|b| d2 < b.0) {
                            best = Some((d2, [qx, qy, qz], m));
                        }
                        break;
                    }
                }
            }
        }
        if let Some((_, p, m)) = best {
            // Ancien portail d'un seul bloc : on arrive a cote.
            let off = if m & VEIL != 0 { 0.5 } else { -0.5 };
            return [p[0] as f64 + off, p[1] as f64 + 0.05, p[2] as f64 + 0.5];
        }
        // Aucun : on dresse un torii de 4 x 5, sur une dalle si le sol manque.
        let y = self.arrival_y(x, z);
        for i in -1..=2 {
            for j in -1..=3 {
                let frame = i == -1 || i == 2 || j == -1 || j == 3;
                self.world.set([x + i, y + j, z], if frame { OBSIDIAN } else { ABYSS_PORTAL }, if frame { 0 } else { VEIL });
            }
            for side in [-1, 1] {
                let floor = [x + i, y - 1, z + side];
                if !self.world.get(floor).is_some_and(|(b, m)| blocks::solid(b, m)) {
                    self.world.set(floor, OBSIDIAN, 0);
                }
                for j in 0..3 {
                    let q = [x + i, y + j, z + side];
                    if !self.is_block(q, AIR) {
                        self.world.set(q, AIR, 0);
                    }
                }
            }
        }
        [x as f64 + 0.5, y as f64 + 0.05, z as f64 + 0.5]
    }

    /// Deplace un joueur a la surface du monde courant, en (x, z).
    pub(crate) fn warp(&mut self, id: u32, x: i32, z: i32) {
        for dz in -1..=1 {
            for dx in -1..=1 {
                self.world.ensure(((x >> 4) + dx, (z >> 4) + dz));
            }
        }
        let y = self.arrival_y(x, z);
        self.place_player(id, [x as f64 + 0.5, y as f64 + 0.05, z as f64 + 0.5]);
    }

    fn place_player(&mut self, id: u32, pos: [f64; 3]) {
        if let Some(c) = self.clients.get_mut(&id) {
            c.pos = pos;
            c.prev = pos;
            c.fall_peak = pos[1];
            c.moved = true;
            c.send(ServerMsg::Teleport { pos });
        }
    }

    /// Porte celeste : de l'ile du Ryujin a celle de la pagode, et retour.
    pub(crate) fn use_gateway(&mut self, id: u32) {
        let Some(c) = self.clients.get(&id) else { return };
        let far = Vec3::new(c.pos[0], 0.0, c.pos[2]).length() > 150.0;
        let (x, z) = if far { (0, -6) } else { (gen::PAGODA[0], gen::PAGODA[1] - 11) };
        self.warp(id, x, z);
        self.info(id, if far { "Retour sur l'ile du Ryujin" } else { "L'ile de la pagode celeste" });
    }

    /// Peche celeste : transporte a quelques pas, sur un sol ferme (seize essais, sinon rien).
    pub(crate) fn blink(&mut self, id: u32) {
        let Some(c) = self.clients.get(&id) else { return };
        let o = [c.pos[0].floor() as i32, c.pos[1].floor() as i32, c.pos[2].floor() as i32];
        for _ in 0..16 {
            let (x, z) = (o[0] + self.rng.range(-8, 9), o[2] + self.rng.range(-8, 9));
            let spot = (o[1] - 8..=o[1] + 8).rev().find(|&y| {
                self.world.get([x, y - 1, z]).is_some_and(|(b, m)| blocks::solid(b, m)) && self.is_block([x, y, z], AIR) && self.is_block([x, y + 1, z], AIR)
            });
            if let Some(y) = spot {
                if (x, y, z) != (o[0], o[1], o[2]) {
                    self.place_player(id, [x as f64 + 0.5, y as f64 + 0.05, z as f64 + 0.5]);
                    return;
                }
            }
        }
    }

    /// Robe de plumes : s'use d'un cran par seconde de vol, et se replie des qu'elle manque.
    pub(crate) fn tick_gliders(&mut self) {
        for c in self.clients.values_mut().filter(|c| c.gliding) {
            // La robe portee sur le torse passe avant celle du sac.
            let is_robe = |s: &Slot| s.is_some_and(|st| st.id == items::HAGOROMO);
            let body = items::BODY as usize;
            let in_bag = c.inv.iter().position(is_robe);
            if !is_robe(&c.armor[body]) && in_bag.is_none() {
                c.gliding = false;
                continue;
            }
            c.glide_acc += DT;
            if c.glide_acc >= 1.0 {
                c.glide_acc = 0.0;
                let robe = if is_robe(&c.armor[body]) { &mut c.armor[body] } else { &mut c.inv[in_bag.unwrap_or(0)] };
                if let Some(st) = robe.as_mut() {
                    st.dur = st.dur.saturating_sub(1);
                    if st.dur == 0 {
                        *robe = None;
                        c.gliding = false;
                    }
                }
                c.inv_dirty = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::mobs as mk;

    fn lab() -> (Server, [i32; 3]) {
        let mut srv = Server::new(7, None);
        srv.world.ensure((0, 0));
        for dz in -6..=6 {
            for dx in -6..=6 {
                srv.world.set([8 + dx, 300, 8 + dz], STONE, 0);
                for dy in 1..=12 {
                    srv.world.set([8 + dx, 300 + dy, 8 + dz], AIR, 0);
                }
            }
        }
        srv.world.updates.clear();
        (srv, [8, 301, 8])
    }

    fn player(srv: &mut Server, pos: [f64; 3]) -> Receiver<ServerMsg> {
        let (tx, rx) = mpsc::channel();
        let mut c = Client::new(tx, pos);
        c.ready = true;
        srv.clients.insert(1, c);
        rx
    }

    /// Dresse un cadre d'obsidienne dont l'interieur fait `w` x `h`, coin bas en `o`, dans l'axe X.
    fn frame(srv: &mut Server, o: [i32; 3], w: i32, h: i32) {
        for i in -1..=w {
            for j in -1..=h {
                if i == -1 || i == w || j == -1 || j == h {
                    srv.world.set([o[0] + i, o[1] + j, o[2]], OBSIDIAN, 0);
                }
            }
        }
    }

    #[test]
    fn torii_lights_only_when_complete() {
        let (mut srv, c) = lab();
        let o = [c[0], c[1] + 1, c[2]];
        frame(&mut srv, o, 2, 3);
        // Un bloc manque au cadre : rien ne s'allume.
        srv.world.set([o[0] + 2, o[1] + 1, o[2]], AIR, 0);
        assert!(!srv.light_torii(o));
        srv.world.set([o[0] + 2, o[1] + 1, o[2]], OBSIDIAN, 0);
        assert!(srv.light_torii([o[0] + 1, o[1] + 2, o[2]]), "n'importe quelle case interieure convient");
        for i in 0..2 {
            for j in 0..3 {
                assert_eq!(srv.world.get([o[0] + i, o[1] + j, o[2]]), Some((ABYSS_PORTAL, 2)));
            }
        }
        // Trop petit : interieur de 1 x 3.
        let small = [c[0] - 5, c[1] + 1, c[2] + 4];
        frame(&mut srv, small, 1, 3);
        assert!(!srv.light_torii(small));
        // Dans l'autre axe aussi.
        let turned = [c[0] + 4, c[1] + 1, c[2] - 4];
        for i in -1..=2 {
            for j in -1..=3 {
                if i == -1 || i == 2 || j == -1 || j == 3 {
                    srv.world.set([turned[0], turned[1] + j, turned[2] + i], OBSIDIAN, 0);
                }
            }
        }
        assert!(srv.light_torii(turned));
        assert_eq!(srv.world.get(turned), Some((ABYSS_PORTAL, 3)));

        // Cadre brise : le voile se dissipe de proche en proche.
        srv.world.set([o[0] - 1, o[1], o[2]], AIR, 0);
        for _ in 0..8 {
            srv.tick_updates();
        }
        for i in 0..2 {
            for j in 0..3 {
                assert_eq!(srv.world.get([o[0] + i, o[1] + j, o[2]]).map(|x| x.0), Some(AIR));
            }
        }
        assert_eq!(srv.world.get(turned).map(|x| x.0), Some(ABYSS_PORTAL), "l'autre torii tient toujours");
    }

    #[test]
    fn veil_carries_across_at_one_to_eight() {
        let (mut srv, _) = lab();
        // Un torii a la surface, loin de l'origine, pour voir le rapport des distances.
        srv.world.ensure((5, 2));
        let o = [88, 112, 40];
        for dy in -2..6 {
            for dz in -2..=2 {
                for dx in -2..=3 {
                    srv.world.set([o[0] + dx, o[1] + dy, o[2] + dz], if dy < 0 { STONE } else { AIR }, 0);
                }
            }
        }
        frame(&mut srv, o, 2, 3);
        assert!(srv.light_torii(o));
        let rx = player(&mut srv, [o[0] as f64 + 0.5, o[1] as f64, o[2] as f64 + 0.5]);
        // Trois secondes ne suffisent pas ; quatre, si.
        for _ in 0..(3.0 / DT) as usize {
            srv.tick_veils();
        }
        assert_eq!(srv.clients[&1].dim, 0);
        for _ in 0..(1.2 / DT) as usize {
            srv.tick_veils();
        }
        assert_eq!(srv.clients[&1].dim, 1);
        assert!(rx.try_iter().any(|m| matches!(m, ServerMsg::Dimension { dim: 1, .. })));
        let p = srv.clients[&1].pos;
        assert!((p[0] - 11.5).abs() < 0.01 && (p[2] - 5.5).abs() < 0.01, "88 / 8 = 11, 40 / 8 = 5 : arrive en {p:?}");
        // Un torii neuf attend a l'arrivee, voile leve, sur un sol ferme.
        srv.enter(1);
        let feet = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
        assert_eq!(srv.world.get(feet), Some((ABYSS_PORTAL, 2)));
        assert_eq!(srv.world.get([feet[0], feet[1] - 1, feet[2]]).map(|x| x.0), Some(OBSIDIAN));
        assert!(blocks::solid(srv.world.get([feet[0], feet[1] - 1, feet[2] + 1]).unwrap().0, 0));

        // Rester dans le voile d'arrivee ne remporte pas ; en sortir puis y revenir, si.
        for _ in 0..(6.0 / DT) as usize {
            srv.tick_veils();
        }
        assert_eq!(srv.clients[&1].dim, 1);
        srv.clients.get_mut(&1).unwrap().pos[2] += 2.0;
        srv.tick_veils();
        srv.clients.get_mut(&1).unwrap().pos[2] -= 2.0;
        for _ in 0..(4.2 / DT) as usize {
            srv.tick_veils();
        }
        assert_eq!(srv.clients[&1].dim, 0);
        // Retour par le torii d'origine, pas par un neuf.
        let back = srv.clients[&1].pos;
        assert!((back[0] - 88.5).abs() < 1.1 && (back[2] - 40.5).abs() < 0.01, "retour en {back:?}");
    }

    #[test]
    fn yomi_rules_and_ryujin_crystals() {
        let (mut srv, c) = lab();
        let _rx = player(&mut srv, [c[0] as f64 + 0.5, c[1] as f64, c[2] as f64 + 0.5]);
        srv.travel(1, 1);
        let p = srv.clients[&1].pos;
        let feet = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
        // L'eau s'evapore : le seau se vide sans rien poser.
        srv.clients.get_mut(&1).unwrap().inv[0] = Some(Stack::new(items::WATER_BUCKET, 1));
        let ground = [feet[0], feet[1] - 1, feet[2] + 1];
        let above = [ground[0], ground[1] + 1, ground[2]];
        assert!(srv.use_tool(1, ground, above));
        assert_eq!(srv.world.get(above).map(|x| x.0), Some(AIR));
        assert_eq!(srv.clients[&1].inv[0].map(|s| s.id), Some(items::BUCKET));
        // Un futon y explose au lieu d'offrir le repos.
        srv.world.set(above, BED, 0);
        srv.clients.get_mut(&1).unwrap().inv_t = 0.0;
        srv.sleep(1, above);
        assert_eq!(srv.world.get(above).map(|x| x.0), Some(AIR));
        assert!(srv.clients[&1].hp < 20.0, "l'explosion blesse");
        assert!(srv.clients[&1].home.is_none());

        // Takamagahara : tant qu'un cristal brille, le Ryujin blesse se soigne.
        srv.clients.get_mut(&1).unwrap().hp = 20.0;
        srv.travel(1, 2);
        srv.spawn_cycle();
        let crystals = gen::zenith_crystals(srv.world.seed);
        for p in &crystals {
            srv.world.ensure((p[0] >> 4, p[2] >> 4));
            assert_eq!(srv.world.get(*p).map(|x| x.0), Some(CRYSTAL));
        }
        srv.mobs.iter_mut().find(|m| m.kind == mk::WYRM).unwrap().hp = 100.0;
        srv.spawn_cycle();
        let hp = srv.mobs.iter().find(|m| m.kind == mk::WYRM).unwrap().hp;
        assert_eq!(hp, 102.0);
        // Une fleche dans un cristal le fait eclater, et le dragon en souffre.
        srv.hit_target(crystals[0], Vec3::ZERO);
        assert_eq!(srv.world.get(crystals[0]).map(|x| x.0), Some(AIR));
        assert_eq!(srv.mobs.iter().find(|m| m.kind == mk::WYRM).unwrap().hp, 92.0);
        // Tous brises : plus de soin.
        for p in &crystals {
            srv.world.set(*p, AIR, 0);
        }
        srv.spawn_cycle();
        assert_eq!(srv.mobs.iter().find(|m| m.kind == mk::WYRM).unwrap().hp, 92.0);

        // Le dragon vaincu, une porte celeste s'ouvre ; elle mene a l'ile de la pagode, et en ramene.
        srv.mobs.iter_mut().find(|m| m.kind == mk::WYRM).unwrap().hp = 0.0;
        srv.tick_mobs();
        let gate = (1..H as i32).rev().map(|y| [0, y, -8]).find(|q| srv.world.get(*q).is_some_and(|x| x.0 == GATEWAY)).expect("porte celeste");
        srv.clients.get_mut(&1).unwrap().pos = [0.5, gate[1] as f64, -6.5];
        srv.use_item(1, Some((gate, gate)));
        let p = srv.clients[&1].pos;
        assert!((p[2] - gen::PAGODA[1] as f64).abs() < 20.0, "sur l'ile de la pagode : {p:?}");
        assert!(blocks::solid(srv.world.get([p[0].floor() as i32, p[1].floor() as i32 - 1, p[2].floor() as i32]).unwrap().0, 0));
        srv.use_gateway(1);
        assert!(Vec3::from(srv.clients[&1].pos).length() < 120.0);
    }

    #[test]
    fn peach_and_feather_robe() {
        let (mut srv, c) = lab();
        let start = [c[0] as f64 + 0.5, c[1] as f64, c[2] as f64 + 0.5];
        let rx = player(&mut srv, start);
        // Peche celeste : on se retrouve ailleurs, a huit blocs au plus, les pieds sur du solide.
        {
            let cl = srv.clients.get_mut(&1).unwrap();
            cl.inv[0] = Some(Stack::new(items::PEACH, 2));
            cl.hunger = 10.0;
        }
        srv.eat(1);
        let p = srv.clients[&1].pos;
        assert_ne!(p, start);
        assert!((p[0] - start[0]).abs() <= 8.0 && (p[2] - start[2]).abs() <= 8.0);
        assert!(blocks::solid(srv.world.get([p[0].floor() as i32, p[1].floor() as i32 - 1, p[2].floor() as i32]).unwrap().0, 0));
        assert!(rx.try_iter().any(|m| matches!(m, ServerMsg::Teleport { .. })));

        // Robe de plumes : sans elle, pas de vol ; avec elle, la chute ne blesse plus et elle s'use.
        srv.on_msg(1, ClientMsg::Glide(true));
        assert!(!srv.clients[&1].gliding);
        srv.clients.get_mut(&1).unwrap().inv[4] = Some(Stack::new(items::HAGOROMO, 1));
        srv.on_msg(1, ClientMsg::Glide(true));
        assert!(srv.clients[&1].gliding);
        for _ in 0..(3.2 / DT) as usize {
            srv.tick_gliders();
        }
        assert_eq!(srv.clients[&1].inv[4].map(|s| s.dur), Some(items::max_dur(items::HAGOROMO) - 3));
        {
            let cl = srv.clients.get_mut(&1).unwrap();
            cl.on_ground = false;
            cl.fall_peak = cl.pos[1] + 40.0;
        }
        let pos = srv.clients[&1].pos;
        srv.on_pos(1, pos, 0.0, 0.0, true);
        assert_eq!(srv.clients[&1].hp, 20.0, "atterrissage en douceur");
        assert!(!srv.clients[&1].gliding, "au sol, la robe se replie");
    }
}
