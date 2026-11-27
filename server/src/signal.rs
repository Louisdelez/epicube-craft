//! Simulation des circuits de reiseki.
//!
//! Le temps des circuits avance par « battements » de deux ticks (0,1 s). A chaque battement :
//! 1. les dalles sentent ce qui marche dessus, les daruma ce qui a change devant eux ;
//! 2. les changements programmes arrivent a echeance (lanterne 1 battement, relais 1 a 4, balance 1...) ;
//! 3. si quelque chose a bouge, les fils sont recalcules d'un coup, puis chaque composant lit ses
//!    entrees et programme son prochain changement.
//! Un composant n'a jamais qu'un changement en attente : c'est ce qui allonge les impulsions
//! trop courtes dans les relais et rend les horloges regulieres.
use super::*;
use crate::world::Pending;
use shared::blocks::*;
use shared::signal::{self as sg, add, neg, Push, DIRS4, DIRS6};

const K_TORCH: u8 = 0;
const K_REPEATER: u8 = 1;
const K_COMPARATOR: u8 = 2;
const K_OBS_ON: u8 = 3;
const K_OBS_OFF: u8 = 4;
const K_LAMP_OFF: u8 = 5;
const K_BUTTON_OFF: u8 = 6;
const K_PISTON: u8 = 7;
const K_EJECT: u8 = 8;
const K_TARGET_OFF: u8 = 9;
/// Une tremie deplace un objet tous les 4 battements.
const HOPPER_BEATS: u64 = 4;

/// Duree d'appui d'un sceau, en battements.
pub(crate) const BUTTON_BEATS: u64 = 10;
/// Une dalle se relache ce nombre de battements apres le dernier passage.
const PLATE_BEATS: u64 = 10;
/// Blocs qu'un poussoir peut deplacer d'un coup.
const PUSH_LIMIT: usize = 12;
/// Une lanterne forcee de s'eteindre 8 fois en 30 battements grille, puis se repose.
const BURN_COUNT: usize = 8;
const BURN_WINDOW: u64 = 30;
const BURN_REST: u64 = 80;

fn cell_of(p: Vec3) -> [i32; 3] {
    [p.x.floor() as i32, (p.y + 0.05).floor() as i32, p.z.floor() as i32]
}

impl Server {
    /// Un battement de circuit pour la dimension active.
    pub(crate) fn tick_signal(&mut self) {
        self.world.rt += 1;
        let rt = self.world.rt;
        self.press_plates(rt);
        self.watch(rt);
        if self.world.pending.iter().any(|e| e.due <= rt) {
            let mut due = Vec::new();
            self.world.pending.retain(|e| {
                if e.due <= rt {
                    due.push(*e);
                }
                e.due > rt
            });
            for e in due {
                self.apply(e, rt);
            }
        }
        if !self.world.burnt.is_empty() {
            let before = self.world.burnt.len();
            self.world.burnt.retain(|_, until| *until > rt);
            if self.world.burnt.len() != before {
                self.world.signal_dirty = true;
            }
        }
        if rt % HOPPER_BEATS == 0 {
            self.tick_hoppers();
        }
        if self.world.signal_dirty {
            self.world.signal_dirty = false;
            self.evaluate(rt);
        }
    }

    /// Fait sonner un tambour taiko.
    pub(crate) fn drum(&mut self, pos: [i32; 3], m: u8) {
        self.sound(pos, shared::proto::sound::DRUM, m & 31);
    }

    /// Une fleche vient de se planter dans une cible : plus elle est pres du centre, plus le signal est fort.
    pub(crate) fn hit_target(&mut self, cell: [i32; 3], at: Vec3) {
        // A Takamagahara, une fleche fait eclater un cristal de pilier, et le Ryujin en souffre.
        if self.dim == 2 && self.world.get(cell).is_some_and(|(b, _)| b == CRYSTAL) && gen::zenith_crystals(self.world.seed).contains(&cell) {
            self.world.set(cell, AIR, 0);
            self.explode(Vec3::new(cell[0] as f64 + 0.5, cell[1] as f64 + 0.5, cell[2] as f64 + 0.5), 2.0);
            if let Some(m) = self.mobs.iter_mut().find(|m| m.kind == shared::mobs::WYRM) {
                m.hp -= 10.0;
            }
            return;
        }
        if !self.world.get(cell).is_some_and(|(b, _)| b == TARGET) {
            return;
        }
        let o = at - Vec3::new(cell[0] as f64 + 0.5, cell[1] as f64 + 0.5, cell[2] as f64 + 0.5);
        // Sur la face touchee, l'ecart au centre est la plus grande des deux autres coordonnees.
        let mut a = [o.x.abs(), o.y.abs(), o.z.abs()];
        a.sort_by(|x, y| x.total_cmp(y));
        let strength = (15.0 - a[1] * 28.0).round().clamp(1.0, 15.0) as u8;
        self.world.set(cell, TARGET, strength);
        self.world.pending.retain(|e| e.pos != cell);
        let due = self.world.rt + 10;
        self.schedule(due, cell, K_TARGET_OFF, 0);
    }

    /// Les tremies poussent un objet vers le conteneur vise, puis en tirent un de celui du dessus
    /// (ou ramassent ce qui est tombe sur elles). Alimentees, elles se bloquent.
    fn tick_hoppers(&mut self) {
        let list: Vec<([i32; 3], u8)> = self
            .world
            .signal
            .iter()
            .filter_map(|p| match self.world.get(*p) {
                Some((HOPPER, m)) if m & 8 == 0 => Some((*p, m)),
                _ => None,
            })
            .collect();
        for (p, m) in list {
            let d = sg::facing6(m);
            let first = self.world.chests.entry(p).or_insert_with(|| vec![None; 27]).iter().flatten().next().copied();
            if let Some(st) = first {
                if self.insert_into(add(p, d), Stack { n: 1, ..st }, d) {
                    if let Some(src) = self.world.chests.get_mut(&p) {
                        items::remove(src, st.id, 1);
                    }
                    self.cont_dirty.insert(p);
                }
            }
            let above = add(p, [0, 1, 0]);
            let room = |slots: &[Slot], id: u16| slots.iter().any(|s| s.is_none_or(|x| x.id == id && x.n < items::max_stack(id)));
            let peek = match self.world.get(above).map(|x| x.0) {
                Some(CHEST | DROPPER | HOPPER) => self.world.chests.get(&above).and_then(|s| s.iter().flatten().next().copied()),
                Some(FURNACE | SMOKER) => self.world.furnaces.get(&above).and_then(|f| f.output),
                _ => None,
            };
            if let Some(st) = peek {
                if room(&self.world.chests[&p], st.id) {
                    if let Some(f) = self.world.furnaces.get_mut(&above) {
                        f.output = if st.n > 1 { Some(Stack { n: st.n - 1, ..st }) } else { None };
                    } else if let Some(src) = self.world.chests.get_mut(&above) {
                        items::remove(src, st.id, 1);
                    }
                    if let Some(dst) = self.world.chests.get_mut(&p) {
                        add_to(dst, Stack { n: 1, ..st });
                    }
                    self.cont_dirty.insert(above);
                    self.cont_dirty.insert(p);
                }
            } else if let Some(i) = self.drops.iter().position(|dr| dr.delay <= 0.0 && cell_of(dr.pos) == above) {
                let st = self.drops[i].stack;
                if room(&self.world.chests[&p], st.id) {
                    if let Some(dst) = self.world.chests.get_mut(&p) {
                        add_to(dst, Stack { n: 1, ..st });
                    }
                    self.cont_dirty.insert(p);
                    if st.n > 1 {
                        self.drops[i].stack.n -= 1;
                    } else {
                        let gone = self.drops.remove(i);
                        self.broadcast(ServerMsg::DropRemove { id: gone.id }, None);
                    }
                }
            }
        }
    }

