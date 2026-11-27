//! Ferme et cuisine : engrais, compost, cueillette du the, peche, oeufs, foyer irori et repas.
use super::*;
use shared::blocks::*;
use shared::signal::add;

/// Ligne a l'eau : ou elle est, quand ca mordra, et si le joueur a ete prevenu.
pub(crate) struct Fishing {
    pos: [i32; 3],
    bite: Instant,
    told: bool,
}

/// Secondes pour cuire un aliment sur le foyer irori.
const IRORI_TIME: f64 = 30.0;
/// Apres la touche, on a deux secondes pour ferrer.
const BITE_WINDOW: f64 = 2.0;

/// Ce qu'on remonte au bout de la ligne : surtout du poisson, parfois un rebut, rarement un tresor.
pub(crate) fn fishing_loot(rng: &mut Rng) -> Stack {
    let (kind, r) = (rng.d(), rng.d());
    let id = if kind < 0.85 {
        if r < 0.87 {
            items::RAW_FISH
        } else {
            items::FUGU
        }
    } else if kind < 0.95 {
        match (r * 6.0) as u32 {
            0 | 1 => items::NORI,
            2 => items::BONE,
            3 => items::BOWL,
            4 => items::STRING,
            _ => items::STICK,
        }
    } else if r < 0.5 {
        items::GOLD_INGOT
    } else if r < 0.85 {
        items::PEARL
    } else {
        items::DIAMOND
    };
    Stack::new(id, 1)
}

impl Server {
    fn held_id(&self, id: u32) -> Option<u16> {
        self.clients.get(&id).and_then(|c| c.held())
    }

    /// Retire un objet de la pile tenue.
    fn consume_held(&mut self, id: u32) {
        if let Some(c) = self.clients.get_mut(&id) {
            if let Some(st) = c.inv[c.sel] {
                c.inv[c.sel] = if st.n > 1 { Some(Stack { n: st.n - 1, ..st }) } else { None };
                c.inv_dirty = true;
            }
        }
    }

    /// Use d'un cran l'outil tenu.
    pub(crate) fn wear_held(&mut self, id: u32) {
        if let Some(c) = self.clients.get_mut(&id) {
            if let Some(st) = c.inv[c.sel].as_mut().filter(|s| items::max_dur(s.id) > 0) {
                st.dur = st.dur.saturating_sub(1);
                if st.dur == 0 {
                    c.inv[c.sel] = None;
                }
                c.inv_dirty = true;
            }
        }
    }

    /// Clic droit lie a la ferme ou a la cuisine. Renvoie vrai s'il a ete traite.
    pub(crate) fn farm_use(&mut self, id: u32, pos: [i32; 3], place: [i32; 3], b: B, m: u8) -> bool {
        let held = self.held_id(id);
        match b {
            // Cueillette : le theier garde ses branches et refera des pousses.
            TEA if m >= 3 => {
                let n = self.rng.range(2, 4) as u8;
                self.world.set(pos, TEA, 1);
                self.drop_at_block(pos, Stack::new(items::TEA_LEAF, n));
                return true;
            }
            COMPOSTER => {
                if m >= 8 {
                    self.world.set(pos, COMPOSTER, 0);
                    self.drop_at_block(add(pos, [0, 1, 0]), Stack::new(items::FERTILIZER, 1));
                    return true;
                }
                let chance = held.map(items::compost_chance).unwrap_or(0.0);
                if chance <= 0.0 {
                    return false;
                }
                self.consume_held(id);
                // Le premier dechet prend toujours ; a 7 crans, le compost est mur.
                if m == 0 || self.rng.d() < chance {
                    self.world.set(pos, COMPOSTER, if m + 1 >= 7 { 8 } else { m + 1 });
                }
                return true;
            }
            SMOKER => {
                self.world.furnaces.entry(pos).or_default();
                self.open_window(id, Win::Furnace(pos));
                return true;
            }
            IRORI => {
                let Some(st) = self.clients.get(&id).and_then(|c| c.inv[c.sel]).filter(|s| craft::grills(s.id)) else { return false };
                let grid = self.world.irori.entry(pos).or_default();
                if grid.len() < 4 {
                    grid.push((Stack { n: 1, ..st }, 0.0));
                    self.consume_held(id);
                    self.world.set(pos, IRORI, 1);
                }
                return true;
            }
            _ => {}
        }
        match held {
            Some(items::FERTILIZER) => {
                if self.fertilize(pos, b, m) {
                    self.consume_held(id);
                    return true;
                }
                false
            }
            Some(items::FISHING_ROD) => {
                self.fish(id, pos, b);
                true
            }
            Some(items::EGG) => {
                // Un oeuf lance sur huit donne un petit.
                self.consume_held(id);
                if self.rng.chance(0.125) {
                    let at = Vec3::new(place[0] as f64 + 0.5, place[1] as f64 + 0.1, place[2] as f64 + 0.5);
                    self.spawn_mob(shared::mobs::CHICKEN, at, -180.0, shared::mobs::max_hp(shared::mobs::CHICKEN));
                }
                true
            }
            _ => false,
        }
    }

