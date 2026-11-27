//! Villageois et commerce.
//!
//! Chaque maison de village abrite un villageois, ne avec le chunk ; son metier tient dans son age
//! (0 a 4), ce qui le conserve d'une session a l'autre sans changer le format de sauvegarde.
//! Un clic droit ouvre ses six offres ; la monnaie est le koban.
use super::*;
use shared::mobs as mk;
use shared::trade;

/// Distance au-dela de laquelle l'echange s'interrompt.
const TRADE_REACH: f64 = 8.0;

impl Server {
    /// Fait naitre les habitants des maisons que la generation vient de poser.
    pub(crate) fn settle_villagers(&mut self) {
        for bed in std::mem::take(&mut self.world.new_homes) {
            // Le futon est dans un coin : le villageois apparait au milieu de la piece.
            let pos = Vec3::new(bed[0] as f64 - 1.5, bed[1] as f64, bed[2] as f64 - 1.5);
            let profession = shared::util::hash(bed[0], bed[1], bed[2], self.world.seed) % 5;
            self.spawn_mob(mk::VILLAGER, pos, profession as f64, mk::max_hp(mk::VILLAGER));
        }
    }

    /// Ouvre la fenetre d'echange avec un villageois.
    pub(crate) fn open_trade(&mut self, id: u32, mob: u32, profession: u8) {
        self.open_window(id, Win::Trade(mob));
        if let Some(c) = self.clients.get(&id) {
            let offers = trade::offers(profession).into_iter().map(|o| (o.cost, o.out)).collect();
            c.send(ServerMsg::Trades { title: trade::profession_name(profession).to_string(), offers });
        }
    }

    /// Le villageois avec qui ce joueur commerce est-il toujours la, a portee ?
    pub(crate) fn trade_partner(&self, c: &Client) -> Option<u8> {
        let Win::Trade(mob) = c.window else { return None };
        let m = self.mobs.iter().find(|m| m.id == mob && m.kind == mk::VILLAGER && m.hp > 0.0)?;
        (c.dim == self.dim && Vec3::from(c.pos).distance(m.pos) <= TRADE_REACH).then_some(m.age.max(0.0) as u8)
    }

