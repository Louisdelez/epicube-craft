//! Le son du jeu : bruitages places dans l'espace, boucles d'ambiance et musique.
//!
//! Les fichiers sont dans le dossier `audio/` (tous libres de droits, voir audio/CREDITS.md) :
//! - `sfx/<nom>_<n>.*` : bruitages ; plusieurs variantes d'un meme nom sont tirees au hasard ;
//! - `ambiance/<nom>.*` : boucles (oiseaux, grillons, grotte, feu, eau...) dont on regle le niveau ;
//! - `musique/<lieu>_<n>.*` : morceaux, choisis selon l'endroit ou l'on se trouve.
//!
//! Tout passe par des fonctions libres (`audio::play`, `audio::ambience`...) : la sortie audio vit
//! dans le fil principal, et le jeu reste simplement muet si elle manque ou si le dossier est absent.
use glam::Vec3;
use rodio::buffer::SamplesBuffer;
use rodio::source::ChannelVolume;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub const DIR: &str = "audio";
const RATE: u32 = 44_100;
/// Portee par defaut d'un bruitage, en blocs.
pub const RANGE: f32 = 24.0;

/// Ce que la musique doit accompagner.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mood {
    Silence,
    Menu,
    Day,
    Sky,
    Yomi,
    Boss,
    Death,
}

impl Mood {
    /// Prefixe des fichiers de musique de ce lieu.
    fn prefix(self) -> &'static str {
        match self {
            Mood::Silence => "",
            Mood::Menu => "menu",
            Mood::Day => "jour",
            Mood::Sky => "ciel",
            Mood::Yomi => "yomi",
            Mood::Boss => "boss",
            Mood::Death => "mort",
        }
    }

    /// Le morceau reprend-il aussitot fini ? Sinon un silence separe les morceaux, et la mort ne joue qu'une fois.
    fn loops(self) -> bool {
        matches!(self, Mood::Menu | Mood::Boss)
    }
}

/// Niveaux regles par le joueur, de 0 a 1.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Volumes {
    /// Voix des autres joueurs.
    pub voice: f32,
    pub master: f32,
    pub sfx: f32,
    pub ambience: f32,
    pub music: f32,
}

/// Bruitage decode, en mono.
struct Clip {
    rate: u32,
    data: Vec<f32>,
}

struct Loop {
    sink: Sink,
    /// Niveau entendu et niveau vise : le premier rejoint le second en douceur.
    cur: f32,
    target: f32,
}

/// Voix d'un joueur en cours de lecture : ce qui reste a jouer, et son placement.
#[derive(Default)]
struct VoiceState {
    queue: std::collections::VecDeque<f32>,
    /// Gains gauche et droite, regles a chaque image selon la position de celui qui parle.
    gain: (f32, f32),
    /// La lecture attend d'avoir un peu d'avance, pour absorber les irregularites du reseau.
    playing: bool,
    done: bool,
}

/// Source audio d'une voix : stereo, 16 kHz, silencieuse quand il n'y a rien a dire.
struct VoiceOut {
    state: Arc<Mutex<VoiceState>>,
    right: bool,
    cur: f32,
}

/// Avance prise avant de commencer a jouer une voix (60 ms), et retard maximal tolere (0,5 s).
const VOICE_LEAD: usize = shared::voice::FRAME * 3;
const VOICE_LATE: usize = shared::voice::RATE as usize / 2;

impl Iterator for VoiceOut {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let mut s = self.state.lock().ok()?;
        if s.done {
            return None;
        }
        let side = self.right;
        self.right = !self.right;
        if !side {
            // Nouvel instant : un echantillon de plus, pour l'oreille gauche puis la droite.
            if !s.playing && s.queue.len() >= VOICE_LEAD {
                s.playing = true;
            }
            // Tant que l'avance n'est pas prise, on garde tout ce qui arrive.
            let next = if s.playing { s.queue.pop_front() } else { None };
            self.cur = match next {
                Some(x) => x,
                None => {
                    s.playing = false;
                    0.0
                }
            };
            Some(self.cur * s.gain.0)
        } else {
            Some(self.cur * s.gain.1)
        }
    }
}

