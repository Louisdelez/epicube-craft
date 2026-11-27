//! Capture du microphone pour le chat vocal.
//!
//! Le micro n'est ouvert que si le joueur l'a demande dans les parametres (le systeme lui demande
//! alors son accord). Le son capte est ramene en mono a 16 kHz, decoupe en tranches de 20 ms, et
//! une tranche n'est transmise que si la « porte » est ouverte : touche maintenue, ou voix assez forte.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use shared::voice;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

/// Tranches encore transmises apres la fin d'un mot, pour ne pas couper les syllabes (0,4 s).
const HANGOVER: u32 = 20;

/// Etat partage entre le jeu et le fil de capture.
#[derive(Default)]
struct Shared {
    /// La touche de parole est tenue (ou le micro est toujours ouvert).
    held: AtomicBool,
    /// Declenchement par la voix : on transmet quand le niveau depasse le seuil.
    by_voice: AtomicBool,
    /// Seuil et dernier niveau mesure, en bits de f32.
    threshold: AtomicU32,
    level: AtomicU32,
    talking: AtomicBool,
}

/// Decoupe le son en tranches et decide lesquelles partent.
pub struct Framer {
    buf: Vec<f32>,
    hang: u32,
}

impl Framer {
    pub fn new() -> Framer {
        Framer { buf: Vec::with_capacity(voice::FRAME * 2), hang: 0 }
    }

    /// Ajoute du son (mono, 16 kHz). Pour chaque tranche complete, appelle `emit(niveau, Some(octets))`
    /// si elle doit partir, `emit(niveau, None)` sinon.
    pub fn push(&mut self, samples: &[f32], held: bool, by_voice: bool, threshold: f32, mut emit: impl FnMut(f32, Option<Vec<u8>>)) {
        self.buf.extend_from_slice(samples);
        while self.buf.len() >= voice::FRAME {
            let frame: Vec<f32> = self.buf.drain(..voice::FRAME).collect();
            let level = voice::level(&frame);
            let loud = by_voice && level >= threshold;
            if loud {
                self.hang = HANGOVER;
            }
            let open = held || loud || (by_voice && self.hang > 0);
            if !loud {
                self.hang = self.hang.saturating_sub(1);
            }
            emit(level, open.then(|| voice::encode(&frame)));
        }
    }
}

pub struct Mic {
    /// Le flux doit vivre aussi longtemps que la capture.
    _stream: cpal::Stream,
    rx: Receiver<Vec<u8>>,
    shared: Arc<Shared>,
    pub device: String,
}

impl Mic {
    /// Ouvre le micro par defaut. C'est a ce moment que le systeme demande l'autorisation.
    pub fn open() -> Result<Mic, String> {
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or("aucun microphone trouvé")?;
        let name = device.name().unwrap_or_else(|_| "microphone".into());
        let config = device.default_input_config().map_err(|e| format!("micro inutilisable : {e}"))?;
        let (rate, channels) = (config.sample_rate().0, config.channels().max(1) as usize);
        let shared = Arc::new(Shared::default());
        let (tx, rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel();

        // Traitement commun a tous les formats d'echantillons : `data` est deja en f32, canaux entrelaces.
        let work = {
            let shared = shared.clone();
            let (mut state, mut mono, mut framer) = ((0u32, 0.0f32, 0u32), Vec::<f32>::new(), Framer::new());
            move |data: &[f32]| {
                mono.clear();
                let mut down = Vec::with_capacity(data.len() / channels);
                for f in data.chunks(channels) {
                    down.push(f.iter().sum::<f32>() / channels as f32);
                }
                voice::resample(&down, rate, &mut state, &mut mono);
                let (held, by_voice) = (shared.held.load(Ordering::Relaxed), shared.by_voice.load(Ordering::Relaxed));
                let threshold = f32::from_bits(shared.threshold.load(Ordering::Relaxed));
                framer.push(&mono, held, by_voice, threshold, |level, frame| {
                    shared.level.store(level.to_bits(), Ordering::Relaxed);
                    shared.talking.store(frame.is_some(), Ordering::Relaxed);
                    if let Some(f) = frame {
                        let _ = tx.send(f);
                    }
                });
            }
        };
        let fail = |e: cpal::StreamError| eprintln!("Micro : {e}");
        let cfg: cpal::StreamConfig = config.clone().into();
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                let mut work = work;
                device.build_input_stream(&cfg, move |d: &[f32], _: &cpal::InputCallbackInfo| work(d), fail, None)
            }
            cpal::SampleFormat::I16 => {
                let mut work = work;
                device.build_input_stream(
                    &cfg,
                    move |d: &[i16], _: &cpal::InputCallbackInfo| work(&d.iter().map(|x| *x as f32 / 32768.0).collect::<Vec<_>>()),
                    fail,
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let mut work = work;
                device.build_input_stream(
                    &cfg,
                    move |d: &[u16], _: &cpal::InputCallbackInfo| work(&d.iter().map(|x| (*x as f32 - 32768.0) / 32768.0).collect::<Vec<_>>()),
                    fail,
                    None,
                )
            }
            other => return Err(format!("format de micro non géré : {other:?}")),
        }
        .map_err(|e| format!("micro refusé ou indisponible : {e}"))?;
        stream.play().map_err(|e| format!("micro refusé ou indisponible : {e}"))?;
        Ok(Mic { _stream: stream, rx, shared, device: name })
    }