    /// Range un objet dans le conteneur en `to`, s'il y en a un et qu'il a de la place.
    /// `d` : sens d'arrivee ; par le haut, un fourneau recoit a cuire, par le cote, du combustible.
    fn insert_into(&mut self, to: [i32; 3], st: Stack, d: [i32; 3]) -> bool {
        let done = match self.world.get(to).map(|x| x.0) {
            Some(CHEST | DROPPER | HOPPER) => add_to(self.world.chests.entry(to).or_insert_with(|| vec![None; 27]), st).is_none(),
            Some(FURNACE | SMOKER) => {
                let f = self.world.furnaces.entry(to).or_default();
                let slot = if d[1] < 0 { &mut f.input } else { &mut f.fuel };
                match slot {
                    None => {
                        *slot = Some(st);
                        true
                    }
                    Some(x) if x.id == st.id && x.n < items::max_stack(st.id) => {
                        x.n += 1;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        };
        if done {
            self.cont_dirty.insert(to);
        }
        done
    }

    fn schedule(&mut self, due: u64, pos: [i32; 3], kind: u8, val: u8) {
        self.world.pending.push(Pending { due, pos, kind, val });
    }

    /// Appui sur un sceau hanko.
    pub(crate) fn press_button(&mut self, pos: [i32; 3], m: u8) {
        if m & 1 == 0 {
            self.sound(pos, shared::proto::sound::BUTTON, 0);
            self.world.set(pos, BUTTON, m | 1);
            let due = self.world.rt + BUTTON_BEATS;
            self.schedule(due, pos, K_BUTTON_OFF, 0);
        }
    }

    /// Les dalles s'enfoncent sous les joueurs, les creatures et les objets ; les cordes shimenawa
    /// se tendent a leur passage ; les rails guetteurs ne sentent que les chariots.
    fn press_plates(&mut self, rt: u64) {
        let dim = self.dim;
        let mut cells: Vec<([i32; 3], bool)> = self
            .clients
            .values()
            .filter(|c| c.ready && !c.dead && c.dim == dim)
            .map(|c| (cell_of(Vec3::from(c.pos)), false))
            .collect();
        cells.extend(self.mobs.iter().map(|m| (cell_of(m.pos), m.kind == shared::mobs::CART)));
        cells.extend(self.drops.iter().map(|d| (cell_of(d.pos), false)));
        for (c, cart) in cells {
            let Some((b, m)) = self.world.get(c) else { continue };
            let bit = match b {
                PLATE | TRIPWIRE => 1,
                DETECTOR_RAIL if cart => 8,
                _ => continue,
            };
            self.world.plates.insert(c, rt);
            if m & bit == 0 {
                self.world.set(c, b, m | bit);
            }
        }
        if self.world.plates.is_empty() {
            return;
        }
        let old: Vec<[i32; 3]> = self.world.plates.iter().filter(|(_, t)| rt - **t >= PLATE_BEATS).map(|(p, _)| *p).collect();
        for p in old {
            self.world.plates.remove(&p);
            match self.world.get(p) {
                Some((b @ (PLATE | TRIPWIRE), m)) => self.world.set(p, b, m & !1),
                Some((DETECTOR_RAIL, m)) => self.world.set(p, DETECTOR_RAIL, m & !8),
                _ => false,
            };
        }
    }

    /// Les daruma guetteurs remarquent tout bloc qui change devant eux.
    fn watch(&mut self, rt: u64) {
        if self.world.observed.is_empty() {
            return;
        }
        let changed: HashSet<[i32; 3]> = std::mem::take(&mut self.world.observed).into_iter().collect();
        for c in changed {
            for e in DIRS6 {
                let o = add(c, e);
                if let Some((OBSERVER, m)) = self.world.get(o) {
                    if sg::facing6(m) == neg(e) && m & 8 == 0 && !self.world.pending.iter().any(|x| x.pos == o) {
                        self.schedule(rt + 1, o, K_OBS_ON, 0);
                    }
                }
            }
        }
    }

    /// Entrees d'une balance onmyo : (arriere, cote le plus fort).
    fn comparator_inputs(&self, p: [i32; 3], m: u8) -> (u8, u8) {
        let get = |q: [i32; 3]| self.world.get(q).unwrap_or((AIR, 0));
        let f = sg::facing4(m);
        let back = add(p, neg(f));
        let rear = match get(back).0 {
            CHEST | DROPPER | HOPPER => self.world.chests.get(&back).map(|s| sg::fullness(s)).unwrap_or(0),
            FURNACE | SMOKER => self.world.furnaces.get(&back).map(|x| sg::fullness(&[x.input, x.fuel, x.output])).unwrap_or(0),
            // Bac a compost : son niveau, de 0 a 8.
            COMPOSTER => get(back).1.min(8),
            _ => sg::input(&get, p, neg(f)),
        };
        let mut side = 0u8;
        for e in DIRS4 {
            if e == f || e == neg(f) {
                continue;
            }
            let q = add(p, e);
            let (b, qm) = get(q);
            let s = match b {
                WIRE => qm & 15,
                REPEATER | COMPARATOR | VOLT_BLOCK => sg::emit_to(&get, q, neg(e)),
                _ => 0,
            };
            side = side.max(s);
        }
        (rear, side)
    }

    /// Un poussoir recoit-il du ki ? Sa face avant ne compte pas.
    fn piston_powered(&self, p: [i32; 3], m: u8) -> bool {
        let get = |q: [i32; 3]| self.world.get(q).unwrap_or((AIR, 0));
        let f = sg::facing6(m);
        DIRS6.iter().any(|e| *e != f && sg::input(&get, p, *e) > 0)
    }

    /// Recalcule les fils, puis fait lire leurs entrees a tous les composants.
    fn evaluate(&mut self, rt: u64) {
        let nodes: Vec<([i32; 3], B, u8)> = self.world.signal.iter().filter_map(|p| self.world.get(*p).map(|(b, m)| (*p, b, m))).collect();

        // 1. Fils : la force des sources, puis un de moins a chaque fil traverse.
        let mut sets: Vec<([i32; 3], B, u8)> = Vec::new();
        {
            let get = |q: [i32; 3]| self.world.get(q).unwrap_or((AIR, 0));
            let mut power: HashMap<[i32; 3], u8> = HashMap::new();
            let mut queue: Vec<[i32; 3]> = Vec::new();
            for (p, b, _) in &nodes {
                if *b == WIRE {
                    let s = sg::wire_source(&get, *p);
                    if s > 0 {
                        power.insert(*p, s);
                        queue.push(*p);
                    }
                }
            }
            let mut head = 0;
            while head < queue.len() {
                let p = queue[head];
                head += 1;
                let pw = power[&p];
                if pw <= 1 {
                    continue;
                }
                for q in sg::wire_neighbors(&get, p) {
                    if power.get(&q).copied().unwrap_or(0) < pw - 1 {
                        power.insert(q, pw - 1);
                        queue.push(q);
                    }
                }
            }
            for (p, b, m) in &nodes {
                if *b == WIRE {
                    let want = power.get(p).copied().unwrap_or(0);
                    if *m != want {
                        sets.push((*p, WIRE, want));
                    }
                }
            }
        }
        for (p, b, m) in sets.drain(..) {
            self.world.set(p, b, m);
        }

        // 2. Composants.
        let busy: HashSet<[i32; 3]> = self.world.pending.iter().map(|e| e.pos).collect();
        let mut plan: Vec<Pending> = Vec::new();
        let mut primes: Vec<[i32; 3]> = Vec::new();
        let mut held: Vec<[i32; 3]> = Vec::new();
        let mut drums: Vec<([i32; 3], u8)> = Vec::new();
        let (mut fed, mut rails): (Vec<[i32; 3]>, Vec<([i32; 3], u8)>) = (Vec::new(), Vec::new());
        {
            let get = |q: [i32; 3]| self.world.get(q).unwrap_or((AIR, 0));
            for (p, b, m) in &nodes {
                let (p, b, m) = (*p, *b, *m);
                let free = !busy.contains(&p);
                match b {
                    SIGTORCH => {
                        // Eteinte tant que le bloc qui la porte est charge.
                        let lit = sg::input(&get, p, sg::attach_of(m)) == 0;
                        if free && lit != (m & 1 == 1) && !self.world.burnt.contains_key(&p) {
                            plan.push(Pending { due: rt + 1, pos: p, kind: K_TORCH, val: lit as u8 });
                        }
                    }
                    REPEATER => {
                        let f = sg::facing4(m);
                        let on = sg::input(&get, p, neg(f)) > 0;
                        // Verrouille par un relais ou une balance allumes qui le visent de cote.
                        let locked = DIRS4.iter().any(|e| {
                            let q = add(p, *e);
                            *e != f && *e != neg(f) && matches!(get(q).0, REPEATER | COMPARATOR) && sg::emit_to(&get, q, neg(*e)) > 0
                        });
                        let nm = if locked { m | 32 } else { m & !32 };
                        if nm != m {
                            sets.push((p, b, nm));
                        }
                        if free && !locked && on != (m & 16 != 0) {
                            let delay = 1 + ((m >> 2) & 3) as u64;
                            plan.push(Pending { due: rt + delay, pos: p, kind: K_REPEATER, val: on as u8 });
                        }
                    }
                    COMPARATOR => {
                        let (rear, side) = self.comparator_inputs(p, m);
                        let out = if m & 4 != 0 {
                            rear.saturating_sub(side)
                        } else if rear >= side {
                            rear
                        } else {
                            0
                        };
                        if free && out != m >> 4 {
                            plan.push(Pending { due: rt + 1, pos: p, kind: K_COMPARATOR, val: out });
                        }
                    }
                    LAMP => {
                        let on = sg::powered(&get, p) > 0;
                        if on && m & 1 == 0 {
                            sets.push((p, b, m | 1));
                        } else if !on && m & 1 == 1 && free {
                            plan.push(Pending { due: rt + 2, pos: p, kind: K_LAMP_OFF, val: 0 });
                        }
                    }
                    DOOR => {
                        // Bit 1 : memoire du dernier etat du signal, pour laisser la main au joueur entre-temps.
                        let on = sg::powered(&get, p) > 0;
                        if on != (m & 2 != 0) {
                            sets.push((p, b, if on { 3 } else { 0 }));
                        }
                    }
                    EXPLOSIVE if m == 0 && sg::powered(&get, p) > 0 => primes.push(p),
                    PISTON | STICKY_PISTON => {
                        let want = self.piston_powered(p, m);
                        if free && want != (m & 8 != 0) {
                            plan.push(Pending { due: rt + 1, pos: p, kind: K_PISTON, val: want as u8 });
                        }
                    }
                    DROPPER => {
                        // Declenche sur le front montant seulement.
                        let on = sg::powered(&get, p) > 0;
                        if on && m & 8 == 0 {
                            sets.push((p, b, m | 8));
                            if free {
                                plan.push(Pending { due: rt + 2, pos: p, kind: K_EJECT, val: 0 });
                            }
                        } else if !on && m & 8 != 0 {
                            sets.push((p, b, m & !8));
                        }
                    }
                    NOTE => {
                        // Le tambour sonne quand le signal arrive (bit 5 : deja frappe).
                        let on = sg::powered(&get, p) > 0;
                        if on != (m & 32 != 0) {
                            sets.push((p, b, m ^ 32));
                            if on {
                                drums.push((p, m));
                            }
                        }
                    }
                    HOPPER => {
                        let on = sg::powered(&get, p) > 0;
                        if on != (m & 8 != 0) {
                            sets.push((p, b, m ^ 8));
                        }
                    }
                    HOOK => {
                        // Actif si la corde tendue jusqu'au piquet d'en face est touchee quelque part.
                        let d = neg(sg::attach_of(m));
                        let (mut q, mut touched, mut closed) = (add(p, d), false, false);
                        for _ in 0..40 {
                            match get(q) {
                                (TRIPWIRE, qm) => touched |= qm & 1 == 1,
                                (HOOK, qm) => {
                                    closed = sg::attach_of(qm) == d;
                                    break;
                                }
                                _ => break,
                            }
                            q = add(q, d);
                        }
                        let on = closed && touched;
                        if on != (m & 1 == 1) {
                            sets.push((p, b, m ^ 1));
                        }
                    }
                    POWERED_RAIL => {
                        if sg::powered(&get, p) > 0 {
                            fed.push(p);
                        }
                        rails.push((p, m));
                    }
                    TARGET if m & 15 != 0 && free => plan.push(Pending { due: rt + 10, pos: p, kind: K_TARGET_OFF, val: 0 }),
                    // Etats restes actifs dans une sauvegarde : on leur redonne une echeance.
                    PLATE | TRIPWIRE if m & 1 == 1 => held.push(p),
                    DETECTOR_RAIL if m & 8 != 0 => held.push(p),
                    BUTTON if m & 1 == 1 && free => plan.push(Pending { due: rt + BUTTON_BEATS, pos: p, kind: K_BUTTON_OFF, val: 0 }),
                    OBSERVER if m & 8 != 0 && free => plan.push(Pending { due: rt + 1, pos: p, kind: K_OBS_OFF, val: 0 }),
                    _ => {}
                }
            }
        }
        // Rails de reiseki : le ki passe de rail en rail, jusqu'a 8 cases de la source.
        if !rails.is_empty() {
            let all: HashSet<[i32; 3]> = rails.iter().map(|r| r.0).collect();
            let mut reach: HashMap<[i32; 3], u8> = fed.iter().map(|p| (*p, 8)).collect();
            let mut head = 0;
            while head < fed.len() {
                let p = fed[head];
                head += 1;
                let left = reach[&p];
                for d in DIRS4 {
                    let q = add(p, d);
                    if left > 0 && all.contains(&q) && reach.get(&q).copied().unwrap_or(0) < left - 1 {
                        reach.insert(q, left - 1);
                        fed.push(q);
                    }
                }
            }
            for (p, m) in rails {
                let on = reach.contains_key(&p);
                if on != (m & 8 != 0) {
                    sets.push((p, POWERED_RAIL, m ^ 8));
                }
            }
        }
        for (p, b, m) in sets {
            self.world.set(p, b, m);
        }
        for (p, m) in drums {
            self.drum(p, m);
        }
        for p in held {
            self.world.plates.entry(p).or_insert(rt);
        }
        self.world.pending.extend(plan);
        for p in primes {
            self.prime(p, 4.0);
        }
    }

    /// Applique un changement arrive a echeance, si le composant est toujours la.
    fn apply(&mut self, e: Pending, rt: u64) {
        let Some((b, m)) = self.world.get(e.pos) else { return };
        match (e.kind, b) {
            (K_TORCH, SIGTORCH) => {
                if e.val == 0 {
                    let log = self.world.toggles.entry(e.pos).or_default();
                    log.retain(|t| rt - *t < BURN_WINDOW);
                    log.push(rt);
                    if log.len() >= BURN_COUNT {
                        log.clear();
                        self.world.burnt.insert(e.pos, rt + BURN_REST);
                    }
                }
                self.world.set(e.pos, b, (m & !1) | e.val);
            }
            (K_REPEATER, REPEATER) => {
                self.world.set(e.pos, b, if e.val == 1 { m | 16 } else { m & !16 });
            }
            (K_COMPARATOR, COMPARATOR) => {
                self.world.set(e.pos, b, (m & 15) | (e.val << 4));
            }
            (K_OBS_ON, OBSERVER) => {
                self.world.set(e.pos, b, m | 8);
                self.schedule(rt + 1, e.pos, K_OBS_OFF, 0);
            }
            (K_OBS_OFF, OBSERVER) => {
                self.world.set(e.pos, b, m & !8);
            }
            (K_LAMP_OFF, LAMP) => {
                let get = |q: [i32; 3]| self.world.get(q).unwrap_or((AIR, 0));
                if sg::powered(&get, e.pos) == 0 {
                    self.world.set(e.pos, b, m & !1);
                }
            }
            (K_BUTTON_OFF, BUTTON) => {
                self.world.set(e.pos, b, m & !1);
            }
            (K_PISTON, PISTON | STICKY_PISTON) => self.move_piston(e.pos, b, m, e.val == 1),
            (K_EJECT, DROPPER) => self.eject(e.pos, m),
            (K_TARGET_OFF, TARGET) => {
                self.world.set(e.pos, b, 0);
            }
            _ => {}
        }
        // Un composant deplace ou casse entre-temps : on relit simplement le circuit.
        self.world.signal_dirty = true;
    }

    /// Sort ou rentre la tete d'un poussoir karakuri.
    fn move_piston(&mut self, pos: [i32; 3], b: B, m: u8, extend: bool) {
        if extend == (m & 8 != 0) || extend != self.piston_powered(pos, m) {
            return;
        }
        let d = sg::facing6(m);
        let head = add(pos, d);
        if !extend {
            self.sound(pos, shared::proto::sound::PISTON, 1);
            if self.world.get(head).is_some_and(|(hb, _)| hb == PISTON_HEAD) {
                self.world.set(head, AIR, 0);
            }
            self.world.set(pos, b, m & !8);
            if b == STICKY_PISTON {
                // La glu de riz ramene le bloc qui touchait la tete.
                let c = add(head, d);
                if let Some((cb, cm)) = self.world.get(c) {
                    if sg::push_kind(cb, cm) == Push::Move {
                        self.world.set(head, cb, cm);
                        self.world.set(c, AIR, 0);
                    }
                }
            }
            return;
        }
        // File des blocs a pousser, du plus proche au plus loin.
        let mut line: Vec<([i32; 3], B, u8)> = Vec::new();
        let mut q = head;
        loop {
            if q[1] < 1 || q[1] >= H as i32 {
                return;
            }
            let Some((qb, qm)) = self.world.get(q) else { return };
            match sg::push_kind(qb, qm) {
                Push::Empty => break,
                Push::Break => {
                    self.break_block(q, true);
                    break;
                }
                Push::Fixed => return,
                Push::Move => {
                    if line.len() == PUSH_LIMIT {
                        return;
                    }
                    line.push((q, qb, qm));
                    q = add(q, d);
                }
            }
        }
        let mut cells = vec![head];
        for (c, cb, cm) in line.iter().rev() {
            let to = add(*c, d);
            self.world.set(to, *cb, *cm);
            cells.push(to);
            if *cb == OBSERVER {
                // Un daruma deplace sursaute.
                let due = self.world.rt + 1;
                self.schedule(due, to, K_OBS_ON, 0);
            }
        }
        self.world.set(head, PISTON_HEAD, (m & 7) | if b == STICKY_PISTON { 8 } else { 0 });
        self.world.set(pos, b, m | 8);
        self.sound(pos, shared::proto::sound::PISTON, 0);
        self.shove(&cells, d);
    }

    /// Ecarte les joueurs et les creatures des cases ou un bloc vient d'arriver.
    fn shove(&mut self, cells: &[[i32; 3]], d: [i32; 3]) {
        let inside = |p: Vec3, half: f64, height: f64| {
            cells.iter().any(|c| {
                p.x + half > c[0] as f64
                    && p.x - half < c[0] as f64 + 1.0
                    && p.y + height > c[1] as f64
                    && p.y < c[1] as f64 + 1.0
                    && p.z + half > c[2] as f64
                    && p.z - half < c[2] as f64 + 1.0
            })
        };
        let dv = Vec3::new(d[0] as f64, d[1] as f64, d[2] as f64);
        let dim = self.dim;
        for c in self.clients.values_mut().filter(|c| c.ready && !c.dead && c.dim == dim) {
            if inside(Vec3::from(c.pos), 0.3, 1.8) {
                // Un peu plus qu'un bloc vers le haut, pour retomber dessus et non dedans.
                let to = Vec3::from(c.pos) + dv * if d[1] > 0 { 1.02 } else { 1.0 };
                c.pos = to.to_array();
                c.prev = c.pos;
                c.fall_peak = c.pos[1];
                c.send(ServerMsg::Teleport { pos: c.pos });
            }
        }
        for mob in self.mobs.iter_mut() {
            if inside(mob.pos, 0.3, 0.9) {
                mob.pos += dv * if d[1] > 0 { 1.02 } else { 1.0 };
            }
        }
    }

    /// Le distributeur karakuri sort un objet pris au hasard.
    fn eject(&mut self, pos: [i32; 3], m: u8) {
        let d = sg::facing6(m);
        let Some(slots) = self.world.chests.get_mut(&pos) else { return };
        let filled: Vec<usize> = (0..slots.len()).filter(|i| slots[*i].is_some()).collect();
        if filled.is_empty() {
            return;
        }
        let i = filled[self.rng.range(0, filled.len() as i32) as usize];
        let Some(st) = slots[i] else { return };
        slots[i] = if st.n > 1 { Some(Stack { n: st.n - 1, ..st }) } else { None };
        let one = Stack { n: 1, ..st };
        self.cont_dirty.insert(pos);
        self.sound(pos, shared::proto::sound::DROPPER, 0);
        self.world.signal_dirty = true;
        let to = add(pos, d);
        let dv = Vec3::new(d[0] as f64, d[1] as f64, d[2] as f64);
        let mouth = Vec3::new(pos[0] as f64 + 0.5, pos[1] as f64 + 0.5, pos[2] as f64 + 0.5) + dv * 0.75;
        match self.world.get(to).map(|x| x.0) {
            // Face a un conteneur, l'objet y entre directement.
            Some(CHEST | DROPPER | HOPPER) => {
                let dst = self.world.chests.entry(to).or_insert_with(|| vec![None; 27]);
                if let Some(left) = add_to(dst, one) {
                    if let Some(src) = self.world.chests.get_mut(&pos) {
                        add_to(src, left);
                    }
                }
                self.cont_dirty.insert(to);
            }
            _ if one.id == items::ARROW => self.spawn_proj(mouth, dv * 20.0 + Vec3::Y * 1.5, 0),
            _ => {
                let vel = dv * 3.0 + Vec3::new(self.rng.d() - 0.5, 0.6, self.rng.d() - 0.5) * 0.4;
                self.spawn_drop(mouth - Vec3::Y * 0.2, vel, one, 0.3);
            }
        }
    }

    /// Le cadran solaire suit la hauteur du soleil (une fois par seconde).
    pub(crate) fn tick_sundials(&mut self) {
        let sun = if self.dim == 0 { ((self.time * std::f64::consts::TAU).sin().max(0.0) * 15.0).round() as u8 } else { 0 };
        let list: Vec<([i32; 3], u8)> = self
            .world
            .signal
            .iter()
            .filter_map(|p| match self.world.get(*p) {
                Some((SUNDIAL, m)) => Some((*p, m)),
                _ => None,
            })
            .collect();
        for (p, m) in list {
            // Bit 4 : mode nuit, le cadran donne l'inverse.
            let s = if m & 16 != 0 { 15 - sun } else { sun };
            if m & 15 != s {
                self.world.set(p, SUNDIAL, (m & 16) | s);
            }
        }
    }
}

impl Server {
    /// Aide au developpement : EPICUBE_DEMO=circuits construit une vitrine de circuits devant le joueur.
    pub(crate) fn demo_circuits(&mut self) {
        let o = [self.spawn[0].floor() as i32 - 7, self.spawn[1].floor() as i32 - 2, self.spawn[2].floor() as i32 + 3];
        let put = |srv: &mut Server, x: i32, dy: i32, z: i32, b: B, m: u8| {
            let p = [o[0] + x, o[1] + dy, o[2] + z];
            srv.world.ensure((p[0] >> 4, p[2] >> 4));
            srv.world.set(p, b, m);
            p
        };
        for z in -1..=13 {
            for x in -1..=15 {
                put(self, x, -1, z, STONE, 0);
                for dy in 0..7 {
                    put(self, x, dy, z, AIR, 0);
                }
            }
        }
        // Levier, fil, lampe.
        put(self, 0, 0, 0, LEVER, 1);
        for x in 1..=5 {
            put(self, x, 0, 0, WIRE, 0);
        }
        put(self, 6, 0, 0, LAMP, 0);
        // Deux relais, retards 2 et 4.
        put(self, 0, 0, 2, LEVER, 1);
        put(self, 1, 0, 2, WIRE, 0);
        put(self, 2, 0, 2, REPEATER, 1 << 2);
        put(self, 3, 0, 2, WIRE, 0);
        put(self, 4, 0, 2, REPEATER, 3 << 2);
        put(self, 5, 0, 2, WIRE, 0);
        put(self, 6, 0, 2, LAMP, 0);
        // Inverseur : bloc de reiseki, fil, bloc, lanterne eteinte ; a cote, une lanterne libre et sa lampe.
        put(self, 0, 0, 4, VOLT_BLOCK, 0);
        put(self, 1, 0, 4, WIRE, 0);
        put(self, 2, 0, 4, WIRE, 0);
        put(self, 3, 0, 4, STONE, 0);
        put(self, 4, 0, 4, SIGTORCH, 1 | 2 << 1);
        put(self, 5, 0, 4, LAMP, 0);
        put(self, 8, 0, 4, STONE, 0);
        put(self, 9, 0, 4, SIGTORCH, 1 | 2 << 1);
        put(self, 10, 0, 4, LAMP, 0);
        // Horloge de daruma qui actionne un poussoir gluant.
        put(self, 1, 0, 6, OBSERVER, 0);
        put(self, 2, 0, 6, OBSERVER, 1);
        put(self, 0, 0, 6, WIRE, 0);
        put(self, 0, 0, 7, WIRE, 0);
        put(self, 1, 0, 7, REPEATER, 3 << 2);
        put(self, 2, 0, 7, STICKY_PISTON, 2);
        put(self, 2, 1, 7, PLANKS, 0);
        // Balance qui lit un coffre a moitie plein, puis le fil qui s'affaiblit.
        let chest = put(self, 0, 0, 9, CHEST, 0);
        let mut slots = vec![None; 27];
        for s in slots.iter_mut().take(13) {
            *s = Some(Stack::new(DIRT as u16, 64));
        }
        self.world.chests.insert(chest, slots);
        put(self, 1, 0, 9, COMPARATOR, 0);
        for x in 2..=10 {
            put(self, x, 0, 9, WIRE, 0);
        }
        // Dalle et porte, sceau et lampe, cadran solaire, distributeur, poussoir deploye.
        put(self, 0, 0, 11, PLATE, 0);
        put(self, 1, 0, 11, DOOR, 0);
        put(self, 3, 0, 11, BUTTON, 0);
        put(self, 4, 0, 11, LAMP, 0);
        put(self, 6, 0, 11, SUNDIAL, 0);
        put(self, 7, 0, 11, WIRE, 0);
        put(self, 8, 0, 11, LAMP, 0);
        put(self, 10, 0, 11, DROPPER, 2);
        put(self, 12, 0, 12, LEVER, 1);
        put(self, 12, 0, 11, PISTON, 0);
        put(self, 13, 0, 11, COBBLE, 0);
        // Rails : boucle avec rail de reiseki allume et rail guetteur, et un chariot dessus.
        for i in 0..5 {
            for (x, z) in [(10 + i, 0), (10 + i, 2), (10, i.min(2)), (14, i.min(2))] {
                let b = if (x, z) == (12, 0) { POWERED_RAIL } else if (x, z) == (12, 2) { DETECTOR_RAIL } else { RAIL };
                let p = put(self, x, 0, z, b, 0);
                self.fix_rails_around(p);
            }
        }
        put(self, 12, 0, -1, VOLT_BLOCK, 0);
        put(self, 12, 0, 3, LAMP, 0);
        let cart = Vec3::new(o[0] as f64 + 11.5, o[1] as f64, o[2] as f64 + 0.5);
        self.spawn_mob(shared::mobs::CART, cart, 0.0, 1.0);
        // Tremie sous un coffre, tambour taiko, cible, et corde shimenawa entre deux piquets.
        let top = put(self, 5, 1, 6, CHEST, 0);
        self.world.chests.insert(top, vec![Some(Stack::new(DIRT as u16, 8)); 3]);
        if let Some(c) = self.world.chests.get_mut(&top) {
            c.resize(27, None);
        }
        put(self, 5, 0, 6, HOPPER, 0);
        put(self, 6, 0, 6, CHEST, 0);
        put(self, 8, 0, 6, NOTE, 7);
        put(self, 9, 0, 6, BUTTON, 0);
        put(self, 11, 0, 6, TARGET, 0);
        put(self, 5, 0, 13, STONE, 0);
        put(self, 6, 0, 13, HOOK, 2 << 1);
        for x in 7..=9 {
            put(self, x, 0, 13, TRIPWIRE, 0);
        }
        put(self, 10, 0, 13, HOOK, 1 << 1);
        put(self, 11, 0, 13, STONE, 0);
        self.world.updates.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dalle de pierre de 13 x 13 en altitude ; renvoie le serveur et le centre, un bloc au-dessus du sol.
    fn lab() -> (Server, [i32; 3]) {
        let mut srv = Server::new(7, None);
        srv.world.ensure((0, 0));
        for dz in -6..=6 {
            for dx in -6..=6 {
                srv.world.set([8 + dx, 300, 8 + dz], STONE, 0);
                for dy in 1..=17 {
                    srv.world.set([8 + dx, 300 + dy, 8 + dz], AIR, 0);
                }
            }
        }
        srv.world.updates.clear();
        srv.world.observed.clear();
        (srv, [8, 301, 8])
    }

    fn at(c: [i32; 3], x: i32, z: i32) -> [i32; 3] {
        [c[0] + x, c[1], c[2] + z]
    }

    fn beats(srv: &mut Server, n: usize) {
        for _ in 0..n {
            srv.tick_signal();
        }
    }

    fn meta(srv: &Server, p: [i32; 3]) -> u8 {
        srv.world.get(p).map(|x| x.1).unwrap_or(0)
    }

    #[test]
    fn inverter_and_lamp_delay() {
        let (mut srv, c) = lab();
        let (lever, block, torch, lamp) = (at(c, -3, 0), at(c, 0, 0), at(c, 1, 0), at(c, 2, 0));
        srv.world.set(lever, LEVER, 0);
        srv.world.set(at(c, -2, 0), WIRE, 0);
        srv.world.set(at(c, -1, 0), WIRE, 0);
        srv.world.set(block, STONE, 0);
        // Lanterne accrochee au flanc +X du bloc : son support est vers -X.
        srv.world.set(torch, SIGTORCH, 1 | 2 << 1);
        srv.world.set(lamp, LAMP, 0);
        beats(&mut srv, 6);
        assert_eq!(meta(&srv, torch) & 1, 1);
        assert_eq!(meta(&srv, lamp), 1, "la lanterne allumee eclaire la lampe");

        srv.world.set(lever, LEVER, 1);
        beats(&mut srv, 1);
        assert_eq!(meta(&srv, at(c, -1, 0)), 14);
        assert_eq!(meta(&srv, torch) & 1, 1, "la lanterne met un battement a reagir");
        beats(&mut srv, 1);
        assert_eq!(meta(&srv, torch) & 1, 0);
        assert_eq!(meta(&srv, lamp), 1, "la lampe reste allumee deux battements");
        beats(&mut srv, 2);
        assert_eq!(meta(&srv, lamp), 0);
        // Un fil qui longe un bloc sans pointer vers lui ne le charge pas.
        let (side, side_torch) = (at(c, -2, 1), [c[0] - 2, c[1] + 1, c[2] + 1]);
        srv.world.set(side, STONE, 0);
        srv.world.set(side_torch, SIGTORCH, 1);
        beats(&mut srv, 6);
        assert_eq!(meta(&srv, side_torch) & 1, 1);
    }

    #[test]
    fn repeater_delay_stretch_and_lock() {
        let (mut srv, c) = lab();
        let (lever, rep, lamp) = (at(c, -3, 0), at(c, -1, 0), at(c, 0, 0));
        srv.world.set(lever, LEVER, 0);
        srv.world.set(at(c, -2, 0), WIRE, 0);
        // Relais tourne vers +X, retard maximal (4 battements).
        srv.world.set(rep, REPEATER, 3 << 2);
        srv.world.set(lamp, LAMP, 0);
        beats(&mut srv, 4);

        // Impulsion d'un seul battement : le relais l'allonge a son retard.
        srv.world.set(lever, LEVER, 1);
        beats(&mut srv, 1);
        srv.world.set(lever, LEVER, 0);
        beats(&mut srv, 3);
        assert_eq!(meta(&srv, rep) & 16, 0, "pas encore : 4 battements de retard");
        beats(&mut srv, 1);
        assert_eq!(meta(&srv, rep) & 16, 16);
        assert_eq!(meta(&srv, lamp), 1);
        beats(&mut srv, 3);
        assert_eq!(meta(&srv, rep) & 16, 16, "l'impulsion dure autant que le retard");
        beats(&mut srv, 1);
        assert_eq!(meta(&srv, rep) & 16, 0);

        // Verrou : un second relais allume vise le flanc du premier, qui ne change plus.
        let (lock, lock_lever) = (at(c, -1, 1), at(c, -1, 2));
        srv.world.set(lock, REPEATER, 3);
        srv.world.set(lock_lever, LEVER, 1);
        beats(&mut srv, 4);
        assert_eq!(meta(&srv, lock) & 16, 16);
        assert_eq!(meta(&srv, rep) & 32, 32, "verrouille");
        srv.world.set(lever, LEVER, 1);
        beats(&mut srv, 8);
        assert_eq!(meta(&srv, rep) & 16, 0, "verrouille eteint : il le reste");
        srv.world.set(lock_lever, LEVER, 0);
        beats(&mut srv, 8);
        assert_eq!(meta(&srv, rep) & 16, 16, "deverrouille, il suit de nouveau son entree");
    }

    #[test]
    fn comparator_modes_and_containers() {
        let (mut srv, c) = lab();
        let (chest, cmp) = (at(c, -1, 0), at(c, 0, 0));
        srv.world.set(chest, CHEST, 0);
        srv.world.set(cmp, COMPARATOR, 0);
        let mut slots = vec![None; 27];
        slots[0] = Some(Stack::new(DIRT as u16, 1));
        srv.world.chests.insert(chest, slots);
        beats(&mut srv, 4);
        assert_eq!(meta(&srv, cmp) >> 4, 1, "un seul objet : force 1");

        srv.world.chests.insert(chest, vec![Some(Stack::new(DIRT as u16, 64)); 27]);
        srv.world.signal_dirty = true;
        beats(&mut srv, 4);
        assert_eq!(meta(&srv, cmp) >> 4, 15, "coffre plein : force 15");

        // Entree de cote a 13 : la comparaison laisse passer 15, la soustraction donne 2.
        srv.world.set(at(c, 0, 4), VOLT_BLOCK, 0);
        for z in 1..=3 {
            srv.world.set(at(c, 0, z), WIRE, 0);
        }
        beats(&mut srv, 4);
        assert_eq!(meta(&srv, at(c, 0, 1)), 13);
        assert_eq!(meta(&srv, cmp) >> 4, 15);
        srv.world.set(cmp, COMPARATOR, meta(&srv, cmp) | 4);
        beats(&mut srv, 4);
        assert_eq!(meta(&srv, cmp) >> 4, 2);

        // Cote plus fort que l'arriere : la comparaison coupe.
        srv.world.chests.insert(chest, { let mut s = vec![None; 27]; s[0] = Some(Stack::new(DIRT as u16, 1)); s });
        srv.world.set(cmp, COMPARATOR, meta(&srv, cmp) & !4);
        beats(&mut srv, 4);
        assert_eq!(meta(&srv, cmp) >> 4, 0);
    }

    #[test]
    fn fast_torch_clock_burns_out() {
        let (mut srv, c) = lab();
        let (block, torch) = (at(c, 0, 0), at(c, 1, 0));
        srv.world.set(block, STONE, 0);
        srv.world.set(torch, SIGTORCH, 1 | 2 << 1);
        // La lanterne charge le bloc au-dessus d'elle, qui alimente un fil pose sur son propre support.
        srv.world.set([torch[0], torch[1] + 1, torch[2]], STONE, 0);
        srv.world.set([block[0], block[1] + 1, block[2]], WIRE, 0);
        let mut flips = 0;
        let mut last = 1;
        let mut burnt = false;
        for _ in 0..60 {
            beats(&mut srv, 1);
            let lit = meta(&srv, torch) & 1;
            flips += (lit != last) as u32;
            last = lit;
            burnt |= srv.world.burnt.contains_key(&torch);
        }
        assert!(flips >= 8, "l'horloge a bien battu ({flips} changements)");
        assert!(burnt, "une horloge aussi rapide grille la lanterne");
        assert!(flips < 40, "grillee, elle s'arrete un moment");
    }

    #[test]
    fn torch_latch_remembers() {
        // Bascule RS : deux lanternes, chacune eteint l'autre.
        let (mut srv, c) = lab();
        let (a_block, b_block) = (at(c, -1, 0), at(c, 1, 2));
        let (a_torch, b_torch) = (at(c, 0, 0), at(c, 0, 2));
        srv.world.set(a_block, STONE, 0);
        srv.world.set(b_block, STONE, 0);
        srv.world.set(a_torch, SIGTORCH, 1 | 2 << 1);
        // B part eteinte : deux lanternes allumees en meme temps oscilleraient jusqu'a griller.
        srv.world.set(b_torch, SIGTORCH, 1 << 1);
        // Fil de la lanterne A vers le bloc B, et de la lanterne B vers le bloc A.
        srv.world.set(at(c, 1, 0), WIRE, 0);
        srv.world.set(at(c, 1, 1), WIRE, 0);
        srv.world.set(at(c, -1, 2), WIRE, 0);
        srv.world.set(at(c, -1, 1), WIRE, 0);
        beats(&mut srv, 30);
        let (a, b) = (meta(&srv, a_torch) & 1, meta(&srv, b_torch) & 1);
        assert_ne!(a, b, "une seule des deux lanternes reste allumee");
        // Un sceau pose sur le bloc de la lanterne allumee fait basculer la memoire.
        let (lit_block, a_wins) = if a == 1 { (a_block, true) } else { (b_block, false) };
        let button = [lit_block[0], lit_block[1] + 1, lit_block[2]];
        srv.world.set(button, BUTTON, 0);
        srv.press_button(button, 0);
        beats(&mut srv, 30);
        assert_eq!(meta(&srv, button) & 1, 0, "le sceau est remonte");
        assert_eq!(meta(&srv, a_torch) & 1, if a_wins { 0 } else { 1 });
        assert_eq!(meta(&srv, b_torch) & 1, if a_wins { 1 } else { 0 });
    }

    #[test]
    fn pistons_push_pull_and_stop() {
        let (mut srv, c) = lab();
        let (piston, lever) = (at(c, 0, 0), at(c, 1, 0));
        let up = |n: i32| [c[0], c[1] + n, c[2]];
        // Poussoir tourne vers le haut, douze pierres au-dessus.
        srv.world.set(piston, PISTON, 2);
        for n in 1..=12 {
            srv.world.set(up(n), STONE, 0);
        }
        srv.world.set(lever, LEVER, 1);
        beats(&mut srv, 3);
        assert_eq!(srv.world.get(up(1)), Some((PISTON_HEAD, 2)));
        assert_eq!(srv.world.get(up(13)), Some((STONE, 0)), "douze blocs pousses");
        assert_eq!(meta(&srv, piston) & 8, 8);
        srv.world.set(lever, LEVER, 0);
        beats(&mut srv, 3);
        assert_eq!(srv.world.get(up(1)), Some((AIR, 0)));
        assert_eq!(srv.world.get(up(2)), Some((STONE, 0)), "un poussoir simple ne ramene rien");

        // Treize blocs : trop lourd.
        srv.world.set(up(1), STONE, 0);
        srv.world.set(lever, LEVER, 1);
        beats(&mut srv, 3);
        assert_eq!(meta(&srv, piston) & 8, 0);
        assert_eq!(srv.world.get(up(1)), Some((STONE, 0)));
        srv.world.set(lever, LEVER, 0);
        for n in 1..=14 {
            srv.world.set(up(n), AIR, 0);
        }
        beats(&mut srv, 3);

        // L'obsidienne ne bouge pas.
        srv.world.set(up(1), OBSIDIAN, 0);
        srv.world.set(lever, LEVER, 1);
        beats(&mut srv, 3);
        assert_eq!(meta(&srv, piston) & 8, 0);
        srv.world.set(lever, LEVER, 0);
        srv.world.set(up(1), AIR, 0);
        beats(&mut srv, 3);

        // Le poussoir gluant ramene son bloc.
        srv.world.set(piston, STICKY_PISTON, 2);
        srv.world.set(up(1), PLANKS, 0);
        srv.world.set(lever, LEVER, 1);
        beats(&mut srv, 3);
        assert_eq!(srv.world.get(up(1)), Some((PISTON_HEAD, 2 | 8)));
        assert_eq!(srv.world.get(up(2)), Some((PLANKS, 0)));
        srv.world.set(lever, LEVER, 0);
        beats(&mut srv, 3);
        assert_eq!(srv.world.get(up(1)), Some((PLANKS, 0)));
        assert_eq!(srv.world.get(up(2)), Some((AIR, 0)));

        // Casser la tete emporte le corps.
        srv.world.set(lever, LEVER, 1);
        beats(&mut srv, 3);
        srv.break_block(up(1), true);
        assert_eq!(srv.world.get(piston), Some((AIR, 0)));
    }

    #[test]
    fn observers_pulse_and_clock() {
        let (mut srv, c) = lab();
        let (obs, lamp, seen) = (at(c, 0, 0), at(c, -1, 0), at(c, 1, 0));
        // Regarde vers +X ; le signal sort par le dos, vers la lampe.
        srv.world.set(obs, OBSERVER, 0);
        srv.world.set(lamp, LAMP, 0);
        beats(&mut srv, 8);
        assert_eq!(meta(&srv, obs) & 8, 0);
        srv.world.set(seen, DIRT, 0);
        beats(&mut srv, 1);
        assert_eq!(meta(&srv, obs) & 8, 0, "un battement de retard");
        beats(&mut srv, 1);
        assert_eq!(meta(&srv, obs) & 8, 8);
        assert_eq!(meta(&srv, lamp), 1);
        beats(&mut srv, 1);
        assert_eq!(meta(&srv, obs) & 8, 0, "impulsion d'un battement");

        // Deux daruma face a face se surveillent sans fin : une horloge.
        srv.world.set(seen, OBSERVER, 1);
        let mut flips = 0;
        let mut last = 0;
        for _ in 0..40 {
            beats(&mut srv, 1);
            let on = meta(&srv, obs) & 8;
            flips += (on != last) as u32;
            last = on;
        }
        assert!(flips >= 10, "horloge de daruma : {flips} changements");
    }

    #[test]
    fn button_plate_door_and_sundial() {
        let (mut srv, c) = lab();
        let (button, lamp) = (at(c, 0, 0), at(c, 1, 0));
        srv.world.set(button, BUTTON, 0);
        srv.world.set(lamp, LAMP, 0);
        beats(&mut srv, 3);
        srv.press_button(button, 0);
        beats(&mut srv, 2);
        assert_eq!(meta(&srv, lamp), 1);
        beats(&mut srv, BUTTON_BEATS as usize - 3);
        assert_eq!(meta(&srv, button) & 1, 1);
        beats(&mut srv, 5);
        assert_eq!(meta(&srv, button) & 1, 0);
        assert_eq!(meta(&srv, lamp), 0);

        // Plancher rossignol : enfonce tant qu'un joueur est dessus.
        let (plate, door) = (at(c, 0, 3), at(c, 1, 3));
        srv.world.set(plate, PLATE, 0);
        srv.world.set(door, DOOR, 0);
        let (tx, _rx) = mpsc::channel();
        let mut cl = Client::new(tx, [plate[0] as f64 + 0.5, plate[1] as f64, plate[2] as f64 + 0.5]);
        cl.ready = true;
        srv.clients.insert(1, cl);
        beats(&mut srv, 3);
        assert_eq!(meta(&srv, plate), 1);
        assert_eq!(meta(&srv, door) & 1, 1, "la porte s'ouvre");
        if let Some(cl) = srv.clients.get_mut(&1) {
            cl.pos = [c[0] as f64 - 4.5, c[1] as f64, c[2] as f64 - 4.5];
        }
        beats(&mut srv, 15);
        assert_eq!(meta(&srv, plate), 0);
        assert_eq!(meta(&srv, door) & 1, 0);

        // Cadran solaire : plein midi, puis mode nuit.
        let dial = at(c, -3, -3);
        srv.world.set(dial, SUNDIAL, 0);
        srv.time = 0.25;
        srv.tick_sundials();
        assert_eq!(meta(&srv, dial), 15);
        srv.world.set(dial, SUNDIAL, 16 | 15);
        srv.tick_sundials();
        assert_eq!(meta(&srv, dial), 16);
    }

    #[test]
    fn hopper_moves_items_and_locks() {
        let (mut srv, c) = lab();
        let (hopper, top, dest, lever) = (at(c, 0, 0), [c[0], c[1] + 1, c[2]], at(c, 1, 0), at(c, 0, 1));
        srv.world.set(hopper, HOPPER, 0);
        srv.world.set(top, CHEST, 0);
        srv.world.set(dest, CHEST, 0);
        let mut slots = vec![None; 27];
        slots[2] = Some(Stack::new(DIRT as u16, 3));
        srv.world.chests.insert(top, slots);
        // Alimentee, la tremie est bloquee.
        srv.world.set(lever, LEVER, 1);
        beats(&mut srv, 20);
        assert_eq!(meta(&srv, hopper) & 8, 8);
        assert_eq!(items::count(&srv.world.chests[&top], DIRT as u16), 3);
        srv.world.set(lever, LEVER, 0);
        beats(&mut srv, 30);
        assert_eq!(items::count(&srv.world.chests[&top], DIRT as u16), 0, "le coffre du dessus est vide");
        assert_eq!(srv.world.chests.get(&dest).map(|s| items::count(s, DIRT as u16)), Some(3), "tout est arrive en face");

        // Une balance lit le contenu de la tremie.
        srv.world.chests.insert(hopper, vec![Some(Stack::new(DIRT as u16, 64)); 27]);
        srv.world.set(dest, AIR, 0);
        srv.world.chests.remove(&dest);
        let cmp = at(c, -1, 0);
        srv.world.set(cmp, COMPARATOR, 1);
        beats(&mut srv, 3);
        assert_eq!(meta(&srv, cmp) >> 4, 15);
    }

    #[test]
    fn drum_target_rope_and_detector_rail() {
        let (mut srv, c) = lab();
        // Tambour : frappe quand le signal arrive.
        let (drum, lever) = (at(c, -4, -4), at(c, -3, -4));
        srv.world.set(drum, NOTE, 5);
        srv.world.set(lever, LEVER, 1);
        beats(&mut srv, 2);
        assert_eq!(meta(&srv, drum), 5 | 32, "frappe, la note est conservee");

        // Cible : une fleche au centre donne 15, au bord beaucoup moins ; le signal retombe ensuite.
        let (target, lamp) = (at(c, 0, 0), at(c, 1, 0));
        srv.world.set(target, TARGET, 0);
        srv.world.set(lamp, LAMP, 0);
        let face = Vec3::new(target[0] as f64 + 0.5, target[1] as f64 + 0.5, target[2] as f64 + 0.02);
        srv.hit_target(target, face);
        assert_eq!(meta(&srv, target), 15);
        beats(&mut srv, 2);
        assert_eq!(meta(&srv, lamp), 1);
        srv.hit_target(target, face + Vec3::new(0.4, 0.0, 0.0));
        assert!(meta(&srv, target) <= 5, "pres du bord : {}", meta(&srv, target));
        beats(&mut srv, 14);
        assert_eq!(meta(&srv, target), 0);
        assert_eq!(meta(&srv, lamp), 0);

        // Corde shimenawa entre deux piquets : un joueur qui la traverse les declenche.
        let (left, right) = (at(c, -2, 3), at(c, 2, 3));
        srv.world.set(at(c, -3, 3), STONE, 0);
        srv.world.set(at(c, 3, 3), STONE, 0);
        srv.world.set(left, HOOK, 2 << 1);
        srv.world.set(right, HOOK, 1 << 1);
        for x in -1..=1 {
            srv.world.set(at(c, x, 3), TRIPWIRE, 0);
        }
        beats(&mut srv, 3);
        assert_eq!(meta(&srv, left) & 1, 0);
        let (tx, _rx) = mpsc::channel();
        let mut cl = Client::new(tx, [c[0] as f64 + 0.5, c[1] as f64, c[2] as f64 + 3.5]);
        cl.ready = true;
        srv.clients.insert(1, cl);
        beats(&mut srv, 3);
        assert_eq!(meta(&srv, left) & 1, 1);
        assert_eq!(meta(&srv, right) & 1, 1);
        srv.clients.clear();
        beats(&mut srv, 15);
        assert_eq!(meta(&srv, left) & 1, 0);
        // Corde coupee : plus rien ne se declenche.
        srv.world.set(at(c, 1, 3), AIR, 0);
        let (tx, _rx) = mpsc::channel();
        let mut cl = Client::new(tx, [c[0] as f64 + 0.5, c[1] as f64, c[2] as f64 + 3.5]);
        cl.ready = true;
        srv.clients.insert(1, cl);
        beats(&mut srv, 4);
        assert_eq!(meta(&srv, left) & 1, 0);

        // Rail guetteur : il ne sent que les chariots.
        let rail = at(c, 4, -4);
        srv.world.set(rail, DETECTOR_RAIL, 0);
        beats(&mut srv, 2);
        assert_eq!(meta(&srv, rail) & 8, 0);
        srv.spawn_mob(shared::mobs::CART, Vec3::new(rail[0] as f64 + 0.5, rail[1] as f64, rail[2] as f64 + 0.5), 0.0, 1.0);
        beats(&mut srv, 2);
        assert_eq!(meta(&srv, rail) & 8, 8);
    }

    #[test]
    fn dropper_feeds_a_chest() {
        let (mut srv, c) = lab();
        let (dropper, chest, lever) = (at(c, 0, 0), at(c, 1, 0), at(c, 0, 1));
        srv.world.set(dropper, DROPPER, 0);
        srv.world.set(chest, CHEST, 0);
        let mut slots = vec![None; 27];
        slots[4] = Some(Stack::new(DIRT as u16, 3));
        srv.world.chests.insert(dropper, slots);
        srv.world.set(lever, LEVER, 0);
        beats(&mut srv, 3);
        for round in 1..=2u32 {
            srv.world.set(lever, LEVER, 1);
            beats(&mut srv, 5);
            srv.world.set(lever, LEVER, 0);
            beats(&mut srv, 3);
            let got = srv.world.chests.get(&chest).map(|s| items::count(s, DIRT as u16)).unwrap_or(0);
            assert_eq!(got, round, "un objet par front montant");
        }
        assert_eq!(items::count(&srv.world.chests[&dropper], DIRT as u16), 1);
    }
}