    /// L'engrais fait pousser d'un coup ce qui prendrait des minutes.
    pub(crate) fn fertilize(&mut self, pos: [i32; 3], b: B, m: u8) -> bool {
        match b {
            WHEAT | CARROT | SOY | STEM if m < 3 => {
                let gain = 1 + self.rng.range(0, 2) as u8;
                self.world.set(pos, b, (m + gain).min(3));
            }
            TEA if m < 3 => {
                self.world.set(pos, b, m + 1);
            }
            SAPLING => {
                if self.rng.chance(0.45) {
                    self.grow_tree(pos);
                }
            }
            BAMBOO => {
                let mut top = pos;
                while self.world.get(add(top, [0, 1, 0])).is_some_and(|x| x.0 == BAMBOO) {
                    top = add(top, [0, 1, 0]);
                }
                let mut grown = false;
                for _ in 0..self.rng.range(1, 3) {
                    top = add(top, [0, 1, 0]);
                    if self.world.get(top).is_some_and(|x| x.0 == AIR) {
                        grown |= self.world.set(top, BAMBOO, 0);
                    }
                }
                return grown;
            }
            GRASS => {
                // Herbes et fleurs jaillissent autour.
                for _ in 0..10 {
                    let q = add(pos, [self.rng.range(-3, 4), 0, self.rng.range(-3, 4)]);
                    let up = add(q, [0, 1, 0]);
                    if self.world.get(q).is_some_and(|x| x.0 == GRASS) && self.world.get(up).is_some_and(|x| x.0 == AIR) {
                        let plant = if self.rng.chance(0.15) { FLOWER } else { TALLGRASS };
                        self.world.set(up, plant, 0);
                    }
                }
            }
            _ => return false,
        }
        true
    }

    /// Canne a peche : un clic sur l'eau lance la ligne, un autre la remonte.
    fn fish(&mut self, id: u32, pos: [i32; 3], b: B) {
        let now = Instant::now();
        let wait = Duration::from_secs_f64(5.0 + self.rng.d() * 25.0);
        let catch = fishing_loot(&mut self.rng);
        let Some(c) = self.clients.get_mut(&id) else { return };
        match c.fishing.take() {
            None if b == WATER => {
                c.fishing = Some(Fishing { pos, bite: now + wait, told: false });
                c.send(ServerMsg::Info { text: "Ligne lancee... attendez la touche".to_string() });
            }
            None => {}
            Some(f) => {
                if f.told && now.duration_since(f.bite).as_secs_f64() <= BITE_WINDOW {
                    c.send(ServerMsg::Info { text: format!("Prise : {}", items::name(catch.id)) });
                    self.give(id, catch);
                    self.wear_held(id);
                } else {
                    c.send(ServerMsg::Info { text: "Ligne remontee : rien au bout".to_string() });
                }
            }
        }
    }

    /// Previent les pecheurs quand ca mord ; une touche manquee, et l'attente recommence.
    pub(crate) fn tick_fishing(&mut self) {
        let now = Instant::now();
        let again = Duration::from_secs_f64(5.0 + self.rng.d() * 25.0);
        for c in self.clients.values_mut() {
            let (dead, held, at) = (c.dead, c.held(), Vec3::from(c.pos));
            let Some(f) = c.fishing.as_mut() else { continue };
            let far = at.distance(Vec3::new(f.pos[0] as f64, f.pos[1] as f64, f.pos[2] as f64)) > 16.0;
            if dead || far || held != Some(items::FISHING_ROD) {
                c.fishing = None;
            } else if !f.told && now >= f.bite {
                f.told = true;
                let _ = c.tx.send(ServerMsg::Info { text: "Ca mord ! Remontez la ligne".to_string() });
            } else if f.told && now.duration_since(f.bite).as_secs_f64() > BITE_WINDOW {
                f.told = false;
                f.bite = now + again;
            }
        }
    }