impl Source for VoiceOut {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        shared::voice::RATE
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

struct Audio {
    voices: HashMap<u32, Arc<Mutex<VoiceState>>>,
    /// La sortie doit vivre aussi longtemps que les sons.
    _stream: OutputStream,
    handle: OutputStreamHandle,
    clips: HashMap<String, Vec<Clip>>,
    loops: HashMap<String, Loop>,
    tracks: HashMap<String, Vec<PathBuf>>,
    music: Option<Sink>,
    mood: Mood,
    /// Lieu du morceau en cours, niveau du fondu, silence restant avant le prochain morceau.
    playing: Mood,
    fade: f32,
    gap: f32,
    last_track: Option<PathBuf>,
    vol: Volumes,
    ear: Vec3,
    right: Vec3,
    seed: u32,
    recent: HashMap<String, Instant>,
}

thread_local! {
    static AUDIO: RefCell<Option<Audio>> = const { RefCell::new(None) };
}

/// Nom d'un fichier sans son extension ni son numero de variante : "pas_herbe_3.ogg" -> "pas_herbe".
pub fn group(file: &str) -> &str {
    let stem = file.rsplit_once('.').map_or(file, |x| x.0);
    match stem.rsplit_once('_') {
        Some((head, tail)) if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => stem,
    }
}

fn audio_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|f| f.path())
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("ogg" | "wav" | "mp3" | "flac")))
        .collect();
    v.sort();
    v
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

/// Decode un fichier entier en mono.
fn decode(path: &Path) -> Option<Clip> {
    let src = Decoder::new(BufReader::new(File::open(path).ok()?)).ok()?;
    let (ch, rate) = (src.channels().max(1) as usize, src.sample_rate());
    let all: Vec<f32> = src.convert_samples().collect();
    let data = all.chunks(ch).map(|c| c.iter().sum::<f32>() / ch as f32).collect();
    Some(Clip { rate, data })
}

/// Ouvre la sortie audio et charge le dossier `dir`. Renvoie un compte rendu, ou None si la machine est muette.
pub fn init(dir: &Path, vol: Volumes) -> Option<String> {
    let (stream, handle) = OutputStream::try_default().ok()?;
    let mut clips: HashMap<String, Vec<Clip>> = HashMap::new();
    for p in audio_files(&dir.join("sfx")) {
        if let Some(c) = decode(&p) {
            clips.entry(group(&name_of(&p)).to_string()).or_default().push(c);
        }
    }
    let mut loops = HashMap::new();
    for p in audio_files(&dir.join("ambiance")) {
        let Ok(file) = File::open(&p) else { continue };
        let (Ok(src), Ok(sink)) = (Decoder::new_looped(BufReader::new(file)), Sink::try_new(&handle)) else { continue };
        sink.set_volume(0.0);
        sink.append(src);
        sink.pause();
        loops.insert(group(&name_of(&p)).to_string(), Loop { sink, cur: 0.0, target: 0.0 });
    }
    let mut tracks: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for p in audio_files(&dir.join("musique")) {
        tracks.entry(group(&name_of(&p)).to_string()).or_default().push(p);
    }
    let report = format!(
        "Son : {} bruitages, {} ambiances, {} musiques",
        clips.values().map(|v| v.len()).sum::<usize>(),
        loops.len(),
        tracks.values().map(|v| v.len()).sum::<usize>()
    );
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(7, |d| d.subsec_nanos()) | 1;
    AUDIO.with(|a| {
        *a.borrow_mut() = Some(Audio {
            voices: HashMap::new(),
            _stream: stream,
            handle,
            clips,
            loops,
            tracks,
            music: None,
            mood: Mood::Silence,
            playing: Mood::Silence,
            fade: 0.0,
            gap: 0.0,
            last_track: None,
            vol,
            ear: Vec3::ZERO,
            right: Vec3::X,
            seed,
            recent: HashMap::new(),
        });
    });
    Some(report)
}

fn with<R>(f: impl FnOnce(&mut Audio) -> R) -> Option<R> {
    AUDIO.with(|a| a.borrow_mut().as_mut().map(f))
}

/// Gains (gauche, droite) d'un son emis en `at` pour une oreille en `ear` dont la droite est `right`.
pub fn spatial(ear: Vec3, right: Vec3, at: Vec3, range: f32) -> (f32, f32) {
    let to = at - ear;
    let d = to.length();
    let gain = (1.0 - d / range).clamp(0.0, 1.0).powf(1.6);
    let pan = if d > 0.5 { (to / d).dot(right) } else { 0.0 };
    (gain * (1.0 - 0.65 * pan.max(0.0)), gain * (1.0 + 0.65 * pan.min(0.0)))
}

