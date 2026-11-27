//! Connexion au serveur : un fil de lecture, un fil d'ecriture.
use shared::proto::{read_msg, write_msg, ClientMsg, ServerMsg};
use std::io::{BufReader, BufWriter};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::thread;

pub struct Net {
    pub tx: Sender<ClientMsg>,
    pub rx: Receiver<ServerMsg>,
    /// Instant d'arrivee du dernier Pong, note par le fil de lecture : le jeu ne lit ses messages
    /// qu'une fois par image, ce qui fausserait la mesure du ping de plusieurs millisecondes.
    pub pong_at: Arc<Mutex<Option<Instant>>>,
}

impl Net {
    pub fn connect(addr: &str) -> std::io::Result<Net> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        let read_half = stream.try_clone()?;

        let (in_tx, in_rx) = mpsc::channel::<ServerMsg>();
        let pong_at = Arc::new(Mutex::new(None));
        let stamp = pong_at.clone();
        thread::spawn(move || {
            let mut r = BufReader::new(read_half);
            while let Ok(msg) = read_msg::<_, ServerMsg>(&mut r) {
                if matches!(msg, ServerMsg::Pong { .. }) {
                    if let Ok(mut at) = stamp.lock() {
                        *at = Some(Instant::now());
                    }
                }
                if in_tx.send(msg).is_err() {
                    break;
                }
            }
        });

        let (out_tx, out_rx) = mpsc::channel::<ClientMsg>();
        thread::spawn(move || {
            let mut w = BufWriter::new(stream);
            while let Ok(msg) = out_rx.recv() {
                if write_msg(&mut w, &msg).is_err() {
                    break;
                }
            }
        });

        Ok(Net { tx: out_tx, rx: in_rx, pong_at })
    }

    pub fn send(&self, msg: ClientMsg) {
        let _ = self.tx.send(msg);
    }
}
