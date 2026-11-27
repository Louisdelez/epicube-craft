//! Chat et commandes.
//!
//! Une ligne recue d'un joueur est soit un message, diffuse a tous, soit une commande (elle commence
//! par « / »), executee ici. La table des commandes et l'analyse des mots sont dans `shared::commands`,
//! pour que le client propose exactement ce que le serveur comprend.
use super::*;
use shared::commands::{self as cmd, Arg};
use shared::proto::chat;
use shared::rules;

impl Server {
    /// Ecrit une ligne dans le chat d'un joueur.
    pub(crate) fn tell(&self, id: u32, text: &str, kind: u8) {
        if let Some(c) = self.clients.get(&id) {
            c.send(ServerMsg::Chat { text: text.to_string(), kind });
        }
    }

    /// Ecrit une ligne dans le chat de tout le monde, quelle que soit la dimension.
    pub(crate) fn announce(&self, text: &str, kind: u8) {
        self.broadcast_all(ServerMsg::Chat { text: text.to_string(), kind }, None);
    }

    fn player_named(&self, name: &str) -> Option<u32> {
        let name = cmd::plain(name);
        self.clients.iter().find(|(_, c)| c.ready && cmd::plain(&c.name) == name).map(|(id, _)| *id)
    }

    /// Joueur vise par une commande : celui qui est nomme, ou celui qui parle.
    fn target(&self, id: u32, word: Option<&&str>) -> Result<u32, String> {
        match word {
            Some(w) => self.player_named(w).ok_or_else(|| format!("Aucun joueur ne s'appelle « {w} »")),
            None => Ok(id),
        }
    }

    /// Voix d'un joueur : relayee a ceux qui sont assez pres pour l'entendre, dans le meme monde.
    /// Un spectateur ne se fait entendre que des autres spectateurs.
    pub(crate) fn voice(&mut self, id: u32, data: Vec<u8>) {
        use shared::voice;
        let Some(c) = self.clients.get_mut(&id) else { return };
        if data.is_empty() || data.len() > voice::MAX_PACKET || c.dead {
            return;
        }
        // Debit borne : un client ne peut pas noyer les autres sous les envois.
        let now = Instant::now();
        if now.duration_since(c.voice_t).as_secs_f64() >= 1.0 {
            (c.voice_t, c.voice_n) = (now, 0);
        }
        c.voice_n += 1;
        if c.voice_n > voice::MAX_RATE {
            return;
        }
        let (pos, dim, ghost) = (c.pos, c.dim, c.mode == rules::SPECTATOR);
        for (oid, o) in &self.clients {
            if *oid != id && o.ready && o.dim == dim && voice::hears(pos, o.pos) && (!ghost || o.mode == rules::SPECTATOR) {
                o.send(ServerMsg::Voice { id, data: data.clone() });
            }
        }
    }

    /// Une ligne tapee dans le chat.
    pub(crate) fn chat(&mut self, id: u32, text: String) {
        let text: String = text.chars().filter(|c| !c.is_control()).take(cmd::MAX_LEN).collect();
        let text = text.trim();
        let Some(name) = self.clients.get(&id).map(|c| c.name.clone()) else { return };
        if text.is_empty() {
            return;
        }
        let Some(line) = text.strip_prefix('/') else {
            println!("<{name}> {text}");
            return self.announce(&format!("<{name}> {text}"), chat::SAY);
        };
        match self.command(id, &name, line) {
            Ok(reply) if reply.is_empty() => {}
            Ok(reply) => self.tell(id, &reply, chat::INFO),
            Err(why) => self.tell(id, &why, chat::ERROR),
        }
    }