impl Audio {
    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    fn emit(&mut self, name: &str, at: Option<Vec3>, volume: f32, pitch: f32, range: f32) {
        let (mut l, mut r) = at.map_or((1.0, 1.0), |p| spatial(self.ear, self.right, p, range));
        let k = volume * self.vol.master * self.vol.sfx;
        l *= k;
        r *= k;
        if l.max(r) < 0.004 {
            return;
        }
        // Le meme bruit lance dix fois dans le meme instant (une explosion qui casse dix blocs) ne joue qu'une fois.
        let now = Instant::now();
        if self.recent.get(name).is_some_and(|t| now.duration_since(*t).as_millis() < 45) {
            return;
        }
        let (pick, jitter) = (self.rand(), self.rand());
        let Some(list) = self.clips.get(name).filter(|l| !l.is_empty()) else { return };
        let clip = &list[((pick * list.len() as f32) as usize).min(list.len() - 1)];
        let src = SamplesBuffer::new(1, clip.rate, clip.data.clone()).speed((pitch * (0.94 + 0.12 * jitter)).clamp(0.3, 3.0));
        let _ = self.handle.play_raw(ChannelVolume::new(src, vec![l.min(1.5), r.min(1.5)]));
        self.recent.insert(name.to_string(), now);
    }

    fn next_track(&mut self) -> Option<PathBuf> {
        let pick = self.rand();
        let list = self.tracks.get(self.mood.prefix())?;
        let fresh: Vec<&PathBuf> = list.iter().filter(|p| list.len() == 1 || Some(*p) != self.last_track.as_ref()).collect();
        fresh.get(((pick * fresh.len() as f32) as usize).min(fresh.len().saturating_sub(1))).map(|p| (*p).clone())
    }

    fn update(&mut self, dt: f32) {
        // Ambiances : chaque boucle glisse vers son niveau, et se tait tout a fait a zero.
        let k = self.vol.master * self.vol.ambience;
        for l in self.loops.values_mut() {
            let step = dt / 1.2;
            l.cur = if l.cur < l.target { (l.cur + step).min(l.target) } else { (l.cur - step).max(l.target) };
            if l.cur * k > 0.001 {
                l.sink.set_volume(l.cur * k);
                if l.sink.is_paused() {
                    l.sink.play();
                }
            } else if !l.sink.is_paused() {
                l.sink.pause();
            }
        }
        // Musique : fondu de sortie quand le lieu change, puis le morceau du nouveau lieu.
        let level = self.vol.master * self.vol.music;
        if self.playing != self.mood {
            self.fade = (self.fade - dt / 1.5).max(0.0);
            if self.fade <= 0.0 || self.music.as_ref().is_none_or(|s| s.empty()) {
                if let Some(s) = self.music.take() {
                    s.stop();
                }
                self.playing = self.mood;
                self.gap = if self.mood == Mood::Day { 4.0 } else { 0.5 };
                self.last_track = None;
            }
        } else if self.mood != Mood::Silence {
            self.fade = (self.fade + dt / 2.0).min(1.0);
            if self.music.as_ref().is_none_or(|s| s.empty()) {
                self.gap -= dt;
                let once_done = self.mood == Mood::Death && self.last_track.is_some();
                if self.gap <= 0.0 && !once_done && level > 0.001 {
                    if let Some(path) = self.next_track() {
                        let src = File::open(&path).ok().and_then(|f| Decoder::new(BufReader::new(f)).ok());
                        if let (Some(src), Ok(sink)) = (src, Sink::try_new(&self.handle)) {
                            sink.set_volume(0.0);
                            sink.append(src);
                            self.music = Some(sink);
                            self.fade = 0.0;
                        }
                        self.last_track = Some(path);
                    }
                    // Entre deux morceaux, un long silence : la musique vient et repart, comme le vent.
                    self.gap = if self.mood.loops() { 0.0 } else { 40.0 + 80.0 * self.rand() };
                }
            }
        }
        if let Some(s) = &self.music {
            s.set_volume(self.fade * level * 0.6);
        }
        if self.recent.len() > 64 {
            self.recent.clear();
        }
    }
}

/// Bruitage sans position (interface, sons du joueur lui-meme).
pub fn play(name: &str, volume: f32) {
    with(|a| a.emit(name, None, volume, 1.0, RANGE));
}

