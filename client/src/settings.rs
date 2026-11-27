//! Parametres du joueur, conserves dans le fichier epicube-craft.cfg.
use std::fs;

const FILE: &str = "epicube-craft.cfg";
const OLD_FILE: &str = "voxterra.cfg";

#[derive(Clone, PartialEq)]
pub struct Settings {
    /// Distance de vue en chunks.
    pub view: i32,
    /// Champ de vision vertical, en degres.
    pub fov: f32,
    /// Sensibilite de la souris (1 = normale).
    pub sens: f32,
    /// Taille de l'interface : 0 = automatique, sinon le facteur.
    pub gui: u32,
    pub vsync: bool,
    pub fullscreen: bool,
    pub clouds: bool,
    /// Balancement de la camera en marchant.
    pub bobbing: bool,
    /// Contours a l'encre autour des formes.
    pub ink: bool,
    /// Ouvre la partie solo aux autres machines du reseau.
    pub lan: bool,
    /// Resolution d'affichage : 0 = automatique (celle de l'ecran, divisee par sa densite), sinon 50, 75 ou 100 %.
    pub res: u32,
    /// Images par seconde au plus ; 0 = sans limite.
    pub fps_max: u32,
    /// Micro : 0 = coupe, 1 = appuyer sur V pour parler, 2 = declenchement par la voix, 3 = ouvert en permanence.
    pub mic: u8,
    /// Niveau sonore a partir duquel la voix declenche la transmission (mode 2).
    pub mic_gate: f32,
    pub vol_voice: f32,
    /// Volumes de 0 a 1 : general, musique, bruitages, ambiance.
    pub vol_master: f32,
    pub vol_music: f32,
    pub vol_sfx: f32,
    pub vol_ambience: f32,
    pub name: String,
    pub address: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            view: 8,
            fov: 70.0,
            sens: 1.0,
            gui: 0,
            vsync: true,
            fullscreen: false,
            clouds: true,
            bobbing: true,
            ink: true,
            lan: false,
            res: 0,
            fps_max: 0,
            mic: 0,
            mic_gate: 0.02,
            vol_voice: 1.0,
            vol_master: 1.0,
            vol_music: 0.6,
            vol_sfx: 1.0,
            vol_ambience: 0.7,
            name: "Joueur".to_string(),
            address: "127.0.0.1".to_string(),
        }
    }
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings::default();
        // Le fichier portait l'ancien nom du jeu : on le reprend tel quel.
        if !std::path::Path::new(FILE).exists() && std::path::Path::new(OLD_FILE).exists() {
            let _ = fs::rename(OLD_FILE, FILE);
        }
        let Ok(text) = fs::read_to_string(FILE) else { return s };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let flag = v == "1" || v == "true";
            match k {
                "vue" => s.view = v.parse().unwrap_or(s.view).clamp(4, 12),
                "champ" => s.fov = v.parse().unwrap_or(s.fov).clamp(50.0, 110.0),
                "sensibilite" => s.sens = v.parse().unwrap_or(s.sens).clamp(0.2, 3.0),
                "interface" => s.gui = v.parse().unwrap_or(0).min(4),
                "vsync" => s.vsync = flag,
                "plein_ecran" => s.fullscreen = flag,
                "nuages" => s.clouds = flag,
                "balancement" => s.bobbing = flag,
                "contours" => s.ink = flag,
                "reseau_local" => s.lan = flag,
                "resolution" => s.res = v.parse().ok().filter(|r| [0, 50, 75, 100].contains(r)).unwrap_or(0),
                "images_max" => s.fps_max = v.parse().unwrap_or(0).min(1000),
                "micro" => s.mic = v.parse().unwrap_or(0).min(3),
                "seuil_micro" => s.mic_gate = v.parse().unwrap_or(s.mic_gate).clamp(0.002, 0.2),
                "voix" => s.vol_voice = v.parse().unwrap_or(s.vol_voice).clamp(0.0, 1.0),
                "volume" => s.vol_master = v.parse().unwrap_or(s.vol_master).clamp(0.0, 1.0),
                "musique" => s.vol_music = v.parse().unwrap_or(s.vol_music).clamp(0.0, 1.0),
                "bruitages" => s.vol_sfx = v.parse().unwrap_or(s.vol_sfx).clamp(0.0, 1.0),
                "ambiance" => s.vol_ambience = v.parse().unwrap_or(s.vol_ambience).clamp(0.0, 1.0),
                "pseudo" if !v.is_empty() => s.name = v.chars().take(16).collect(),
                "adresse" if !v.is_empty() => s.address = v.chars().take(40).collect(),
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        let text = format!(
            "vue={}\nchamp={}\nsensibilite={}\ninterface={}\nvsync={}\nplein_ecran={}\nnuages={}\nbalancement={}\ncontours={}\nreseau_local={}\nresolution={}\nimages_max={}\nmicro={}\nseuil_micro={}\nvoix={}\nvolume={}\nmusique={}\nbruitages={}\nambiance={}\npseudo={}\nadresse={}\n",
            self.view,
            self.fov,
            self.sens,
            self.gui,
            self.vsync as u8,
            self.fullscreen as u8,
            self.clouds as u8,
            self.bobbing as u8,
            self.ink as u8,
            self.lan as u8,
            self.res,
            self.fps_max,
            self.mic,
            self.mic_gate,
            self.vol_voice,
            self.vol_master,
            self.vol_music,
            self.vol_sfx,
            self.vol_ambience,
            self.name,
            self.address
        );
        if let Err(e) = fs::write(FILE, text) {
            eprintln!("Parametres non enregistres : {e}");
        }
    }

    /// Part de la resolution de la fenetre a dessiner, pour un ecran de densite `density`.
    pub fn res_scale(&self, density: f64) -> f32 {
        match self.res {
            // Automatique : les pixels « logiques » de l'ecran, ceux que dessine toute application ordinaire.
            0 => (1.0 / density.max(1.0)) as f32,
            r => r as f32 / 100.0,
        }
    }

    pub fn volumes(&self) -> crate::audio::Volumes {
        crate::audio::Volumes { voice: self.vol_voice, master: self.vol_master, sfx: self.vol_sfx, ambience: self.vol_ambience, music: self.vol_music }
    }

    /// Facteur d'echelle de l'interface pour une fenetre de hauteur `h`.
    pub fn scale(&self, h: f32) -> f32 {
        if self.gui == 0 {
            (h / 360.0).floor().max(1.0)
        } else {
            (self.gui as f32).min((h / 240.0).floor().max(1.0))
        }
    }
}