    /// Regle la porte : `held` = on parle maintenant ; `by_voice` = declenchement au niveau sonore.
    pub fn set(&self, held: bool, by_voice: bool, threshold: f32) {
        self.shared.held.store(held, Ordering::Relaxed);
        self.shared.by_voice.store(by_voice, Ordering::Relaxed);
        self.shared.threshold.store(threshold.to_bits(), Ordering::Relaxed);
    }

    /// Niveau sonore capte en ce moment, de 0 a 1.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.shared.level.load(Ordering::Relaxed))
    }

    /// Est-on en train de transmettre ?
    pub fn talking(&self) -> bool {
        self.shared.talking.load(Ordering::Relaxed)
    }

    /// Tranches pretes a partir.
    pub fn frames(&self) -> Vec<Vec<u8>> {
        self.rx.try_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(amp: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| amp * (i as f32 * 0.2).sin()).collect()
    }

    #[test]
    fn the_gate_decides_what_is_sent() {
        let run = |f: &mut Framer, samples: &[f32], held: bool, by_voice: bool| -> Vec<bool> {
            let mut out = Vec::new();
            f.push(samples, held, by_voice, 0.02, |_, frame| out.push(frame.is_some()));
            out
        };
        // Touche relachee, pas de declenchement vocal : rien ne part, meme en criant.
        let mut f = Framer::new();
        assert_eq!(run(&mut f, &tone(0.5, voice::FRAME * 3), false, false), [false; 3]);
        // Touche tenue : tout part, meme le silence. Les tranches font exactement 20 ms.
        let mut sizes = Vec::new();
        f.push(&vec![0.0; voice::FRAME * 2 + 100], true, false, 0.02, |_, fr| sizes.push(fr.map(|d| d.len())));
        assert_eq!(sizes, [Some(voice::FRAME); 2], "le reste attend la tranche suivante");
        // Declenchement par la voix : le silence ne part pas, la voix si, et la porte reste
        // ouverte un instant apres le dernier mot, puis se referme.
        let mut f = Framer::new();
        assert_eq!(run(&mut f, &vec![0.001; voice::FRAME * 2], false, true), [false; 2]);
        assert_eq!(run(&mut f, &tone(0.3, voice::FRAME * 2), false, true), [true; 2]);
        let after = run(&mut f, &vec![0.0; voice::FRAME * (HANGOVER as usize + 5)], false, true);
        assert!(after[..HANGOVER as usize - 1].iter().all(|x| *x), "la fin du mot n'est pas coupee");
        assert!(!after.last().unwrap(), "puis le silence ne part plus");
    }
}
