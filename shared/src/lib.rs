//! Code partage entre le client et le serveur d'Epicube Craft.
pub mod blocks;
pub mod chunk;
pub mod commands;
pub mod craft;
pub mod gen;
pub mod items;
pub mod mobs;
pub mod physics;
pub mod proto;
pub mod rules;
pub mod seed;
pub mod signal;
pub mod structures;
pub mod trade;
pub mod util;
pub mod voice;

/// Port par defaut du serveur.
pub const DEFAULT_PORT: u16 = 25600;
/// Duree d'un jour complet, en secondes.
pub const DAY_SECONDS: f64 = 600.0;
/// Bordure du monde, comme dans Minecraft : on ne va pas au-dela de cette coordonnee, en X comme en Z.
/// Le monde fait donc pres de 60 millions de blocs de cote.
pub const WORLD_BORDER: f64 = 29_999_984.0;
/// Ticks serveur par seconde.
pub const TPS: u32 = 20;