    /// Foyers irori : ce qui est pose dessus cuit en trente secondes, sans combustible.
    pub(crate) fn tick_irori(&mut self) {
        if self.world.irori.is_empty() {
            return;
        }
        let mut done: Vec<([i32; 3], Stack)> = Vec::new();
        let mut cold: Vec<[i32; 3]> = Vec::new();
        for (pos, grid) in self.world.irori.iter_mut() {
            grid.retain_mut(|(st, t)| {
                *t += DT;
                if *t >= IRORI_TIME {
                    done.push((*pos, Stack::new(craft::smelt(st.id).unwrap_or(st.id), 1)));
                }
                *t < IRORI_TIME
            });
            if grid.is_empty() {
                cold.push(*pos);
            }
        }
        for (pos, st) in done {
            self.drop_at_block(add(pos, [0, 1, 0]), st);
        }
        for pos in cold {
            self.world.irori.remove(&pos);
            if self.world.get(pos).is_some_and(|x| x.0 == IRORI) {
                self.world.set(pos, IRORI, 0);
            }
        }
    }

    /// Mange ou boit ce qui est tenu.
    pub(crate) fn eat(&mut self, id: u32) {
        let now = Instant::now();
        let roll = self.rng.d();
        let Some(c) = self.clients.get_mut(&id) else { return };
        let Some(st) = c.inv[c.sel] else { return };
        let Some((points, sat)) = items::meal(st.id) else { return };
        // Le the se boit meme le ventre plein.
        let tea = st.id == items::GREEN_TEA;
        if (c.hunger >= 20.0 && !tea) || now < c.next_eat {
            return;
        }
        c.hunger = (c.hunger + points).min(20.0);
        c.saturation = (c.saturation + sat).min(c.hunger);
        match items::risk(st.id) {
            items::Risk::Queasy(p) if roll < p => c.queasy_t = 30.0,
            items::Risk::Poison => {
                c.poison_t = 60.0;
                c.queasy_t = 15.0;
            }
            _ => {}
        }
        if tea {
            // Le the vert chasse le poison et la nausee, et requinque un moment.
            c.poison_t = 0.0;
            c.queasy_t = 0.0;
            c.tea_t = 10.0;
        }
        c.next_eat = now + Duration::from_millis(900);
        c.inv[c.sel] = if st.n > 1 {
            Some(Stack { n: st.n - 1, ..st })
        } else if items::served_in_bowl(st.id) {
            Some(Stack::new(items::BOWL, 1))
        } else {
            None
        };
        c.inv_dirty = true;
        c.stats_dirty = true;
        if st.id == items::PEACH {
            self.blink(id);
        }
    }
}

