//! Rubis, côté serveur : un gros mod d'exemple, qui utilise tout ce que le jeu ouvre aux mods.
//!
//! - un minerai semé dans la roche du monde, qui laisse des gemmes ;
//! - une gemme, une pioche, une lame, une tarte et un bloc décoratif, avec leurs recettes ;
//! - une fleur lumineuse et un golem gardien, qui remplace un oni sur vingt ;
//! - une boutique, ouverte dans le jeu par la touche G (voir le script client) ;
//! - le compte des gemmes minées par chaque joueur, enregistré avec le monde.
use epicube_mod::server::*;

epicube_mod::start!(init);

/// Ce que vend la boutique : objet, quantité, prix en gemmes.
const SHOP: [(&str, u32, u32); 4] = [("rubis:tarte", 2, 1), ("rubis:fleur", 4, 2), ("rubis:pioche", 1, 6), ("rubis:lame", 1, 8)];

fn init() {
    // Blocs.
    blocks::register(
        Block::new("minerai").name("Minerai de rubis").hardness(3.0).tool(Tool::Pick).drop(Drop::Item("gemme".into())),
    );
    blocks::register(Block::new("rubis").name("Bloc de rubis").hardness(3.0).light(9).tool(Tool::Pick));
    blocks::register(Block::new("fleur").name("Fleur de rubis").hardness(0.0).light(6).plant(true));

    // Objets.
    let gemme = items::register(Item::new("gemme").name("Rubis"));
    items::register(Item::new("pioche").name("Pioche de rubis").tool(ToolKind::Pick, 3, 900));
    items::register(Item::new("lame").name("Lame de rubis").weapon(10.0, 600));
    items::register(Item::new("tarte").name("Tarte au rubis").stack(16).food(6.0, 5.0));

    // Recettes et cuisson.
    recipes::shaped("rubis", 1, &["GGG", "GGG", "GGG"], &[('G', "gemme")]);
    recipes::shaped("gemme", 9, &["B"], &[('B', "rubis")]);
    recipes::shaped("pioche", 1, &["GGG", " S ", " S "], &[('G', "gemme"), ('S', "baton")]);
    recipes::shaped("lame", 1, &["G", "G", "S"], &[('G', "gemme"), ('S', "baton")]);
    recipes::shaped("tarte", 2, &["GP", "O "], &[('G', "gemme"), ('P', "pomme"), ('O', "oeuf")]);
    recipes::smelt("minerai", "gemme");

    // Le monde : des filons dans la roche profonde.
    worldgen::ore(Ore::new("minerai").veins(5, 6).height(4, 90));

    // Une créature : le golem, un oni plus grand, plus solide, teinté de rouge.
    let golem = mobs::register(
        Mob::new("golem", "oni").name("Golem de rubis").health(60.0).damage(7.0).scale(1.4).color(1.0, 0.45, 0.5).rarity(0.05).loot("gemme", 2, 5),
    );

    on_join(|p| p.tell("Mod Rubis : touche G pour la boutique, /rubis pour ton compte."));

    // Chaque minerai cassé est compté ; le script client affiche le total.
    on_break(|p, _pos, block| {
        if block == "rubis:minerai" {
            let key = format!("mines:{}", p.name);
            let n = store::get(&key).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) + 1;
            store::set(&key, &n.to_string());
            p.send("mines", &n.to_string());
        }
        true
    });

    on_mob_death(move |mob, at| {
        if mob == golem {
            say("Un golem de rubis est tombé !");
            world::sound("rubis:achat", at[0], at[1], at[2], 1.0);
        }
    });

    // La boutique : le script client demande un article, le serveur vérifie et fait l'échange.
    let coin = gemme.clone();
    on_message(move |p, name, data| {
        if name != "acheter" {
            return;
        }
        // Un numéro d'article inconnu n'achète rien : il sert seulement à demander le contenu de la bourse.
        if let Some((item, n, price)) = data.parse::<usize>().ok().and_then(|i| SHOP.get(i)) {
            if p.take(&coin, *price) {
                p.give(item, *n);
                p.sound("rubis:achat", 1.0);
                p.send("achat", "ok");
            } else {
                p.send("achat", "non");
            }
        }
        p.send("bourse", &p.count(&coin).to_string());
    });

    command("rubis", "Dit combien de rubis tu as minés", |p, _args| {
        let n = store::get(&format!("mines:{}", p.name)).unwrap_or_else(|| "0".into());
        Some(format!("{} : {n} minerais cassés, {} rubis en poche", p.name, p.count("rubis:gemme")))
    });

    command("golem", "Fait apparaître un golem de rubis (triche)", |p, _args| {
        let [x, y, z] = p.pos()?;
        world::spawn("rubis:golem", x + 3.0, y + 1.0, z).then(|| "Un golem approche...".to_string())
    });
}
