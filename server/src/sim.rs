//! Simulation du monde : fluides, chute du sable, pousse des cultures et des arbres.
use super::*;
use shared::blocks::*;

const DIRS4: [[i32; 3]; 4] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
const DIRS6: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0], [0, -1, 0]];
/// Mises a jour de blocs traitees par cycle, au maximum.
const MAX_UPDATES: usize = 4000;

fn add(p: [i32; 3], d: [i32; 3]) -> [i32; 3] {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

/// Blocs qu'un fluide ou du sable peut remplacer.
fn flowable(b: B) -> bool {
    matches!(b, AIR | TALLGRASS | FLOWER)
}

impl Server {
    pub(crate) fn is_day(&self) -> bool {
        self.dim == 0 && (self.time * std::f64::consts::TAU).sin() * 3.0 + 0.5 > 0.3
    }

    /// Amorce un explosif : il brille, puis saute.
    pub(crate) fn prime(&mut self, p: [i32; 3], delay: f64) {
        if self.world.get(p) == Some((EXPLOSIVE, 0)) {
            self.world.set(p, EXPLOSIVE, 1);
            self.world.primed.push((p, delay));
        }
    }

    pub(crate) fn tick_primed(&mut self) {
        if self.world.primed.is_empty() {
            return;
        }
        let mut boom = Vec::new();
        self.world.primed.retain_mut(|(p, t)| {
            *t -= DT;
            if *t <= 0.0 {
                boom.push(*p);
            }
            *t > 0.0
        });
        for p in boom {
            if self.world.get(p).is_some_and(|(b, _)| b == EXPLOSIVE) {
                self.world.set(p, AIR, 0);
                self.explode(Vec3::new(p[0] as f64 + 0.5, p[1] as f64 + 0.5, p[2] as f64 + 0.5), 4.0);
            }
        }
    }

    /// Traite les blocs dont un voisin a change (4 fois par seconde).
    pub(crate) fn tick_updates(&mut self) {
        self.cycle += 1;
        let mut list: Vec<[i32; 3]> = std::mem::take(&mut self.world.updates).into_iter().collect();
        if list.len() > MAX_UPDATES {
            self.world.updates.extend(list.split_off(MAX_UPDATES));
        }
        for p in list {
            self.update_block(p);
        }
    }

    fn update_block(&mut self, p: [i32; 3]) {
        let Some((b, m)) = self.world.get(p) else { return };
        let below = add(p, [0, -1, 0]);
        let under = self.world.get(below).map(|x| x.0);
        match b {
            SAND | GRAVEL => {
                if under.is_some_and(|u| flowable(u) || u == WATER || u == LAVA) {
                    self.world.set(p, AIR, 0);
                    self.world.set(below, b, 0);
                }
            }
            WATER | LAVA => {
                // La lave coule quatre fois moins vite.
                if b == LAVA && self.cycle % 4 != 0 {
                    self.world.updates.insert(p);
                } else {
                    self.fluid(p, b, m);
                }
            }
            // Voile de torii : il lui faut son cadre.
            ABYSS_PORTAL if m & 2 != 0 => {
                if !self.veil_holds(p, m) {
                    self.world.set(p, AIR, 0);
                }
            }
            // Tete de poussoir sans son corps, ou corps deploye sans sa tete.
            PISTON_HEAD => {
                let body = add(p, shared::signal::neg(shared::signal::facing6(m)));
                let ok = self.world.get(body).is_none_or(|(bb, bm)| matches!(bb, PISTON | STICKY_PISTON) && bm & 8 != 0 && bm & 7 == m & 7);
                if !ok {
                    self.world.set(p, AIR, 0);
                }
            }
            PISTON | STICKY_PISTON if m & 8 != 0 => {
                let head = add(p, shared::signal::facing6(m));
                if self.world.get(head).is_some_and(|(hb, _)| hb != PISTON_HEAD) {
                    self.world.set(p, b, m & !8);
                }
            }
            _ if items::needs_support(b) => {
                // Le support est dessous, ou sur le cote pour ce qui s'accroche aux murs.
                let support = self.world.get(add(p, items::support_dir(b, m))).map(|x| x.0);
                if support.is_some_and(|u| !items::support_ok(b, u)) {
                    self.break_block(p, true);
                }
            }
            _ => {}
        }
    }

    /// Metadonnee d'un fluide : 0 = source, 1..=7 = distance a la source, 8 = chute.
    fn fluid(&mut self, p: [i32; 3], b: B, m: u8) {
        let max = if b == WATER { 7 } else { 3 };
        let get = |w: &World, q: [i32; 3]| w.get(q).unwrap_or((BEDROCK, 0));

        // Eau contre lave : obsidienne sur une source, pierre taillee sinon.
        if b == LAVA {
            if DIRS6.iter().any(|d| get(&self.world, add(p, *d)).0 == WATER) {
                self.world.set(p, if m == 0 { OBSIDIAN } else { COBBLE }, 0);
                return;
            }
        } else {
            for d in DIRS6 {
                let q = add(p, d);
                let (nb, nm) = get(&self.world, q);
                if nb == LAVA {
                    self.world.set(q, if nm == 0 { OBSIDIAN } else { COBBLE }, 0);
                }
            }
        }

        let below = add(p, [0, -1, 0]);
        let (under, under_m) = get(&self.world, below);
        let mut lvl = m;
        if m != 0 {
            // Un bloc qui coule doit toujours etre alimente, sinon il disparait.
            let sides: Vec<u8> = DIRS4
                .iter()
                .map(|d| get(&self.world, add(p, *d)))
                .filter(|(nb, _)| *nb == b)
                .map(|(_, nm)| if nm >= 8 { 0 } else { nm })
                .collect();
            let floor_ok = under == b && under_m == 0 || blocks::solid(under, 0);
            if b == WATER && floor_ok && sides.iter().filter(|&&s| s == 0).count() >= 2 {
                // Deux sources voisines en creent une nouvelle.
                lvl = 0;
            } else if get(&self.world, add(p, [0, 1, 0])).0 == b {
                lvl = 8;
            } else {
                match sides.iter().min() {
                    Some(&best) if best < max => lvl = best + 1,
                    _ => {
                        self.world.set(p, AIR, 0);
                        return;
                    }
                }
            }
            if lvl != m {
                self.world.set(p, b, lvl);
            }
        }

        // Vers le bas d'abord ; sur le cote seulement quand le sol est plein.
        if flowable(under) {
            self.world.set(below, b, 8);
            return;
        }
        if under == b {
            return;
        }
        let next = if lvl >= 8 { 1 } else { lvl + 1 };
        if next > max {
            return;
        }
        for d in DIRS4 {
            let q = add(p, d);
            let (nb, nm) = get(&self.world, q);
            if flowable(nb) || (nb == b && nm != 0 && nm < 8 && nm > next) {
                self.world.set(q, b, next);
            }
        }
    }

    /// Quelques blocs tires au hasard dans chaque chunk proche d'un joueur.
    pub(crate) fn random_ticks(&mut self) {
        let mut keys: HashSet<(i32, i32)> = HashSet::new();
        for c in self.clients.values().filter(|c| c.ready && c.dim == self.dim) {
            let (cx, cz) = c.chunk();
            for dz in -5..=5 {
                for dx in -5..=5 {
                    keys.insert((cx + dx, cz + dz));
                }
            }
        }
        for key in keys {
            if !self.world.chunks.contains_key(&key) {
                continue;
            }
            // Trois fois plus haut qu'avant, donc trois fois plus de tirages pour le meme rythme de pousse.
            for _ in 0..48 {
                let p = [
                    key.0 * 16 + self.rng.range(0, 16),
                    self.rng.range(1, H as i32 - 1),
                    key.1 * 16 + self.rng.range(0, 16),
                ];
                self.random_tick(p);
            }
        }
    }

    fn water_near(&self, p: [i32; 3]) -> bool {
        (-4..=4).any(|dz| (-4..=4).any(|dx| self.world.get(add(p, [dx, 0, dz])).is_some_and(|(b, _)| b == WATER)))
    }

    /// Une culture pousse a ciel ouvert, ou pres d'une lumiere.
    fn crop_light(&self, p: [i32; 3]) -> bool {
        self.sky_above(Vec3::new(p[0] as f64, p[1] as f64, p[2] as f64)) || self.lit(p)
    }

    /// Chance de gagner un stade a chaque tick aleatoire. La terre sous le plant compte le plus
    /// (4 points irriguee, 2 seche), les huit voisines un peu (0,75 ou 0,25) ; la meme culture en
    /// diagonale, ou sur les deux axes a la fois, divise le tout par deux : planter en rangs alternes.
    pub(crate) fn growth_chance(&self, p: [i32; 3], crop: B) -> f64 {
        let soil = add(p, [0, -1, 0]);
        let mut points = 1.0f64;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let q = add(soil, [dx, 0, dz]);
                if self.world.get(q).is_some_and(|x| x.0 == FARMLAND) {
                    let wet = self.water_near(q);
                    points += match (dx == 0 && dz == 0, wet) {
                        (true, true) => 3.0,
                        (true, false) => 1.0,
                        (false, true) => 0.75,
                        (false, false) => 0.25,
                    };
                }
            }
        }
        let same = |dx: i32, dz: i32| self.world.get(add(p, [dx, 0, dz])).is_some_and(|x| x.0 == crop);
        let diagonal = same(1, 1) || same(1, -1) || same(-1, 1) || same(-1, -1);
        if diagonal || ((same(1, 0) || same(-1, 0)) && (same(0, 1) || same(0, -1))) {
            points /= 2.0;
        }
        1.0 / ((25.0 / points).floor() + 1.0)
    }

    pub(crate) fn random_tick(&mut self, p: [i32; 3]) {
        let Some((b, m)) = self.world.get(p) else { return };
        if !matches!(b, WHEAT | CARROT | SOY | STEM | TEA | BAMBOO | SAPLING | GRASS | FARMLAND) {
            return;
        }
        let above = self.world.get(add(p, [0, 1, 0])).map(|x| x.0).unwrap_or(AIR);
        match b {
            WHEAT | CARROT | SOY if m < 3 => {
                if self.crop_light(p) && self.rng.chance(self.growth_chance(p, b)) {
                    self.world.set(p, b, m + 1);
                }
            }
            STEM => {
                if !self.crop_light(p) || !self.rng.chance(self.growth_chance(p, b)) {
                    return;
                }
                if m < 3 {
                    self.world.set(p, b, m + 1);
                    return;
                }
                // Tige mure : une courge sur une case voisine libre, une seule a la fois.
                if DIRS4.iter().any(|d| self.world.get(add(p, *d)).is_some_and(|x| x.0 == KABOCHA)) {
                    return;
                }
                let q = add(p, DIRS4[self.rng.range(0, 4) as usize]);
                let free = self.world.get(q).is_some_and(|x| x.0 == AIR);
                let ground = self.world.get(add(q, [0, -1, 0])).is_some_and(|x| matches!(x.0, GRASS | DIRT | FARMLAND));
                if free && ground {
                    self.world.set(q, KABOCHA, 0);
                }
            }
            // Le theier refait des pousses tout seul, une fois sur cinq.
            TEA if m < 3 => {
                if self.rng.chance(0.2) {
                    self.world.set(p, b, m + 1);
                }
            }
            BAMBOO => {
                if above == AIR && self.rng.chance(1.0 / 3.0) {
                    let mut height = 1;
                    while height < 12 && self.world.get(add(p, [0, -height, 0])).is_some_and(|x| x.0 == BAMBOO) {
                        height += 1;
                    }
                    if height < 12 {
                        self.world.set(add(p, [0, 1, 0]), BAMBOO, 0);
                    }
                }
            }
            SAPLING => {
                if self.rng.chance(0.15) {
                    self.grow_tree(p);
                }
            }
            GRASS => {
                if opaque(above) || above == WATER {
                    self.world.set(p, DIRT, 0);
                } else {
                    // L'herbe gagne un bloc de terre voisin a l'air libre.
                    let q = add(p, [self.rng.range(-1, 2), self.rng.range(-1, 2), self.rng.range(-1, 2)]);
                    let q_above = self.world.get(add(q, [0, 1, 0])).map(|x| x.0).unwrap_or(STONE);
                    if self.world.get(q).is_some_and(|(qb, _)| qb == DIRT) && !opaque(q_above) && q_above != WATER {
                        self.world.set(q, GRASS, 0);
                    }
                }
            }
            FARMLAND => {
                if !matches!(above, WHEAT | CARROT | SOY | STEM) && !self.water_near(p) && self.rng.chance(0.3) {
                    self.world.set(p, DIRT, 0);
                }
            }
            _ => {}
        }
    }

    /// Transforme une pousse en arbre s'il y a la place.
    pub(crate) fn grow_tree(&mut self, p: [i32; 3]) {
        let trunk = 4 + self.rng.range(0, 3);
        let top = p[1] + trunk;
        if top + 2 >= H as i32 {
            return;
        }
        let free = (1..=trunk).all(|dy| self.world.get(add(p, [0, dy, 0])).is_some_and(|(b, _)| matches!(b, AIR | LEAVES)));
        if !free {
            return;
        }
        for dy in -2..=1 {
            let r: i32 = if dy >= 0 { 1 } else { 2 };
            for dz in -r..=r {
                for dx in -r..=r {
                    let corner = dx.abs() == r && dz.abs() == r;
                    if corner && (dy == 1 || self.rng.chance(0.5)) {
                        continue;
                    }
                    let q = [p[0] + dx, top + dy, p[2] + dz];
                    if self.world.get(q).is_some_and(|(b, _)| b == AIR) {
                        self.world.set(q, LEAVES, 0);
                    }
                }
            }
        }
        for y in p[1]..top {
            self.world.set([p[0], y, p[2]], LOG, 0);
        }
    }
}
