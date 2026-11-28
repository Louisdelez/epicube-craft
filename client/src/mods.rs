//! Mods cote client : a la connexion, le serveur annonce ses mods ; ceux qui manquent sont telecharges
//! par la connexion du jeu, verifies, ranges dans le cache, puis leurs textures et leurs scripts
//! d'ecran sont charges. Le joueur n'a rien a installer.
use crate::atlas::{Img, TILE};
use crate::ui::Ui;
use shared::mods::{self as sm, Info, Pack, TileDef};
use shared::proto::{ClientMsg, ServerMsg};
use shared::script::{kind, op, Api, Script, Val};
use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::rc::Rc;

/// Temps accorde a un script d'ecran par evenement, en millisecondes.
const EVENT_MS: u64 = 20;
/// Dessins qu'un mod peut demander par image.
const MAX_DRAWS: usize = 2000;

enum Draw {
    /// Image d'un mod, etiree dans un rectangle.
    Image { x: f32, y: f32, w: f32, h: f32, tile: u16, color: [f32; 4] },
    /// Icone d'un objet ou d'un bloc, comme dans l'inventaire.
    Icon { x: f32, y: f32, size: f32, id: u16 },
    Text { x: f32, y: f32, text: String, color: [f32; 4] },
    Rect { x: f32, y: f32, w: f32, h: f32, color: [f32; 4] },
}

/// Ce que le script d'un mod voit du jeu, et ce qu'il lui demande.
#[derive(Default)]
struct State {
    listens: HashSet<String>,
    /// Messages a envoyer au script serveur du mod.
    out: Vec<(String, String)>,
    /// Lignes a ecrire dans le chat du joueur.
    chat: Vec<String>,
    draws: Vec<Draw>,
    /// Bruitages a jouer : (nom, volume).
    sounds: Vec<(String, f32)>,
    /// Images du mod : fichier -> tuile.
    images: std::collections::HashMap<String, u16>,
    /// Le mod affiche une fenetre : la souris est libre, le jeu ne recoit plus les gestes.
    screen_open: bool,
    /// Souris, en points d'interface.
    mouse: (f32, f32),
    pos: [f64; 3],
    /// Taille de l'ecran en points d'interface.
    screen: (f32, f32),
    errors: u32,
}

struct Running {
    id: String,
    state: Rc<RefCell<State>>,
    script: Script,
}

/// Les mods de la partie en cours.
pub struct Mods {
    infos: Vec<Info>,
    tiles: Vec<TileDef>,
    packs: Vec<Pack>,
    /// Mods a telecharger, puis celui en cours avec ce qui en est deja arrive.
    queue: VecDeque<Info>,
    current: Option<(Info, Vec<u8>)>,
    running: Vec<Running>,
    /// Tous les mods sont la, leurs scripts sont charges.
    pub ready: bool,
    /// Le carre des textures des mods, a ranger sous l'atlas du jeu. Pris une fois par le jeu.
    pub paint: Option<Img>,
    /// Modeles des creatures de mods qui ont le leur, par numero de creature.
    pub models: std::collections::HashMap<u8, crate::models::Model>,
}

fn num(a: &[Val], i: usize) -> Result<f64, String> {
    match a.get(i) {
        Some(Val::Num(n)) => Ok(*n),
        _ => Err(format!("valeur n° {} : un nombre est attendu", i + 1)),
    }
}

fn text(a: &[Val], i: usize) -> Result<&str, String> {
    match a.get(i) {
        Some(Val::Str(s)) => Ok(s),
        _ => Err(format!("valeur n° {} : un texte est attendu", i + 1)),
    }
}

fn color(a: &[Val], from: usize) -> [f32; 4] {
    let c = |i: usize, default: f64| num(a, from + i).unwrap_or(default).clamp(0.0, 1.0) as f32;
    [c(0, 1.0), c(1, 1.0), c(2, 1.0), c(3, 1.0)]
}

