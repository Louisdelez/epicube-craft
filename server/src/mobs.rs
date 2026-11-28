//! Creatures : intelligence, combat, elevage, apparitions et projectiles.
use super::*;
use crate::world::MobSave;
use shared::blocks::*;
use shared::mobs as mk;
use shared::physics::d::{move_body, overlaps};

pub struct Mob {
    /// Dernier joueur a l'avoir frappee, et le niveau de Butin de son arme : a sa mort, il recoit
    /// l'experience et le butin en plus.
    pub slayer: Option<(u32, u8)>,
    pub id: u32,
    pub kind: u8,
    /// Creature de mod dont elle a l'apparence et les caracteristiques (0 : aucune).
    pub skin: u8,
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f64,
    pub hp: f64,
    on_ground: bool,
    /// Direction de promenade et temps avant d'en changer.
    wander: Vec3,
    timer: f64,
    atk_cd: f64,
    pub(crate) hurt_cd: f64,
    /// Fuite apres un coup (animaux).
    panic: f64,
    flee_from: Vec3,
    /// Envie de se reproduire, puis repos.
    love: f64,
    breed_cd: f64,
    /// Negatif tant que l'animal est un petit.
    pub age: f64,
    egg: f64,
    /// Secondes avant que la fourrure tondue ait repousse.
    shorn: f64,
    /// Ou la creature est apparue : un villageois ne s'en eloigne pas.
    home: Vec3,
    fuse: f64,
    burn_t: f64,
    tp_cd: f64,
    sent: Vec3,
    sent_yaw: f64,
    dead: bool,
    /// Wyrm : 0 = tourne autour de l'ile, 1 = pique sur un joueur.
    phase: u8,
    phase_t: f64,
    angle: f64,
}

pub struct Proj {
    id: u32,
    pos: Vec3,
    vel: Vec3,
    age: f64,
    /// 0 = fleche, 1 = boule de feu, 2 = lance, 3 = kunai.
    kind: u8,
    dmg: f64,
    /// Joueur qui a tire : son projectile blesse les creatures, pas lui.
    owner: Option<u32>,
    /// Objet a laisser au sol la ou le projectile s'arrete.
    stack: Option<Stack>,
}

/// Effets decides pendant la mise a jour des creatures, appliques ensuite.
enum Act {
    Hurt(u32, f64, Vec3),
    Explode(Vec3),
    Shoot(Vec3, Vec3, u8),
    Drop(Vec3, Stack),
    Send(ServerMsg),
}

fn cell(p: Vec3) -> [i32; 3] {
    [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32]
}

fn loot(kind: u8, rng: &mut Rng) -> Vec<Stack> {
    let some = |id: u16, lo: i32, hi: i32, rng: &mut Rng| {
        let n = rng.range(lo, hi + 1);
        (n > 0).then(|| Stack::new(id, n as u8))
    };
    let list = match kind {
        mk::COW => vec![some(items::RAW_MEAT, 1, 3, rng), some(items::LEATHER, 0, 2, rng)],
        mk::PIG => vec![some(items::RAW_MEAT, 1, 3, rng)],
        mk::SHEEP => vec![some(WOOL as u16, 1, 2, rng), some(items::RAW_MEAT, 1, 2, rng)],
        mk::CHICKEN => vec![some(items::RAW_BIRD, 1, 1, rng), some(items::FEATHER, 0, 2, rng)],
        mk::ZOMBIE => vec![some(items::ROTTEN, 0, 2, rng)],
        mk::SKELETON => vec![some(items::BONE, 0, 2, rng), some(items::ARROW, 0, 2, rng)],
        mk::SPIDER => vec![some(items::STRING, 0, 2, rng)],
        mk::BOMBYX => vec![some(items::SPORE, 0, 2, rng)],
        mk::SHADE => vec![some(items::PEARL, 0, 1, rng)],
        mk::BRAISIER => vec![some(items::EMBER_CRYSTAL, 0, 2, rng)],
        _ => vec![],
    };
    list.into_iter().flatten().collect()
}

impl Server {
    fn mob_msg(m: &Mob) -> ServerMsg {
        ServerMsg::MobSpawn { id: m.id, kind: m.kind, skin: m.skin, pos: m.pos.to_array(), yaw: m.yaw, baby: m.age < 0.0 }
    }

    pub(crate) fn mob_msgs(&self) -> Vec<ServerMsg> {
        self.mobs.iter().map(Self::mob_msg).collect()
    }

    /// Fait naitre une creature du jeu. Une part d'entre elles prend la forme d'une creature de mod
    /// batie sur ce genre, selon la rarete que le mod a fixee.
    pub(crate) fn spawn_mob(&mut self, kind: u8, pos: Vec3, age: f64, hp: f64) {
        let content = shared::mods::content();
        for d in content.mobs.iter().filter(|d| d.base == kind && d.rarity > 0.0) {
            if self.rng.d() < d.rarity as f64 {
                return self.spawn_skin(kind, d.id, pos, age, d.hp as f64);
            }
        }
        self.spawn_skin(kind, 0, pos, age, hp);
    }

    /// Fait naitre la creature de mod de ce nom complet.
    pub(crate) fn spawn_mod_mob(&mut self, key: &str, pos: Vec3) -> bool {
        let content = shared::mods::content();
        let Some(d) = content.mobs.iter().find(|d| d.key == key) else { return false };
        self.spawn_skin(d.base, d.id, pos, 0.0, d.hp as f64);
        true
    }

    fn spawn_skin(&mut self, kind: u8, skin: u8, pos: Vec3, age: f64, hp: f64) {
        // Paisible : aucune creature hostile ne nait (le Ryujin, lui, garde son ciel).
        if self.rules.difficulty == shared::rules::PEACEFUL && mk::hostile(kind) && kind != mk::WYRM {
            return;
        }
        let id = self.next_drop;
        self.next_drop += 1;
        let m = Mob {
            id,
            kind,
            skin,
            pos,
            vel: Vec3::ZERO,
            yaw: self.rng.d() * 6.28,
            hp,
            on_ground: false,
            wander: Vec3::ZERO,
            timer: 0.0,
            atk_cd: 1.0,
            hurt_cd: 0.0,
            slayer: None,
            panic: 0.0,
            flee_from: Vec3::ZERO,
            love: 0.0,
            breed_cd: 0.0,
            age,
            egg: 120.0 + self.rng.d() * 180.0,
            shorn: 0.0,
            home: pos,
            fuse: 0.0,
            burn_t: 0.0,
            tp_cd: 3.0,
            sent: pos,
            sent_yaw: 0.0,
            dead: false,
            phase: 0,
            phase_t: 6.0,
            angle: 0.0,
        };
        self.broadcast(Self::mob_msg(&m), None);
        self.mobs.push(m);
    }