impl Server {
    /// Aide au developpement : EPICUBE_DEMO=ferme construit une ferme de demonstration devant le joueur.
    pub(crate) fn demo_farm(&mut self) {
        let o = [self.spawn[0].floor() as i32 - 6, self.spawn[1].floor() as i32 - 2, self.spawn[2].floor() as i32 + 3];
        let put = |srv: &mut Server, x: i32, dy: i32, z: i32, b: B, m: u8| {
            let p = [o[0] + x, o[1] + dy, o[2] + z];
            srv.world.ensure((p[0] >> 4, p[2] >> 4));
            srv.world.set(p, b, m);
            p
        };
        for z in -1..=11 {
            for x in -1..=13 {
                put(self, x, -1, z, GRASS, 0);
                for dy in 0..9 {
                    put(self, x, dy, z, AIR, 0);
                }
            }
        }
        // Quatre rangs de cultures, du semis a la recolte, de part et d'autre d'une rigole.
        for (row, crop) in [WHEAT, CARROT, SOY, STEM].into_iter().enumerate() {
            for stage in 0..4 {
                let (x, z) = (stage, if row < 2 { row as i32 } else { row as i32 + 1 });
                put(self, x, -1, z, FARMLAND, 0);
                put(self, x, 0, z, crop, stage as u8);
            }
        }
        for x in 0..4 {
            put(self, x, -1, 2, WATER, 0);
        }
        put(self, 4, 0, 4, KABOCHA, 0);
        // Theiers aux quatre stades, bambous, compost, gril, foyer.
        for stage in 0..4 {
            put(self, 6 + stage, 0, 0, TEA, stage as u8);
        }
        for (x, h) in [(6, 3), (7, 6), (8, 4)] {
            for dy in 0..h {
                put(self, x, dy, 3, BAMBOO, 0);
            }
        }
        put(self, 10, 0, 3, COMPOSTER, 0);
        put(self, 11, 0, 3, COMPOSTER, 4);
        put(self, 12, 0, 3, COMPOSTER, 8);
        put(self, 6, 0, 6, FURNACE, 0);
        let grill = put(self, 8, 0, 6, SMOKER, 0);
        let f = self.world.furnaces.entry(grill).or_default();
        f.input = Some(Stack::new(items::RAW_FISH, 8));
        f.fuel = Some(Stack::new(items::COAL, 4));
        let hearth = put(self, 10, 0, 6, IRORI, 1);
        self.world.irori.insert(hearth, vec![(Stack::new(items::RAW_BIRD, 1), 0.0)]);
        self.world.updates.clear();
        for (kind, x) in [(shared::mobs::SHEEP, 2.5), (shared::mobs::CHICKEN, 5.5), (shared::mobs::COW, 9.5)] {
            let at = Vec3::new(o[0] as f64 + x, o[1] as f64, o[2] as f64 + 9.5);
            self.spawn_mob(kind, at, 0.0, shared::mobs::max_hp(kind));
        }
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
                srv.world.set([8 + dx, 300, 8 + dz], DIRT, 0);
                for dy in 1..=14 {
                    srv.world.set([8 + dx, 300 + dy, 8 + dz], AIR, 0);
                }
            }
        }
        srv.world.updates.clear();
        (srv, [8, 301, 8])
    }

    fn player(srv: &mut Server, pos: [i32; 3], held: Option<Stack>) -> Receiver<ServerMsg> {
        let (tx, rx) = mpsc::channel();
        let mut c = Client::new(tx, [pos[0] as f64 + 0.5, pos[1] as f64, pos[2] as f64 + 0.5]);
        c.ready = true;
        c.inv[0] = held;
        srv.clients.insert(1, c);
        rx
    }

    fn meta(srv: &Server, p: [i32; 3]) -> u8 {
        srv.world.get(p).map(|x| x.1).unwrap_or(0)
    }

    #[test]
    fn growth_depends_on_water_and_rows() {
        let (mut srv, c) = lab();
        let soil = [c[0], c[1] - 1, c[2]];
        // Terre labouree seche, isolee : le pire cas.
        srv.world.set(soil, FARMLAND, 0);
        srv.world.set(c, WHEAT, 0);
        let dry = srv.growth_chance(c, WHEAT);
        assert!((dry - 1.0 / 13.0).abs() < 1e-4, "seche et seule (2 points) : 1 chance sur 13, obtenu {dry}");
        // Champ entier, irrigue : le meilleur cas sans voisins de la meme culture.
        for dz in -1..=1 {
            for dx in -1..=1 {
                srv.world.set([soil[0] + dx, soil[1], soil[2] + dz], FARMLAND, 0);
            }
        }
        srv.world.set([soil[0] + 3, soil[1], soil[2]], WATER, 0);
        let best = srv.growth_chance(c, WHEAT);
        assert!((best - 1.0 / 3.0).abs() < 1e-4, "irrigue et entoure : 1 chance sur 3, obtenu {best}");
        // La meme culture en diagonale gene : la vitesse est divisee.
        srv.world.set([c[0] + 1, c[1], c[2] + 1], WHEAT, 0);
        let crowded = srv.growth_chance(c, WHEAT);
        assert!(crowded < best, "serre : {crowded}");
        // En rangs alternes avec une autre culture, pas de gene.
        srv.world.set([c[0] + 1, c[1], c[2] + 1], AIR, 0);
        srv.world.set([c[0] + 1, c[1], c[2]], SOY, 0);
        assert!((srv.growth_chance(c, WHEAT) - best).abs() < 1e-4);

        // A force de ticks, le riz murit ; il ne depasse jamais le stade 3.
        for _ in 0..400 {
            srv.random_tick(c);
        }
        assert_eq!(meta(&srv, c), 3);
    }

    #[test]
    fn stem_bamboo_and_tea() {
        let (mut srv, c) = lab();
        // Tige de kabocha : mure, elle pose une courge a cote et s'arrete la.
        let soil = [c[0], c[1] - 1, c[2]];
        srv.world.set(soil, FARMLAND, 0);
        srv.world.set([soil[0] + 2, soil[1], soil[2]], WATER, 0);
        srv.world.set(c, STEM, 0);
        for _ in 0..2000 {
            srv.random_tick(c);
        }
        assert_eq!(meta(&srv, c), 3);
        let fruits = |srv: &Server| shared::signal::DIRS4.iter().filter(|d| srv.world.get(add(c, **d)).is_some_and(|x| x.0 == KABOCHA)).count();
        assert_eq!(fruits(&srv), 1, "une seule courge par tige");

        // Bambou : il monte bloc par bloc, jusqu'a douze.
        let b0 = [c[0] - 4, c[1], c[2] - 4];
        srv.world.set(b0, BAMBOO, 0);
        for _ in 0..600 {
            for dy in 0..14 {
                srv.random_tick([b0[0], b0[1] + dy, b0[2]]);
            }
        }
        let height = (0..14).take_while(|dy| srv.world.get([b0[0], b0[1] + dy, b0[2]]).is_some_and(|x| x.0 == BAMBOO)).count();
        assert_eq!(height, 12);

        // Theier : il se cueille au clic droit et repart du stade 1.
        let tea = [c[0] + 4, c[1], c[2] + 4];
        srv.world.set(tea, TEA, 0);
        for _ in 0..200 {
            srv.random_tick(tea);
        }
        assert_eq!(meta(&srv, tea), 3);
        let _rx = player(&mut srv, [tea[0] + 1, tea[1], tea[2]], None);
        assert!(srv.farm_use(1, tea, tea, TEA, 3));
        assert_eq!(meta(&srv, tea), 1);
        assert!(srv.drops.iter().any(|d| d.stack.id == items::TEA_LEAF));
    }

    #[test]
    fn fertilizer_and_compost() {
        let (mut srv, c) = lab();
        srv.world.set([c[0], c[1] - 1, c[2]], FARMLAND, 0);
        srv.world.set(c, SOY, 0);
        let _rx = player(&mut srv, [c[0] + 1, c[1], c[2]], Some(Stack::new(items::FERTILIZER, 4)));
        for _ in 0..3 {
            let m = meta(&srv, c);
            srv.farm_use(1, c, c, SOY, m);
        }
        assert_eq!(meta(&srv, c), 3, "trois doses suffisent toujours");
        assert!(!srv.farm_use(1, c, c, SOY, 3), "une plante mure n'en veut plus");
        assert_eq!(srv.clients[&1].inv[0].map(|s| s.n), Some(2), "au plus trois doses, souvent deux");
        let _ = srv.clients.get_mut(&1).map(|cl| cl.inv[0] = Some(Stack::new(items::BENTO, 1)));

        // Compost : un bento (100 %) fait toujours monter d'un cran ; au septieme, l'engrais est pret.
        let bin = [c[0] - 2, c[1], c[2]];
        srv.world.set(bin, COMPOSTER, 0);
        for step in 1..=7u8 {
            let _ = srv.clients.get_mut(&1).map(|cl| cl.inv[0] = Some(Stack::new(items::BENTO, 1)));
            let m = meta(&srv, bin);
            assert!(srv.farm_use(1, bin, bin, COMPOSTER, m));
            assert_eq!(meta(&srv, bin), if step == 7 { 8 } else { step });
        }
        let _ = srv.clients.get_mut(&1).map(|cl| cl.inv[0] = Some(Stack::new(STONE as u16, 1)));
        assert!(srv.farm_use(1, bin, bin, COMPOSTER, 8));
        assert_eq!(meta(&srv, bin), 0);
        assert!(srv.drops.iter().any(|d| d.stack.id == items::FERTILIZER));
        assert!(!srv.farm_use(1, bin, bin, COMPOSTER, 0), "la pierre ne se composte pas");
    }

    #[test]
    fn meals_saturation_and_effects() {
        let (mut srv, c) = lab();
        let rx = player(&mut srv, c, Some(Stack::new(items::COOKED_MEAT, 2)));
        {
            let cl = srv.clients.get_mut(&1).unwrap();
            cl.hunger = 6.0;
            cl.saturation = 0.0;
        }
        srv.eat(1);
        let cl = &srv.clients[&1];
        assert_eq!((cl.hunger, cl.saturation), (14.0, 12.8));
        assert_eq!(cl.inv[0].map(|s| s.n), Some(1));

        // La saturation part avant la faim.
        {
            let cl = srv.clients.get_mut(&1).unwrap();
            cl.exhaustion = 8.5;
        }
        srv.tick_survival();
        let cl = &srv.clients[&1];
        assert_eq!(cl.hunger, 14.0);
        assert!((cl.saturation - 10.8).abs() < 0.01, "saturation {}", cl.saturation);

        // Le fugu empoisonne ; le the vert guerit et rend son bol.
        {
            let cl = srv.clients.get_mut(&1).unwrap();
            cl.inv[0] = Some(Stack::new(items::FUGU, 1));
            cl.next_eat = Instant::now();
            cl.hunger = 10.0;
            cl.hp = 20.0;
        }
        srv.eat(1);
        assert!(srv.clients[&1].poison_t > 0.0);
        for _ in 0..100 {
            srv.tick_survival();
        }
        assert!(srv.clients[&1].hp <= 17.0, "le poison ronge la vie : {}", srv.clients[&1].hp);
        {
            let cl = srv.clients.get_mut(&1).unwrap();
            cl.inv[0] = Some(Stack::new(items::GREEN_TEA, 1));
            cl.next_eat = Instant::now();
        }
        srv.eat(1);
        let cl = &srv.clients[&1];
        assert_eq!(cl.poison_t, 0.0);
        assert_eq!(cl.inv[0].map(|s| s.id), Some(items::BOWL));
        drop(rx);
    }

    #[test]
    fn grill_is_fast_and_hearth_needs_no_fuel() {
        let (mut srv, c) = lab();
        // Fourneau et gril cote a cote, meme contenu : le gril finit deux fois plus vite.
        let (furnace, grill) = ([c[0], c[1], c[2]], [c[0] + 2, c[1], c[2]]);
        srv.world.set(furnace, FURNACE, 0);
        srv.world.set(grill, SMOKER, 0);
        for p in [furnace, grill] {
            let f = srv.world.furnaces.entry(p).or_default();
            f.input = Some(Stack::new(items::RAW_FISH, 1));
            f.fuel = Some(Stack::new(items::COAL, 1));
        }
        let half = (craft::COOK_TIME / 2.0 / DT) as usize + 4;
        for _ in 0..half {
            srv.tick_furnaces();
        }
        assert_eq!(srv.world.furnaces[&grill].output.map(|s| s.id), Some(items::COOKED_FISH));
        assert!(srv.world.furnaces[&furnace].output.is_none());
        assert_eq!(meta(&srv, grill) & 1, 1);
        // Le gril refuse ce qui n'est pas de la cuisine.
        srv.world.furnaces.get_mut(&grill).unwrap().input = Some(Stack::new(IRON_ORE as u16, 1));
        for _ in 0..400 {
            srv.tick_furnaces();
        }
        assert_eq!(srv.world.furnaces[&grill].input.map(|s| s.id), Some(IRON_ORE as u16));

        // Foyer irori : quatre aliments au plus, cuits en trente secondes, sans rien bruler.
        let hearth = [c[0] - 3, c[1], c[2]];
        srv.world.set(hearth, IRORI, 0);
        let _rx = player(&mut srv, [hearth[0] + 1, hearth[1], hearth[2]], Some(Stack::new(items::RAW_BIRD, 6)));
        for _ in 0..6 {
            assert!(srv.farm_use(1, hearth, hearth, IRORI, 0));
        }
        assert_eq!(srv.clients[&1].inv[0].map(|s| s.n), Some(2), "quatre places seulement");
        assert_eq!(meta(&srv, hearth), 1);
        for _ in 0..(IRORI_TIME / DT) as usize + 2 {
            srv.tick_irori();
        }
        assert_eq!(srv.drops.iter().filter(|d| d.stack.id == items::YAKITORI).count(), 4);
        assert_eq!(meta(&srv, hearth), 0);
    }

    #[test]
    fn fishing_loot_is_mostly_fish() {
        let mut rng = Rng::new(3);
        let mut fish = 0;
        for _ in 0..2000 {
            let st = fishing_loot(&mut rng);
            fish += matches!(st.id, items::RAW_FISH | items::FUGU) as u32;
        }
        assert!((1600..1800).contains(&fish), "environ 85 % de poisson : {fish} sur 2000");
    }
}