/// Les fonctions du jeu offertes au script d'ecran du mod `id`.
fn api(id: &str, state: &Rc<RefCell<State>>) -> Api {
    let (id, state) = (id.to_string(), state.clone());
    Rc::new(move |code: u32, a: &[Val]| -> Result<Vec<Val>, String> {
        let mut st = state.borrow_mut();
        match code {
            op::C_LOG => println!("[mod {id}] {}", text(a, 0)?),
            op::LISTEN => {
                st.listens.insert(text(a, 0)?.to_string());
            }
            op::C_SEND => {
                let (name, data) = (text(a, 0)?, text(a, 1).unwrap_or(""));
                if name.len() > sm::MSG_NAME || data.len() > sm::MSG_DATA || st.out.len() >= 64 {
                    return Err("send : message trop long, ou trop de messages d'un coup".into());
                }
                st.out.push((name.to_string(), data.to_string()));
            }
            op::C_CHAT => {
                if st.chat.len() < 32 {
                    st.chat.push(text(a, 0)?.chars().filter(|c| !c.is_control()).take(200).collect());
                }
            }
            op::C_POS => return Ok(st.pos.map(Val::Num).to_vec()),
            op::C_SCREEN => return Ok(vec![Val::Num(st.screen.0 as f64), Val::Num(st.screen.1 as f64)]),
            op::C_TEXT_WIDTH => return Ok(vec![Val::Num(Ui::text_width(text(a, 0)?, 1.0) as f64)]),
            op::C_TEXT if st.draws.len() < MAX_DRAWS => {
                let d = Draw::Text { x: num(a, 0)? as f32, y: num(a, 1)? as f32, text: text(a, 2)?.chars().take(200).collect(), color: color(a, 3) };
                st.draws.push(d);
            }
            op::C_RECT if st.draws.len() < MAX_DRAWS => {
                let d = Draw::Rect { x: num(a, 0)? as f32, y: num(a, 1)? as f32, w: num(a, 2)? as f32, h: num(a, 3)? as f32, color: color(a, 4) };
                st.draws.push(d);
            }
            op::C_OPEN => st.screen_open = true,
            op::C_CLOSE => st.screen_open = false,
            op::C_MOUSE => return Ok(vec![Val::Num(st.mouse.0 as f64), Val::Num(st.mouse.1 as f64)]),
            op::C_IMAGE if st.draws.len() < MAX_DRAWS => {
                let file = text(a, 4)?;
                let tile = *st.images.get(file).ok_or_else(|| format!("image inconnue : {file} (elle doit être dans le dossier textures du mod)"))?;
                let d = Draw::Image { x: num(a, 0)? as f32, y: num(a, 1)? as f32, w: num(a, 2)? as f32, h: num(a, 3)? as f32, tile, color: color(a, 5) };
                st.draws.push(d);
            }
            op::C_ICON if st.draws.len() < MAX_DRAWS => {
                let word = text(a, 3)?;
                let item = sm::item_by_key(word).or_else(|| shared::commands::item_from(word)).ok_or_else(|| format!("objet inconnu : {word}"))?;
                let d = Draw::Icon { x: num(a, 0)? as f32, y: num(a, 1)? as f32, size: num(a, 2).unwrap_or(16.0) as f32, id: item };
                st.draws.push(d);
            }
            op::C_SOUND => {
                if st.sounds.len() < 16 {
                    st.sounds.push((text(a, 0)?.chars().take(64).collect(), num(a, 1).unwrap_or(1.0).clamp(0.0, 2.0) as f32));
                }
            }
            op::C_TEXT | op::C_RECT | op::C_IMAGE | op::C_ICON => {}
            _ => return Err(format!("fonction n° {code} inconnue de cette version du jeu")),
        }
        Ok(Vec::new())
    })
}

/// Ce qui, en Lua, habille les fonctions du jeu pour un script d'ecran.
fn prelude() -> String {
    let ops = [
        ("LOG", op::C_LOG), ("LISTEN", op::LISTEN), ("SEND", op::C_SEND), ("CHAT", op::C_CHAT), ("POS", op::C_POS),
        ("TEXT", op::C_TEXT), ("RECT", op::C_RECT), ("SCREEN", op::C_SCREEN), ("TEXT_WIDTH", op::C_TEXT_WIDTH),
        ("OPEN", op::C_OPEN), ("CLOSE", op::C_CLOSE), ("MOUSE", op::C_MOUSE), ("IMAGE", op::C_IMAGE), ("ICON", op::C_ICON), ("SOUND", op::C_SOUND),
    ];
    let table: Vec<String> = ops.iter().map(|(n, v)| format!("{n}={v}")).collect();
    format!("local OP = {{{}}}\n{PRELUDE}", table.join(","))
}

