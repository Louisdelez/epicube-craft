//! Maillage des chunks hors du fil principal.
//!
//! Construire le maillage d'un chunk (lumiere comprise) prend une dizaine de millisecondes : fait dans
//! la boucle d'affichage, cela coutait plusieurs images a chaque chunk charge ou bloc casse. Des fils de
//! travail s'en chargent ; le fil principal leur confie une copie du chunk et de ses voisins, puis
//! recupere le resultat pret a envoyer a la carte graphique.
use crate::gfx::MeshData;
use crate::mesh;
use shared::chunk::Chunk;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};

type Key = (i32, i32);

struct Job {
    key: Key,
    epoch: u32,
    /// Le chunk et ceux de ses huit voisins qui sont charges.
    chunks: HashMap<Key, Chunk>,
}

pub struct Done {
    pub key: Key,
    pub epoch: u32,
    pub mesh: Option<MeshData>,
}

type Queue = Arc<(Mutex<VecDeque<Job>>, Condvar)>;

pub struct Mesher {
    queue: Queue,
    rx: Receiver<Done>,
    flying: HashSet<Key>,
    threads: usize,
}

impl Mesher {
    pub fn new() -> Mesher {
        // On laisse deux coeurs au jeu et au serveur local.
        let threads = std::thread::available_parallelism().map_or(2, |n| n.get().saturating_sub(2)).clamp(1, 6);
        let queue: Queue = Arc::new((Mutex::new(VecDeque::new()), Condvar::new()));
        let (tx, rx): (Sender<Done>, Receiver<Done>) = mpsc::channel();
        for k in 0..threads {
            let (queue, tx) = (queue.clone(), tx.clone());
            let _ = std::thread::Builder::new().name(format!("maillage-{k}")).spawn(move || loop {
                let job = {
                    let (lock, wake) = &*queue;
                    let Ok(mut q) = lock.lock() else { return };
                    loop {
                        if let Some(j) = q.pop_front() {
                            break j;
                        }
                        q = match wake.wait(q) {
                            Ok(q) => q,
                            Err(_) => return,
                        };
                    }
                };
                let mesh = mesh::mesh_chunk(&job.chunks, job.key.0, job.key.1);
                // Le jeu est ferme : plus personne n'attend le resultat.
                if tx.send(Done { key: job.key, epoch: job.epoch, mesh }).is_err() {
                    return;
                }
            });
        }
        Mesher { queue, rx, flying: HashSet::new(), threads }
    }

    /// Ce chunk est-il deja en cours de maillage ?
    pub fn busy(&self, key: Key) -> bool {
        self.flying.contains(&key)
    }

    /// Reste-t-il de la place dans la file ? (deux travaux par fil : au-dela, mieux vaut attendre de savoir
    /// quels chunks seront les plus proches du joueur.)
    pub fn has_room(&self) -> bool {
        self.flying.len() < self.threads * 2
    }

    /// Confie un chunk aux fils de travail. `urgent` : il passe devant les autres (un bloc vient de changer).
    pub fn submit(&mut self, key: Key, epoch: u32, world: &HashMap<Key, Chunk>, urgent: bool) {
        let mut chunks = HashMap::with_capacity(9);
        for dz in -1..=1 {
            for dx in -1..=1 {
                let k = (key.0 + dx, key.1 + dz);
                if let Some(c) = world.get(&k) {
                    chunks.insert(k, c.clone());
                }
            }
        }
        let job = Job { key, epoch, chunks };
        let (lock, wake) = &*self.queue;
        if let Ok(mut q) = lock.lock() {
            if urgent {
                q.push_front(job);
            } else {
                q.push_back(job);
            }
            self.flying.insert(key);
            wake.notify_one();
        }
    }

    /// Un maillage termine, s'il y en a un.
    pub fn poll(&mut self) -> Option<Done> {
        let done = self.rx.try_recv().ok()?;
        self.flying.remove(&done.key);
        Some(done)
    }
}