    /// Conclut l'offre numero `index` si le joueur a de quoi payer.
    pub(crate) fn trade(&mut self, id: u32, index: u8) {
        let Some(c) = self.clients.get(&id) else { return };
        let Some(profession) = self.trade_partner(c) else { return };
        let Some(offer) = trade::offers(profession).get(index as usize).copied() else { return };
        let Some(c) = self.clients.get_mut(&id) else { return };
        if items::count(&c.inv, offer.cost.id) < offer.cost.n as u32 {
            return;
        }
        items::remove(&mut c.inv, offer.cost.id, offer.cost.n as u32);
        c.inv_dirty = true;
        self.give(id, offer.out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::blocks::*;
    use shared::structures::{self as st, Kind};

    const SEED: u32 = 20251;

    fn find(kind: Kind) -> st::Site {
        (0..60)
            .flat_map(|r: i32| (-r..=r).flat_map(move |a| [(a, -r), (a, r), (-r, a), (r, a)]))
            .find_map(|(rx, rz)| st::site(SEED, rx, rz).filter(|s| s.kind == kind))
            .unwrap()
    }

    /// Serveur dont les chunks autour du site sont charges.
    fn around(s: &st::Site) -> Server {
        let mut srv = Server::new(SEED, None);
        for cz in (s.z - 40).div_euclid(16)..=(s.z + 40).div_euclid(16) {
            for cx in (s.x - 40).div_euclid(16)..=(s.x + 40).div_euclid(16) {
                srv.world.ensure((cx, cz));
            }
        }
        srv.world.updates.clear();
        srv
    }

    fn player(srv: &mut Server, pos: [f64; 3]) -> Receiver<ServerMsg> {
        let (tx, rx) = mpsc::channel();
        let mut c = Client::new(tx, pos);
        c.ready = true;
        srv.clients.insert(1, c);
        rx
    }

    #[test]
    fn villagers_are_born_once_and_trade() {
        let s = find(Kind::Village);
        let mut srv = around(&s);
        let houses = st::lots(SEED, &s).iter().filter(|l| matches!(l.0, st::Lot::House(_))).count();
        assert_eq!(srv.world.new_homes.len(), houses);
        srv.settle_villagers();
        let folk: Vec<(u32, Vec3, u8)> = srv.mobs.iter().filter(|m| m.kind == mk::VILLAGER).map(|m| (m.id, m.pos, m.age as u8)).collect();
        assert_eq!(folk.len(), houses, "un habitant par maison");
        assert!(folk.iter().all(|f| f.2 < 5));
        // Les coffres des maisons sont garnis du butin des maisons, pas de celui des mines.
        let home = st::lots(SEED, &s).into_iter().find(|l| matches!(l.0, st::Lot::House(_))).unwrap();
        let chest = [home.1 - 2, home.3 + 1, home.2 + 2];
        assert_eq!(srv.world.chests[&chest], shared::gen::chest_loot(0, SEED, chest, st::LOOT_HOUSE));
        // Recharger le chunk ne fait pas renaitre les habitants : il est marque comme modifie.
        let key = ((home.1 + 2).div_euclid(16), (home.2 + 2).div_euclid(16));
        assert!(srv.world.chunks[&key].modified);

        // Commerce : on arrive devant un villageois, on ouvre, on vend, on achete.
        let (mob, at, profession) = folk[0];
        let rx = player(&mut srv, [at.x + 1.0, at.y, at.z]);
        srv.interact(1, mob);
        assert_eq!(srv.clients[&1].window, Win::Trade(mob));
        let offers = rx
            .try_iter()
            .find_map(|m| match m {
                ServerMsg::Trades { offers, .. } => Some(offers),
                _ => None,
            })
            .expect("les offres sont envoyees");
        assert_eq!(offers.len(), 6);
        let list = trade::offers(profession);
        let (buy, sell) = (list[0], list[3]);
        // Pas assez : rien ne se passe.
        srv.clients.get_mut(&1).unwrap().inv[0] = Some(Stack::new(buy.cost.id, buy.cost.n - 1));
        srv.trade(1, 0);
        assert_eq!(items::count(&srv.clients[&1].inv, items::KOBAN), 0);
        // Le compte y est : un koban.
        srv.clients.get_mut(&1).unwrap().inv[0] = Some(Stack::new(buy.cost.id, buy.cost.n));
        srv.trade(1, 0);
        assert_eq!(items::count(&srv.clients[&1].inv, items::KOBAN), 1);
        assert_eq!(items::count(&srv.clients[&1].inv, buy.cost.id), 0);
        // Et ce koban achete la marchandise du metier (si elle ne coute qu'une piece).
        srv.clients.get_mut(&1).unwrap().inv[1] = Some(Stack::new(items::KOBAN, sell.cost.n));
        let before = items::count(&srv.clients[&1].inv, items::KOBAN);
        srv.trade(1, 3);
        assert_eq!(items::count(&srv.clients[&1].inv, items::KOBAN), before - sell.cost.n as u32);
        assert_eq!(items::count(&srv.clients[&1].inv, sell.out.id), sell.out.n as u32);
        // Trop loin, l'echange ne se fait plus.
        srv.clients.get_mut(&1).unwrap().pos = [at.x + 30.0, at.y, at.z];
        srv.clients.get_mut(&1).unwrap().inv[2] = Some(Stack::new(buy.cost.id, buy.cost.n));
        srv.trade(1, 0);
        assert_eq!(items::count(&srv.clients[&1].inv, buy.cost.id), buy.cost.n as u32);
    }

    #[test]
    fn temple_trap_shoots_and_tomb_trap_explodes() {
        // Temple : franchir la corde fait tirer les deux distributeurs.
        let s = find(Kind::Temple);
        let mut srv = around(&s);
        let droppers = [[s.x - 3, s.y + 2, s.z - 2], [s.x + 3, s.y + 2, s.z - 2]];
        for d in droppers {
            assert_eq!(items::count(&srv.world.chests[&d], items::ARROW), 16, "distributeur charge");
        }
        for _ in 0..6 {
            srv.tick_signal();
        }
        assert!(srv.projs.is_empty(), "au repos, rien ne part");
        let _rx = player(&mut srv, [s.x as f64 + 0.5, s.y as f64 + 1.0, s.z as f64 - 1.5]);
        for _ in 0..8 {
            srv.tick_signal();
        }
        assert_eq!(srv.projs.len(), 2, "une fleche de chaque cote");
        for d in droppers {
            assert_eq!(items::count(&srv.world.chests[&d], items::ARROW), 15);
        }
        // Les fleches traversent la salle a hauteur de poitrine et touchent l'intrus.
        for _ in 0..20 {
            srv.tick_projs();
        }
        assert!(srv.clients[&1].hp < 20.0, "le piege blesse : {}", srv.clients[&1].hp);

        // Tombe : marcher sur la dalle amorce la charge, qui emporte les coffres.
        let s = find(Kind::Kofun);
        let mut srv = around(&s);
        let chest = [s.x + 2, s.y + 1, s.z + 2];
        assert_eq!(srv.world.get(chest).map(|x| x.0), Some(CHEST));
        assert!(srv.world.chests[&chest].iter().flatten().count() >= 1);
        let _rx = player(&mut srv, [s.x as f64 + 0.5, s.y as f64 + 1.0, s.z as f64 + 0.5]);
        for _ in 0..4 {
            srv.tick_signal();
        }
        assert_eq!(srv.world.get([s.x, s.y - 1, s.z]), Some((EXPLOSIVE, 1)), "amorcee");
        for _ in 0..(5.0 / DT) as usize {
            srv.tick_primed();
        }
        assert_eq!(srv.world.get([s.x, s.y - 1, s.z]).map(|x| x.0), Some(AIR));
        assert_eq!(srv.world.get(chest).map(|x| x.0), Some(AIR), "le souffle detruit le tresor");
    }
}