const PRELUDE: &str = r##"
local call = _call
_call = nil
local handlers = {}

function on(event, fn)
  if type(event) ~= "string" or type(fn) ~= "function" then error("on(evenement, fonction)", 2) end
  local list = handlers[event]
  if not list then
    list = {}
    handlers[event] = list
    call(OP.LISTEN, event)
  end
  list[#list + 1] = fn
end

function print(...)
  local t = {}
  for i = 1, select("#", ...) do t[i] = tostring((select(i, ...))) end
  call(OP.LOG, table.concat(t, " "))
end

function send(name, data) call(OP.SEND, tostring(name), tostring(data or "")) end
function chat(text) call(OP.CHAT, tostring(text)) end
player = { pos = function() return call(OP.POS) end }
draw = {
  text = function(x, y, text, r, g, b, a) call(OP.TEXT, x, y, tostring(text), r or 1, g or 1, b or 1, a or 1) end,
  rect = function(x, y, w, h, r, g, b, a) call(OP.RECT, x, y, w, h, r or 1, g or 1, b or 1, a or 1) end,
  screen = function() return call(OP.SCREEN) end,
  text_width = function(text) return call(OP.TEXT_WIDTH, tostring(text)) end,
  image = function(x, y, w, h, file, r, g, b, a) call(OP.IMAGE, x, y, w, h, file, r or 1, g or 1, b or 1, a or 1) end,
  icon = function(x, y, size, item) call(OP.ICON, x, y, size, item) end,
}
-- Une fenetre de mod : la souris est liberee, les clics arrivent par l'evenement « click ».
screen = {
  open = function() call(OP.OPEN) end,
  close = function() call(OP.CLOSE) end,
  mouse = function() return call(OP.MOUSE) end,
}
sound = { play = function(name, volume) call(OP.SOUND, name, volume or 1) end }

return function(kind, name, _, _, ...)
  local list = handlers[name]
  if not list then return true end
  for i = 1, #list do list[i](...) end
  return true
end
"##;

/// Met une image de mod a la taille d'une tuile, sans la lisser : le pixel art reste net.
fn tile_pixels(png: &[u8]) -> Result<Vec<u8>, String> {
    let (rgba, w, h) = crate::assets::decode_png(png)?;
    if w == 0 || h == 0 || w > 1024 || h > 1024 {
        return Err("image vide ou trop grande".into());
    }
    let mut out = vec![0u8; TILE * TILE * 4];
    for y in 0..TILE {
        for x in 0..TILE {
            let (sx, sy) = (x * w / TILE, y * h / TILE);
            let (i, o) = ((sy * w + sx) * 4, (y * TILE + x) * 4);
            out[o..o + 4].copy_from_slice(&rgba[i..i + 4]);
        }
    }
    Ok(out)
}

/// Peint les textures des mods dans leur carre, celui qui se range sous l'atlas du jeu.
pub fn paint(tiles: &[(u16, Vec<u8>)]) -> Img {
    let mut img = Img(vec![0u8; crate::atlas::AT * crate::atlas::AT * 4]);
    for (tile, px) in tiles {
        let t = (*tile - sm::FIRST_TILE) as usize;
        let (ox, oy) = ((t % 32) * TILE, (t / 32) * TILE);
        for y in 0..TILE {
            for x in 0..TILE {
                let i = (y * TILE + x) * 4;
                img.set_px(ox + x, oy + y, [px[i], px[i + 1], px[i + 2], px[i + 3]]);
            }
        }
    }
    img
}

impl Mods {
    /// Le serveur annonce ses mods. Renvoie ce qu'il faut lui repondre tout de suite.
    pub fn announced(infos: Vec<Info>, content: sm::Content, tiles: Vec<TileDef>) -> (Mods, Vec<ClientMsg>) {
        sm::set_content(content);
        // Deja la : installe par le joueur, ou recu d'un serveur une autre fois.
        let (installed, _) = sm::load_dir(Path::new(sm::DIR));
        let mut m = Mods { infos, tiles, packs: Vec::new(), queue: VecDeque::new(), current: None, running: Vec::new(), ready: false, paint: None, models: Default::default() };
        for info in m.infos.clone() {
            let cached = || std::fs::read(Path::new(sm::CACHE).join(format!("{}.zip", sm::hex(&info.hash)))).ok().filter(|b| sm::hash_of(b) == info.hash).and_then(|b| Pack::from_zip(b).ok());
            match installed.iter().find(|p| p.hash == info.hash).cloned().or_else(cached) {
                Some(p) => m.packs.push(p),
                None => m.queue.push_back(info),
            }
        }
        let out = m.next();
        (m, out)
    }

    /// Demande le mod suivant, ou termine s'il n'en manque plus.
    fn next(&mut self) -> Vec<ClientMsg> {
        match self.queue.pop_front() {
            Some(info) => {
                let ask = ClientMsg::ModWant { hash: info.hash, offset: 0 };
                self.current = Some((info, Vec::new()));
                vec![ask]
            }
            None => {
                self.finish();
                vec![ClientMsg::ModsReady]
            }
        }
    }

    /// Un morceau de mod arrive. Erreur : le serveur envoie autre chose que ce qu'il a annonce.
    pub fn part(&mut self, hash: sm::Hash, offset: u32, total: u32, data: Vec<u8>) -> Result<Vec<ClientMsg>, String> {
        let Some((info, buf)) = self.current.as_mut() else { return Ok(Vec::new()) };
        if hash != info.hash || offset as usize != buf.len() {
            return Ok(Vec::new());
        }
        if total != info.size || total as usize > sm::MAX_SIZE || data.is_empty() || buf.len() + data.len() > total as usize {
            return Err(format!("le mod « {} » envoyé par le serveur n'a pas la taille annoncée", info.name));
        }
        buf.extend(data);
        if buf.len() < total as usize {
            return Ok(vec![ClientMsg::ModWant { hash, offset: buf.len() as u32 }]);
        }
        let (info, bytes) = self.current.take().unwrap_or_else(|| unreachable!());
        if sm::hash_of(&bytes) != info.hash {
            return Err(format!("le mod « {} » est arrivé abîmé", info.name));
        }
        let pack = Pack::from_zip(bytes).map_err(|e| format!("mod « {} » : {e}", info.name))?;
        // Garde pour la prochaine fois ; sans cache, il sera simplement telecharge de nouveau.
        let _ = std::fs::create_dir_all(sm::CACHE);
        let _ = std::fs::write(Path::new(sm::CACHE).join(format!("{}.zip", sm::hex(&info.hash))), &pack.bytes);
        println!("Mod reçu du serveur : {} {}", info.name, info.version);
        self.packs.push(pack);
        Ok(self.next())
    }

    /// Avancement du telechargement : (mod en cours, mods recus, mods en tout, part du mod en cours).
    pub fn progress(&self) -> Option<(String, usize, usize, f32)> {
        let (info, buf) = self.current.as_ref()?;
        let total = self.infos.len();
        Some((info.name.clone(), total - self.queue.len() - 1, total, buf.len() as f32 / info.size.max(1) as f32))
    }

    /// Tous les mods sont la : textures et scripts d'ecran.
    fn finish(&mut self) {
        let mut paint = Vec::new();
        for t in &self.tiles {
            let png = self.packs.iter().find(|p| p.manifest.id == t.mod_id).and_then(|p| p.file(&t.file));
            match png.ok_or_else(|| "fichier absent".to_string()).and_then(tile_pixels) {
                Ok(px) if (sm::FIRST_TILE..sm::FIRST_TILE + sm::TILES).contains(&t.tile) => paint.push((t.tile, px)),
                Ok(_) => {}
                Err(e) => eprintln!("[mod {}] texture {} : {e}", t.mod_id, t.file),
            }
        }
        // Les autres images des mods servent a leurs fenetres : elles prennent les tuiles encore libres.
        let mut next = self.tiles.iter().map(|t| t.tile + 1).max().unwrap_or(sm::FIRST_TILE).max(sm::FIRST_TILE);
        let mut images: Vec<(String, String, u16)> = self.tiles.iter().map(|t| (t.mod_id.clone(), t.file.clone(), t.tile)).collect();
        for p in &self.packs {
            for file in p.names().filter(|f| f.starts_with("textures/") && f.ends_with(".png")) {
                if images.iter().any(|(m, f, _)| *m == p.manifest.id && f == file) || next >= sm::FIRST_TILE + sm::TILES {
                    continue;
                }
                if let Some(px) = p.file(file).and_then(|d| tile_pixels(d).ok()) {
                    paint.push((next, px));
                    images.push((p.manifest.id.clone(), file.to_string(), next));
                    next += 1;
                }
            }
            // Les bruitages du mod : « mod:fichier ».
            for file in p.names().filter(|f| f.starts_with("sons/")) {
                let stem = file["sons/".len()..].rsplit_once('.').map_or("", |x| x.0);
                if let (false, Some(data)) = (stem.is_empty(), p.file(file)) {
                    crate::audio::add_clip(&format!("{}:{stem}", p.manifest.id), data.to_vec());
                }
            }
        }
        let mut img = self::paint(&paint);
        // Modeles des creatures : leur texture prend une case de 256 pixels, en partant du bas du carre.
        let content = sm::content();
        for (n, d) in content.mobs.iter().filter(|d| !d.model.is_empty()).take(sm::MODELS).enumerate() {
            let side = crate::models::CELL;
            let cell = ((n % 8) * side, crate::atlas::AT - side * (1 + n / 8), side);
            let mod_id = d.key.split(':').next().unwrap_or("");
            let text = self.packs.iter().find(|p| p.manifest.id == mod_id).and_then(|p| p.text(&d.model));
            match text.ok_or_else(|| "fichier absent".to_string()).and_then(|t| crate::assets::read_bbmodel(t, cell, 0.0, &mut img)) {
                Ok(mut model) => {
                    // Le carre des mods est sous l'atlas du jeu : ses textures ont un v entre 1 et 2.
                    for part in &mut model.parts {
                        for uv in &mut part.uv {
                            uv[1] += 1.0;
                            uv[3] += 1.0;
                        }
                    }
                    self.models.insert(d.id, model);
                }
                Err(e) => eprintln!("[mod {mod_id}] modèle {} : {e}", d.model),
            }
        }
        self.paint = Some(img);
        for p in &self.packs {
            let id = p.manifest.id.clone();
            let state = Rc::new(RefCell::new(State::default()));
            for (_, file, tile) in images.iter().filter(|(m, _, _)| *m == id) {
                // Une image se nomme avec ou sans son dossier : « textures/a.png » ou « a.png ».
                let mut st = state.borrow_mut();
                st.images.insert(file.clone(), *tile);
                st.images.insert(file["textures/".len().min(file.len())..].to_string(), *tile);
            }
            let (file, built) = if let Some(bytes) = p.file("client.wasm") {
                ("client.wasm", Script::wasm(bytes, api(&id, &state)))
            } else if let Some(source) = p.text("client.lua") {
                ("client.lua", Script::lua(&format!("{id}/client.lua"), &prelude(), source, api(&id, &state)))
            } else {
                continue;
            };
            match built {
                Ok(script) => self.running.push(Running { id, state, script }),
                Err(e) => eprintln!("[mod {id}] {file} n'a pas pu être chargé : {e}"),
            }
        }
        self.ready = true;
    }

    /// Previent les scripts d'ecran (un seul mod si `only` est donne).
    fn emit(&mut self, only: Option<&str>, event: &str, args: &[Val]) {
        for r in &mut self.running {
            if only.is_some_and(|id| id != r.id) || !r.state.borrow().listens.contains(event) {
                continue;
            }
            let mut vals = vec![Val::Str(event.to_string()), Val::Nil, Val::Nil];
            vals.extend_from_slice(args);
            if let Err(e) = r.script.handle(kind::EVENT, &vals, EVENT_MS) {
                let mut st = r.state.borrow_mut();
                st.errors += 1;
                if st.errors <= 20 {
                    eprintln!("[mod {}] événement « {event} » : {e}", r.id);
                }
            }
        }
    }

    /// Message du script serveur d'un mod.
    pub fn message(&mut self, mod_id: &str, name: String, data: String) {
        self.emit(Some(mod_id), "message", &[Val::Str(name), Val::Str(data)]);
    }

    pub fn key(&mut self, name: &str) {
        self.emit(None, "key", &[Val::Str(name.to_string())]);
    }

    /// Un mod affiche-t-il une fenetre ?
    pub fn screen_open(&self) -> bool {
        self.running.iter().any(|r| r.state.borrow().screen_open)
    }

    /// Ferme les fenetres des mods (touche Echap) et les en previent.
    pub fn close_screens(&mut self) {
        for r in &self.running {
            r.state.borrow_mut().screen_open = false;
        }
        self.emit(None, "close", &[]);
    }

    /// Souris et saisie, pour les mods qui affichent une fenetre : position et clics en points
    /// d'interface (`button` : 0 gauche, 1 droit), texte tape.
    pub fn pointer(&mut self, mouse: (f32, f32), click: Option<u8>, typed: &str) {
        for r in &self.running {
            r.state.borrow_mut().mouse = mouse;
        }
        let open: Vec<String> = self.running.iter().filter(|r| r.state.borrow().screen_open).map(|r| r.id.clone()).collect();
        for id in open {
            if let Some(button) = click {
                self.emit(Some(&id), "click", &[Val::Num(mouse.0 as f64), Val::Num(mouse.1 as f64), Val::Num(button as f64)]);
            }
            if !typed.is_empty() {
                self.emit(Some(&id), "text", &[Val::Str(typed.to_string())]);
            }
        }
    }

    /// Une image : les scripts dessinent par-dessus le jeu. Renvoie ce qu'ils veulent envoyer au serveur
    /// et ecrire dans le chat.
    pub fn frame(&mut self, ui: &mut Ui, s: f32, pos: [f64; 3]) -> (Vec<ClientMsg>, Vec<String>) {
        for r in &self.running {
            let mut st = r.state.borrow_mut();
            st.pos = pos;
            st.screen = (ui.w / s, ui.h / s);
        }
        self.emit(None, "hud", &[]);
        let (mut out, mut lines) = (Vec::new(), Vec::new());
        for r in &self.running {
            let mut st = r.state.borrow_mut();
            for (name, volume) in st.sounds.drain(..) {
                crate::audio::play(&name, volume);
            }
            for d in st.draws.drain(..) {
                match d {
                    Draw::Image { x, y, w, h, tile, color } => {
                        let ((u, v), e) = (crate::atlas::tile_uv(tile), 1.0 / 32.0);
                        let (x0, y0, x1, y1) = ((x * s).floor(), (y * s).floor(), ((x + w) * s).floor(), ((y + h) * s).floor());
                        ui.quad4([[x0, y0], [x1, y0], [x1, y1], [x0, y1]], [[u, v], [u + e, v], [u + e, v + e], [u, v + e]], [color; 4]);
                    }
                    Draw::Icon { x, y, size, id } => ui.icon((x * s).floor(), (y * s).floor(), (size * s).floor(), id, [1.0; 4]),
                    Draw::Text { x, y, text, color } => ui.text((x * s).floor(), (y * s).floor(), s, &text, color),
                    Draw::Rect { x, y, w, h, color } => ui.rect((x * s).floor(), (y * s).floor(), (w * s).floor(), (h * s).floor(), color),
                }
            }
            out.extend(st.out.drain(..).map(|(name, data)| ClientMsg::ModMsg { mod_id: r.id.clone(), name, data }));
            lines.append(&mut st.chat);
        }
        (out, lines)
    }
}

/// Aiguille les messages du serveur qui concernent les mods. Erreur : la connexion doit etre abandonnee.
pub fn on_msg(mods: &mut Option<Mods>, msg: ServerMsg) -> Result<Vec<ClientMsg>, String> {
    match msg {
        ServerMsg::Mods { mods: infos, content, tiles } => {
            if infos.len() > 256 || infos.iter().any(|i| i.size as usize > sm::MAX_SIZE) {
                return Err("ce serveur annonce des mods trop nombreux ou trop gros".into());
            }
            let (m, out) = Mods::announced(infos, content, tiles);
            *mods = Some(m);
            Ok(out)
        }
        ServerMsg::ModPart { hash, offset, total, data } => match mods {
            Some(m) => m.part(hash, offset, total, data),
            None => Ok(Vec::new()),
        },
        ServerMsg::ModMsg { mod_id, name, data } => {
            if let Some(m) = mods {
                m.message(&mod_id, name, data);
            }
            Ok(Vec::new())
        }
        _ => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ce que les mods ajoutent est connu de tout le programme : ces essais passent l'un apres l'autre.
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn mods_of(packs: Vec<Pack>, tiles: Vec<TileDef>) -> Mods {
        let mut m = Mods { infos: packs.iter().map(|p| p.info()).collect(), tiles, packs, queue: VecDeque::new(), current: None, running: Vec::new(), ready: false, paint: None, models: Default::default() };
        m.finish();
        m
    }

    fn folder(name: &str, files: &[(&str, &[u8])]) -> Pack {
        let dir = std::env::temp_dir().join(format!("epicube-client-mod-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("textures")).unwrap();
        std::fs::write(dir.join("mod.json"), format!(r#"{{"id":"{name}","name":"Essai","version":"1"}}"#)).unwrap();
        for (file, data) in files {
            std::fs::create_dir_all(dir.join(file).parent().unwrap()).unwrap();
            std::fs::write(dir.join(file), data).unwrap();
        }
        let pack = Pack::from_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(dir);
        pack
    }

    #[test]
    fn a_lua_screen_script_draws_and_talks_to_its_server() {
        let script = br#"
            local seen = "rien"
            on("message", function(name, data) seen = name .. ":" .. data end)
            on("key", function(key) send("touche", key) end)
            on("hud", function()
                local w, h = draw.screen()
                draw.rect(0, 0, w, 10, 0, 0, 0, 0.5)
                draw.text(2, 1, seen)
                local x, y, z = player.pos()
                if y == 70 then chat("en haut") end
            end)
        "#;
        // La police se prepare avec l'atlas : sans elle, aucun texte ne se dessine.
        let _ = crate::atlas::build();
        let mut m = mods_of(vec![folder("ecran", &[("client.lua", script)])], Vec::new());
        assert!(m.ready);
        m.message("ecran", "score".into(), "12".into());
        m.message("autre", "score".into(), "99".into());
        m.key("KeyG");
        let mut ui = Ui::new(640.0, 360.0);
        let (out, lines) = m.frame(&mut ui, 1.0, [0.0, 70.0, 0.0]);
        // Un rectangle (six sommets), puis les lettres de « score:12 ».
        assert!(ui.v.len() >= 6 * (1 + "score:12".len()), "{} sommets", ui.v.len());
        assert!(matches!(&out[..], [ClientMsg::ModMsg { mod_id, name, data }] if mod_id == "ecran" && name == "touche" && data == "KeyG"));
        assert_eq!(lines, ["en haut"]);
    }

    #[test]
    fn textures_are_painted_where_the_blocks_look_for_them() {
        // Une image de 2 x 2 pixels : rouge, vert / bleu, blanc.
        let png = crate::assets::encode_png(&[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255], 2, 2).unwrap();
        let tile = sm::FIRST_TILE + 3;
        let mut m = mods_of(vec![folder("image", &[("textures/a.png", &png)])], vec![TileDef { tile, mod_id: "image".into(), file: "textures/a.png".into() }]);
        let img = m.paint.take().unwrap();
        let t = (tile - sm::FIRST_TILE) as usize;
        let (ox, oy) = ((t % 32) * TILE, (t / 32) * TILE);
        let px = |x: usize, y: usize| {
            let i = ((oy + y) * crate::atlas::AT + ox + x) * 4;
            [img.0[i], img.0[i + 1], img.0[i + 2], img.0[i + 3]]
        };
        assert_eq!(px(0, 0), [255, 0, 0, 255]);
        assert_eq!(px(TILE - 1, 0), [0, 255, 0, 255]);
        assert_eq!(px(0, TILE - 1), [0, 0, 255, 255]);
        // Le rendu lit cette tuile dans le second carre de l'atlas : son v depasse 1.
        let (u, v) = crate::atlas::tile_uv(tile);
        assert_eq!((u, v), (ox as f32 / crate::atlas::AT as f32, 1.0 + oy as f32 / crate::atlas::AT as f32));
    }

    /// Le script d'ecran du mod d'exemple en Rust (compile par exemples/mods/rubis/construire.sh).
    #[test]
    fn a_rust_screen_script_draws_too() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../exemples/mods/rubis");
        if !dir.join("client.wasm").exists() {
            eprintln!("mod Rubis non compile : essai saute");
            return;
        }
        let _ = crate::atlas::build();
        let _alone = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        // En partie, le serveur annonce les objets du mod avant que son script d'ecran ne tourne.
        let item = |k: u16, name: &str| sm::ItemDef { id: sm::FIRST_ITEM + k, key: format!("rubis:{name}"), name: name.into(), tile: sm::FIRST_TILE, stack: 64, ..sm::ItemDef::default() };
        sm::set_content(sm::Content { items: vec![item(0, "gemme"), item(1, "tarte"), item(2, "fleur"), item(3, "pioche"), item(4, "lame")], ..sm::Content::default() });
        let mut m = mods_of(vec![Pack::from_dir(&dir).unwrap()], Vec::new());
        let mut ui = Ui::new(640.0, 360.0);
        m.frame(&mut ui, 1.0, [0.0; 3]);
        assert!(ui.v.is_empty());
        m.message("rubis", "mines".into(), "4".into());
        m.frame(&mut ui, 1.0, [0.0; 3]);
        let hud = ui.v.len();
        assert!(hud > 6 * 10, "{hud} sommets");
        // La touche G ouvre la boutique : la souris est liberee, et le mod demande la bourse au serveur.
        assert!(!m.screen_open());
        m.key("KeyG");
        assert!(m.screen_open());
        m.message("rubis", "bourse".into(), "7".into());
        let mut ui = Ui::new(640.0, 360.0);
        let (out, _) = m.frame(&mut ui, 1.0, [0.0; 3]);
        assert!(matches!(&out[..], [ClientMsg::ModMsg { name, data, .. }] if name == "acheter" && data == "-1"));
        assert!(ui.v.len() > hud * 4, "la fenetre se dessine : {} sommets", ui.v.len());
        // Un clic sur le troisieme article l'achete ; Echap ferme la fenetre.
        m.pointer((320.0, (360.0 - 150.0) / 2.0 + 30.0 + 2.0 * 24.0 + 5.0), Some(0), "");
        let (out, _) = m.frame(&mut Ui::new(640.0, 360.0), 1.0, [0.0; 3]);
        assert!(matches!(&out[..], [ClientMsg::ModMsg { name, data, .. }] if name == "acheter" && data == "2"));
        m.close_screens();
        assert!(!m.screen_open());
        m.key("KeyG");
        assert!(m.screen_open(), "apres Echap, G rouvre la boutique");
        sm::clear_content();
    }

    #[test]
    fn a_mod_creature_can_bring_its_own_model() {
        let _alone = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        // Le modele de l'oni, tel que Blockbench l'enregistre, sert de modele a une creature de mod.
        let mut atlas = crate::atlas::build();
        let built = crate::models::build(&mut atlas);
        let (ox, oy) = crate::models::cell_origin(4);
        let text = crate::assets::bbmodel("golem", (ox, oy, crate::models::CELL), 0.0, &built[4], &atlas).unwrap();
        let pack = folder("betes", &[("modeles/golem.bbmodel", text.as_bytes())]);
        sm::set_content(sm::Content {
            mobs: vec![sm::MobDef { id: 3, key: "betes:golem".into(), base: 4, scale: 1.0, model: "modeles/golem.bbmodel".into(), ..sm::MobDef::default() }],
            ..sm::Content::default()
        });
        let mut m = mods_of(vec![pack], Vec::new());
        let model = m.models.get(&3).expect("modele charge");
        assert_eq!(model.parts.len(), built[4].parts.len());
        // Sa texture est dans le carre des mods : en bas de l'image, lue avec un v entre 1 et 2.
        assert!(model.parts.iter().all(|p| p.uv.iter().all(|uv| uv[1] > 1.0 && uv[3] < 2.0)));
        let img = m.paint.take().unwrap();
        let side = crate::models::CELL;
        let painted = (0..side * side).filter(|k| img.0[((crate::atlas::AT - side + k / side) * crate::atlas::AT + k % side) * 4 + 3] != 0).count();
        assert!(painted > 1000, "{painted} pixels de texture");
        sm::clear_content();
    }
}
