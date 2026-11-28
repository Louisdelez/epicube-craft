//! Rubis, côté client : le compteur à l'écran et la fenêtre de la boutique (touche G).
use epicube_mod::client::*;
use std::cell::{Cell, RefCell};

epicube_mod::start!(init);

/// Les articles, dans le même ordre que côté serveur : objet, texte, prix en gemmes.
const SHOP: [(&str, &str, u32); 4] = [
    ("rubis:tarte", "2 tartes au rubis", 1),
    ("rubis:fleur", "4 fleurs de rubis", 2),
    ("rubis:pioche", "Pioche de rubis", 6),
    ("rubis:lame", "Lame de rubis", 8),
];
const WHITE: [f64; 4] = [1.0, 1.0, 1.0, 1.0];
const GREY: [f64; 4] = [0.72, 0.74, 0.8, 1.0];

thread_local! {
    static MINED: Cell<u32> = const { Cell::new(0) };
    static OPEN: Cell<bool> = const { Cell::new(false) };
    static PURSE: Cell<u32> = const { Cell::new(0) };
    static NOTE: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Rectangle d'un article dans la fenêtre : x, y, largeur, hauteur.
fn row(i: usize) -> (f64, f64, f64, f64) {
    let (w, h) = draw::screen();
    ((w - 220.0) / 2.0 + 8.0, (h - 150.0) / 2.0 + 30.0 + i as f64 * 24.0, 204.0, 20.0)
}

fn inside((x, y, w, h): (f64, f64, f64, f64), (mx, my): (f64, f64)) -> bool {
    mx >= x && mx < x + w && my >= y && my < y + h
}

fn init() {
    on_message(|name, data| match name {
        "mines" => MINED.with(|m| m.set(data.parse().unwrap_or(0))),
        "bourse" => PURSE.with(|p| p.set(data.parse().unwrap_or(0))),
        "achat" => NOTE.with(|n| *n.borrow_mut() = if data == "ok" { "Merci !".into() } else { "Pas assez de rubis.".into() }),
        _ => {}
    });

    on_key(|key| {
        if key == "KeyG" {
            let open = !OPEN.with(|o| o.get());
            OPEN.with(|o| o.set(open));
            NOTE.with(|n| n.borrow_mut().clear());
            if open {
                screen::open();
                // Un achat impossible ne coûte rien, et le serveur répond avec le contenu de la bourse.
                send("acheter", "-1");
                sound("clic", 0.6);
            } else {
                screen::close();
            }
        }
    });

    on_close(|| OPEN.with(|o| o.set(false)));

    on_click(|x, y, button| {
        if button != 0 {
            return;
        }
        if let Some(i) = (0..SHOP.len()).find(|i| inside(row(*i), (x, y))) {
            send("acheter", &i.to_string());
        }
    });

    on_hud(|| {
        let (w, h) = draw::screen();
        let mined = MINED.with(|m| m.get());
        if mined > 0 {
            let text = format!("Rubis minés : {mined}");
            let tw = draw::text_width(&text);
            draw::rect(w - tw - 26.0, 4.0, tw + 22.0, 14.0, [0.0, 0.0, 0.0, 0.5]);
            draw::image(w - tw - 24.0, 5.0, 12.0, 12.0, "gemme.png", WHITE);
            draw::text(w - tw - 8.0, 7.0, &text, [1.0, 0.45, 0.55, 1.0]);
        }
        if !OPEN.with(|o| o.get()) {
            return;
        }
        // La fenêtre : un panneau, son titre, la bourse, puis un article par ligne.
        let (x, y) = ((w - 220.0) / 2.0, (h - 150.0) / 2.0);
        draw::rect(x - 1.0, y - 1.0, 222.0, 152.0, [0.3, 0.07, 0.1, 1.0]);
        draw::rect(x, y, 220.0, 150.0, [0.09, 0.08, 0.12, 0.96]);
        draw::text(x + 8.0, y + 8.0, "Boutique du rubis", [1.0, 0.86, 0.36, 1.0]);
        let purse = format!("{} rubis", PURSE.with(|p| p.get()));
        draw::icon(x + 196.0 - draw::text_width(&purse) - 14.0, y + 5.0, 12.0, "rubis:gemme");
        draw::text(x + 212.0 - draw::text_width(&purse), y + 8.0, &purse, WHITE);
        let mouse = screen::mouse();
        for (i, (item, label, price)) in SHOP.iter().enumerate() {
            let r = row(i);
            let hot = inside(r, mouse);
            draw::rect(r.0, r.1, r.2, r.3, if hot { [0.85, 0.65, 0.16, 0.9] } else { [0.2, 0.18, 0.26, 0.9] });
            draw::icon(r.0 + 3.0, r.1 + 2.0, 16.0, item);
            draw::text(r.0 + 24.0, r.1 + 6.0, label, WHITE);
            let cost = format!("{price} rubis");
            draw::text(r.0 + r.2 - draw::text_width(&cost) - 6.0, r.1 + 6.0, &cost, if hot { WHITE } else { GREY });
        }
        NOTE.with(|n| draw::text(x + 8.0, y + 132.0, &n.borrow(), [0.6, 1.0, 0.6, 1.0]));
        draw::text(x + 212.0 - draw::text_width("G ou Échap : fermer"), y + 132.0, "G ou Échap : fermer", GREY);
    });
}
