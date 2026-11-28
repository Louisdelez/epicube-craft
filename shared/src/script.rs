//! Le script d'un mod, quel que soit son langage : Lua pour les petits mods, Rust compile en
//! WebAssembly pour les gros. Les deux tournent dans un bac a sable et parlent au jeu par les memes
//! fonctions numerotees (voir `epicube_mod::op`), avec les memes valeurs.
pub use epicube_mod::wire::{decode, encode, Val};
pub use epicube_mod::{kind, op};
use mlua::{Function, Lua, Value, Variadic};
use std::rc::Rc;
use std::sync::OnceLock;
use wasmtime::{Caller, Config, Engine, Instance, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc};

/// Les fonctions du jeu, vues d'un mod : numero de la fonction, valeurs donnees, valeurs rendues.
pub type Api = Rc<dyn Fn(u32, &[Val]) -> Result<Vec<Val>, String>>;

/// Memoire qu'un mod WebAssembly peut occuper.
const WASM_MEMORY: usize = 512 << 20;
/// Pas de l'horloge qui interrompt un mod trop lent, en millisecondes.
const EPOCH_MS: u64 = 5;

struct Ctx {
    api: Api,
    limits: StoreLimits,
    /// Derniere erreur rendue par une fonction du jeu : c'est elle qui explique l'arret du mod.
    last_error: Option<String>,
}

pub struct Wasm {
    store: Store<Ctx>,
    memory: Memory,
    alloc: TypedFunc<u32, u32>,
    handle: TypedFunc<(u32, u32, u32), u64>,
}

pub enum Script {
    Lua { lua: Lua, handle: Function },
    Wasm(Box<Wasm>),
}

fn to_val(v: Value) -> mlua::Result<Val> {
    Ok(match v {
        Value::Nil => Val::Nil,
        Value::Boolean(b) => Val::Bool(b),
        Value::Integer(i) => Val::Num(i as f64),
        Value::Number(n) => Val::Num(n),
        Value::String(s) => Val::Str(s.to_string_lossy().to_string()),
        other => return Err(mlua::Error::runtime(format!("valeur de type {} non acceptée ici", other.type_name()))),
    })
}

fn from_val(lua: &Lua, v: &Val) -> mlua::Result<Value> {
    Ok(match v {
        Val::Nil => Value::Nil,
        Val::Bool(b) => Value::Boolean(*b),
        // Un nombre entier reste entier en Lua : « 5 », pas « 5.0 ».
        Val::Num(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => Value::Integer(*n as i64),
        Val::Num(n) => Value::Number(*n),
        Val::Str(s) => Value::String(lua.create_string(s)?),
    })
}

/// Le moteur WebAssembly, partage par tous les mods, et son horloge.
fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = Config::new();
        config.epoch_interruption(true);
        let engine = Engine::new(&config).expect("moteur WebAssembly");
        let clock = engine.clone();
        let _ = std::thread::Builder::new().name("horloge des mods".into()).spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(EPOCH_MS));
            clock.increment_epoch();
        });
        engine
    })
}

/// Lit des octets dans la memoire du mod.
fn read(memory: &Memory, store: impl wasmtime::AsContext, ptr: u32, len: u32) -> Option<Vec<u8>> {
    let data = memory.data(&store);
    data.get(ptr as usize..(ptr as usize).checked_add(len as usize)?).map(|s| s.to_vec())
}

impl Script {
    /// Charge un script Lua. `prelude` construit, a partir de la fonction `_call(numero, ...)`, les
    /// fonctions offertes au mod ; il rend la fonction qui recoit evenements et commandes.
    pub fn lua(name: &str, prelude: &str, source: &str, api: Api) -> Result<Script, String> {
        let built = (|| -> mlua::Result<Script> {
            let lua = crate::mods::sandbox()?;
            let call = lua.create_function(move |lua, (op, args): (u32, Variadic<Value>)| {
                let vals: Vec<Val> = args.into_iter().map(to_val).collect::<mlua::Result<_>>()?;
                let out = api(op, &vals).map_err(mlua::Error::runtime)?;
                out.iter().map(|v| from_val(lua, v)).collect::<mlua::Result<Variadic<Value>>>()
            })?;
            lua.globals().set("_call", call)?;
            crate::mods::allow(&lua, 2000);
            let handle: Function = lua.load(prelude).set_name("=jeu").call(())?;
            crate::mods::run(&lua, name, source)?;
            Ok(Script::Lua { lua, handle })
        })();
        built.map_err(|e| crate::mods::error_line(&e))
    }

