//! Le côté du mod : appels vers le jeu, et aiguillage des événements que le jeu lui envoie.
use crate::wire::{self, Val};
use std::cell::RefCell;

type Handler = Box<dyn Fn(&[Val]) -> bool>;
type Command = Box<dyn Fn(&[Val]) -> Option<String>>;

thread_local! {
    static HANDLERS: RefCell<Vec<(String, std::rc::Rc<Handler>)>> = const { RefCell::new(Vec::new()) };
    static COMMANDS: RefCell<Vec<(String, std::rc::Rc<Command>)>> = const { RefCell::new(Vec::new()) };
    /// Dernière réponse rendue au jeu : elle vit jusqu'à la suivante.
    static REPLY: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "epicube")]
extern "C" {
    #[link_name = "call"]
    fn host_call(op: u32, ptr: *const u8, len: u32) -> u64;
}

/// Appelle une fonction du jeu par son numéro (voir `op`). Une erreur du jeu arrête l'appel en cours
/// du mod avec son message.
#[cfg(target_arch = "wasm32")]
pub fn call(op: u32, args: &[Val]) -> Vec<Val> {
    let data = wire::encode(args);
    let packed = unsafe { host_call(op, data.as_ptr(), data.len() as u32) };
    let (ptr, len) = ((packed >> 32) as u32, packed as u32);
    if len == 0 {
        return Vec::new();
    }
    let reply = unsafe { wire::take(ptr, len) };
    match reply.split_first() {
        Some((0, rest)) => wire::decode(rest).unwrap_or_default(),
        Some((_, rest)) => panic!("{}", String::from_utf8_lossy(rest)),
        None => Vec::new(),
    }
}

/// Hors du jeu (essais sur l'ordinateur du créateur), les fonctions du jeu ne répondent rien.
#[cfg(not(target_arch = "wasm32"))]
pub fn call(_op: u32, _args: &[Val]) -> Vec<Val> {
    Vec::new()
}

/// Écoute un événement du jeu par son nom. La fonction reçoit ses valeurs (pour un événement de
/// joueur : son numéro, son nom, puis le reste) et renvoie `false` pour refuser l'action.
pub fn on(event: &str, f: impl Fn(&[Val]) -> bool + 'static) {
    let first = HANDLERS.with(|h| {
        let mut h = h.borrow_mut();
        let first = !h.iter().any(|(e, _)| e == event);
        h.push((event.to_string(), std::rc::Rc::new(Box::new(f))));
        first
    });
    if first {
        // Le même numéro sert des deux côtés : le jeu sait de quel côté il a chargé le mod.
        call(crate::op::LISTEN, &[event.into()]);
    }
}

pub(crate) fn add_command(name: String, f: Command) {
    COMMANDS.with(|c| {
        let mut c = c.borrow_mut();
        c.retain(|(n, _)| *n != name);
        c.push((name, std::rc::Rc::new(f)));
    });
}

fn reply(vals: &[Val]) -> u64 {
    REPLY.with(|r| {
        let mut r = r.borrow_mut();
        *r = wire::encode(vals);
        ((r.as_ptr() as usize as u64) << 32) | r.len() as u64
    })
}

/// Point d'entrée appelé par le jeu.
#[doc(hidden)]
pub fn __handle(kind: u32, ptr: u32, len: u32) -> u64 {
    let data = if len == 0 { Vec::new() } else { unsafe { wire::take(ptr, len) } };
    let vals = wire::decode(&data).unwrap_or_default();
    let (Some(Val::Str(name)), rest) = (vals.first(), vals.get(1..).unwrap_or_default()) else { return reply(&[]) };
    if kind == crate::kind::COMMAND {
        let f = COMMANDS.with(|c| c.borrow().iter().find(|(n, _)| n == name).map(|(_, f)| f.clone()));
        return match f.and_then(|f| f(rest)) {
            Some(text) => reply(&[Val::Str(text)]),
            None => reply(&[]),
        };
    }
    // Un événement sans joueur arrive avec deux cases vides a la place du joueur.
    let rest = if matches!(rest, [Val::Nil, Val::Nil, ..]) { &rest[2..] } else { rest };
    let list: Vec<std::rc::Rc<Handler>> = HANDLERS.with(|h| h.borrow().iter().filter(|(e, _)| e == name).map(|(_, f)| f.clone()).collect());
    let mut ok = true;
    for f in list {
        ok &= f(rest);
    }
    reply(&[Val::Bool(ok)])
}