/// Bruitage emis a un endroit du monde.
pub fn play_at(name: &str, at: Vec3, volume: f32) {
    with(|a| a.emit(name, Some(at), volume, 1.0, RANGE));
}

/// Comme `play_at`, avec la hauteur (1 = normale) et la portee choisies.
pub fn play_tuned(name: &str, at: Option<Vec3>, volume: f32, pitch: f32, range: f32) {
    with(|a| a.emit(name, at, volume, pitch, range));
}

/// Place l'oreille : la position des yeux et la direction de la droite.
pub fn listener(ear: Vec3, right: Vec3) {
    with(|a| (a.ear, a.right) = (ear, right));
}

/// Regle le niveau vise d'une boucle d'ambiance, de 0 a 1.
pub fn ambience(name: &str, level: f32) {
    with(|a| {
        if let Some(l) = a.loops.get_mut(name) {
            l.target = level.clamp(0.0, 1.0);
        }
    });
}

/// Fait taire toutes les ambiances (retour au menu).
pub fn hush() {
    with(|a| a.loops.values_mut().for_each(|l| l.target = 0.0));
}

pub fn music(mood: Mood) {
    with(|a| a.mood = mood);
}

pub fn volumes(vol: Volumes) {
    with(|a| a.vol = vol);
}

/// A appeler a chaque image : fondus des ambiances et enchainement des musiques.
pub fn update(dt: f32) {
    with(|a| a.update(dt.min(0.25)));
}

/// Ajoute de la voix a jouer pour le joueur `id` (mono, 16 kHz).
pub fn voice_feed(id: u32, samples: &[f32]) {
    with(|a| {
        let state = a.voices.entry(id).or_insert_with(|| {
            let state = Arc::new(Mutex::new(VoiceState::default()));
            let _ = a.handle.play_raw(VoiceOut { state: state.clone(), right: false, cur: 0.0 });
            state
        });
        if let Ok(mut s) = state.lock() {
            s.queue.extend(samples.iter().copied());
            // Trop de retard accumule : on saute a l'instant present plutot que de trainer.
            while s.queue.len() > VOICE_LATE {
                s.queue.pop_front();
            }
        }
    });
}

/// Place la voix du joueur `id` : a un endroit du monde, ou droit dans les oreilles (None).
pub fn voice_place(id: u32, at: Option<Vec3>) {
    with(|a| {
        let (l, r) = at.map_or((1.0, 1.0), |p| spatial(a.ear, a.right, p, shared::voice::RANGE as f32));
        let k = a.vol.master * a.vol.voice;
        if let Some(Ok(mut s)) = a.voices.get(&id).map(|s| s.lock()) {
            s.gain = (l * k, r * k);
        }
    });
}

/// Oublie la voix d'un joueur parti.
pub fn voice_drop(id: u32) {
    with(|a| {
        if let Some(Ok(mut s)) = a.voices.remove(&id).as_ref().map(|s| s.lock()) {
            s.done = true;
        }
    });
}

/// Coup de tambour taiko. `pitch` : de 0 a 24 demi-tons au-dessus de la note grave.
pub fn drum(pitch: u8, at: Vec3) {
    with(|a| {
        let (l, r) = spatial(a.ear, a.right, at, 32.0);
        let k = a.vol.master * a.vol.sfx;
        if (l.max(r)) * k > 0.01 {
            let src = SamplesBuffer::new(1, RATE, drum_samples(pitch, 1.0));
            let _ = a.handle.play_raw(ChannelVolume::new(src, vec![l * k, r * k]));
        }
    });
}