    /// Charge un mod Rust compile en WebAssembly, et appelle sa fonction de demarrage.
    pub fn wasm(bytes: &[u8], api: Api) -> Result<Script, String> {
        let engine = engine();
        let module = Module::new(engine, bytes).map_err(|e| format!("WebAssembly illisible : {e}"))?;
        let mut linker: Linker<Ctx> = Linker::new(engine);
        linker
            .func_wrap("epicube", "call", |mut caller: Caller<'_, Ctx>, op: u32, ptr: u32, len: u32| -> wasmtime::Result<u64> {
                let memory = caller.get_export("memory").and_then(|e| e.into_memory()).ok_or_else(|| wasmtime::Error::msg("mémoire du mod absente"))?;
                let args = read(&memory, &caller, ptr, len).and_then(|d| decode(&d)).ok_or_else(|| wasmtime::Error::msg("valeurs illisibles"))?;
                let api = caller.data().api.clone();
                // La reponse commence par 0 (valeurs) ou 1 (message d'erreur).
                let reply = match api(op, &args) {
                    Ok(vals) if vals.is_empty() => return Ok(0),
                    Ok(vals) => [vec![0u8], encode(&vals)].concat(),
                    Err(e) => {
                        caller.data_mut().last_error = Some(e.clone());
                        [vec![1u8], e.into_bytes()].concat()
                    }
                };
                let alloc = caller.get_export("ec_alloc").and_then(|e| e.into_func()).ok_or_else(|| wasmtime::Error::msg("ec_alloc absent"))?.typed::<u32, u32>(&caller)?;
                let at = alloc.call(&mut caller, reply.len() as u32)?;
                memory.write(&mut caller, at as usize, &reply)?;
                Ok(((at as u64) << 32) | reply.len() as u64)
            })
            .map_err(|e| e.to_string())?;
        let limits = StoreLimitsBuilder::new().memory_size(WASM_MEMORY).instances(1).memories(1).tables(4).build();
        let mut store = Store::new(engine, Ctx { api, limits, last_error: None });
        store.limiter(|c| &mut c.limits);
        store.set_epoch_deadline(2000 / EPOCH_MS);
        let fail = |store: &mut Store<Ctx>, e: wasmtime::Error| store.data_mut().last_error.take().unwrap_or_else(|| format!("{e}").lines().next().unwrap_or("erreur").to_string());
        let instance: Instance = match linker.instantiate(&mut store, &module) {
            Ok(i) => i,
            Err(e) => return Err(format!("le mod demande une fonction que le jeu n'a pas : {}", fail(&mut store, e))),
        };
        let memory = instance.get_memory(&mut store, "memory").ok_or("mémoire du mod absente")?;
        let missing = |what: &str| format!("{what} absent : le mod doit utiliser epicube_mod::start!");
        let alloc = instance.get_typed_func::<u32, u32>(&mut store, "ec_alloc").map_err(|_| missing("ec_alloc"))?;
        let handle = instance.get_typed_func::<(u32, u32, u32), u64>(&mut store, "ec_handle").map_err(|_| missing("ec_handle"))?;
        let init = instance.get_typed_func::<(), ()>(&mut store, "ec_init").map_err(|_| missing("ec_init"))?;
        if let Err(e) = init.call(&mut store, ()) {
            return Err(fail(&mut store, e));
        }
        Ok(Script::Wasm(Box::new(Wasm { store, memory, alloc, handle })))
    }

    /// Donne au mod un evenement ou une commande (`kind`), avec `ms` millisecondes pour repondre.
    pub fn handle(&mut self, kind: u32, args: &[Val], ms: u64) -> Result<Vec<Val>, String> {
        match self {
            Script::Lua { lua, handle } => {
                let call = || -> mlua::Result<Vec<Val>> {
                    let mut v: Vec<Value> = vec![Value::Integer(kind as i64)];
                    for a in args {
                        v.push(from_val(lua, a)?);
                    }
                    crate::mods::allow(lua, ms);
                    let out: Variadic<Value> = handle.call(Variadic::from_iter(v))?;
                    out.into_iter().map(to_val).collect()
                };
                call().map_err(|e| crate::mods::error_line(&e))
            }
            Script::Wasm(w) => {
                let Wasm { store, memory, alloc, handle } = &mut **w;
                let data = encode(args);
                store.data_mut().last_error = None;
                store.set_epoch_deadline(ms / EPOCH_MS + 1);
                let run = (|| -> wasmtime::Result<Vec<Val>> {
                    let at = alloc.call(&mut *store, data.len() as u32)?;
                    memory.write(&mut *store, at as usize, &data)?;
                    let packed = handle.call(&mut *store, (kind, at, data.len() as u32))?;
                    let (ptr, len) = ((packed >> 32) as u32, packed as u32);
                    read(memory, &*store, ptr, len).and_then(|d| decode(&d)).ok_or_else(|| wasmtime::Error::msg("réponse illisible"))
                })();
                run.map_err(|e| {
                    store.data_mut().last_error.take().unwrap_or_else(|| {
                        let text = format!("{e:#}");
                        if text.contains("interrupt") {
                            "le mod a mis trop de temps : interrompu".to_string()
                        } else {
                            text.lines().next().unwrap_or("erreur").to_string()
                        }
                    })
                })
            }
        }
    }
}