    /// Execute une commande (sans son « / »). Renvoie ce qu'il faut repondre a son auteur.
    fn command(&mut self, id: u32, name: &str, line: &str) -> Result<String, String> {
        let words: Vec<&str> = line.split_whitespace().collect();
        let Some(first) = words.first() else { return Err("Commande vide. Tapez /aide.".into()) };
        let Some(spec) = cmd::find(first) else {
            return Err(format!("Commande inconnue : /{first}. Tapez /aide."));
        };
        if spec.cheat && !self.rules.cheats {
            return Err(format!("/{} est réservée aux mondes où la triche est autorisée", spec.name));
        }
        let args = &words[1..];
        let required = spec.args.iter().filter(|a| a.1).count();
        // Un lieu se donne en un mot (un joueur) ou en trois (des coordonnees).
        if args.len() < required {
            return Err(format!("Il manque quelque chose : {}", cmd::usage(spec)));
        }
        let rest = |from: usize| -> String {
            line.split_whitespace().skip(1 + from).collect::<Vec<_>>().join(" ")
        };
        match spec.name {
            "aide" => match args.first() {
                Some(w) => cmd::find(w).map(|s| format!("{} - {}", cmd::usage(s), s.help)).ok_or_else(|| format!("Commande inconnue : /{w}")),
                None => {
                    let list: Vec<String> = cmd::COMMANDS.iter().filter(|c| self.rules.cheats || !c.cheat).map(|c| format!("/{}", c.name)).collect();
                    Ok(format!("Commandes : {}. /aide <commande> pour le détail.", list.join(" ")))
                }
            },
            "mode" => {
                let mode = cmd::index_of(&cmd::MODES, args[0]).ok_or_else(|| format!("Mode inconnu : {}. Choix : {}", args[0], cmd::MODES.join(", ")))?;
                if self.rules.hardcore {
                    return Err("Mode extrême : on ne change pas de mode de jeu".into());
                }
                let who = self.target(id, args.get(1))?;
                self.set_mode(who, mode);
                let whom = self.clients.get(&who).map(|c| c.name.clone()).unwrap_or_default();
                Ok(if who == id { String::new() } else { format!("{whom} passe en mode {}", rules::mode_name(mode).to_lowercase()) })
            }
            "difficulte" => {
                let d = cmd::index_of(&cmd::DIFFICULTIES, args[0]).ok_or_else(|| format!("Difficulté inconnue : {}. Choix : {}", args[0], cmd::DIFFICULTIES.join(", ")))?;
                if self.rules.hardcore {
                    return Err("Mode extrême : la difficulté est verrouillée".into());
                }
                self.rules.difficulty = d;
                self.rules_changed();
                self.announce(&format!("Difficulté : {}", rules::difficulty_name(d).to_lowercase()), chat::INFO);
                Ok(String::new())
            }
            "heure" => {
                let hour = cmd::hour_from(args[0]).ok_or_else(|| format!("Heure inconnue : {}. Un moment (aube, jour, midi, crepuscule, nuit, minuit) ou une heure de 0 à 24.", args[0]))?;
                // L'heure du monde est une fraction de journee qui commence a six heures.
                self.time = ((hour - 6.0) / 24.0).rem_euclid(1.0);
                self.world.time = self.time;
                self.broadcast_all(ServerMsg::Time { time: self.time }, None);
                Ok(format!("Il est {:02}h{:02}", hour.floor() as u32 % 24, ((hour.fract() * 60.0).round() as u32).min(59)))
            }
            "tp" => {
                let to: [f64; 3] = if args.len() >= 3 {
                    let mut p = [0.0; 3];
                    let here = self.clients.get(&id).map(|c| c.pos).unwrap_or_default();
                    for (k, w) in args[..3].iter().enumerate() {
                        // « ~ » : la coordonnee actuelle, « ~5 » : cinq blocs plus loin.
                        p[k] = match w.strip_prefix('~') {
                            Some("") => here[k],
                            Some(d) => here[k] + d.parse::<f64>().map_err(|_| format!("Nombre invalide : {w}"))?,
                            None => w.parse::<f64>().map_err(|_| format!("Nombre invalide : {w}"))?,
                        };
                    }
                    let b = shared::WORLD_BORDER - 1.0;
                    if !p.iter().all(|v| v.is_finite()) || p[0].abs() > b || p[2].abs() > b || !(-64.0..=1024.0).contains(&p[1]) {
                        return Err("Ces coordonnées sont hors du monde".into());
                    }
                    p
                } else {
                    let other = self.player_named(args[0]).ok_or_else(|| format!("Aucun joueur ne s'appelle « {} »", args[0]))?;
                    let (dim, mine) = (self.clients[&other].dim, self.clients[&id].dim);
                    if dim != mine {
                        return Err("Ce joueur est dans un autre monde".into());
                    }
                    self.clients[&other].pos
                };
                let c = self.clients.get_mut(&id).ok_or("Joueur introuvable")?;
                (c.pos, c.prev, c.fall_peak, c.moved) = (to, to, to[1], true);
                c.send(ServerMsg::Teleport { pos: to });
                Ok(format!("Téléporté en {:.1} {:.1} {:.1}", to[0], to[1], to[2]))
            }
            "donner" => {
                let item = cmd::item_from(args[0]).ok_or_else(|| format!("Objet inconnu : {}", args[0]))?;
                let n = match args.get(1) {
                    Some(w) => w.parse::<u32>().ok().filter(|n| (1..=2304).contains(n)).ok_or_else(|| format!("Quantité invalide : {w} (de 1 à 2304)"))?,
                    None => 1,
                };
                let who = self.target(id, args.get(2))?;
                let per = items::max_stack(item) as u32;
                let mut left = n;
                while left > 0 {
                    let take = left.min(per);
                    self.give(who, Stack::new(item, take as u8));
                    left -= take;
                }
                let whom = self.clients.get(&who).map(|c| c.name.clone()).unwrap_or_default();
                Ok(format!("{n} x {} pour {whom}", items::name(item)))
            }
            "vider" => {
                let who = self.target(id, args.first())?;
                let c = self.clients.get_mut(&who).ok_or("Joueur introuvable")?;
                let n = c.inv.iter().chain(c.armor.iter()).flatten().count();
                (c.inv, c.armor, c.inv_dirty) = ([None; 36], [None; 4], true);
                Ok(format!("{n} piles retirées de l'inventaire de {}", c.name))
            }
            "tuer" => {
                let who = self.target(id, args.first())?;
                self.kill(who);
                Ok(String::new())
            }
            "invoquer" => {
                let kind = cmd::mob_from(args[0]).ok_or_else(|| format!("Créature inconnue : {}. Choix : {}", args[0], cmd::MOBS.map(|m| m.0).join(", ")))?;
                if self.rules.difficulty == rules::PEACEFUL && shared::mobs::hostile(kind) && kind != shared::mobs::WYRM {
                    return Err("En paisible, les créatures hostiles ne viennent pas".into());
                }
                let c = self.clients.get(&id).ok_or("Joueur introuvable")?;
                // Deux blocs devant soi, a sa hauteur.
                let at = Vec3::from(c.pos) + Vec3::new(c.yaw.sin(), 0.0, c.yaw.cos()) * 2.0 + Vec3::Y * 0.2;
                self.spawn_mob(kind, at, 1.0, shared::mobs::max_hp(kind));
                Ok(format!("Un(e) {} apparaît", args[0]))
            }
            "apparition" => {
                let c = self.clients.get_mut(&id).ok_or("Joueur introuvable")?;
                if c.dim != 0 {
                    return Err("On ne réapparaît qu'à la surface".into());
                }
                c.home = Some(c.pos);
                Ok("Point de réapparition fixé ici".into())
            }
            "graine" => Ok(format!("Graine du monde : {}", self.world.seed)),
            "liste" => {
                let mut names: Vec<&str> = self.clients.values().filter(|c| c.ready).map(|c| c.name.as_str()).collect();
                names.sort_unstable();
                Ok(format!("{} joueur(s) : {}", names.len(), names.join(", ")))
            }
            "dire" => {
                self.announce(&format!("[{name}] {}", rest(0)), chat::INFO);
                Ok(String::new())
            }
            "moi" => {
                self.announce(&format!("* {name} {}", rest(0)), chat::SAY);
                Ok(String::new())
            }
            "msg" => {
                let who = self.player_named(args[0]).ok_or_else(|| format!("Aucun joueur ne s'appelle « {} »", args[0]))?;
                let text = rest(1);
                let whom = self.clients[&who].name.clone();
                self.tell(who, &format!("{name} vous chuchote : {text}"), chat::WHISPER);
                self.tell(id, &format!("Vous chuchotez à {whom} : {text}"), chat::WHISPER);
                Ok(String::new())
            }
            _ => {
                // Une commande de la table sans code ici : le test `every_command_runs` l'interdit.
                let _ = Arg::Text;
                Err(format!("/{} n'est pas encore disponible", spec.name))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> (Server, Receiver<ServerMsg>) {
        let mut srv = Server::new(7, None);
        srv.world.ensure((0, 0));
        let (tx, rx) = mpsc::channel();
        let mut c = Client::new(tx, [8.5, 300.0, 8.5]);
        c.ready = true;
        c.name = "Aiko".into();
        srv.clients.insert(1, c);
        let (tx2, _rx2) = mpsc::channel();
        let mut c = Client::new(tx2, [20.5, 310.0, 4.5]);
        c.ready = true;
        c.name = "Ren".into();
        srv.clients.insert(2, c);
        std::mem::forget(_rx2);
        (srv, rx)
    }

    /// Lignes de chat recues depuis le dernier appel.
    fn lines(rx: &Receiver<ServerMsg>) -> Vec<(String, u8)> {
        rx.try_iter()
            .filter_map(|m| match m {
                ServerMsg::Chat { text, kind } => Some((text, kind)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn messages_reach_everyone_and_whispers_only_one() {
        let (mut srv, rx) = server();
        srv.chat(1, "  bonjour tout le monde  ".into());
        assert_eq!(lines(&rx), [("<Aiko> bonjour tout le monde".to_string(), chat::SAY)]);
        srv.chat(1, "   ".into());
        assert!(lines(&rx).is_empty(), "une ligne vide ne s'envoie pas");
        srv.chat(1, "/moi salue".into());
        assert_eq!(lines(&rx)[0].0, "* Aiko salue");
        srv.chat(1, "/msg ren tu es la ?".into());
        assert_eq!(lines(&rx), [("Vous chuchotez à Ren : tu es la ?".to_string(), chat::WHISPER)]);
        srv.chat(1, "/msg personne coucou".into());
        assert_eq!(lines(&rx)[0].1, chat::ERROR);
        // Un message trop long est coupe, les caracteres de controle retires.
        srv.chat(1, format!("a\u{7}{}", "b".repeat(400)));
        assert_eq!(lines(&rx)[0].0.chars().count(), "<Aiko> ".len() + cmd::MAX_LEN);
    }

    #[test]
    fn voice_reaches_only_those_nearby() {
        let mut srv = Server::new(7, None);
        let mut ears = Vec::new();
        // Aiko parle ; Ren est a 20 blocs, Sora a 60, Yui tout pres mais dans un autre monde.
        for (id, pos, dim) in [(1, [0.0, 100.0, 0.0], 0), (2, [20.0, 100.0, 0.0], 0), (3, [60.0, 100.0, 0.0], 0), (4, [1.0, 100.0, 0.0], 1)] {
            let (tx, rx) = mpsc::channel();
            let mut c = Client::new(tx, pos);
            (c.ready, c.dim, c.name) = (true, dim, format!("J{id}"));
            srv.clients.insert(id, c);
            ears.push(rx);
        }
        let heard = |ears: &[Receiver<ServerMsg>]| -> Vec<usize> {
            ears.iter().map(|rx| rx.try_iter().filter(|m| matches!(m, ServerMsg::Voice { id: 1, .. })).count()).collect()
        };
        let frame = vec![128u8; shared::voice::FRAME];
        srv.voice(1, frame.clone());
        assert_eq!(heard(&ears), [0, 1, 0, 0], "seul le voisin entend, et l'on ne s'entend pas soi-meme");
        // Envois vides ou trop gros : ignores.
        srv.voice(1, Vec::new());
        srv.voice(1, vec![0; shared::voice::MAX_PACKET + 1]);
        assert_eq!(heard(&ears), [0, 0, 0, 0]);
        // Un spectateur ne trouble pas les vivants.
        srv.clients.get_mut(&1).unwrap().mode = rules::SPECTATOR;
        srv.voice(1, frame.clone());
        assert_eq!(heard(&ears), [0, 0, 0, 0]);
        srv.clients.get_mut(&2).unwrap().mode = rules::SPECTATOR;
        srv.voice(1, frame.clone());
        assert_eq!(heard(&ears), [0, 1, 0, 0]);
        // Debit : au-dela de la limite par seconde, le surplus est jete.
        for _ in 0..200 {
            srv.voice(1, frame.clone());
        }
        let n = heard(&ears)[1];
        assert!(n < shared::voice::MAX_RATE as usize && n > 40, "{n} envois relayes");
    }

    #[test]
    fn cheat_commands_need_permission() {
        let (mut srv, rx) = server();
        srv.chat(1, "/mode creatif".into());
        assert_eq!(lines(&rx)[0].1, chat::ERROR);
        assert_eq!(srv.clients[&1].mode, rules::SURVIVAL);
        // Les commandes libres passent, et l'aide ne montre que celles-la.
        srv.chat(1, "/liste".into());
        assert_eq!(lines(&rx), [("2 joueur(s) : Aiko, Ren".to_string(), chat::INFO)]);
        srv.chat(1, "/aide".into());
        let help = &lines(&rx)[0].0;
        assert!(help.contains("/graine") && !help.contains("/donner"));
        srv.chat(1, "/abracadabra".into());
        assert!(lines(&rx)[0].0.contains("inconnue"));
    }

    #[test]
    fn commands_do_what_they_say() {
        let (mut srv, rx) = server();
        srv.rules.cheats = true;
        // Mode de jeu, par nom, par alias anglais ou par debut de mot.
        srv.chat(1, "/gamemode creatif".into());
        assert_eq!(srv.clients[&1].mode, rules::CREATIVE);
        srv.chat(1, "/mode spec Ren".into());
        assert_eq!(srv.clients[&2].mode, rules::SPECTATOR);
        srv.chat(1, "/mode volant".into());
        assert_eq!(srv.clients[&1].mode, rules::CREATIVE, "un mode inconnu ne change rien");
        srv.chat(1, "/mode survie".into());
        let _ = lines(&rx);
        // Difficulte et heure.
        srv.chat(1, "/difficulte difficile".into());
        assert_eq!(srv.rules.difficulty, rules::HARD);
        srv.chat(1, "/heure midi".into());
        assert!((srv.time - 0.25).abs() < 1e-9);
        srv.chat(1, "/heure 99".into());
        assert!((srv.time - 0.25).abs() < 1e-9);
        let _ = lines(&rx);
        // Donner : par nom, avec quantite decoupee en piles, pour soi ou pour un autre.
        srv.chat(1, "/donner diamant 70".into());
        assert_eq!(items::count(&srv.clients[&1].inv, items::DIAMOND), 70);
        srv.chat(1, "/donner katana_en_fer 1 Ren".into());
        assert_eq!(items::count(&srv.clients[&2].inv, items::tool_id(items::SWORD, 2)), 1);
        srv.chat(1, "/donner licorne".into());
        srv.chat(1, "/donner diamant beaucoup".into());
        assert!(lines(&rx).iter().filter(|l| l.1 == chat::ERROR).count() == 2);
        srv.chat(1, "/vider".into());
        assert!(srv.clients[&1].inv.iter().all(|s| s.is_none()));
        // Teleportation : coordonnees, relatives ou non, ou vers un joueur.
        srv.chat(1, "/tp 100 200 -50".into());
        assert_eq!(srv.clients[&1].pos, [100.0, 200.0, -50.0]);
        srv.chat(1, "/tp ~ ~10 ~-5".into());
        assert_eq!(srv.clients[&1].pos, [100.0, 210.0, -55.0]);
        srv.chat(1, "/tp Ren".into());
        assert_eq!(srv.clients[&1].pos, srv.clients[&2].pos);
        srv.chat(1, "/tp 1e99 0 0".into());
        assert_eq!(srv.clients[&1].pos, srv.clients[&2].pos, "hors du monde : refuse");
        // Creature, point de reapparition, mort.
        let n = srv.mobs.len();
        srv.chat(1, "/invoquer tanuki".into());
        assert_eq!(srv.mobs.len(), n + 1);
        srv.chat(1, "/apparition".into());
        assert_eq!(srv.clients[&1].home, Some(srv.clients[&1].pos));
        srv.chat(1, "/tuer".into());
        assert!(srv.clients[&1].dead);
        // Argument manquant : le mode d'emploi est rappele.
        let _ = lines(&rx);
        srv.chat(1, "/donner".into());
        assert!(lines(&rx)[0].0.contains("/donner <objet>"));
    }

    /// Chaque commande de la table est executee par le serveur.
    #[test]
    fn every_command_runs() {
        for spec in cmd::COMMANDS {
            let (mut srv, rx) = server();
            srv.rules.cheats = true;
            let args = match spec.name {
                "mode" => " survie",
                "difficulte" => " facile",
                "heure" => " jour",
                "tp" => " 0 300 0",
                "donner" => " diamant",
                "invoquer" => " cerf",
                "dire" | "moi" => " bonjour",
                "msg" => " Ren bonjour",
                _ => "",
            };
            srv.chat(1, format!("/{}{args}", spec.name));
            let out = lines(&rx);
            assert!(out.iter().all(|l| l.1 != chat::ERROR && !l.0.contains("pas encore disponible")), "/{} : {out:?}", spec.name);
            // L'alias anglais mene a la meme commande.
            assert_eq!(cmd::find(spec.alias).map(|s| s.name), Some(spec.name));
        }
    }
}