/// Peau frappee : une note grave dont la hauteur retombe vite, plus le claquement de la baguette.
pub fn drum_samples(pitch: u8, volume: f32) -> Vec<f32> {
    let base = 62.0 * 2f32.powf(pitch.min(24) as f32 / 12.0);
    let n = (RATE as f32 * 0.7) as usize;
    let mut phase = 0.0f32;
    let mut seed = 0x2545_F491u32;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let freq = base * (1.0 + 1.4 * (-t * 28.0).exp());
            phase += freq / RATE as f32;
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = (seed >> 9) as f32 / (1u32 << 22) as f32 - 1.0;
            let tone = (phase * std::f32::consts::TAU).sin() + 0.35 * (phase * 2.0 * std::f32::consts::TAU).sin();
            (tone * (-t * 6.5).exp() * 0.7 + noise * (-t * 70.0).exp() * 0.5) * volume.min(1.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drum_is_audible_and_fades() {
        let s = drum_samples(12, 1.0);
        let peak = |r: std::ops::Range<usize>| s[r].iter().fold(0.0f32, |a, x| a.max(x.abs()));
        assert!(peak(0..4000) > 0.4, "l'attaque s'entend");
        assert!(peak(s.len() - 2000..s.len()) < 0.05, "le son s'eteint");
        assert!(s.iter().all(|x| x.abs() <= 1.3));
    }

    #[test]
    fn voices_wait_for_a_little_lead_then_play_in_stereo() {
        let state = Arc::new(Mutex::new(VoiceState { gain: (1.0, 0.25), ..Default::default() }));
        let mut out = VoiceOut { state: state.clone(), right: false, cur: 0.0 };
        // Pas assez d'avance : silence, et rien n'est consomme.
        state.lock().unwrap().queue.extend(vec![0.5; VOICE_LEAD - 1]);
        assert!((0..40).all(|_| out.next() == Some(0.0)));
        assert_eq!(state.lock().unwrap().queue.len(), VOICE_LEAD - 1);
        // L'avance est la : chaque echantillon sort a gauche puis a droite, avec le gain de chaque oreille.
        state.lock().unwrap().queue.push_back(0.5);
        assert_eq!((out.next(), out.next()), (Some(0.5), Some(0.125)));
        // La file se vide : retour au silence, sans s'arreter.
        for _ in 0..VOICE_LEAD * 2 {
            out.next();
        }
        assert_eq!(out.next(), Some(0.0));
        assert!(!state.lock().unwrap().playing);
        // Le joueur est parti : la source s'eteint.
        state.lock().unwrap().done = true;
        assert_eq!(out.next(), None);
        assert_eq!((out.channels(), out.sample_rate()), (2, shared::voice::RATE));
    }

    #[test]
    fn variants_share_a_name() {
        assert_eq!(group("pas_herbe_3.ogg"), "pas_herbe");
        assert_eq!(group("jour.wav"), "jour");
        assert_eq!(group("sous_eau.ogg"), "sous_eau");
        assert_eq!(group("boss_1.mp3"), "boss");
    }

    #[test]
    fn sounds_fade_with_distance_and_pan() {
        let (ear, right) = (Vec3::ZERO, Vec3::X);
        let (l, r) = spatial(ear, right, Vec3::new(0.0, 0.0, -2.0), RANGE);
        assert!((l - r).abs() < 1e-5 && l > 0.8, "devant : centre et fort");
        let (l, r) = spatial(ear, right, Vec3::new(6.0, 0.0, 0.0), RANGE);
        assert!(r > l * 2.0, "a droite : surtout dans l'oreille droite ({l}, {r})");
        let far = spatial(ear, right, Vec3::new(0.0, 0.0, 20.0), RANGE).0;
        let near = spatial(ear, right, Vec3::new(0.0, 0.0, 5.0), RANGE).0;
        assert!(far < near * 0.3 && far > 0.0);
        assert_eq!(spatial(ear, right, Vec3::new(0.0, 0.0, 30.0), RANGE), (0.0, 0.0), "hors de portee : silence");
    }

    /// Tous les sons que le jeu demande existent dans le dossier audio, et se decodent.
    #[test]
    fn every_sound_the_game_uses_exists() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(DIR);
        let have = |sub: &str| -> std::collections::HashSet<String> { audio_files(&dir.join(sub)).iter().map(|p| group(&name_of(p)).to_string()).collect() };
        let (sfx, amb, mus) = (have("sfx"), have("ambiance"), have("musique"));
        for name in crate::sounds::SFX {
            assert!(sfx.contains(*name), "bruitage manquant : {name}");
        }
        for name in crate::sounds::AMBIENCES {
            assert!(amb.contains(*name), "ambiance manquante : {name}");
        }
        for mood in [Mood::Menu, Mood::Day, Mood::Sky, Mood::Yomi, Mood::Boss, Mood::Death] {
            assert!(mus.contains(mood.prefix()), "musique manquante : {}", mood.prefix());
        }
        for p in audio_files(&dir.join("sfx")) {
            let c = decode(&p).unwrap_or_else(|| panic!("{} illisible", p.display()));
            assert!(!c.data.is_empty() && c.data.len() < c.rate as usize * 12, "{} : duree anormale", p.display());
        }
    }
}