    pub(crate) fn restore_mob(&mut self, s: MobSave) {
        if s.pos.iter().all(|v| v.is_finite()) {
            self.spawn_mob(s.kind, Vec3::from(s.pos), s.age, s.hp);
        }
    }

    /// Les animaux survivent aux redemarrages ; les monstres non.
    pub(crate) fn mob_saves(&self) -> Vec<MobSave> {
        self.mobs
            .iter()
            // Les creatures de mods ne sont pas enregistrees : leur mod peut ne plus etre la.
            .filter(|m| !mk::hostile(m.kind) && m.skin == 0)
            .map(|m| MobSave { kind: m.kind, pos: m.pos.to_array(), hp: m.hp, age: m.age })
            .collect()
    }

    /// Bloc plein le plus haut de la colonne.
    fn top_solid(&self, x: i32, z: i32) -> Option<i32> {
        (1..H as i32).rev().find(|&y| self.world.get([x, y, z]).is_some_and(|(b, m)| b == LEAVES || blocks::solid(b, m)))
    }

    pub(crate) fn sky_above(&self, p: Vec3) -> bool {
        let c = cell(p);
        self.top_solid(c[0], c[2]).is_none_or(|top| top < c[1])
    }

    /// Y a-t-il une source de lumiere a moins de 6 blocs ?
    pub(crate) fn lit(&self, c: [i32; 3]) -> bool {
        for dy in -4..=4 {
            for dz in -6..=6 {
                for dx in -6..=6 {
                    if let Some((b, m)) = self.world.get([c[0] + dx, c[1] + dy, c[2] + dz]) {
                        if b != AIR && emit(b, m) > 0 {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn fits(&self, kind: u8, pos: Vec3) -> bool {
        let (hw, h) = mk::size(kind);
        let c = cell(pos);
        self.world.get(c).is_some_and(|(b, _)| b == AIR) && !overlaps(&self.world, pos, hw, h)
    }

    pub(crate) fn tick_mobs(&mut self) {
        let day = self.is_day();
        // (id, position, objet tenu, vole)
        let players: Vec<(u32, Vec3, Option<u16>, bool)> = self
            .clients
            .iter()
            .filter(|(_, c)| c.ready && !c.dead && c.dim == self.dim)
            .map(|(id, c)| (*id, Vec3::from(c.pos), c.held(), shared::rules::invulnerable(c.mode)))
            .collect();
        // Regard des passagers, par chariot.
        let riders: HashMap<u32, Vec3> = self
            .clients
            .values()
            .filter(|c| c.ready && !c.dead && c.dim == self.dim)
            .filter_map(|c| c.riding.map(|id| (id, Vec3::new(c.yaw.sin(), 0.0, c.yaw.cos()))))
            .collect();
        let mut mobs = std::mem::take(&mut self.mobs);
        let mut acts: Vec<Act> = Vec::new();
        let mut wyrm_dead = false;

        for m in mobs.iter_mut() {
            let c = cell(m.pos);
            // Hors des chunks charges, tout est fige, sauf le Wyrm qui vole au-dessus du vide.
            if m.kind != mk::WYRM && !self.world.chunks.contains_key(&(c[0] >> 4, c[2] >> 4)) {
                continue;
            }
            if m.kind == mk::CART {
                // Ni faim ni peur : un chariot ne fait que rouler.
                if m.hp <= 0.0 {
                    m.dead = true;
                    continue;
                }
                crate::rails::cart_step(&self.world, &mut m.pos, &mut m.vel, &mut m.yaw, riders.get(&m.id).copied());
                if m.pos.distance_squared(m.sent) > 0.0009 || (m.yaw - m.sent_yaw).abs() > 0.05 {
                    m.sent = m.pos;
                    m.sent_yaw = m.yaw;
                    acts.push(Act::Send(ServerMsg::MobPos { id: m.id, pos: m.pos.to_array(), yaw: m.yaw }));
                }
                continue;
            }
            let near = players
                .iter()
                .map(|p| (p, p.1.distance(m.pos)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            let hostile = mk::hostile(m.kind);
            let flying = matches!(m.kind, mk::BRAISIER | mk::WYRM);
            if m.kind != mk::WYRM && (hostile && near.is_none_or(|(_, d)| d > 90.0) || m.pos.y < -10.0) {
                m.dead = true;
                continue;
            }
            m.atk_cd -= DT;
            m.hurt_cd -= DT;
            m.tp_cd -= DT;
            let (hw, h) = mk::size(m.kind);
            if m.kind == mk::WYRM {
                m.phase_t -= DT;
                let prey = near.filter(|(p, d)| !p.3 && *d < 140.0);
                let goal = match (m.phase, prey) {
                    (1, Some(((pid, ppos, _, _), d))) => {
                        if d < 3.0 {
                            let dir = (*ppos - m.pos).normalize_or_zero();
                            acts.push(Act::Hurt(*pid, 8.0, Vec3::new(dir.x, 0.0, dir.z) * 10.0 + Vec3::Y * 6.0));
                            m.phase = 0;
                            m.phase_t = 9.0;
                        } else if m.phase_t <= 0.0 {
                            m.phase = 0;
                            m.phase_t = 9.0;
                        }
                        *ppos + Vec3::Y
                    }
                    _ => {
                        m.phase = 0;
                        m.angle += 0.35 * DT;
                        if let Some(((_, ppos, _, _), d)) = prey {
                            if m.phase_t <= 0.0 {
                                m.phase = 1;
                                m.phase_t = 7.0;
                            }
                            if m.atk_cd <= 0.0 && d < 80.0 {
                                m.atk_cd = 4.0;
                                let from = m.pos;
                                acts.push(Act::Shoot(from, (*ppos + Vec3::Y - from).normalize_or_zero() * 18.0, 1));
                            }
                        }
                        Vec3::new(m.angle.cos() * 42.0, 92.0 + (m.angle * 2.0).sin() * 6.0, m.angle.sin() * 42.0)
                    }
                };
                let sp = if m.phase == 1 { 22.0 } else { 16.0 };
                let dir = (goal - m.pos).normalize_or_zero();
                m.vel += (dir * sp - m.vel) * (2.5 * DT).min(1.0);
                m.pos += m.vel * DT;
                if m.vel.length_squared() > 0.01 {
                    m.yaw = m.vel.x.atan2(m.vel.z);
                }
            } else if m.kind == mk::BRAISIER {
                // Plane a distance du joueur et lui lance des boules de feu.
                m.timer -= DT;
                if m.timer <= 0.0 {
                    m.timer = 3.0;
                    let a = self.rng.d() * std::f64::consts::TAU;
                    m.wander = Vec3::new(a.sin(), self.rng.d() - 0.5, a.cos());
                }
                let mut want = m.wander;
                if let Some(((_, ppos, _, _), d)) = near.filter(|(p, d)| !p.3 && *d < 24.0) {
                    let to = Vec3::new(ppos.x - m.pos.x, 0.0, ppos.z - m.pos.z).normalize_or_zero();
                    let goal = *ppos - to * 8.0 + Vec3::Y * 3.5;
                    want = (goal - m.pos).clamp_length_max(1.0) * 3.5;
                    m.yaw = to.x.atan2(to.z);
                    if m.atk_cd <= 0.0 && d < 20.0 {
                        m.atk_cd = 3.0;
                        let from = m.pos + Vec3::Y * 0.9;
                        acts.push(Act::Shoot(from, (*ppos + Vec3::Y - from).normalize_or_zero() * 14.0, 1));
                    }
                }
                m.vel += (want - m.vel) * (4.0 * DT).min(1.0);
                move_body(&self.world, &mut m.pos, &mut m.vel, hw, h, DT);
            }
            if !flying {
            let speed = mk::speed(m.kind);
            let mut want = Vec3::ZERO;
            let mut face: Option<Vec3> = None;

            // Promenade au hasard, partagee par tous quand il n'y a rien d'autre a faire.
            m.timer -= DT;
            if m.timer <= 0.0 {
                m.timer = 2.0 + self.rng.d() * 4.0;
                m.wander = if self.rng.chance(0.4) {
                    Vec3::ZERO
                } else {
                    let a = self.rng.d() * std::f64::consts::TAU;
                    Vec3::new(a.sin(), 0.0, a.cos())
                };
            }

            let target = near.filter(|((_, _, _, fly), d)| hostile && !fly && *d < 20.0);
            if let Some(((pid, ppos, _, _), dist)) = target {
                let to = Vec3::new(ppos.x - m.pos.x, 0.0, ppos.z - m.pos.z);
                let flat = to.length();
                let dir = to.normalize_or_zero();
                face = Some(dir);
                match m.kind {
                    mk::SKELETON => {
                        // Garde ses distances et tire.
                        want = if flat > 10.0 {
                            dir * speed
                        } else if flat < 6.0 {
                            -dir * speed
                        } else {
                            Vec3::ZERO
                        };
                        if dist < 16.0 && m.atk_cd <= 0.0 {
                            m.atk_cd = 2.2;
                            let from = m.pos + Vec3::Y * 1.5;
                            let aim = *ppos + Vec3::Y * (1.0 + dist * 0.035) - from;
                            acts.push(Act::Shoot(from, aim.normalize_or_zero() * 22.0, 0));
                        }
                    }
                    mk::BOMBYX => {
                        if dist < 2.8 {
                            m.fuse += DT;
                            if m.fuse >= 1.5 {
                                m.dead = true;
                                acts.push(Act::Explode(m.pos + Vec3::Y * 0.6));
                            }
                        } else {
                            m.fuse = (m.fuse - DT).max(0.0);
                            want = dir * speed;
                        }
                    }
                    _ => {
                        if flat > 0.8 {
                            want = dir * speed;
                        }
                        let reach = (ppos.y - m.pos.y).abs() < 1.8 && flat < hw + 1.0;
                        if reach && m.atk_cd <= 0.0 {
                            m.atk_cd = 1.0;
                            let dmg = shared::mods::with_mob(m.skin, |d| d.damage as f64).filter(|d| *d > 0.0).unwrap_or_else(|| mk::melee(m.kind));
                            acts.push(Act::Hurt(*pid, dmg, dir * 7.0 + Vec3::Y * 4.5));
                        }
                        // L'ombre comble la distance en se teleportant.
                        if m.kind == mk::SHADE && dist > 9.0 && m.tp_cd <= 0.0 {
                            m.tp_cd = 5.0;
                            let a = self.rng.d() * std::f64::consts::TAU;
                            let dest = *ppos + Vec3::new(a.sin() * 3.0, 0.0, a.cos() * 3.0);
                            if !overlaps(&self.world, dest, hw, h) {
                                m.pos = dest;
                            }
                        }
                    }
                }
            } else if m.panic > 0.0 {
                m.panic -= DT;
                let away = Vec3::new(m.pos.x - m.flee_from.x, 0.0, m.pos.z - m.flee_from.z).normalize_or_zero();
                want = away * speed * 2.2;
            } else {
                // Un animal adulte suit le joueur qui tient sa nourriture preferee.
                let lured = near.filter(|((_, _, held, _), d)| {
                    !hostile && m.age >= 0.0 && *d < 8.0 && held.is_some() && *held == mk::lure(m.kind)
                });
                match lured {
                    Some(((_, ppos, _, _), d)) => {
                        let dir = Vec3::new(ppos.x - m.pos.x, 0.0, ppos.z - m.pos.z).normalize_or_zero();
                        face = Some(dir);
                        if d > 2.2 {
                            want = dir * speed * 1.3;
                        }
                    }
                    None => want = m.wander * speed * if hostile { 0.5 } else { 0.8 },
                }
            }

            // Zombies et squelettes brulent au soleil.
            if day && matches!(m.kind, mk::ZOMBIE | mk::SKELETON) {
                m.burn_t += DT;
                if m.burn_t >= 1.0 {
                    m.burn_t = 0.0;
                    if self.top_solid(c[0], c[2]).is_none_or(|top| top < c[1]) {
                        m.hp -= 2.0;
                        acts.push(Act::Send(ServerMsg::MobHurt { id: m.id }));
                    }
                }
            }

            // Physique.
            let body = self.world.get([c[0], (m.pos.y + h * 0.5).floor() as i32, c[2]]).map(|x| x.0).unwrap_or(AIR);
            let k = (8.0 * DT).min(1.0);
            m.vel.x += (want.x - m.vel.x) * k;
            m.vel.z += (want.z - m.vel.z) * k;
            if body == WATER {
                m.vel.y = (m.vel.y + 14.0 * DT).min(2.0);
            } else {
                m.vel.y = (m.vel.y - 28.0 * DT).max(-50.0);
            }
            if body == LAVA {
                m.burn_t += DT * 4.0;
                m.hp -= 6.0 * DT;
            }
            let r = move_body(&self.world, &mut m.pos, &mut m.vel, hw, h, DT);
            m.on_ground = r.on_ground;
            if r.hit_wall && m.on_ground && want.length_squared() > 0.01 {
                m.vel.y = 8.2;
            }
            if let Some(dir) = face.or((want.length_squared() > 0.01).then(|| want.normalize())) {
                m.yaw = dir.x.atan2(dir.z);
            }

            }

            // Vie des animaux : grandir, pondre.
            if !hostile {
                m.love -= DT;
                m.breed_cd -= DT;
                // Un villageois qui s'egare reprend le chemin de sa maison.
                if m.kind == mk::VILLAGER && m.pos.distance(m.home) > 14.0 {
                    m.wander = Vec3::new(m.home.x - m.pos.x, 0.0, m.home.z - m.pos.z).normalize_or_zero();
                    m.timer = m.timer.max(2.0);
                }
                if m.shorn > 0.0 {
                    m.shorn -= DT;
                    if m.shorn <= 0.0 {
                        // La fourrure repousse quand l'animal a broute : l'herbe sous lui disparait.
                        let under = cell(m.pos - Vec3::Y * 0.5);
                        if self.world.get(under).is_some_and(|x| x.0 == GRASS) {
                            self.world.set(under, DIRT, 0);
                        }
                    }
                }
                if m.age < 0.0 {
                    m.age += DT;
                    if m.age >= 0.0 {
                        acts.push(Act::Send(Self::mob_msg(m)));
                    }
                }
                if m.kind == mk::CHICKEN && m.age >= 0.0 {
                    m.egg -= DT;
                    if m.egg <= 0.0 {
                        m.egg = 180.0 + self.rng.d() * 180.0;
                        acts.push(Act::Drop(m.pos + Vec3::Y * 0.3, Stack::new(items::EGG, 1)));
                    }
                }
            }

            if m.hp <= 0.0 {
                m.dead = true;
                wyrm_dead |= m.kind == mk::WYRM;
                self.mod_deaths.push((self.dim, m.kind, m.skin, m.pos));
                let looting = m.slayer.map_or(0, |s| s.1 as i32);
                if let Some((who, _)) = m.slayer {
                    // Une creature hostile rapporte 5 points d'experience, un animal de 1 a 3.
                    let xp = if mk::hostile(m.kind) { 5 } else { self.rng.range(1, 4) as u32 };
                    self.xp_due.push((who, xp));
                }
                // Une creature de mod laisse le butin que son mod a choisi.
                let own = shared::mods::with_mob(m.skin, |d| d.loot.clone());
                let stacks = match own {
                    Some(list) => list.into_iter().filter_map(|(item, lo, hi)| {
                        let n = self.rng.range(lo as i32, hi.max(lo) as i32 + 1);
                        (n > 0).then(|| Stack::new(item, n.min(64) as u8))
                    }).collect(),
                    None => loot(m.kind, &mut self.rng),
                };
                // Butin : jusqu'a un objet de plus par niveau, pour chaque sorte de butin.
                let stacks: Vec<Stack> = stacks.into_iter().map(|st: Stack| Stack { n: (st.n as i32 + self.rng.range(0, looting + 1)).min(64) as u8, ..st }).collect();
                for st in stacks {
                    // Un animal tondu n'a plus de fourrure a donner.
                    if !(m.shorn > 0.0 && st.id == WOOL as u16) {
                        acts.push(Act::Drop(m.pos + Vec3::Y * 0.4, st));
                    }
                }
            } else if m.pos.distance_squared(m.sent) > 0.0009 || (m.yaw - m.sent_yaw).abs() > 0.05 {
                m.sent = m.pos;
                m.sent_yaw = m.yaw;
                acts.push(Act::Send(ServerMsg::MobPos { id: m.id, pos: m.pos.to_array(), yaw: m.yaw }));
            }
        }

        for m in mobs.iter().filter(|m| m.dead) {
            acts.push(Act::Send(ServerMsg::MobRemove { id: m.id }));
            // Un chariot qui disparait depose son passager.
            for c in self.clients.values_mut().filter(|c| c.riding == Some(m.id)) {
                c.riding = None;
                c.send(ServerMsg::Ride { id: None });
            }
        }
        mobs.retain(|m| !m.dead);
        mobs.append(&mut self.mobs);
        self.mobs = mobs;

        for act in acts {
            match act {
                Act::Hurt(id, dmg, knock) => self.hurt_player(id, dmg, knock),
                Act::Explode(at) => self.explode(at, 3.0),
                Act::Shoot(pos, vel, kind) => {
                    let id = self.next_drop;
                    self.next_drop += 1;
                    self.broadcast(ServerMsg::ProjSpawn { id, pos: pos.to_array(), vel: vel.to_array(), kind }, None);
                    let dmg = if kind == 1 { 5.0 } else { 3.0 };
                    self.projs.push(Proj { id, pos, vel, age: 0.0, kind, dmg, owner: None, stack: None });
                }
                Act::Drop(pos, st) => {
                    let vel = Vec3::new(self.rng.d() - 0.5, 2.5, self.rng.d() - 0.5);
                    self.spawn_drop(pos, vel, st, 0.5);
                }
                Act::Send(msg) => self.broadcast(msg, None),
            }
        }
        if wyrm_dead {
            // Le butin apparait au centre de l'ile plutot que dans le vide.
            self.world.set_boss_dead();
            let top = self.top_solid(0, 3).unwrap_or(66) as f64;
            let at = Vec3::new(0.5, top + 2.0, 3.5);
            self.spawn_drop(at, Vec3::Y * 3.0, Stack::new(items::WYRM_HEART, 1), 0.5);
            self.spawn_drop(at, Vec3::new(1.0, 3.0, 0.0), Stack::new(items::DIAMOND, 8), 0.5);
            // Une porte celeste s'ouvre vers l'ile de la pagode.
            let gate = self.top_solid(0, -8).unwrap_or(66);
            self.world.set([0, gate + 1, -8], GATEWAY, 0);
            self.broadcast(ServerMsg::Boss { hp: None }, None);
            self.broadcast(ServerMsg::Info { text: "Ryujin est vaincu ! Une porte celeste s'ouvre...".to_string() }, None);
        }
    }

    pub(crate) fn tick_projs(&mut self) {
        let mut hits = Vec::new();
        let mut gone = Vec::new();
        // Blocs touches par une fleche : (case, point d'impact).
        let mut struck: Vec<([i32; 3], Vec3)> = Vec::new();
        let mut hurt: Vec<u32> = Vec::new();
        // Objets a laisser au sol : (position, pile).
        let mut left: Vec<(Vec3, Stack)> = Vec::new();
        let luck = self.rng.chance(0.5);
        for p in self.projs.iter_mut() {
            p.age += DT;
            if p.kind != 1 {
                p.vel.y -= 20.0 * DT;
            }
            // Deux demi-pas pour ne pas traverser un joueur ou un mur.
            let mut done = p.age > 6.0;
            let mut in_flesh = false;
            for _ in 0..2 {
                if done {
                    break;
                }
                let before = p.pos;
                p.pos += p.vel * DT * 0.5;
                let c = cell(p.pos);
                if self.world.get(c).is_none_or(|(b, m)| blocks::solid(b, m)) {
                    if p.kind == 0 {
                        struck.push((c, p.pos));
                    }
                    // Plante dans le bloc : l'objet retombe juste devant.
                    p.pos = before;
                    done = true;
                    break;
                }
                for (id, cl) in self.clients.iter().filter(|(_, c)| c.ready && !c.dead && c.dim == self.dim) {
                    if p.owner == Some(*id) {
                        continue;
                    }
                    let f = Vec3::from(cl.pos);
                    let d = p.pos - f;
                    if d.x.abs() < 0.4 && d.z.abs() < 0.4 && d.y > -0.1 && d.y < 1.9 {
                        hits.push((*id, p.dmg, p.vel.normalize_or_zero() * 5.0 + Vec3::Y * 3.0));
                        done = true;
                        in_flesh = true;
                        break;
                    }
                }
                // Les tirs d'un joueur blessent les creatures.
                if !done && p.owner.is_some() {
                    for m in self.mobs.iter_mut().filter(|m| m.kind != mk::CART && m.hp > 0.0) {
                        let (hw, h) = mk::size(m.kind);
                        let d = p.pos - m.pos;
                        if d.x.abs() < hw + 0.15 && d.z.abs() < hw + 0.15 && d.y > -0.15 && d.y < h + 0.15 {
                            m.hp -= p.dmg;
                            m.hurt_cd = 0.35;
                            m.panic = 4.0;
                            m.flee_from = p.pos - p.vel.normalize_or_zero() * 4.0;
                            m.vel += Vec3::new(p.vel.x, 0.0, p.vel.z).normalize_or_zero() * 5.0 + Vec3::Y * 3.0;
                            hurt.push(m.id);
                            done = true;
                            in_flesh = true;
                            break;
                        }
                    }
                }
            }
            if done {
                gone.push(p.id);
                // Une fleche qui touche est perdue, un kunai une fois sur deux ; la lance se recupere toujours.
                let keep = !in_flesh || p.kind == 2 || (p.kind == 3 && luck);
                if let Some(st) = p.stack.take().filter(|_| keep && p.age <= 6.0) {
                    left.push((p.pos, st));
                }
            }
        }
        self.projs.retain(|p| !gone.contains(&p.id));
        for id in gone {
            self.broadcast(ServerMsg::ProjRemove { id }, None);
        }
        for (id, dmg, knock) in hits {
            self.hurt_player(id, dmg, knock);
        }
        for (c, at) in struck {
            self.hit_target(c, at);
        }
        for id in hurt {
            self.broadcast(ServerMsg::MobHurt { id }, None);
        }
        for (pos, st) in left {
            self.spawn_drop(pos, Vec3::ZERO, st, 0.6);
        }
    }

    /// Blesse un joueur (avec une courte invulnerabilite) et le repousse.
    pub(crate) fn hurt_player(&mut self, id: u32, dmg: f64, knock: Vec3) {
        self.strike(id, dmg, knock, false, shared::proto::blow::PLAIN);
    }

    /// Porte un coup a un joueur. `axe` : une hache abat le bouclier qui la pare ; `kind` : le genre
    /// du coup, montre aux autres joueurs (voir `shared::proto::blow`).
    pub(crate) fn strike(&mut self, id: u32, dmg: f64, knock: Vec3, axe: bool, kind: u8) -> Struck {
        let Some(c) = self.clients.get_mut(&id) else { return Struck::Missed };
        // La difficulte regle la force des creatures, des projectiles et des explosions.
        let dmg = shared::rules::scale_damage(dmg, self.rules.difficulty);
        if c.dead || shared::rules::invulnerable(c.mode) || dmg <= 0.0 {
            return Struck::Missed;
        }
        if c.inv_t > 0.0 {
            // Encore invulnerable : seul un coup plus fort que le precedent passe, pour la difference, sans recul.
            if dmg <= c.last_dmg + 1e-9 {
                return Struck::Missed;
            }
            let more = dmg - c.last_dmg;
            c.last_dmg = dmg;
            c.hp -= c.absorb(more, 1.0);
            c.stats_dirty = true;
            return Struck::Hurt;
        }
        if self.shield_blocks(id, dmg, knock, axe) {
            return Struck::Blocked;
        }
        let roll = self.rng.d();
        let Some(c) = self.clients.get_mut(&id) else { return Struck::Missed };
        c.inv_t = 0.5;
        c.last_dmg = dmg;
        // L'armure retire sa part du coup et amortit le recul.
        let steady = items::defense(&c.armor).2;
        c.hp -= c.absorb(dmg, roll);
        c.stats_dirty = true;
        c.send(ServerMsg::Knock { vel: (knock * (1.0 - steady).max(0.0)).to_array() });
        let dim = c.dim;
        for o in self.clients.values().filter(|o| o.ready && o.dim == dim) {
            o.send(ServerMsg::PlayerHurt { id, kind });
        }
        Struck::Hurt
    }

    /// Detruit les blocs fragiles dans le rayon et blesse ce qui s'y trouve.
    /// Lance un projectile sans tireur (0 = fleche, 1 = boule de feu).
    pub(crate) fn spawn_proj(&mut self, pos: Vec3, vel: Vec3, kind: u8) {
        self.spawn_shot(pos, vel, kind, if kind == 1 { 5.0 } else { 3.0 }, None, None);
    }

    /// Lance un projectile. `owner` : le joueur qui tire ; `stack` : ce qui restera au sol.
    pub(crate) fn spawn_shot(&mut self, pos: Vec3, vel: Vec3, kind: u8, dmg: f64, owner: Option<u32>, stack: Option<Stack>) {
        let id = self.next_drop;
        self.next_drop += 1;
        self.broadcast(ServerMsg::ProjSpawn { id, pos: pos.to_array(), vel: vel.to_array(), kind }, None);
        self.projs.push(Proj { id, pos, vel, age: 0.0, kind, dmg, owner, stack });
    }

    pub(crate) fn explode(&mut self, at: Vec3, r: f64) {
        let c = cell(at);
        self.sound(c, shared::proto::sound::EXPLOSION, 0);
        let ri = r.ceil() as i32;
        for dy in -ri..=ri {
            for dz in -ri..=ri {
                for dx in -ri..=ri {
                    let p = [c[0] + dx, c[1] + dy, c[2] + dz];
                    let d = Vec3::new(dx as f64, dy as f64, dz as f64).length();
                    if d > r - self.rng.d() * 0.8 {
                        continue;
                    }
                    if let Some((b, _)) = self.world.get(p) {
                        let h = items::hardness(b);
                        if b == EXPLOSIVE {
                            // Reaction en chaine.
                            let delay = 0.2 + self.rng.d() * 0.5;
                            self.prime(p, delay);
                        } else if b != AIR && (0.0..20.0).contains(&h) {
                            let keep = self.rng.chance(0.3);
                            self.break_block(p, keep);
                        }
                    }
                }
            }
        }
        let damage = |d: f64| (1.0 - d / (2.0 * r)).max(0.0) * 5.0 * r;
        let victims: Vec<(u32, Vec3)> =
            self.clients.iter().filter(|(_, c)| c.ready && c.dim == self.dim).map(|(id, c)| (*id, Vec3::from(c.pos) + Vec3::Y * 0.9)).collect();
        for (id, p) in victims {
            let d = p.distance(at);
            if d < 2.0 * r {
                self.hurt_player(id, damage(d), (p - at).normalize_or_zero() * 9.0 + Vec3::Y * 4.0);
            }
        }
        for m in self.mobs.iter_mut() {
            let d = m.pos.distance(at);
            if d < 2.0 * r {
                m.hp -= damage(d);
                m.vel += (m.pos - at).normalize_or_zero() * 8.0 + Vec3::Y * 4.0;
            }
        }
    }

    /// Coup porte par un joueur.
    #[cfg(test)]
    pub(crate) fn attack(&mut self, id: u32, mob_id: u32) {
        self.attack_with(id, mob_id, false);
    }

    /// Coup porte par un joueur a une creature. `sprint` : porte en courant.
    pub(crate) fn attack_with(&mut self, id: u32, mob_id: u32, sprint: bool) {
        let now = Instant::now();
        let Some(c) = self.clients.get_mut(&id) else { return };
        // Temps ecoule depuis le coup precedent : il fixe la force de celui-ci.
        let elapsed = now.duration_since(c.last_hit).as_secs_f64();
        if c.dead || c.blocking || elapsed < 0.1 {
            return;
        }
        // Un coup suffit a demonter un chariot : il redevient un objet.
        if let Some(m) = self.mobs.iter_mut().find(|m| m.id == mob_id && m.kind == mk::CART && m.hp > 0.0) {
            if c.eye().distance(m.pos) > 6.5 {
                return;
            }
            c.last_hit = now;
            m.hp = 0.0;
            let at = m.pos + Vec3::Y * 0.3;
            self.spawn_drop(at, Vec3::Y * 2.0, Stack::new(items::CART, 1), 0.5);
            return;
        }
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == mob_id) else { return };
        let (reach, h) = mk::size(m.kind);
        let held = c.held();
        let w = items::weapon(held);
        let eye = c.eye();
        if eye.distance(m.pos + Vec3::Y * h * 0.5) > 5.5 + reach + w.reach || m.hurt_cd > 0.0 {
            return;
        }
        let power = items::charge(elapsed, w.speed);
        let ready = power > 0.85;
        // En courant, l'arme en garde : le coup repousse loin, mais n'est ni critique ni fauchant.
        let sprint = sprint && ready;
        // Coup critique : porte en retombant, l'arme revenue en garde.
        let drop = c.fall_peak - c.pos[1];
        let falling = !c.on_ground && !c.fly && drop > 0.05;
        let ench = c.inv[c.sel].map_or(shared::ench::Ench::NONE, |st| st.ench);
        let mut dmg = (w.dmg + crate::sharp(ench)) * power * if falling && ready && !sprint { 1.5 } else { 1.0 };
        if held == Some(items::TETSUBO) && falling {
            // La massue ecrase : 3 points par bloc de chute au-dela d'un bloc et demi, et le coup amortit la chute.
            dmg += 3.0 * (drop - 1.5).clamp(0.0, 6.0);
            c.fall_peak = c.pos[1];
        }
        let sweeping = ready && !sprint && c.on_ground && (matches!(held.and_then(items::tool), Some((items::SWORD, _))) || held == Some(items::NAGINATA));
        c.last_hit = now;
        c.exhaustion += 0.1;
        m.hp -= dmg;
        m.hurt_cd = 0.35;
        let from = Vec3::from(c.pos);
        let away = Vec3::new(m.pos.x - from.x, 0.0, m.pos.z - from.z).normalize_or_zero();
        m.vel += (away * (if sprint { 12.5 } else { 7.0 } + 6.0 * ench.level(shared::ench::KNOCKBACK) as f64) + Vec3::Y * 5.0) * w.knock;
        m.slayer = Some((id, ench.level(shared::ench::LOOTING)));
        m.panic = 4.0;
        m.flee_from = from;
        let hit_at = m.pos;
        // Une arme s'use a chaque coup, un outil deux fois plus vite.
        let roll = self.rng.d();
        c.wear(w.wear, roll);
        let mut swept = Vec::new();
        if sweeping {
            // Fauchage : le katana effleure les voisins de la cible, la naginata les frappe a mi-force sur un large arc.
            let (radius, side) = if held == Some(items::NAGINATA) { (2.6, dmg * 0.5) } else { (1.6, crate::swept(ench, dmg)) };
            for o in self.mobs.iter_mut() {
                if o.id != mob_id && o.kind != mk::CART && o.hurt_cd <= 0.0 && o.pos.distance(hit_at) < radius && o.pos.distance(from) < 6.0 {
                    o.hp -= side;
                    o.hurt_cd = 0.35;
                    o.vel += Vec3::new(o.pos.x - from.x, 0.0, o.pos.z - from.z).normalize_or_zero() * 4.0 + Vec3::Y * 3.0;
                    swept.push(o.id);
                }
            }
        }
        for id in swept {
            self.broadcast(ServerMsg::MobHurt { id }, None);
        }
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == mob_id) else { return };
        // L'ombre s'echappe quand on la frappe.
        if m.kind == mk::SHADE && m.hp > 0.0 {
            let a = self.rng.d() * std::f64::consts::TAU;
            let dest = m.pos + Vec3::new(a.sin() * 8.0, 0.5, a.cos() * 8.0);
            let (hw, h) = mk::size(m.kind);
            if !overlaps(&self.world, dest, hw, h) {
                m.pos = dest;
            }
        }
        self.broadcast(ServerMsg::MobHurt { id: mob_id }, None);
    }

    /// Clic droit sur une creature : nourrir un animal adulte le met en mode reproduction.
    pub(crate) fn interact(&mut self, id: u32, mob_id: u32) {
        let Some(c) = self.clients.get_mut(&id) else { return };
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == mob_id) else { return };
        if m.kind == mk::VILLAGER {
            if c.dead || c.eye().distance(m.pos) > 6.0 {
                return;
            }
            let profession = m.age.max(0.0) as u8;
            return self.open_trade(id, mob_id, profession);
        }
        if m.kind == mk::CART {
            // Monter dans le chariot, ou en descendre.
            if c.dead || (c.riding != Some(mob_id) && c.eye().distance(m.pos) > 6.0) {
                return;
            }
            c.riding = if c.riding == Some(mob_id) { None } else { Some(mob_id) };
            c.send(ServerMsg::Ride { id: c.riding });
            return;
        }
        let Some(st) = c.inv[c.sel] else { return };
        if c.dead || c.eye().distance(m.pos) > 6.0 {
            return;
        }
        // Ciseaux : on tond le tanuki sans lui faire de mal.
        if st.id == items::SHEARS && m.kind == mk::SHEEP {
            if m.age < 0.0 || m.shorn > 0.0 {
                return;
            }
            m.shorn = 60.0 + self.rng.d() * 60.0;
            let (at, n) = (m.pos + Vec3::Y * 0.6, self.rng.range(1, 4) as u8);
            self.spawn_drop(at, Vec3::Y * 2.5, Stack::new(WOOL as u16, n), 0.5);
            self.wear_held(id);
            return;
        }
        if mk::lure(m.kind) != Some(st.id) {
            return;
        }
        // Nourrir un petit le fait grandir plus vite : un dixieme du temps restant en moins.
        if m.age < 0.0 {
            m.age *= 0.9;
            c.inv[c.sel] = if st.n > 1 { Some(Stack { n: st.n - 1, ..st }) } else { None };
            c.inv_dirty = true;
            return;
        }
        if m.love > 0.0 || m.breed_cd > 0.0 {
            return;
        }
        m.love = 30.0;
        c.inv[c.sel] = if st.n > 1 { Some(Stack { n: st.n - 1, ..st }) } else { None };
        c.inv_dirty = true;
        c.send(ServerMsg::Info { text: "L'animal est pret a se reproduire".to_string() });
    }

    /// Deux animaux du meme genre, nourris et proches, donnent un petit.
    pub(crate) fn breed_cycle(&mut self) {
        let mut born = Vec::new();
        for i in 0..self.mobs.len() {
            if self.mobs[i].love <= 0.0 {
                continue;
            }
            for j in i + 1..self.mobs.len() {
                let (a, b) = (&self.mobs[i], &self.mobs[j]);
                if b.love > 0.0 && a.kind == b.kind && a.pos.distance(b.pos) < 3.5 {
                    born.push((a.kind, (a.pos + b.pos) * 0.5));
                    for k in [i, j] {
                        self.mobs[k].love = 0.0;
                        self.mobs[k].breed_cd = 120.0;
                    }
                    break;
                }
            }
        }
        for (kind, pos) in born {
            self.spawn_mob(kind, pos, -180.0, mk::max_hp(kind));
        }
    }

    /// Une fois par seconde : monstres dans le noir, animaux sur l'herbe.
    /// Steles maudites : tant qu'un joueur rode a moins de 16 blocs, des oni surgissent autour, quatre au plus.
    fn spawn_from_steles(&mut self) {
        let steles: Vec<[i32; 3]> = self.world.signal.iter().filter(|p| self.world.get(**p).is_some_and(|x| x.0 == SPAWNER)).copied().collect();
        for p in steles {
            let at = Vec3::new(p[0] as f64 + 0.5, p[1] as f64, p[2] as f64 + 0.5);
            let watched = self.clients.values().any(|c| c.ready && !c.dead && c.dim == self.dim && Vec3::from(c.pos).distance(at) < 16.0);
            // Des oni a la surface, des samourais squelettes a Yomi, des yurei a Takamagahara.
            let kind = match self.dim {
                1 => mk::SKELETON,
                2 => mk::SHADE,
                _ => mk::ZOMBIE,
            };
            let crowd = self.mobs.iter().filter(|m| m.kind == kind && m.pos.distance(at) < 9.0).count();
            if watched && crowd < 4 && self.rng.chance(0.15) {
                let pos = at + Vec3::new(self.rng.range(-3, 4) as f64, 0.0, self.rng.range(-3, 4) as f64);
                if self.fits(kind, pos) {
                    self.spawn_mob(kind, pos, 0.0, mk::max_hp(kind));
                }
            }
        }
    }

    pub(crate) fn spawn_cycle(&mut self) {
        // Une partie peut couper la naissance naturelle des creatures (une arene, un Bed Wars).
        if !self.spawning {
            return;
        }
        self.settle_villagers();
        self.spawn_from_steles();
        let day = self.is_day();
        let players: Vec<Vec3> = self
            .clients
            .values()
            .filter(|c| c.ready && !c.dead && c.dim == self.dim)
            .map(|c| Vec3::from(c.pos))
            .collect();
        match self.dim {
            1 => return self.spawn_abyss(&players),
            2 => return self.spawn_zenith(&players),
            _ => {}
        }
        for p in players {
            let count = |hostile: bool, mobs: &[Mob]| {
                mobs.iter().filter(|m| mk::hostile(m.kind) == hostile && m.pos.distance(p) < 64.0).count()
            };
            if count(true, &self.mobs) < 12 {
                for _ in 0..3 {
                    let a = self.rng.d() * std::f64::consts::TAU;
                    let d = 20.0 + self.rng.d() * 24.0;
                    let (x, z) = ((p.x + a.sin() * d).floor() as i32, (p.z + a.cos() * d).floor() as i32);
                    // De nuit a la surface, a toute heure dans les grottes.
                    let y = if !day && self.rng.chance(0.5) {
                        self.top_solid(x, z).map(|t| t + 1)
                    } else {
                        let start = self.rng.range(6, 52);
                        (start - 8..start).rev().find(|&y| {
                            self.world.get([x, y, z]).is_some_and(|(b, _)| b == AIR)
                                && self.world.get([x, y - 1, z]).is_some_and(|(b, m)| blocks::solid(b, m))
                        })
                    };
                    let Some(y) = y else { continue };
                    let pos = Vec3::new(x as f64 + 0.5, y as f64, z as f64 + 0.5);
                    if day && self.sky_above(pos) {
                        continue;
                    }
                    let roll = self.rng.range(0, 100);
                    let kind = match roll {
                        0..=34 => mk::ZOMBIE,
                        35..=59 => mk::SKELETON,
                        60..=79 => mk::SPIDER,
                        80..=94 => mk::BOMBYX,
                        _ => mk::SHADE,
                    };
                    if self.fits(kind, pos) && !self.lit([x, y, z]) {
                        self.spawn_mob(kind, pos, 0.0, mk::max_hp(kind));
                    }
                }
            }
            if day && count(false, &self.mobs) < 8 && self.rng.chance(0.2) {
                let a = self.rng.d() * std::f64::consts::TAU;
                let d = 16.0 + self.rng.d() * 28.0;
                let (x, z) = ((p.x + a.sin() * d).floor() as i32, (p.z + a.cos() * d).floor() as i32);
                let Some(top) = self.top_solid(x, z) else { continue };
                if self.world.get([x, top, z]).is_none_or(|(b, _)| b != GRASS) {
                    continue;
                }
                let kind = self.rng.range(0, 4) as u8;
                for _ in 0..self.rng.range(2, 4) {
                    let (ox, oz) = (self.rng.range(-2, 3), self.rng.range(-2, 3));
                    let Some(t) = self.top_solid(x + ox, z + oz) else { continue };
                    let pos = Vec3::new((x + ox) as f64 + 0.5, t as f64 + 1.0, (z + oz) as f64 + 0.5);
                    if self.fits(kind, pos) {
                        self.spawn_mob(kind, pos, 0.0, mk::max_hp(kind));
                    }
                }
            }
        }
    }

    /// Abime : Braisiers dans les airs, morts-vivants et squelettes au sol.
    fn spawn_abyss(&mut self, players: &[Vec3]) {
        for p in players {
            // Rythme modere : l'Abime reste dangereux sans submerger le joueur des son arrivee.
            if self.mobs.iter().filter(|m| m.pos.distance(*p) < 64.0).count() >= 6 || !self.rng.chance(0.3) {
                continue;
            }
            for _ in 0..1 {
                let a = self.rng.d() * std::f64::consts::TAU;
                let d = 12.0 + self.rng.d() * 24.0;
                let (x, z) = ((p.x + a.sin() * d).floor() as i32, (p.z + a.cos() * d).floor() as i32);
                let air = |w: &World, y: i32| w.get([x, y, z]).is_some_and(|(b, _)| b == AIR);
                let start = self.rng.range(36, 108);
                let Some(y) = (start - 12..start).rev().find(|&y| air(&self.world, y) && air(&self.world, y + 1)) else {
                    continue;
                };
                let flyer = self.rng.chance(0.5);
                let grounded = self.world.get([x, y - 1, z]).is_some_and(|(b, m)| blocks::solid(b, m));
                let kind = match (flyer, grounded) {
                    (true, _) => mk::BRAISIER,
                    // Chaque contree a ses habitants : oni du bois pourpre, morts en armes et yurei de la vallee des ames.
                    (false, true) => match gen::yomi_biome(self.world.seed, x, z) {
                        gen::Yomi::Crimson => mk::ZOMBIE,
                        gen::Yomi::Souls => {
                            if self.rng.chance(0.7) {
                                mk::SKELETON
                            } else {
                                mk::SHADE
                            }
                        }
                        gen::Yomi::Basalt => continue,
                        gen::Yomi::Wastes => {
                            if self.rng.chance(0.6) {
                                mk::ZOMBIE
                            } else {
                                mk::SKELETON
                            }
                        }
                    },
                    _ => continue,
                };
                let pos = Vec3::new(x as f64 + 0.5, y as f64, z as f64 + 0.5);
                if self.fits(kind, pos) {
                    self.spawn_mob(kind, pos, 0.0, mk::max_hp(kind));
                }
            }
        }
    }

    /// Zenith : le Wyrm tant qu'il n'a pas ete vaincu, et des Ombres sur l'ile.
    fn spawn_zenith(&mut self, players: &[Vec3]) {
        if players.is_empty() {
            return;
        }
        if !self.world.boss_dead && !self.mobs.iter().any(|m| m.kind == mk::WYRM) {
            self.spawn_mob(mk::WYRM, Vec3::new(42.0, 95.0, 0.0), 0.0, mk::WYRM_HP);
            self.broadcast(ServerMsg::Info { text: "Ryujin, le dragon de Takamagahara, s'eveille...".to_string() }, None);
        }
        // Tant qu'un cristal brille sur son pilier, le dragon se soigne : 2 points par seconde.
        let lit = gen::zenith_crystals(self.world.seed).iter().any(|p| self.world.get(*p).is_some_and(|x| x.0 == CRYSTAL));
        if let Some(m) = self.mobs.iter_mut().find(|m| m.kind == mk::WYRM && m.hp > 0.0 && m.hp < mk::WYRM_HP) {
            if lit {
                m.hp = (m.hp + 2.0).min(mk::WYRM_HP);
            }
        }
        let hp = self.mobs.iter().find(|m| m.kind == mk::WYRM).map(|m| (m.hp / mk::WYRM_HP).clamp(0.0, 1.0));
        self.broadcast(ServerMsg::Boss { hp }, None);
        for p in players {
            if self.mobs.iter().filter(|m| m.kind == mk::SHADE && m.pos.distance(*p) < 64.0).count() >= 4 {
                continue;
            }
            let a = self.rng.d() * std::f64::consts::TAU;
            let d = 12.0 + self.rng.d() * 20.0;
            let (x, z) = ((p.x + a.sin() * d).floor() as i32, (p.z + a.cos() * d).floor() as i32);
            let Some(top) = self.top_solid(x, z) else { continue };
            let pos = Vec3::new(x as f64 + 0.5, top as f64 + 1.0, z as f64 + 0.5);
            if self.rng.chance(0.3) && self.fits(mk::SHADE, pos) {
                self.spawn_mob(mk::SHADE, pos, 0.0, mk::max_hp(mk::SHADE));
            }
        }
    }
}
