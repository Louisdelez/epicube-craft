//! Client Epicube Craft.
//! Usage : epicube-craft [--solo] [--connect hote:port] [--name Pseudo] [--seed 1337] [--world monde] [--lan]
//!         epicube-craft --export-assets     ecrit textures et modeles dans le dossier assets
//! Sans option, le jeu s'ouvre sur l'ecran-titre.
// Windows : pas de fenetre de terminal a cote du jeu (la version de mise au point la garde, pour ses messages).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod assets;
mod atlas;
mod audio;
mod chatbox;
mod sounds;
mod gfx;
mod hud;
mod itemdefs;
mod items3d;
mod menu;
mod mesh;
mod mesher;
mod mic;
mod models;
mod net;
mod pixels;
mod prof;
mod settings;
mod ui;

use gfx::{Gfx, Post, Uniforms, Vtx};
use glam::{EulerRot, IVec3, Mat4, Quat, Vec3};
use menu::Action;
use models::Model;
use net::Net;
use settings::Settings;
use shared::blocks::{self, shape, t, Shape, AIR, WATER};
use shared::chunk::{idx, Chunk, H};
use shared::items::{self, Slot};
use shared::mobs as mk;
use shared::physics::{move_body, step_drop, BlockSource};
use shared::proto::{ClientMsg, ServerMsg, WindowKind, PROTOCOL_VERSION};
use shared::util::{fbm2, hash};
use shared::{DAY_SECONDS, TPS};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;
use std::sync::Arc;
use std::time::{Duration, Instant};
use ui::Ui;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowBuilder};

const HALF_W: f32 = 0.3;
const HEIGHT: f32 = 1.8;
const EYE: f32 = 1.62;
const REACH: f32 = 5.0;
/// Altitude et taille des cases de nuages.
const CLOUD_Y: f32 = 250.0;
const CLOUD_CELL: f32 = 12.0;

#[derive(Default)]
pub struct Input {
    keys: HashSet<KeyCode>,
    pressed: Vec<KeyCode>,
    dx: f32,
    dy: f32,
    pub lmb: bool,
    rmb: bool,
    pub lmb_click: bool,
    rmb_click: bool,
    scroll: f32,
    /// Position du curseur en pixels.
    pub mouse: (f32, f32),
    /// Texte tape depuis la derniere image, et nombre de retours arriere.
    pub text: String,
    pub backspace: u32,
}

impl Input {
    fn down(&self, k: KeyCode) -> bool {
        self.keys.contains(&k)
    }
    fn axis(&self, pos: KeyCode, neg: KeyCode) -> f32 {
        self.down(pos) as i32 as f32 - self.down(neg) as i32 as f32
    }
    pub fn shift(&self) -> bool {
        self.down(KeyCode::ShiftLeft) || self.down(KeyCode::ShiftRight)
    }
}

struct World {
    chunks: HashMap<(i32, i32), Chunk>,
}

impl World {
    fn block(&self, x: i32, y: i32, z: i32) -> (u8, u8) {
        if y < 0 || y >= H as i32 {
            return (AIR, 0);
        }
        match self.chunks.get(&(x >> 4, z >> 4)) {
            Some(c) => {
                let i = idx((x & 15) as usize, y as usize, (z & 15) as usize);
                (c.b[i], c.m[i])
            }
            None => (AIR, 0),
        }
    }
    fn block_at(&self, p: Vec3) -> u8 {
        self.block(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32).0
    }

    /// Premier bloc touche par le rayon : (bloc, case juste avant).
    fn raycast(&self, o: Vec3, d: Vec3, max: f32, fluids: bool) -> Option<(IVec3, IVec3)> {
        let mut p = o.floor().as_ivec3();
        let step = IVec3::new(d.x.signum() as i32, d.y.signum() as i32, d.z.signum() as i32);
        let delta = |v: f32| if v != 0.0 { 1.0 / v.abs() } else { f32::INFINITY };
        let td = Vec3::new(delta(d.x), delta(d.y), delta(d.z));
        let first = |o: f32, p: i32, v: f32, td: f32| {
            if v > 0.0 {
                (p as f32 + 1.0 - o) * td
            } else if v < 0.0 {
                (o - p as f32) * td
            } else {
                f32::INFINITY
            }
        };
        let mut tm = Vec3::new(first(o.x, p.x, d.x, td.x), first(o.y, p.y, d.y, td.y), first(o.z, p.z, d.z, td.z));
        let mut prev = p;
        loop {
            let (b, m) = self.block(p.x, p.y, p.z);
            // Avec un seau vide, on vise aussi les sources de fluide.
            if b != AIR && (shape(b) != Shape::Fluid || (fluids && m == 0)) {
                return Some((p, prev));
            }
            prev = p;
            if tm.min_element() > max {
                return None;
            }
            if tm.x <= tm.y && tm.x <= tm.z {
                p.x += step.x;
                tm.x += td.x;
            } else if tm.y <= tm.z {
                p.y += step.y;
                tm.y += td.y;
            } else {
                p.z += step.z;
                tm.z += td.z;
            }
        }
    }
}

impl BlockSource for World {
    fn solid_at(&self, x: i32, y: i32, z: i32) -> bool {
        if y < 0 || y >= H as i32 {
            return false;
        }
        match self.chunks.get(&(x >> 4, z >> 4)) {
            Some(c) => {
                let i = idx((x & 15) as usize, y as usize, (z & 15) as usize);
                blocks::solid(c.b[i], c.m[i])
            }
            // Zone pas encore recue : mur invisible.
            None => true,
        }
    }
}

struct Remote {
    name: String,
    target: Vec3,
    shown: Vec3,
    yaw: f32,
    /// Avancement du pas, pour animer les jambes.
    walk: f32,
    /// Pieces d'armure portees (tete, torse, jambes, pieds), 0 = rien.
    gear: [u16; 4],
    /// Secondes pendant lesquelles on le considere encore en train de parler.
    talk: f32,
}

struct MobView {
    kind: u8,
    target: Vec3,
    shown: Vec3,
    yaw: f32,
    baby: bool,
    /// Temps restant du flash rouge apres un coup.
    hurt: f32,
    walk: f32,
    /// Secondes avant son prochain cri.
    voice: f32,
    /// Positions passees de la tete : le corps du Wyrm les suit.
    trail: Vec<Vec3>,
}

struct ProjView {
    pos: Vec3,
    vel: Vec3,
    /// 0 = fleche, 1 = boule de feu.
    kind: u8,
}

struct DropView {
    pos: Vec3,
    vel: Vec3,
    item: u16,
    age: f32,
}

/// Eclat projete quand un bloc casse.
struct Particle {
    pos: Vec3,
    vel: Vec3,
    life: f32,
    tile: u16,
}

/// Distance a laquelle le rayon entre dans la boite, s'il la touche.
fn ray_box(o: Vec3, d: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let inv = Vec3::ONE / d;
    let (t0, t1) = ((min - o) * inv, (max - o) * inv);
    let near = t0.min(t1).max_element();
    let far = t0.max(t1).min_element();
    (far >= near.max(0.0)).then_some(near.max(0.0))
}

/// Vecteur recu du serveur (vitesse, recul) : la simple precision lui suffit.
fn v3(v: [f64; 3]) -> Vec3 {
    Vec3::new(v[0] as f32, v[1] as f32, v[2] as f32)
}

fn chunk_of(p: Vec3) -> (i32, i32) {
    ((p.x.floor() as i32) >> 4, (p.z.floor() as i32) >> 4)
}

fn mix3(a: [f32; 3], b: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k]
}

fn rgb8(c: [f32; 3], a: u8) -> [u8; 4] {
    [(c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, a]
}

/// Texture d'une boite.
#[derive(Clone, Copy)]
enum Tex<'a> {
    /// Couleur unie.
    Flat,
    /// La meme tuile sur les six faces.
    Tile(u16),
    /// Un rectangle de l'atlas par face.
    Faces(&'a [[f32; 4]; 6]),
}

/// Repere (x, haut, avant) d'un objet tourne de `yaw` autour de la verticale.
/// Il est direct, comme celui de Blockbench : un modele s'affiche tel qu'on l'y voit, sans effet miroir.
fn yaw_basis(yaw: f32) -> [Vec3; 3] {
    [Vec3::new(yaw.cos(), 0.0, -yaw.sin()), Vec3::Y, Vec3::new(yaw.sin(), 0.0, yaw.cos())]
}

/// Boite orientee, en triangles non indexes.
#[allow(clippy::too_many_arguments)]
fn push_box(out: &mut Vec<Vtx>, origin: Vec3, basis: [Vec3; 3], center: Vec3, half: Vec3, color: [u8; 4], tex: Tex, light: [u8; 4]) {
    let (wu, wv) = atlas::tile_uv(t::WHITE);
    for (f, (_, corners)) in mesh::FACES.iter().enumerate() {
        let s = mesh::SHADE[f] as u32;
        let col = [
            (color[0] as u32 * s / 255) as u8,
            (color[1] as u32 * s / 255) as u8,
            (color[2] as u32 * s / 255) as u8,
            color[3],
        ];
        let vert = |k: usize| {
            let l = center + (Vec3::from(corners[k]) * 2.0 - Vec3::ONE) * half;
            let [a, b] = mesh::UVS[k];
            let uv = match tex {
                Tex::Flat => [wu + 0.5 / 32.0, wv + 0.5 / 32.0],
                Tex::Tile(tile) => {
                    let (u, v) = atlas::tile_uv(tile);
                    [u + (0.02 + a * 0.96) / 32.0, v + (0.02 + b * 0.96) / 32.0]
                }
                Tex::Faces(r) => [r[f][0] + a * (r[f][2] - r[f][0]), r[f][1] + b * (r[f][3] - r[f][1])],
            };
            Vtx { pos: (origin + basis[0] * l.x + basis[1] * l.y + basis[2] * l.z).to_array(), uv, color: col, light }
        };
        for k in [0, 1, 2, 0, 2, 3] {
            out.push(vert(k));
        }
    }
}

/// Dessine un objet en volume : `size` est sa largeur en blocs, `origin` son centre.
fn push_item(out: &mut Vec<Vtx>, m: &Model, origin: Vec3, basis: [Vec3; 3], size: f32, light: [u8; 4]) {
    let world = |v: Vec3| basis[0] * v.x + basis[1] * v.y + basis[2] * v.z;
    for p in &m.parts {
        let q = Quat::from_euler(EulerRot::ZYX, p.rot.z.to_radians(), p.rot.y.to_radians(), p.rot.x.to_radians());
        let center = p.pivot + q * (p.c - p.pivot);
        let axes = [world(q * Vec3::X), world(q * Vec3::Y), world(q * Vec3::Z)];
        push_box(out, origin + world(center * size), axes, Vec3::ZERO, p.h * size, [255; 4], Tex::Faces(&p.uv), light);
    }
}

/// Dessine un modele ; `walk` balance les membres autour de leur pivot, `k` est l'echelle.
#[allow(clippy::too_many_arguments)]
fn push_model(out: &mut Vec<Vtx>, m: &Model, origin: Vec3, yaw: f32, k: f32, walk: f32, color: [u8; 4], light: [u8; 4]) {
    let basis = yaw_basis(yaw);
    let world = |v: Vec3| basis[0] * v.x + basis[1] * v.y + basis[2] * v.z;
    for p in &m.parts {
        // Meme ordre de rotation que Blockbench : z, puis y, puis x.
        let mut q = Quat::from_euler(EulerRot::ZYX, p.rot.z.to_radians(), p.rot.y.to_radians(), p.rot.x.to_radians());
        if p.leg != 0 {
            q = Quat::from_rotation_x(walk.sin() * 0.6 * p.leg as f32) * q;
        }
        let center = p.pivot + q * (p.c - p.pivot);
        let axes = [world(q * Vec3::X), world(q * Vec3::Y), world(q * Vec3::Z)];
        push_box(out, origin + world(center * k), axes, Vec3::ZERO, p.h * k, color, Tex::Faces(&p.uv), light);
    }
}

/// Armure portee par un joueur : des plaques posees sur son modele, qui suivent le pas des jambes.
fn push_gear(out: &mut Vec<Vtx>, origin: Vec3, yaw: f32, walk: f32, gear: [u16; 4], light: [u8; 4]) {
    let basis = yaw_basis(yaw);
    let world = |v: Vec3| basis[0] * v.x + basis[1] * v.y + basis[2] * v.z;
    gear_boxes(walk, gear, &mut |center, q, h, col| {
        let axes = [world(q * Vec3::X), world(q * Vec3::Y), world(q * Vec3::Z)];
        push_box(out, origin + world(center), axes, Vec3::ZERO, h, [col[0], col[1], col[2], 255], Tex::Flat, light);
    });
}

/// Les plaques d'une tenue, dans le repere du modele du joueur : centre, orientation, demi-tailles, couleur.
pub(crate) fn gear_boxes(walk: f32, gear: [u16; 4], emit: &mut dyn FnMut(Vec3, Quat, Vec3, [u8; 3])) {
    // (plaques, lacage, garniture) de chaque matiere.
    const LOOK: [[[u8; 3]; 3]; 6] = [
        [[160, 106, 62], [228, 216, 180], [110, 70, 48]],
        [[184, 190, 106], [58, 42, 34], [111, 122, 52]],
        [[84, 90, 106], [192, 54, 44], [210, 217, 226]],
        [[232, 185, 58], [142, 42, 38], [255, 227, 138]],
        [[95, 221, 216], [43, 110, 154], [200, 250, 244]],
        [[58, 72, 120], [232, 185, 58], [232, 185, 58]],
    ];
    // Boite fixe (leg = 0) ou attachee a une jambe qui se balance autour de la hanche.
    let mut bx = |c: [f32; 3], h: [f32; 3], col: [u8; 3], leg: i8| {
        let (c, h) = (Vec3::from(c), Vec3::from(h));
        let q = Quat::from_rotation_x(walk.sin() * 0.6 * leg as f32);
        let pivot = Vec3::new(c.x, 0.76, 0.0);
        let center = if leg == 0 { c } else { pivot + q * (c - pivot) };
        emit(center, q, h, col);
    };
    let look = |id: u16| items::armor(id).map(|(_, tier)| LOOK[tier as usize]);
    if let Some([plate, lace, trim]) = look(gear[0]) {
        if items::armor(gear[0]).is_some_and(|a| a.1 == 0) {
            // Chapeau kasa : un large cone de paille.
            let straw = [222, 196, 130];
            bx([0.0, 2.0, 0.0], [0.44, 0.025, 0.44], straw, 0);
            bx([0.0, 2.07, 0.0], [0.27, 0.05, 0.27], straw, 0);
            bx([0.0, 2.15, 0.0], [0.11, 0.04, 0.11], plate, 0);
        } else {
            // Kabuto : bol, visiere, couvre-nuque lace et cornes du cimier.
            bx([0.0, 1.95, -0.01], [0.27, 0.12, 0.27], plate, 0);
            bx([0.0, 2.08, -0.01], [0.19, 0.04, 0.19], plate, 0);
            bx([0.0, 1.88, 0.3], [0.2, 0.02, 0.05], plate, 0);
            bx([0.0, 1.74, -0.29], [0.3, 0.1, 0.04], lace, 0);
            bx([0.0, 1.66, -0.31], [0.32, 0.03, 0.04], plate, 0);
            for sx in [-1.0, 1.0] {
                bx([sx * 0.29, 1.74, -0.08], [0.04, 0.1, 0.2], lace, 0);
                bx([sx * 0.12, 2.12, 0.27], [0.03, 0.12, 0.015], trim, 0);
                bx([sx * 0.19, 2.24, 0.27], [0.03, 0.05, 0.015], trim, 0);
            }
            bx([0.0, 2.02, 0.28], [0.06, 0.05, 0.015], trim, 0);
        }
    }
    if gear[1] == items::HAGOROMO {
        // Robe de plumes : une longue robe claire bordee de duvet.
        bx([0.0, 1.04, 0.0], [0.28, 0.42, 0.17], [246, 238, 248], 0);
        bx([0.0, 0.84, 0.0], [0.29, 0.05, 0.18], [208, 112, 160], 0);
        bx([0.0, 0.64, 0.0], [0.3, 0.04, 0.19], [200, 226, 255], 0);
    } else if let Some([plate, lace, trim]) = look(gear[1]) {
        // Cuirasse do : plastron lace, epaulieres et jupe de plaques.
        bx([0.0, 1.1, 0.0], [0.275, 0.33, 0.165], plate, 0);
        for y in [0.94, 1.1, 1.26] {
            bx([0.0, y, 0.0], [0.28, 0.018, 0.17], lace, 0);
        }
        bx([0.0, 0.76, 0.0], [0.29, 0.06, 0.18], trim, 0);
        for sx in [-1.0, 1.0] {
            bx([sx * 0.37, 1.36, 0.0], [0.12, 0.1, 0.13], plate, 0);
            bx([sx * 0.37, 1.3, 0.0], [0.125, 0.018, 0.135], lace, 0);
        }
    }
    if let Some([plate, lace, _]) = look(gear[2]) {
        // Cuissards haidate.
        for (sx, leg) in [(1.0, 1i8), (-1.0, -1)] {
            bx([sx * 0.12, 0.5, 0.0], [0.125, 0.22, 0.135], plate, leg);
            bx([sx * 0.12, 0.5, 0.0], [0.13, 0.018, 0.14], lace, leg);
        }
    }
    if let Some([plate, lace, trim]) = look(gear[3]) {
        // Jambieres suneate et sandales.
        for (sx, leg) in [(1.0, 1i8), (-1.0, -1)] {
            bx([sx * 0.12, 0.17, 0.0], [0.13, 0.12, 0.14], plate, leg);
            bx([sx * 0.12, 0.27, 0.0], [0.135, 0.02, 0.145], lace, leg);
            bx([sx * 0.12, 0.03, 0.03], [0.13, 0.03, 0.18], trim, leg);
        }
    }
}

/// Secondes laissees au monde pour se charger avant de mesurer.
const BENCH_WARMUP: f32 = 14.0;

/// Aretes d'un cube : contour du bloc vise.
fn push_outline(out: &mut Vec<Vtx>, p: IVec3) {
    let c = p.as_vec3() + Vec3::splat(0.5);
    let (h, th) = (0.502, 0.006);
    for axis in 0..3 {
        for (a, b) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
            let mut off = [0.0f32; 3];
            let mut half = [th; 3];
            half[axis] = h;
            off[(axis + 1) % 3] = a * h;
            off[(axis + 2) % 3] = b * h;
            push_box(out, c, yaw_basis(0.0), Vec3::from(off), Vec3::from(half), [12, 12, 12, 255], Tex::Flat, [0, 0, 255, 0]);
        }
    }
}

/// Carre face a la camera dans le ciel (soleil, lune, etoile), sans eclairage ni brouillard.
fn push_sky_quad(out: &mut Vec<Vtx>, cam: Vec3, dir: Vec3, dist: f32, size: f32, color: [u8; 4], tile: u16) {
    let r = dir.cross(Vec3::Z).normalize_or_zero() * size;
    let up = r.cross(dir).normalize_or_zero() * size;
    let c = cam + dir * dist;
    let (u, v) = atlas::tile_uv(tile);
    let e = 1.0 / 32.0;
    let (uvs, p) = if tile == t::WHITE {
        ([[u + e / 2.0, v + e / 2.0]; 4], [c - r - up, c + r - up, c + r + up, c - r + up])
    } else {
        let m = 0.6 / atlas::AT as f32;
        ([[u + m, v + e - m], [u + e - m, v + e - m], [u + e - m, v + m], [u + m, v + m]], [c - r - up, c + r - up, c + r + up, c - r + up])
    };
    for k in [0, 1, 2, 0, 2, 3] {
        out.push(Vtx { pos: p[k].to_array(), uv: uvs[k], color, light: [0, 0, 255, 0] });
    }
}

/// Voute du ciel : horizon clair, zenith plus fonce.
fn push_sky_dome(out: &mut Vec<Vtx>, cam: Vec3, zenith: [f32; 3], horizon: [f32; 3]) {
    let (u, v) = atlas::tile_uv(t::WHITE);
    let uv = [u + 0.5 / 32.0, v + 0.5 / 32.0];
    let (zc, hc) = (rgb8(zenith, 255), rgb8(horizon, 255));
    let r = 100.0;
    let top = r * 0.55;
    let mut quad = |p: [Vec3; 4], c: [[u8; 4]; 4]| {
        for k in [0, 1, 2, 0, 2, 3] {
            out.push(Vtx { pos: (cam + p[k]).to_array(), uv, color: c[k], light: [0, 0, 255, 0] });
        }
    };
    for (a, b) in [((1.0, 1.0), (1.0, -1.0)), ((1.0, -1.0), (-1.0, -1.0)), ((-1.0, -1.0), (-1.0, 1.0)), ((-1.0, 1.0), (1.0, 1.0))] {
        let (p0, p1) = (Vec3::new(a.0 * r, 0.0, a.1 * r), Vec3::new(b.0 * r, 0.0, b.1 * r));
        // Sous l'horizon : couleur unie ; au-dessus : degrade jusqu'au zenith.
        quad([p0 - Vec3::Y * r, p1 - Vec3::Y * r, p1, p0], [hc; 4]);
        quad([p0, p1, p1 + Vec3::Y * top, p0 + Vec3::Y * top], [hc, hc, zc, zc]);
    }
    let y = Vec3::Y * top;
    quad([Vec3::new(-r, 0.0, -r) + y, Vec3::new(r, 0.0, -r) + y, Vec3::new(r, 0.0, r) + y, Vec3::new(-r, 0.0, r) + y], [zc; 4]);
}

/// Ce qu'une image de jeu demande au moteur de rendu.
pub struct FrameOut {
    u: Uniforms,
    clear: [f32; 3],
    eye: Vec3,
    dir: Vec3,
    sky: Vec<Vtx>,
    ents: Vec<Vtx>,
    overlay: Vec<Vtx>,
    ui: Ui,
    /// Tangente du demi-champ de vision, pour la passe de contours.
    tan_half: f32,
    /// Le joueur a appuye sur Echap : ouvrir le menu de pause.
    pause: bool,
}

/// Une partie en cours : l'etat recu du serveur et le joueur local.
pub struct App {
    net: Net,
    world: World,
    dirty: HashSet<(i32, i32)>,
    /// Chunks a reconstruire tout de suite (un bloc y a change).
    urgent: HashSet<(i32, i32)>,
    /// Fils de travail du maillage, et numero du monde en cours (les resultats d'un autre monde sont ignores).
    mesher: mesher::Mesher,
    /// Le chat : lignes recues et saisie en cours.
    chat: chatbox::Chat,
    /// Notre micro transmet en ce moment (pour l'indicateur a l'ecran).
    pub mic_live: bool,
    /// Mode de jeu du joueur et regles du monde, annonces par le serveur.
    pub mode: u8,
    pub rules: shared::rules::Rules,
    /// Secondes depuis le dernier appui sur Espace (deux appuis rapproches : on s'envole, en creatif).
    space_t: f32,
    /// Premiere rangee affichee de l'inventaire creatif.
    pub palette_row: usize,
    /// Messages du serveur recus mais pas encore traites.
    inbox: std::collections::VecDeque<ServerMsg>,
    epoch: u32,
    joined: bool,
    lost: Option<String>,
    pos: Vec3,
    vel: Vec3,
    yaw: f32,
    pitch: f32,
    on_ground: bool,
    /// Le joueur a bute contre un bloc a la derniere image.
    hit_wall: bool,
    fly: bool,
    time: f32,
    others: HashMap<u32, Remote>,
    drops: HashMap<u32, DropView>,
    mobs: HashMap<u32, MobView>,
    /// Chariot dans lequel on est assis.
    riding: Option<u32>,
    /// Graine du monde, telle qu'elle a ete choisie (partie solo) ou recue du serveur.
    pub seed_label: Option<String>,
    /// Origine d'affichage, en blocs (X, Z), multiple de 1024. Le monde fait 60 millions de blocs de large,
    /// bien plus que ce qu'un flottant simple sait placer au bloc pres : le client travaille donc en
    /// coordonnees locales, proches de zero, et ne convertit qu'en parlant au serveur.
    pub origin: [i32; 2],
    /// Une nouvelle origine a ete demandee au serveur ; on attend qu'il renvoie tout.
    rebasing: bool,
    /// Son : dernier pas compte, etat dans l'eau, cadences des coups de pioche et de l'ecoute des alentours.
    step_n: i32,
    wet: bool,
    dig_snd: f32,
    scan_t: f32,
    near: [f32; 3],
    projs: HashMap<u32, ProjView>,
    particles: Vec<Particle>,
    /// Lumiere calculee avec les maillages, pour eclairer les entites.
    light: HashMap<(i32, i32), Vec<u8>>,
    air: f32,
    hit_cd: f32,
    /// Secondes ecoulees depuis le dernier coup : l'arme revient en garde.
    since_hit: f32,
    /// Secondes de tension de l'arc.
    draw_t: f32,
    blocking: bool,
    /// Robe de plumes deployee : on plane.
    gliding: bool,
    jump_prev: bool,
    /// 0 surface, 1 Abime, 2 Zenith.
    dim: u8,
    /// Vie du boss (0 a 1) quand il y en a un.
    boss: Option<f32>,
    /// Pseudos a afficher : (x, y a l'ecran, texte).
    labels: Vec<(f32, f32, String)>,
    debug: bool,
    fps: f32,
    /// Durees des dernieres images, en millisecondes (les plus recentes a la fin).
    pub frames: Vec<f32>,
    /// Delai aller-retour avec le serveur, en millisecondes, et cadence qu'il annonce.
    pub ping_ms: f32,
    ping_acc: f32,
    pub server_tps: f32,
    pub server_mspt: f32,
    born: Instant,
    /// Graine reduite a 32 bits : elle permet de retrouver le biome et la structure du lieu.
    pub world_seed: u32,
    /// Lignes des frontieres de chunks (F6).
    chunk_borders: bool,
    send_acc: f32,
    /// Secondes ecoulees depuis le debut de la partie (animations).
    clock: f32,
    // Etat recu du serveur.
    inv: Vec<Slot>,
    /// Equipement porte : tete, torse, jambes, pieds.
    armor: Vec<Slot>,
    cursor: Slot,
    craft: Vec<Slot>,
    craft_out: Slot,
    window: WindowKind,
    /// Offres du villageois avec qui l'on commerce : (prix, marchandise).
    trades: Vec<(items::Stack, items::Stack)>,
    trade_title: String,
    cont: Vec<Slot>,
    cont_prog: [f32; 2],
    hp: f32,
    hunger: f32,
    /// Effets en cours : bit 0 poison, bit 1 nausee.
    effects: u8,
    // Interaction.
    sel: usize,
    target: Option<(IVec3, IVec3)>,
    dig: Option<(IVec3, f32)>,
    dig_frac: Option<f32>,
    dig_cd: f32,
    use_cd: f32,
    info: Option<(String, f32)>,
    name_t: f32,
    // Habillage.
    hurt_t: f32,
    underwater: bool,
    /// Avancement du pas (balancement de la camera et de la main).
    bob: f32,
    /// Temps restant du mouvement de la main.
    swing: f32,
    fov: f32,
    view: i32,
    stars: Vec<Vec3>,
    /// Aides au developpement (variable EPICUBE_DEMO).
    demo: Option<String>,
    auto_walk: f32,
}

impl App {
    fn new(net: Net, st: &Settings) -> App {
        // Etoiles : directions fixes tirees une fois pour toutes.
        let stars = (0..260)
            .map(|i| {
                let f = |k: i32| (hash(i, k, 0, 77) & 0xFFFF) as f32 / 32767.5 - 1.0;
                Vec3::new(f(0), f(1), f(2)).normalize_or(Vec3::Y)
            })
            .collect();
        App {
            net,
            world: World { chunks: HashMap::new() },
            dirty: HashSet::new(),
            urgent: HashSet::new(),
            mesher: mesher::Mesher::new(),
            chat: chatbox::Chat::default(),
            mic_live: false,
            mode: 0,
            rules: shared::rules::Rules::default(),
            space_t: 1.0,
            palette_row: 0,
            inbox: std::collections::VecDeque::new(),
            epoch: 0,
            joined: false,
            lost: None,
            pos: Vec3::ZERO,
            vel: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            hit_wall: false,
            fly: false,
            time: 0.0,
            others: HashMap::new(),
            drops: HashMap::new(),
            mobs: HashMap::new(),
            riding: None,
            seed_label: None,
            origin: [0, 0],
            rebasing: false,
            step_n: 0,
            wet: false,
            dig_snd: 0.0,
            scan_t: 0.0,
            near: [0.0; 3],
            projs: HashMap::new(),
            particles: Vec::new(),
            light: HashMap::new(),
            air: 10.0,
            hit_cd: 0.0,
            since_hit: 10.0,
            draw_t: 0.0,
            blocking: false,
            gliding: false,
            jump_prev: false,
            dim: 0,
            boss: None,
            labels: Vec::new(),
            debug: false,
            fps: 60.0,
            frames: Vec::with_capacity(121),
            ping_ms: 0.0,
            ping_acc: 0.0,
            server_tps: TPS as f32,
            server_mspt: 0.0,
            born: Instant::now(),
            world_seed: 0,
            chunk_borders: false,
            send_acc: 0.0,
            clock: 0.0,
            inv: vec![None; 36],
            armor: vec![None; 4],
            cursor: None,
            craft: vec![None; 9],
            craft_out: None,
            window: WindowKind::None,
            trades: Vec::new(),
            trade_title: String::new(),
            cont: Vec::new(),
            cont_prog: [0.0; 2],
            hp: 20.0,
            hunger: 20.0,
            effects: 0,
            sel: 0,
            target: None,
            dig: None,
            dig_frac: None,
            dig_cd: 0.0,
            use_cd: 0.0,
            info: None,
            name_t: 0.0,
            hurt_t: 0.0,
            underwater: false,
            bob: 0.0,
            swing: 0.0,
            fov: st.fov,
            view: st.view,
            stars,
            demo: std::env::var("EPICUBE_DEMO").ok(),
            auto_walk: 0.0,
        }
    }

    fn dead(&self) -> bool {
        self.joined && self.hp <= 0.0
    }

    /// La souris doit-elle etre capturee (mode jeu) ?
    fn want_grab(&self) -> bool {
        self.joined && self.lost.is_none() && self.window == WindowKind::None && !self.dead() && !self.chat.open
    }

    fn poll_net(&mut self, gfx: &mut Gfx) {
        loop {
            match self.net.rx.try_recv() {
                Ok(m) => self.inbox.push_back(m),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if self.lost.is_none() {
                        self.lost = Some("Connexion au serveur perdue".into());
                    }
                    break;
                }
            }
        }
        // Decoder un chunk coute un tiers de milliseconde et le serveur en envoie douze d'un coup : on en
        // prend quelques-uns par image. Les messages qui suivent attendent leur tour, pour garder l'ordre.
        let mut decoded = 0;
        while let Some(msg) = self.inbox.pop_front() {
            if matches!(msg, ServerMsg::Chunk { .. }) {
                if decoded >= 3 {
                    self.inbox.push_front(msg);
                    break;
                }
                decoded += 1;
            }
            match msg {
                ServerMsg::Welcome { spawn, time, dim, seed, .. } => {
                    if self.seed_label.is_none() {
                        self.seed_label = Some(seed.to_string());
                    }
                    self.world_seed = seed;
                    self.set_origin(spawn);
                    self.pos = self.loc(spawn);
                    self.dim = dim;
                    self.time = time as f32;
                    self.joined = true;
                }
                ServerMsg::Refused { reason } => self.lost = Some(format!("Connexion refusee : {reason}")),
                ServerMsg::Chunk { cx, cz, data } => {
                    let (cx, cz) = self.loc_chunk(cx, cz);
                    if let Some(c) = Chunk::decode(&data) {
                        self.world.chunks.insert((cx, cz), c);
                        // La lumiere deborde sur les voisins : on les reconstruit aussi.
                        for dz in -1..=1 {
                            for dx in -1..=1 {
                                self.dirty.insert((cx + dx, cz + dz));
                            }
                        }
                    }
                }
                ServerMsg::Unload { cx, cz } => {
                    let (cx, cz) = self.loc_chunk(cx, cz);
                    self.world.chunks.remove(&(cx, cz));
                    gfx.meshes.remove(&(cx, cz));
                    self.dirty.remove(&(cx, cz));
                    self.urgent.remove(&(cx, cz));
                    self.light.remove(&(cx, cz));
                }
                ServerMsg::BlockChange { pos, b, m } => self.block_change(self.loc_cell(pos), b, m),
                ServerMsg::PlayerJoin { id, name } => {
                    self.info = Some((format!("{name} est dans la partie"), 4.0));
                    self.others.insert(id, Remote { name, target: Vec3::ZERO, shown: Vec3::ZERO, yaw: 0.0, walk: 0.0, gear: [0; 4], talk: 0.0 });
                }
                ServerMsg::PlayerPos { id, pos, yaw, .. } => {
                    let p = self.loc(pos);
                    if let Some(r) = self.others.get_mut(&id) {
                        if r.shown == Vec3::ZERO || r.shown.distance(p) > 8.0 {
                            r.shown = p;
                        }
                        r.target = p;
                        r.yaw = yaw as f32;
                    }
                }
                ServerMsg::Voice { id, data } => {
                    // La voix d'un joueur proche : on la joue la ou il se trouve.
                    if data.len() <= shared::voice::MAX_PACKET {
                        audio::voice_feed(id, &shared::voice::decode(&data));
                        if let Some(r) = self.others.get_mut(&id) {
                            r.talk = 0.4;
                        }
                    }
                }
                ServerMsg::PlayerLeave { id } => {
                    audio::voice_drop(id);
                    if let Some(r) = self.others.remove(&id) {
                        self.info = Some((format!("{} a quitte la partie", r.name), 4.0));
                    }
                }
                ServerMsg::PlayerHide { id } => {
                    if let Some(r) = self.others.get_mut(&id) {
                        r.target = Vec3::ZERO;
                        r.shown = Vec3::ZERO;
                    }
                }
                ServerMsg::Time { time } => self.time = time as f32,
                ServerMsg::Rules { mode, rules } => {
                    if self.joined && (mode != self.mode || rules.difficulty != self.rules.difficulty) {
                        let text = if mode != self.mode {
                            format!("Mode de jeu : {}", shared::rules::mode_name(mode))
                        } else {
                            format!("Difficulté : {}", shared::rules::difficulty_name(rules.difficulty))
                        };
                        self.info = Some((text, 3.0));
                    }
                    self.mode = mode;
                    self.rules = rules;
                    // Un spectateur vole toujours ; hors creatif et spectateur, on retombe.
                    let fly = mode == shared::rules::SPECTATOR || (self.fly && shared::rules::flies(mode));
                    if fly != self.fly {
                        self.fly = fly;
                        self.net.send(ClientMsg::SetFly(fly));
                    }
                }
                ServerMsg::Chat { text, kind } => {
                    if kind == shared::proto::chat::SAY || kind == shared::proto::chat::WHISPER {
                        audio::play("clic", 0.4);
                    }
                    self.chat.push(text, kind);
                }
                ServerMsg::Roster { name, here } => {
                    self.chat.roster.retain(|n| *n != name);
                    if here {
                        self.chat.roster.push(name);
                    }
                }
                ServerMsg::Gear { id, armor } => {
                    if let Some(r) = self.others.get_mut(&id) {
                        r.gear = armor;
                    }
                }
                ServerMsg::Inventory { inv, mut armor, cursor, craft, craft_out } => {
                    armor.resize(4, None);
                    // Une piece de plus sur le dos : bruit d'etoffe ou de metal.
                    if self.joined && armor.iter().zip(&self.armor).any(|(new, old)| new.is_some() && new.map(|s| s.id) != old.map(|s| s.id)) {
                        let heavy = armor.iter().flatten().any(|s| items::armor(s.id).is_some_and(|a| a.1 >= 2));
                        audio::play(if heavy { "coup_metal" } else { "coup_tissu" }, 0.7);
                    }
                    self.armor = armor;
                    self.inv = inv;
                    self.inv.resize(36, None);
                    self.cursor = cursor;
                    self.craft = craft;
                    self.craft_out = craft_out;
                }
                ServerMsg::Window { kind } => {
                    if kind != self.window {
                        audio::play(if kind == WindowKind::None { "ferme" } else { "ouvre" }, 0.6);
                    }
                    self.window = kind;
                    self.cont.clear();
                    self.cont_prog = [0.0; 2];
                }
                ServerMsg::Container { slots, progress } => {
                    self.cont = slots;
                    self.cont_prog = [progress[0] as f32, progress[1] as f32];
                }
                ServerMsg::Stats { hp, hunger, air, effects } => {
                    let (hp, hunger, air) = (hp as f32, hunger as f32, air as f32);
                    self.effects = effects;
                    if hp < self.hp - 0.01 && self.joined {
                        self.hurt_t = 0.4;
                        audio::play("blessure", 0.9);
                    }
                    if hunger > self.hunger + 0.4 && self.joined {
                        audio::play("mange", 0.8);
                    }
                    self.hp = hp;
                    self.hunger = hunger;
                    self.air = air;
                }
                ServerMsg::Teleport { pos } => {
                    self.pos = self.loc(pos);
                    self.vel = Vec3::ZERO;
                    self.rebase_if_far();
                }
                ServerMsg::DropSpawn { id, pos, vel, item } => {
                    self.drops.insert(id, DropView { pos: self.loc(pos), vel: v3(vel), item, age: 0.0 });
                }
                ServerMsg::DropRemove { id } => {
                    // Disparu tout pres de nous : on vient de le ramasser.
                    if self.drops.remove(&id).is_some_and(|d| d.pos.distance(self.pos + Vec3::Y * 0.8) < 2.2) {
                        audio::play("ramasse", 0.6);
                    }
                }
                ServerMsg::Info { text } => self.info = Some((text, 4.0)),
                ServerMsg::MobSpawn { id, kind, pos, yaw, baby } => {
                    let (p, yaw) = (self.loc(pos), yaw as f32);
                    // Renvoye aussi quand un petit devient adulte : on garde alors ce qui est affiche.
                    let old = self.mobs.remove(&id);
                    let shown = old.as_ref().map(|m| m.shown).unwrap_or(p);
                    let trail = old.map(|m| m.trail).unwrap_or_default();
                    let voice = 3.0 + (id % 13) as f32;
                    self.mobs.insert(id, MobView { kind, target: p, shown, yaw, baby, hurt: 0.0, walk: 0.0, trail, voice });
                }
                ServerMsg::MobPos { id, pos, yaw } => {
                    let p = self.loc(pos);
                    if let Some(m) = self.mobs.get_mut(&id) {
                        m.target = p;
                        if m.shown.distance(m.target) > 6.0 {
                            m.shown = m.target;
                        }
                        m.yaw = yaw as f32;
                    }
                }
                ServerMsg::MobHurt { id } => {
                    if let Some(m) = self.mobs.get_mut(&id) {
                        m.hurt = 0.3;
                        audio::play_at("frappe", m.shown + Vec3::Y * 0.6, 0.9);
                        sounds::speak(m.kind, m.baby, m.shown, 1);
                    }
                }
                ServerMsg::MobRemove { id } => {
                    // Disparue juste apres un coup : c'etait son dernier cri.
                    if let Some(m) = self.mobs.remove(&id).filter(|m| m.hurt > 0.0) {
                        sounds::speak(m.kind, m.baby, m.shown, 2);
                    }
                }
                ServerMsg::ProjSpawn { id, pos, vel, kind } => {
                    let at = self.loc(pos);
                    if kind == 0 {
                        audio::play_at("tir", at, 0.8);
                    } else {
                        audio::play_tuned("wanyudo", Some(at), 0.8, 1.3, 32.0);
                    }
                    self.projs.insert(id, ProjView { pos: at, vel: v3(vel), kind });
                }
                ServerMsg::ProjRemove { id } => {
                    if let Some(p) = self.projs.remove(&id) {
                        audio::play_at(if p.kind == 0 { "fleche" } else { "casse_terre" }, p.pos, 0.8);
                    }
                }
                ServerMsg::Knock { vel } => {
                    self.vel = v3(vel);
                    self.on_ground = false;
                }
                ServerMsg::Dimension { dim, pos } => {
                    // Nouveau monde, ou nouvelle origine d'affichage : on oublie tout ce qui etait charge.
                    let arrived = dim != self.dim;
                    self.dim = dim;
                    self.set_origin(pos);
                    self.rebasing = false;
                    self.pos = self.loc(pos);
                    self.vel = Vec3::ZERO;
                    self.world.chunks.clear();
                    gfx.meshes.clear();
                    self.dirty.clear();
                    self.urgent.clear();
                    self.epoch += 1;
                    self.light.clear();
                    self.mobs.clear();
                    self.drops.clear();
                    self.projs.clear();
                    self.particles.clear();
                    self.boss = None;
                    for r in self.others.values_mut() {
                        r.target = Vec3::ZERO;
                        r.shown = Vec3::ZERO;
                    }
                    let name = ["la surface", "Yomi, le pays des morts", "Takamagahara, la plaine celeste"][dim.min(2) as usize];
                    if arrived {
                        audio::play("gong", 0.9);
                        self.info = Some((format!("Vous entrez dans {name}"), 4.0));
                    }
                }
                ServerMsg::Boss { hp } => self.boss = hp.map(|h| h as f32),
                ServerMsg::Ride { id } => self.riding = id,
                ServerMsg::Pong { t, tps, mspt } => {
                    // Aller-retour mesure sur notre propre horloge ; lisse pour ne pas sautiller.
                    // On prend l'instant ou la reponse est arrivee sur le reseau, pas celui ou on la lit.
                    let arrived = self.net.pong_at.lock().ok().and_then(|a| *a).unwrap_or_else(Instant::now);
                    let now = arrived.saturating_duration_since(self.born).as_micros() as u64;
                    let ms = now.saturating_sub(t) as f32 / 1000.0;
                    self.ping_ms = if self.ping_ms == 0.0 { ms } else { self.ping_ms + (ms - self.ping_ms) * 0.3 };
                    self.server_tps = tps as f32;
                    self.server_mspt = mspt as f32;
                }
                ServerMsg::Trades { title, offers } => {
                    self.trade_title = title;
                    self.trades = offers;
                }
                ServerMsg::Sound { pos, kind, pitch } => {
                    sounds::server(kind, pitch, IVec3::from(self.loc_cell(pos)).as_vec3() + Vec3::splat(0.5));
                }
            }
        }
    }

    /// Trop loin de l'origine d'affichage (plus de 4096 blocs) : on en demande une nouvelle.
    /// Le serveur repond en renvoyant tout ce qui nous entoure.
    fn rebase_if_far(&mut self) {
        if !self.rebasing && self.joined && (self.pos.x.abs() > 4096.0 || self.pos.z.abs() > 4096.0) {
            self.rebasing = true;
            self.net.send(ClientMsg::Rebase);
        }
    }

    /// Place l'origine d'affichage pres de cette position du monde.
    fn set_origin(&mut self, p: [f64; 3]) {
        self.origin = [(p[0] / 1024.0).floor() as i32 * 1024, (p[2] / 1024.0).floor() as i32 * 1024];
    }

    /// Position du monde -> position locale.
    fn loc(&self, p: [f64; 3]) -> Vec3 {
        Vec3::new((p[0] - self.origin[0] as f64) as f32, p[1] as f32, (p[2] - self.origin[1] as f64) as f32)
    }

    /// Position locale -> position du monde.
    pub fn abs(&self, p: Vec3) -> [f64; 3] {
        [p.x as f64 + self.origin[0] as f64, p.y as f64, p.z as f64 + self.origin[1] as f64]
    }

    fn loc_cell(&self, p: [i32; 3]) -> [i32; 3] {
        [p[0] - self.origin[0], p[1], p[2] - self.origin[1]]
    }

    fn abs_cell(&self, p: [i32; 3]) -> [i32; 3] {
        [p[0] + self.origin[0], p[1], p[2] + self.origin[1]]
    }

    /// Chunk du monde -> chunk local.
    fn loc_chunk(&self, cx: i32, cz: i32) -> (i32, i32) {
        (cx - self.origin[0] / 16, cz - self.origin[1] / 16)
    }

    fn block_change(&mut self, pos: [i32; 3], b: u8, m: u8) {
        let key = (pos[0] >> 4, pos[2] >> 4);
        let Some(c) = self.world.chunks.get_mut(&key) else { return };
        if !(0..H as i32).contains(&pos[1]) {
            return;
        }
        let (lx, lz) = (pos[0] & 15, pos[2] & 15);
        let i = idx(lx as usize, pos[1] as usize, lz as usize);
        let old = c.b[i];
        c.b[i] = b;
        c.m[i] = m;
        sounds::block_changed(old, b, IVec3::from(pos).as_vec3() + Vec3::splat(0.5));
        // Le chunk touche et ceux qui bordent le bloc tout de suite ; le reste suit.
        for dz in -1..=1 {
            for dx in -1..=1 {
                let near = (dx == 0 || (dx < 0 && lx == 0) || (dx > 0 && lx == 15))
                    && (dz == 0 || (dz < 0 && lz == 0) || (dz > 0 && lz == 15));
                let k = (key.0 + dx, key.1 + dz);
                if near {
                    self.urgent.insert(k);
                } else {
                    self.dirty.insert(k);
                }
            }
        }
        // Un bloc qui disparait projette des eclats de sa texture.
        let center = Vec3::new(pos[0] as f32 + 0.5, pos[1] as f32 + 0.5, pos[2] as f32 + 0.5);
        if b == AIR && old != AIR && shape(old) != Shape::Fluid && center.distance(self.pos) < 32.0 {
            let seed = (self.clock * 1000.0) as u32;
            let rnd = |k: i32, n: i32| (hash(k, n, pos[0] ^ pos[2], seed) & 0xFFFF) as f32 / 65535.0;
            for k in 0..12 {
                let off = Vec3::new(rnd(k, 0) - 0.5, rnd(k, 1) - 0.5, rnd(k, 2) - 0.5);
                self.particles.push(Particle {
                    pos: center + off * 0.8,
                    vel: off * 3.0 + Vec3::Y * (1.5 + rnd(k, 3) * 2.5),
                    life: 0.4 + rnd(k, 4) * 0.5,
                    tile: blocks::tile(old, 0, 4),
                });
            }
        }
    }

    /// Eclairage a appliquer a une entite situee en `p`.
    fn light_at(&self, p: Vec3) -> [u8; 4] {
        let (x, y, z) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        if y < 0 || y >= H as i32 {
            return [255, 0, 0, 0];
        }
        match self.light.get(&(x >> 4, z >> 4)) {
            Some(l) => {
                let v = l[idx((x & 15) as usize, y as usize, (z & 15) as usize)];
                [(v >> 4) * 17, (v & 15) * 17, 0, 0]
            }
            None => [255, 0, 0, 0],
        }
    }

    /// Le son d'une image : l'oreille, les pas, les eclaboussures, la voix des creatures, l'ambiance et la musique.
    fn sound_frame(&mut self, dt: f32, eye: Vec3, right: Vec3) {
        audio::listener(eye, right);
        if !self.joined {
            return;
        }
        // Pas : un par demi-balancement, sur la matiere que l'on foule.
        let n = (self.bob / std::f32::consts::PI).floor() as i32;
        if n != self.step_n {
            self.step_n = n;
            let feet = self.world.block_at(self.pos + Vec3::Y * 0.1);
            let under = self.world.block_at(self.pos - Vec3::Y * 0.2);
            let name = if feet == WATER { "eau" } else { sounds::material(if feet == shared::blocks::SNOW { feet } else { under }).0 };
            audio::play(&format!("pas_{name}"), 0.45);
        }
        // Entree dans l'eau.
        let wet = self.world.block_at(self.pos + Vec3::Y * 0.4) == WATER;
        if wet && !self.wet && self.vel.y < -1.5 {
            audio::play("plouf", (0.35 - self.vel.y * 0.06).min(1.0));
        }
        self.wet = wet;
        // Voix des autres joueurs : chacune sort de la bouche de celui qui parle.
        for (id, r) in self.others.iter_mut() {
            r.talk = (r.talk - dt).max(0.0);
            audio::voice_place(*id, Some(r.shown + Vec3::Y * 1.6));
        }
        // Creatures : chacune pousse son cri de temps en temps.
        for (id, m) in self.mobs.iter_mut() {
            m.voice -= dt;
            if m.voice <= 0.0 {
                m.voice = 7.0 + ((*id as f32 * 7.31 + self.clock * 3.7).sin().abs()) * 16.0;
                if m.shown.distance_squared(eye) < 20.0 * 20.0 {
                    sounds::speak(m.kind, m.baby, m.shown + Vec3::Y * 0.5, 0);
                }
            }
        }
        // Alentours : feu, eau et lave les plus proches, ecoutes deux fois par seconde.
        self.scan_t -= dt;
        if self.scan_t <= 0.0 {
            self.scan_t = 0.5;
            let c = eye.floor().as_ivec3();
            let mut best = [f32::MAX; 3];
            for dy in -4..=4 {
                for dz in -7..=7 {
                    for dx in -7..=7 {
                        let p = c + IVec3::new(dx, dy, dz);
                        let (b, m) = self.world.block(p.x, p.y, p.z);
                        let Some(k) = sounds::emitter(b, m) else { continue };
                        best[k] = best[k].min((p.as_vec3() + Vec3::splat(0.5)).distance(eye));
                    }
                }
            }
            self.near = [(1.0 - best[0] / 6.0).max(0.0) * 0.8, (1.0 - best[1] / 8.0).max(0.0) * 0.5, (1.0 - best[2] / 8.0).max(0.0) * 0.8];
        }
        let dead = self.dead();
        let under = self.underwater;
        // A ciel ouvert ou sous terre ? On regarde la lumiere du ciel qui arrive jusqu'a nous.
        let sky = self.light_at(eye)[1] as f32 / 255.0;
        let night = { let t = (self.time / shared::DAY_SECONDS as f32).fract(); !(0.23..0.77).contains(&t) };
        let surface = self.dim == 0 && !under;
        let high = ((eye.y - 170.0) / 60.0).clamp(0.0, 1.0);
        audio::ambience("jour", if surface && !night { sky * 0.7 } else { 0.0 });
        audio::ambience("nuit", if surface && night { sky * 0.6 } else { 0.0 });
        audio::ambience("grotte", if self.dim == 0 && !under { (1.0 - sky * 1.6).max(0.0) * 0.7 } else { 0.0 });
        audio::ambience("vent", if self.dim == 2 { 0.7 } else if surface { high * sky * 0.8 } else { 0.0 });
        audio::ambience("yomi", if self.dim == 1 { 0.8 } else { 0.0 });
        audio::ambience("sous_eau", if under { 0.8 } else { 0.0 });
        audio::ambience("feu", if under { 0.0 } else { self.near[0] });
        audio::ambience("eau", if under { 0.0 } else { self.near[1] });
        audio::ambience("lave", self.near[2]);
        audio::music(if dead {
            audio::Mood::Death
        } else if self.boss.is_some() {
            audio::Mood::Boss
        } else {
            [audio::Mood::Day, audio::Mood::Yomi, audio::Mood::Sky][self.dim.min(2) as usize]
        });
    }

    /// Passe au mode de jeu suivant, si le monde le permet.
    pub fn cycle_mode(&mut self) {
        if self.rules.hardcore {
            self.info = Some(("Mode extrême : on ne change pas de mode de jeu".to_string(), 3.0));
        } else if !self.rules.cheats {
            self.info = Some(("Ce monde n'autorise pas à changer de mode de jeu".to_string(), 3.0));
        } else {
            self.net.send(ClientMsg::SetMode((self.mode + 1) % 4));
        }
    }

    /// Passe a la difficulte suivante (verrouillee en mode extreme).
    pub fn cycle_difficulty(&mut self) {
        if !self.rules.hardcore {
            self.net.send(ClientMsg::SetDifficulty((self.rules.difficulty + 1) % 4));
        }
    }

    /// Objets de l'inventaire creatif : tous les blocs que l'on peut poser, puis tous les objets.
    pub fn palette() -> Vec<u16> {
        (1..256u16).filter(|id| items::place_block(*id) == Some(*id as u8)).chain(items3d::ids()).collect()
    }

    fn close_window(&mut self) {
        if self.window != WindowKind::None {
            audio::play("ferme", 0.6);
        }
        self.window = WindowKind::None;
        self.net.send(ClientMsg::CloseWindow);
    }

    fn select(&mut self, sel: usize) {
        if sel != self.sel {
            self.sel = sel;
            self.name_t = 2.5;
            self.net.send(ClientMsg::Select(sel as u8));
        }
    }

    /// Renvoie vrai si le joueur demande le menu de pause.
    fn handle_keys(&mut self, inp: &Input, grabbed: bool) -> bool {
        // Saisie du chat ouverte : le clavier lui appartient tout entier.
        if self.chat.open {
            self.chat_keys(inp);
            return false;
        }
        // T ouvre le chat ; « / » l'ouvre sur une commande (quelle que soit la disposition du clavier).
        if grabbed && self.window == WindowKind::None {
            if inp.text.contains('/') {
                self.chat.show("/");
                return false;
            }
            if inp.pressed.contains(&KeyCode::KeyT) {
                self.chat.show("");
                return false;
            }
        }
        let mut pause = false;
        for k in &inp.pressed {
            match k {
                KeyCode::F3 => self.debug = !self.debug,
                KeyCode::F6 => self.chunk_borders = !self.chunk_borders,
                KeyCode::F4 => self.cycle_mode(),
                KeyCode::Escape => {
                    if self.window != WindowKind::None {
                        self.close_window();
                    } else {
                        pause = true;
                    }
                }
                KeyCode::KeyE => {
                    if self.window != WindowKind::None {
                        self.close_window();
                    } else if grabbed {
                        self.net.send(ClientMsg::OpenInventory);
                    }
                }
                KeyCode::KeyQ if grabbed => self.net.send(ClientMsg::DropItem { all: inp.down(KeyCode::ControlLeft) }),
                KeyCode::Enter | KeyCode::NumpadEnter if self.dead() => self.net.send(ClientMsg::Respawn),
                _ if grabbed => {
                    const DIGITS: [KeyCode; 9] = [
                        KeyCode::Digit1,
                        KeyCode::Digit2,
                        KeyCode::Digit3,
                        KeyCode::Digit4,
                        KeyCode::Digit5,
                        KeyCode::Digit6,
                        KeyCode::Digit7,
                        KeyCode::Digit8,
                        KeyCode::Digit9,
                    ];
                    if let Some(i) = DIGITS.iter().position(|d| d == k) {
                        self.select(i);
                    }
                }
                _ => {}
            }
        }
        pause
    }

    /// Viser, casser, frapper, poser et utiliser.
    fn interact(&mut self, dt: f32, inp: &Input, eye: Vec3, dir: Vec3) {
        // Un spectateur ne vise rien et ne touche a rien.
        if !shared::rules::touches(self.mode) {
            (self.target, self.dig, self.dig_frac) = (None, None, None);
            return;
        }
        if inp.scroll != 0.0 {
            let step = if inp.scroll > 0.0 { 8 } else { 1 };
            self.select((self.sel + step) % 9);
        }
        let held = self.inv.get(self.sel).copied().flatten().map(|s| s.id);
        self.target = self.world.raycast(eye, dir, REACH, matches!(held, Some(items::BUCKET | items::FISHING_ROD)));
        self.dig_cd -= dt;
        self.use_cd -= dt;
        self.hit_cd -= dt;
        self.since_hit += dt;
        self.dig_frac = None;
        // Arc, armes de jet et bouclier : le clic droit leur appartient.
        let weapon = items::weapon(held);
        let ranged = matches!(held, Some(items::YUMI | items::YARI | items::KUNAI | items::SHIELD));
        match held {
            Some(items::YUMI) => {
                // On tend la corde en maintenant le clic, on tire en relachant.
                if inp.rmb {
                    self.draw_t += dt;
                } else if self.draw_t > 0.0 {
                    self.net.send(ClientMsg::Shoot { charge: self.draw_t.min(1.0) as f64 });
                    self.draw_t = 0.0;
                    self.swing = 0.25;
                }
            }
            Some(items::YARI | items::KUNAI) if inp.rmb_click && self.use_cd <= 0.0 => {
                self.use_cd = 0.35;
                self.swing = 0.25;
                self.net.send(ClientMsg::Shoot { charge: 1.0 });
            }
            _ => self.draw_t = if held == Some(items::YUMI) { self.draw_t } else { 0.0 },
        }
        let block = held == Some(items::SHIELD) && inp.rmb;
        if block != self.blocking {
            self.blocking = block;
            self.net.send(ClientMsg::Block(block));
        }
        if inp.lmb_click {
            self.swing = 0.25;
            // Une lame siffle quand on la brandit.
            if items::weapon(held).dmg > 2.5 {
                audio::play("lame", 0.5);
            }
        }

        // Une creature devant le bloc vise ? Elle prend le clic a sa place.
        let block_t = self
            .target
            .and_then(|(p, _)| ray_box(eye, dir, p.as_vec3(), p.as_vec3() + Vec3::ONE))
            .unwrap_or(REACH);
        let mob = self
            .mobs
            .iter()
            .filter_map(|(id, m)| {
                let (hw, h) = mk::size(m.kind);
                let (hw, h) = (hw as f32, h as f32);
                let k = if m.baby { 0.55 } else { 1.0 };
                let (min, max) = (m.shown - Vec3::new(hw, 0.0, hw) * k, m.shown + Vec3::new(hw, h, hw) * k);
                ray_box(eye, dir, min, max).filter(|t| *t < block_t.min(4.0 + weapon.reach as f32)).map(|t| (t, *id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, id)) = mob {
            self.target = None;
            self.dig = None;
            // Clic : on frappe tout de suite, meme faiblement. Clic maintenu : on attend que l'arme soit revenue en garde.
            let ready = self.since_hit >= 1.0 / weapon.speed as f32;
            if !self.blocking && ((inp.lmb_click && self.since_hit > 0.12) || (inp.lmb && ready)) {
                self.since_hit = 0.0;
                self.swing = 0.25;
                self.net.send(ClientMsg::Attack { id });
            }
            if inp.rmb_click && !ranged {
                self.net.send(ClientMsg::Interact { id });
            }
            return;
        }

        match (inp.lmb, self.target) {
            (true, Some((p, _))) => {
                let (b, _) = self.world.block(p.x, p.y, p.z);
                // Creatif : tout casse d'un coup. Aventure : rien ne se casse.
                let time = match self.mode {
                    shared::rules::CREATIVE => Some(0.0),
                    m if !shared::rules::builds(m) => None,
                    _ => items::break_time(b, held).map(|t| t as f32),
                };
                match time {
                    Some(time) => {
                        let prog = match self.dig {
                            Some((dp, pr)) if dp == p => pr + dt,
                            _ => 0.0,
                        };
                        // La main frappe en rythme tant qu'on mine.
                        if self.swing <= 0.0 {
                            self.swing = 0.25;
                        }
                        self.dig_snd -= dt;
                        if self.dig_snd <= 0.0 {
                            self.dig_snd = 0.26;
                            audio::play_at(&format!("coup_{}", sounds::material(b).1), p.as_vec3() + Vec3::splat(0.5), 0.7);
                        }
                        if prog >= time && self.dig_cd <= 0.0 {
                            self.net.send(ClientMsg::Dig { pos: self.abs_cell(p.to_array()) });
                            self.dig = None;
                            self.dig_cd = 0.22;
                        } else {
                            self.dig = Some((p, prog));
                            if time > 0.0 {
                                self.dig_frac = Some((prog / time).min(1.0));
                            }
                        }
                    }
                    None => self.dig = None,
                }
            }
            _ => self.dig = None,
        }
        if (inp.rmb_click || inp.rmb) && self.use_cd <= 0.0 && !ranged {
            self.use_cd = 0.25;
            self.swing = 0.25;
            // Le serveur parle en coordonnees du monde.
            let target = self.target.map(|(p, prev)| (self.abs_cell(p.to_array()), self.abs_cell(prev.to_array())));
            self.net.send(ClientMsg::Use { target });
        }
    }

    /// Renvoie vrai si le joueur court.
    fn update_player(&mut self, dt: f32, inp: &Input, grabbed: bool, st: &Settings) -> bool {
        if !self.joined || self.dead() || !self.world.chunks.contains_key(&chunk_of(self.pos)) {
            return false;
        }
        if grabbed {
            self.yaw -= inp.dx * 0.0025 * st.sens;
            self.pitch = (self.pitch - inp.dy * 0.0025 * st.sens).clamp(-1.55, 1.55);
        }
        // En chariot : on suit le chariot, on regarde ou l'on veut, Espace pour descendre.
        if let Some(id) = self.riding {
            match self.mobs.get(&id) {
                Some(m) => {
                    self.pos = m.shown + Vec3::Y * 0.3;
                    self.vel = Vec3::ZERO;
                    self.on_ground = true;
                    self.use_cd -= dt;
                    if grabbed && inp.down(KeyCode::Space) && self.use_cd <= 0.0 {
                        self.use_cd = 0.5;
                        self.net.send(ClientMsg::Interact { id });
                    }
                    self.send_acc += dt;
                    if self.send_acc >= 1.0 / TPS as f32 {
                        self.send_acc = 0.0;
                        self.net.send(ClientMsg::Pos { pos: self.abs(self.pos), yaw: self.yaw as f64, pitch: self.pitch as f64, on_ground: true });
                    }
                    return false;
                }
                None => self.riding = None,
            }
        }
        let fwd = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos());
        let right = Vec3::new(-self.yaw.cos(), 0.0, self.yaw.sin());
        let mut wish = Vec3::ZERO;
        let mut jump = false;
        let mut sprint = false;
        let mut down = false;
        if grabbed {
            wish = fwd * inp.axis(KeyCode::KeyW, KeyCode::KeyS) + right * inp.axis(KeyCode::KeyD, KeyCode::KeyA);
            wish = wish.normalize_or_zero();
            jump = inp.down(KeyCode::Space);
            sprint = inp.down(KeyCode::ShiftLeft) && self.hunger > 6.0;
            down = inp.down(KeyCode::ControlLeft);
        }
        if self.auto_walk > 0.0 {
            self.auto_walk -= dt;
            wish = fwd;
        }

        // Les pieds comptent aussi : on reste "dans l'eau" tant qu'on n'en est pas vraiment sorti.
        let in_water = self.world.block_at(self.pos + Vec3::Y * 0.1) == WATER
            || self.world.block_at(self.pos + Vec3::Y * 1.2) == WATER;

        // Robe de plumes : une pression sur Espace en pleine chute la deploie.
        let robed = self.inv.iter().chain(self.armor.iter()).flatten().any(|st| st.id == items::HAGOROMO);
        let fresh_jump = jump && !self.jump_prev;
        // Creatif : deux appuis rapproches sur Espace pour s'envoler ou se poser.
        self.space_t += dt;
        if fresh_jump {
            if self.space_t < 0.3 && self.mode == shared::rules::CREATIVE {
                self.fly = !self.fly;
                self.net.send(ClientMsg::SetFly(self.fly));
            }
            self.space_t = 0.0;
        }
        self.jump_prev = jump;
        if !self.gliding && robed && fresh_jump && !self.on_ground && !in_water && !self.fly && self.vel.y < -2.0 {
            self.gliding = true;
            self.net.send(ClientMsg::Glide(true));
        } else if self.gliding && (!robed || self.on_ground || in_water || self.fly) {
            self.gliding = false;
            self.net.send(ClientMsg::Glide(false));
        }
        if self.fly {
            let speed = if sprint { 28.0 } else { 12.0 };
            self.vel = wish * speed + Vec3::Y * (jump as i32 - down as i32) as f32 * 10.0;
        } else if self.gliding {
            // On file la ou l'on regarde : piquer fait prendre de la vitesse, cabrer la rend en hauteur,
            // et trop lent on decroche. A plat, on perd a peu pres un bloc pour huit parcourus.
            let look = Vec3::new(self.yaw.sin() * self.pitch.cos(), self.pitch.sin(), self.yaw.cos() * self.pitch.cos());
            let speed = (self.vel.length() + (-look.y * 22.0 - 0.6 - self.vel.length() * 0.03) * dt).clamp(4.0, 45.0);
            let sink = 1.6 + (9.0 - speed).max(0.0) * 0.9;
            let want = look * speed - Vec3::Y * sink;
            self.vel += (want - self.vel) * (3.0 * dt).min(1.0);
        } else {
            let mut speed = if sprint { 5.8 } else { 4.3 };
            // Le sable des ames retient les pas.
            if self.world.block_at(self.pos - Vec3::Y * 0.1) == shared::blocks::SOUL_SAND {
                speed *= 0.58;
            }
            // Arc tendu ou bouclier leve : on avance a petits pas.
            if self.blocking || self.draw_t > 0.0 {
                speed *= 0.35;
            }
            if in_water {
                speed *= 0.55;
            }
            let accel = if self.on_ground || in_water { 14.0 } else { 3.5 };
            let k = (accel * dt).min(1.0);
            self.vel.x += (wish.x * speed - self.vel.x) * k;
            self.vel.z += (wish.z * speed - self.vel.z) * k;
            if in_water {
                self.vel.y = (self.vel.y - 8.0 * dt).max(-3.0);
                if jump {
                    self.vel.y = (self.vel.y + 30.0 * dt).min(4.0);
                    // Contre une berge, un coup de rein permet de se hisser sur le bloc.
                    if self.hit_wall {
                        self.vel.y = 7.0;
                    }
                }
            } else {
                self.vel.y = (self.vel.y - 28.0 * dt).max(-60.0);
                if jump && self.on_ground {
                    self.vel.y = 8.6;
                }
            }
        }
        if self.mode == shared::rules::SPECTATOR {
            // Un spectateur traverse les murs.
            self.pos += self.vel * dt;
            (self.on_ground, self.hit_wall) = (false, false);
        } else {
            let r = move_body(&self.world, &mut self.pos, &mut self.vel, HALF_W, HEIGHT, dt);
            self.on_ground = r.on_ground;
            self.hit_wall = r.hit_wall;
        }
        if self.on_ground && !self.fly {
            self.bob += Vec3::new(self.vel.x, 0.0, self.vel.z).length() * dt * 1.9;
        }

        self.rebase_if_far();
        self.send_acc += dt;
        if self.send_acc >= 1.0 / TPS as f32 {
            self.send_acc = 0.0;
            self.net.send(ClientMsg::Pos {
                pos: self.abs(self.pos),
                yaw: self.yaw as f64,
                pitch: self.pitch as f64,
                on_ground: self.on_ground && !self.fly,
            });
        }
        sprint && wish != Vec3::ZERO
    }

    /// Maillages : on recupere ceux que les fils de travail ont termines, puis on leur confie les chunks
    /// a refaire, les blocs modifies d'abord, les plus proches du joueur ensuite.
    fn remesh(&mut self, gfx: &mut Gfx) {
        let start = Instant::now();
        while let Some(done) = self.mesher.poll() {
            // Resultat d'un autre monde, ou d'un chunk decharge entre-temps : il ne sert plus.
            if done.epoch == self.epoch && self.world.chunks.contains_key(&done.key) {
                if let Some(m) = done.mesh {
                    gfx.upload(done.key, &m);
                    self.light.insert(done.key, m.light);
                }
            }
            // L'envoi a la carte graphique a un cout : on n'y passe pas plus d'une fraction d'image.
            if start.elapsed() > Duration::from_micros(1500) {
                break;
            }
        }
        let chunks = &self.world.chunks;
        // Un chunk deja en cours de maillage attend son resultat avant de repartir, pour ne pas se doubler.
        let urgent: Vec<(i32, i32)> = self.urgent.iter().copied().filter(|k| !self.mesher.busy(*k)).collect();
        for key in urgent {
            self.urgent.remove(&key);
            self.dirty.remove(&key);
            if chunks.contains_key(&key) {
                self.mesher.submit(key, self.epoch, chunks, true);
            }
        }
        if self.dirty.is_empty() || !self.mesher.has_room() {
            return;
        }
        let has_all = |&(cx, cz): &(i32, i32)| (-1..=1).all(|dz| (-1..=1).all(|dx| chunks.contains_key(&(cx + dx, cz + dz))));
        let (pcx, pcz) = chunk_of(self.pos);
        self.dirty.retain(|k| chunks.contains_key(k));
        let mut ready: Vec<(i32, (i32, i32))> = self
            .dirty
            .iter()
            .filter(|k| has_all(k) && !self.mesher.busy(**k))
            .map(|&(cx, cz)| ((cx - pcx).pow(2) + (cz - pcz).pow(2), (cx, cz)))
            .collect();
        ready.sort_unstable();
        for (_, key) in ready {
            if !self.mesher.has_room() {
                break;
            }
            self.mesher.submit(key, self.epoch, chunks, false);
            self.dirty.remove(&key);
        }
    }

    /// Nuages d'anime : grappes de boules blanches face a la camera, qui derivent lentement.
    fn push_clouds(&self, out: &mut Vec<Vtx>, eye: Vec3, right: Vec3, up: Vec3, range: f32) {
        let drift = self.clock * 0.9;
        let n = (range / CLOUD_CELL).ceil() as i32 + 1;
        let (ci, cj) = (((eye.x + drift) / CLOUD_CELL).floor() as i32, (eye.z / CLOUD_CELL).floor() as i32);
        let (u, v) = atlas::tile_uv(t::PUFF);
        let (e, m) = (1.0 / 32.0, 0.6 / atlas::AT as f32);
        let uvs = [[u + m, v + e - m], [u + e - m, v + e - m], [u + e - m, v + m], [u + m, v + m]];
        for j in cj - n..=cj + n {
            for i in ci - n..=ci + n {
                if fbm2(i as f32 * 0.11, j as f32 * 0.11, 4242, 3) < 0.16 {
                    continue;
                }
                let h = hash(i, 0, j, 99);
                let rnd = |k: u32| ((h >> (k * 6)) & 63) as f32 / 63.0;
                let c = Vec3::new(
                    (i as f32 + rnd(0)) * CLOUD_CELL - drift,
                    CLOUD_Y + rnd(1) * 8.0,
                    (j as f32 + rnd(2)) * CLOUD_CELL,
                );
                let r = 7.0 + rnd(3) * 7.0;
                let p = [c - right * r - up * r, c + right * r - up * r, c + right * r + up * r, c - right * r + up * r];
                for k in [0, 1, 2, 0, 2, 3] {
                    out.push(Vtx { pos: p[k].to_array(), uv: uvs[k], color: [255, 255, 255, 240], light: [255, 0, 0, 0] });
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn frame(&mut self, dt: f32, inp: &Input, gfx: &mut Gfx, grabbed: bool, active: bool, st: &Settings, models: &[Model], shapes: &items3d::Shapes) -> FrameOut {
        prof::mark("reseau");
        self.poll_net(gfx);
        prof::mark("jeu");
        self.clock += dt;
        let (w, h) = gfx.size();
        let s = st.scale(h);
        let mut pause = false;

        // Aides au developpement.
        if self.joined {
            match self.demo.take().as_deref() {
                Some("inventaire") | Some("armes") | Some("armure") | Some("creatif") => self.net.send(ClientMsg::OpenInventory),
                Some("infos") => {
                    self.debug = true;
                    self.chunk_borders = true;
                }
                // Pres de la bordure du monde : on affiche les coordonnees et on marche un peu.
                Some("lointain") => {
                    self.debug = true;
                    self.auto_walk = 1.5;
                }
                Some("marche") => self.auto_walk = 2.0,
                // Chat ouvert sur une commande en cours de frappe, avec quelques lignes deja recues.
                Some("chat") => {
                    self.rules.cheats = true;
                    self.chat.push("<Ren> quelqu'un a vu le village au nord ?".into(), 0);
                    self.chat.push("Vous chuchotez à Ren : j'arrive".into(), 3);
                    self.chat.push("Commande inconnue : /voler. Tapez /aide.".into(), 2);
                    self.chat.show("/donner kat");
                }
                // Vitrine de circuits : elle est construite en contrebas, on la regarde de haut.
                Some("circuits") | Some("ferme") => self.pitch = -0.62,
                Some("village") => self.pitch = -0.2,
                _ => {}
            }
        }
        if st.view != self.view {
            self.view = st.view;
            self.net.send(ClientMsg::SetView(st.view as u8));
        }

        let look = Vec3::new(self.yaw.sin() * self.pitch.cos(), self.pitch.sin(), self.yaw.cos() * self.pitch.cos());
        if active {
            pause = self.handle_keys(inp, grabbed);
            if grabbed {
                self.interact(dt, inp, self.pos + Vec3::Y * EYE, look);
            } else if self.window != WindowKind::None && (inp.lmb_click || inp.rmb_click) {
                self.ui_click(inp, w, h, s, inp.rmb_click && !inp.lmb_click);
            }
            // Inventaire creatif : la molette fait defiler les objets.
            if !grabbed && self.window == WindowKind::Inventory && self.mode == shared::rules::CREATIVE && inp.scroll != 0.0 {
                let rows = App::palette().len().div_ceil(7);
                self.palette_row = if inp.scroll > 0.0 { self.palette_row.saturating_sub(1) } else { (self.palette_row + 1).min(rows.saturating_sub(4)) };
            }
        }
        if !grabbed || !active {
            self.dig = None;
            self.dig_frac = None;
            self.target = None;
        }
        let sprinting = self.update_player(dt, inp, grabbed && active, st);
        prof::mark("maillage");
        self.remesh(gfx);
        prof::mark("scene");

        self.time = (self.time + dt / DAY_SECONDS as f32).fract();
        self.fps += (1.0 / dt.max(1e-4) - self.fps) * 0.05;
        self.frames.push(dt * 1000.0);
        if self.frames.len() > 120 {
            self.frames.remove(0);
        }
        // Un ping par seconde.
        self.ping_acc += dt;
        if self.joined && self.ping_acc >= 1.0 {
            self.ping_acc = 0.0;
            self.net.send(ClientMsg::Ping { t: self.born.elapsed().as_micros() as u64 });
        }
        self.chat.tick(dt);
        self.name_t = (self.name_t - dt).max(0.0);
        self.hurt_t = (self.hurt_t - dt).max(0.0);
        self.swing = (self.swing - dt).max(0.0);
        if let Some((_, left)) = &mut self.info {
            *left -= dt;
            if *left <= 0.0 {
                self.info = None;
            }
        }
        for r in self.others.values_mut() {
            let before = r.shown;
            r.shown = r.shown.lerp(r.target, (dt * 12.0).min(1.0));
            r.walk += Vec3::new(r.shown.x - before.x, 0.0, r.shown.z - before.z).length() * 2.2;
        }
        for d in self.drops.values_mut() {
            d.age += dt;
            step_drop(&self.world, &mut d.pos, &mut d.vel, dt);
        }
        for m in self.mobs.values_mut() {
            let before = m.shown;
            m.shown = m.shown.lerp(m.target, (dt * 14.0).min(1.0));
            m.walk += Vec3::new(m.shown.x - before.x, 0.0, m.shown.z - before.z).length() * 3.0;
            m.hurt = (m.hurt - dt).max(0.0);
            if m.kind == mk::WYRM && m.trail.first().is_none_or(|t| t.distance(m.shown) > 1.7) {
                m.trail.insert(0, m.shown);
                m.trail.truncate(14);
            }
        }
        for p in self.projs.values_mut() {
            if p.kind != 1 {
                p.vel.y -= 20.0 * dt;
            }
            p.pos += p.vel * dt;
        }
        for p in self.particles.iter_mut() {
            p.vel.y -= 18.0 * dt;
            p.pos += p.vel * dt;
            p.life -= dt;
        }
        self.particles.retain(|p| p.life > 0.0);

        // Camera : balancement en marchant, champ elargi en courant.
        let right = Vec3::new(-self.yaw.cos(), 0.0, self.yaw.sin());
        let up = right.cross(look).normalize_or(Vec3::Y);
        let mut eye = self.pos + Vec3::Y * EYE;
        let moving = Vec3::new(self.vel.x, 0.0, self.vel.z).length().min(4.5) / 4.5;
        let sway = if st.bobbing && self.on_ground && !self.fly { moving } else { 0.0 };
        eye += right * (self.bob.sin() * 0.035 * sway) + Vec3::Y * ((self.bob * 2.0).sin().abs() * 0.045 * sway);
        self.fov += (st.fov + if sprinting { 8.0 } else { 0.0 } - self.fov) * (dt * 8.0).min(1.0);
        let dir = look;
        let proj = Mat4::perspective_rh(self.fov.to_radians(), w / h, gfx::NEAR, gfx::FAR);
        let view = Mat4::look_to_rh(eye, dir, Vec3::Y);

        // Ciel : 0 = lever du soleil, 0.25 = midi, 0.5 = coucher, 0.75 = minuit.
        let ang = self.time * std::f32::consts::TAU;
        let sn = ang.sin();
        let day = (sn * 3.0 + 0.5).clamp(0.0, 1.0);
        let dusk = 1.0 - (sn.abs() * 4.0).min(1.0);
        let mut zenith = mix3([0.01, 0.015, 0.05], [0.26, 0.50, 0.92], day);
        let mut horizon = mix3([0.03, 0.05, 0.11], [0.66, 0.82, 0.97], day);
        horizon = mix3(horizon, [0.98, 0.56, 0.28], dusk * 0.6);
        zenith = mix3(zenith, [0.5, 0.36, 0.5], dusk * 0.25);
        let mut sun_tint = mix3([0.62, 0.72, 1.0], [1.0, 0.98, 0.93], day);
        sun_tint = mix3(sun_tint, [1.0, 0.72, 0.5], dusk * 0.5);
        let mut daylight = 0.3 + 0.7 * day;
        let mut ambient = 0.04;
        match self.dim {
            // Abime : ni ciel ni jour, une lueur rouge permanente.
            1 => {
                zenith = [0.14, 0.03, 0.02];
                horizon = [0.24, 0.06, 0.03];
                daylight = 0.0;
                ambient = 0.16;
            }
            // Zenith : crepuscule violet fige.
            2 => {
                zenith = [0.03, 0.01, 0.07];
                horizon = [0.10, 0.05, 0.16];
                sun_tint = [0.82, 0.74, 1.0];
                daylight = 0.8;
                ambient = 0.10;
            }
            _ => {}
        }
        let range = st.view as f32 * 16.0;
        let mut fog = [horizon[0], horizon[1], horizon[2], range * 0.55];
        let mut fog_end = range - 8.0;
        if self.dim == 1 {
            fog[3] = 12.0;
            fog_end = fog_end.min(90.0);
        }
        self.underwater = self.world.block_at(eye) == WATER;
        self.sound_frame(dt, eye, right);
        if self.underwater {
            horizon = [0.06, 0.2, 0.5];
            zenith = horizon;
            fog = [0.06, 0.2, 0.5, 1.0];
            fog_end = 24.0;
        }
        let u = Uniforms {
            view_proj: (proj * view).to_cols_array_2d(),
            cam: [eye.x, eye.y, eye.z, self.clock],
            fog,
            params: [daylight, fog_end, 0.0, ambient],
            sun: [sun_tint[0], sun_tint[1], sun_tint[2], 0.0],
        };

        let mut sky_v = Vec::new();
        push_sky_dome(&mut sky_v, eye, zenith, horizon);
        if !self.underwater && self.dim != 1 {
            // Etoiles : elles tournent avec le ciel et s'effacent le jour.
            let night = if self.dim == 2 { 1.0 } else { 1.0 - day };
            if night > 0.05 {
                let (ca, sa) = (ang.cos(), ang.sin());
                for (i, d) in self.stars.iter().enumerate() {
                    let r = Vec3::new(d.x * ca - d.y * sa, d.x * sa + d.y * ca, d.z);
                    if r.y > -0.05 {
                        let tw = 0.75 + 0.25 * (self.clock * 2.0 + i as f32).sin();
                        let a = (night * tw * 255.0) as u8;
                        push_sky_quad(&mut sky_v, eye, r, 95.0, 0.18 + (i % 3) as f32 * 0.08, [255, 255, 255, a], t::WHITE);
                    }
                }
            }
            if self.dim == 0 {
                let sun = Vec3::new(ang.cos(), ang.sin(), 0.25).normalize();
                push_sky_quad(&mut sky_v, eye, sun, 90.0, 11.0, [255; 4], t::SUN);
                push_sky_quad(&mut sky_v, eye, -sun, 90.0, 7.0, [255; 4], t::MOON);
            }
        }

        let mut ents = Vec::new();
        for r in self.others.values() {
            // Position inconnue : le joueur est dans une autre dimension. Trop pres : on serait dans sa tete.
            if r.target == Vec3::ZERO || r.shown.distance(self.pos) < 0.7 {
                continue;
            }
            let l = self.light_at(r.shown + Vec3::Y);
            push_model(&mut ents, &models[models::PLAYER], r.shown, r.yaw, 1.0, r.walk, [255; 4], l);
            push_gear(&mut ents, r.shown, r.yaw, r.walk, r.gear, l);
        }
        // Objets au sol : petits cubes qui tournent et flottent.
        for d in self.drops.values() {
            if d.pos.distance_squared(eye) > 48.0 * 48.0 {
                continue;
            }
            let bob = 0.2 + (d.age * 2.5).sin() * 0.06;
            let l = self.light_at(d.pos + Vec3::Y * 0.3);
            match shapes.get(&d.item) {
                // Objet : son modele en volume, qui tourne sur lui-meme.
                Some(parts) => push_item(&mut ents, parts, d.pos + Vec3::Y * (bob + 0.05), yaw_basis(d.age * 1.5), 0.34, l),
                None => push_box(&mut ents, d.pos, yaw_basis(d.age * 1.5), Vec3::Y * bob, Vec3::splat(0.14), [255; 4], Tex::Tile(items::icon(d.item)), l),
            }
        }
        for m in self.mobs.values() {
            if m.shown.distance_squared(eye) > 96.0 * 96.0 {
                continue;
            }
            // Le Braisier brille de lui-meme.
            let mut l = if m.kind == mk::BRAISIER { [0, 255, 0, 0] } else { self.light_at(m.shown + Vec3::Y * 0.5) };
            // La lanterne hantee luit dans le noir.
            if m.kind == mk::BOMBYX {
                l[1] = l[1].max(180);
            }
            let k = if m.baby { 0.55 } else { 1.0 };
            for (i, tr) in m.trail.iter().enumerate().skip(1) {
                let half = 1.05 - i as f32 * 0.06;
                // Corps du dragon : anneaux de jade, un sur trois dore.
                let col = match i % 3 {
                    0 => [244, 204, 92, 255],
                    1 => [64, 176, 126, 255],
                    _ => [40, 128, 96, 255],
                };
                push_box(&mut ents, *tr, yaw_basis(m.yaw), Vec3::Y, Vec3::splat(half), col, Tex::Flat, l);
            }
            // Flash rouge quand la creature vient d'etre touchee.
            let col = if m.hurt > 0.0 { [255, 110, 110, 255] } else { [255; 4] };
            if let Some(model) = models.get(m.kind as usize) {
                push_model(&mut ents, model, m.shown, m.yaw, k, m.walk, col, l);
            }
        }
        for p in self.projs.values() {
            let basis = yaw_basis(p.vel.x.atan2(p.vel.z));
            if p.kind == 1 {
                push_box(&mut ents, p.pos, basis, Vec3::ZERO, Vec3::splat(0.22), [255, 150, 40, 255], Tex::Flat, [0, 0, 255, 0]);
            } else {
                // Fleche, lance (longue hampe de bois) ou kunai (courte lame sombre).
                let l = self.light_at(p.pos);
                let (half, col) = match p.kind {
                    2 => (Vec3::new(0.04, 0.04, 0.75), [150, 104, 64, 255]),
                    3 => (Vec3::new(0.06, 0.02, 0.16), [70, 70, 84, 255]),
                    _ => (Vec3::new(0.03, 0.03, 0.3), [200, 190, 170, 255]),
                };
                push_box(&mut ents, p.pos, basis, Vec3::ZERO, half, col, Tex::Flat, l);
            }
        }
        for p in &self.particles {
            let l = self.light_at(p.pos);
            push_box(&mut ents, p.pos, yaw_basis(p.life * 7.0), Vec3::ZERO, Vec3::splat(0.045), [255; 4], Tex::Tile(p.tile), l);
        }

        let mut overlay = Vec::new();
        if self.chunk_borders {
            // Frontieres du chunk ou l'on se trouve : quatre aretes jaunes, et un cadre bleu tous les 16 blocs de haut.
            let (cx, cz) = (self.pos.x.div_euclid(16.0) * 16.0, self.pos.z.div_euclid(16.0) * 16.0);
            let mid = (self.pos.y / 16.0).floor() * 16.0 + 8.0;
            let light = [0, 0, 255, 0];
            for (dx, dz) in [(0.0, 0.0), (16.0, 0.0), (0.0, 16.0), (16.0, 16.0)] {
                push_box(&mut overlay, Vec3::new(cx + dx, mid, cz + dz), yaw_basis(0.0), Vec3::ZERO, Vec3::new(0.04, 40.0, 0.04), [255, 220, 60, 255], Tex::Flat, light);
            }
            for k in -2..=3 {
                let y = mid - 8.0 + k as f32 * 16.0;
                for (ox, oz, hx, hz) in [(8.0, 0.0, 8.0, 0.03), (8.0, 16.0, 8.0, 0.03), (0.0, 8.0, 0.03, 8.0), (16.0, 8.0, 0.03, 8.0)] {
                    push_box(&mut overlay, Vec3::new(cx + ox, y, cz + oz), yaw_basis(0.0), Vec3::ZERO, Vec3::new(hx, 0.03, hz), [90, 170, 255, 255], Tex::Flat, light);
                }
            }
        }
        if st.clouds && self.dim == 0 && !self.underwater {
            self.push_clouds(&mut overlay, eye, right, up, fog_end);
        }
        if let Some((p, _)) = self.target {
            push_outline(&mut ents, p);
            if let Some(frac) = self.dig_frac {
                // Fissures de plus en plus marquees sur le bloc vise.
                let stage = ((frac * 4.0) as u16).min(3);
                let c = p.as_vec3() + Vec3::splat(0.5);
                push_box(&mut overlay, c, yaw_basis(0.0), Vec3::ZERO, Vec3::splat(0.503), [255; 4], Tex::Tile(t::CRACK0 + stage), [0, 0, 255, 0]);
            }
        }

        // Objet tenu en main, en bas a droite de l'ecran.
        if grabbed && !self.dead() && self.mode != shared::rules::SPECTATOR {
            if let Some(st_held) = self.inv.get(self.sel).copied().flatten() {
                let k = if self.swing > 0.0 { (self.swing / 0.25 * std::f32::consts::PI).sin() } else { 0.0 };
                let bobx = self.bob.sin() * 0.012 * sway;
                let boby = (self.bob * 2.0).sin().abs() * 0.012 * sway;
                let origin = eye + dir * (0.52 + k * 0.12) + right * (0.36 + bobx - k * 0.1) - up * (0.3 + boby + k * 0.08);
                let l = self.light_at(eye);
                // Legerement tourne vers l'interieur pour donner du volume.
                let (r2, f2) = ((right + dir * 0.45).normalize(), (dir - right * 0.45).normalize());
                match shapes.get(&st_held.id) {
                    Some(parts) => {
                        // Un vrai modele se tient comme il se doit : l'outil dresse, l'objet pose dans la paume.
                        let pose = itemdefs::pose(st_held.id);
                        let (hand, size, lift) = match pose {
                            _ if st_held.id < 256 => (glam::Mat3::IDENTITY, 0.3, 0.0),
                            // Un outil est long : on le remonte pour que la poignee reste a l'ecran.
                            items3d::Pose::Tool => (pose.hand(), 0.38, 0.11),
                            _ => (pose.hand(), 0.3, 0.03),
                        };
                        let origin = origin + up * lift;
                        let b = [r2, up, f2];
                        let turn = |v: Vec3| b[0] * v.x + b[1] * v.y + b[2] * v.z;
                        push_item(&mut ents, parts, origin, [turn(hand.x_axis), turn(hand.y_axis), turn(hand.z_axis)], size, l)
                    }
                    None => push_box(&mut ents, origin, [r2, up, f2], Vec3::ZERO, Vec3::splat(0.1), [255; 4], Tex::Tile(items::icon(st_held.id)), l),
                }
            }
        }

        // Pseudos : la tete de chaque joueur projetee a l'ecran.
        self.labels.clear();
        let vp = proj * view;
        for r in self.others.values() {
            let head = r.shown + Vec3::Y * 2.25;
            if r.target == Vec3::ZERO || r.shown.distance(self.pos) < 0.7 || head.distance(eye) > 48.0 {
                continue;
            }
            let clip = vp * head.extend(1.0);
            if clip.w > 0.1 {
                let (nx, ny) = (clip.x / clip.w, clip.y / clip.w);
                self.labels.push(((nx * 0.5 + 0.5) * w, (0.5 - ny * 0.5) * h, r.name.clone()));
            }
        }

        let mut ui = Ui::new(w, h);
        self.draw_ui(&mut ui, inp, grabbed && active, &gfx.backend, gfx.meshes.len(), models, s);
        FrameOut { u, clear: horizon, eye, dir, sky: sky_v, ents, overlay, ui, tan_half: (self.fov.to_radians() / 2.0).tan(), pause }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Title,
    /// Choix du monde et de sa graine.
    Solo,
    Multi,
    Settings,
    Game,
    Pause,
    PauseSettings,
}

/// Le programme autour de la partie : menus, parametres, ressources.
pub struct Shell {
    pub settings: Settings,
    /// Parametres deja appliques a la fenetre et au rendu.
    applied: Settings,
    pub screen: Screen,
    pub game: Option<App>,
    local: Option<server::ServerHandle>,
    pub error: Option<String>,
    /// Message affiche dans les parametres (export, rechargement).
    pub status: Option<String>,
    /// Champ de saisie actif : 1 = adresse, 2 = pseudo.
    pub field: u8,
    /// Curseur en cours de deplacement.
    pub drag: Option<u8>,
    /// Dans les parametres : la page des volumes est-elle affichee ?
    pub sound_page: bool,
    pub perf_page: bool,
    /// Micro ouvert pour le chat vocal, erreur d'ouverture s'il y en a eu une, et test « s'entendre ».
    pub mic: Option<mic::Mic>,
    pub mic_error: Option<String>,
    pub mic_echo: bool,
    pub t: f32,
    models: Vec<Model>,
    /// Modeles 3D des objets.
    shapes: items3d::Shapes,
    atlas: atlas::Img,
    /// Graine saisie (chiffres ou texte) ; vide = au hasard. Ne sert qu'a la creation d'un monde.
    pub seed_text: String,
    /// Regles choisies pour un nouveau monde : mode (4 = extreme), difficulte, triche.
    pub new_mode: u8,
    pub new_difficulty: u8,
    pub new_cheats: bool,
    pub world: String,
    quit: bool,
    /// Mode vitrine (EPICUBE_DEMO) : le jeu s'affiche comme en partie, sans capturer la souris
    /// ni se mettre en pause quand la fenetre perd le focus.
    showcase: bool,
}

/// Construit textures et modeles, puis applique le dossier de ressources s'il existe.
fn build_assets() -> (atlas::Img, Vec<Model>, items3d::Shapes, Vec<String>) {
    let mut img = atlas::build();
    let mut models = models::build(&mut img);
    let mut log = assets::load(Path::new(assets::DIR), &mut img, &mut models);
    // Les objets : leurs modeles integres, puis les fichiers Blockbench s'il y en a ; les icones en decoulent.
    let mut shapes = items3d::build(&mut img);
    log.extend(items3d::load(Path::new(assets::DIR), &mut img, &mut shapes));
    items3d::icons(&mut img, &shapes);
    (img, models, shapes, log)
}

impl Shell {
    pub fn is_local(&self) -> bool {
        self.local.is_some()
    }

    fn connect(&mut self, addr: &str) {
        match Net::connect(addr) {
            Ok(net) => {
                let st = &self.settings;
                net.send(ClientMsg::Hello { version: PROTOCOL_VERSION, name: st.name.clone(), view: st.view as u8 });
                self.game = Some(App::new(net, st));
                self.screen = Screen::Game;
                self.error = None;
            }
            Err(e) => {
                self.error = Some(format!("Connexion a {addr} impossible : {e}"));
                self.stop_local();
            }
        }
    }

    /// Chat vocal : ouvre le micro si le joueur l'a demande, regle sa porte et envoie ce qu'il capte.
    fn voice_frame(&mut self, inp: &Input) {
        let mode = self.settings.mic;
        if mode == 0 {
            // Micro coupe : on le rend au systeme.
            self.mic = None;
            self.mic_error = None;
        } else if self.mic.is_none() && self.mic_error.is_none() {
            match mic::Mic::open() {
                Ok(m) => {
                    println!("Micro : {}", m.device);
                    self.mic = Some(m);
                }
                Err(e) => {
                    eprintln!("Micro : {e}");
                    self.mic_error = Some(e);
                }
            }
        }
        let Some(mic) = &self.mic else {
            if let Some(g) = self.game.as_mut() {
                g.mic_live = false;
            }
            return;
        };
        // On ne parle qu'en jeu, hors saisie du chat ; la touche V ouvre la porte en mode « appuyer pour parler ».
        let playing = self.screen == Screen::Game && self.game.as_ref().is_some_and(|g| g.joined && !g.chat.open);
        // La porte : la touche V en mode « appuyer pour parler », toujours ouverte en mode permanent.
        // Pendant le test « s'entendre », elle s'ouvre aussi depuis les menus.
        let here = playing || self.mic_echo;
        let held = here && ((mode == 1 && inp.down(KeyCode::KeyV)) || mode == 3);
        mic.set(held, mode == 2 && here, self.settings.mic_gate);
        let frames = mic.frames();
        let live = mic.talking() && (playing || self.mic_echo);
        for data in frames {
            if self.mic_echo {
                // Test : on s'entend soi-meme, sans rien envoyer.
                audio::voice_feed(u32::MAX, &shared::voice::decode(&data));
                audio::voice_place(u32::MAX, None);
            } else if playing {
                if let Some(g) = self.game.as_mut() {
                    g.net.send(ClientMsg::Voice { data });
                }
            }
        }
        if let Some(g) = self.game.as_mut() {
            g.mic_live = live && !self.mic_echo;
        }
    }

    fn start_solo(&mut self) {
        if self.world.trim().is_empty() {
            self.world = "monde".to_string();
        }
        let dir = PathBuf::from("saves").join(&self.world);
        // Un monde neuf prend la graine saisie (ou une au hasard) et la note dans son dossier ;
        // un monde existant garde la sienne, quoi qu'on ait saisi.
        let note = dir.join("graine.txt");
        let label = if dir.join("monde.bin").exists() || note.exists() {
            std::fs::read_to_string(&note).ok().map(|t| t.trim().to_string())
        } else {
            let n = shared::seed::parse(&self.seed_text).unwrap_or_else(shared::seed::random);
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(&note, n.to_string());
            // Les regles choisies a la creation accompagnent le monde.
            let rules = shared::rules::Rules { mode: self.new_mode.min(3), difficulty: self.new_difficulty, hardcore: self.new_mode == 4, cheats: self.new_cheats }.checked();
            let _ = std::fs::write(dir.join("regles.txt"), rules.to_text());
            Some(n.to_string())
        };
        let seed = label.as_deref().and_then(|t| t.parse::<i64>().ok()).map(shared::seed::fold).unwrap_or(1337);
        match server::spawn_local(seed, Some(dir), self.settings.lan) {
            Ok(h) => {
                let addr = format!("127.0.0.1:{}", h.port);
                self.local = Some(h);
                self.connect(&addr);
                if let Some(g) = self.game.as_mut() {
                    g.seed_label = label;
                }
            }
            Err(e) => self.error = Some(format!("Impossible de demarrer la partie : {e}")),
        }
    }

    /// Arrete le serveur local, qui sauvegarde le monde avant de rendre la main.
    fn stop_local(&mut self) {
        if let Some(mut s) = self.local.take() {
            s.stop();
        }
    }

    fn leave(&mut self, gfx: &mut Gfx) {
        self.game = None;
        self.stop_local();
        gfx.meshes.clear();
        self.screen = Screen::Title;
    }

    fn act(&mut self, action: Action, gfx: &mut Gfx) {
        match action {
            Action::None => {}
            Action::Solo => self.start_solo(),
            Action::Join => {
                let a = self.settings.address.clone();
                let addr = if a.contains(':') { a } else { format!("{a}:{}", shared::DEFAULT_PORT) };
                self.connect(&addr);
            }
            Action::Quit => self.quit = true,
            Action::Leave => self.leave(gfx),
            Action::Export => {
                let both = assets::export(Path::new(assets::DIR), &self.atlas, &self.models)
                    .and_then(|n| items3d::export(Path::new(assets::DIR), &self.atlas, &self.shapes).map(|k| n + k));
                self.status = Some(match both {
                    Ok(n) => format!("{n} fichiers ecrits dans le dossier {}", assets::DIR),
                    Err(e) => format!("Export impossible : {e}"),
                });
            }
            Action::Reload => {
                let (img, models, shapes, log) = build_assets();
                gfx.set_atlas(&img.0);
                self.atlas = img;
                self.models = models;
                self.shapes = shapes;
                self.status = Some(if log.is_empty() {
                    "Aucun fichier dans assets : ressources integrees".to_string()
                } else {
                    format!("{} fichier(s) de ressources pris en compte", log.len())
                });
                for l in log {
                    println!("{l}");
                }
            }
        }
    }

    fn frame(&mut self, dt: f32, inp: &Input, gfx: &mut Gfx, window: &Window, grabbed: bool) {
        self.t += dt;
        audio::volumes(self.settings.volumes());
        if self.game.is_none() {
            audio::hush();
            audio::music(audio::Mood::Menu);
        }
        audio::update(dt);
        self.voice_frame(inp);
        // Parametres modifies : on les applique et on les enregistre une fois le curseur relache.
        // La resolution suit l'ecran ou se trouve la fenetre (sa densite peut changer en passant de l'un a l'autre).
        gfx.set_res(self.settings.res_scale(window.scale_factor()));
        if self.settings != self.applied && self.drag.is_none() {
            gfx.set_vsync(self.settings.vsync);
            if self.settings.fullscreen != self.applied.fullscreen {
                window.set_fullscreen(self.settings.fullscreen.then_some(Fullscreen::Borderless(None)));
            }
            self.settings.save();
            self.applied = self.settings.clone();
        }

        let active = self.screen == Screen::Game;
        if !active && inp.pressed.contains(&KeyCode::Escape) {
            self.screen = match self.screen {
                Screen::Pause => Screen::Game,
                Screen::PauseSettings => Screen::Pause,
                Screen::Settings | Screen::Multi | Screen::Solo => Screen::Title,
                s => s,
            };
            self.field = 0;
        }
        if inp.pressed.contains(&KeyCode::Tab) && matches!(self.screen, Screen::Multi | Screen::Solo) {
            self.field = if self.field == 1 { 2 } else { 1 };
        }

        let (w, h) = gfx.size();
        let s = self.settings.scale(h);
        let mut out = match &mut self.game {
            Some(g) => Some(g.frame(dt, inp, gfx, grabbed || (self.showcase && active), active, &self.settings, &self.models, &self.shapes)),
            None => None,
        };
        let mut ui = match &mut out {
            Some(o) => std::mem::replace(&mut o.ui, Ui::new(w, h)),
            None => Ui::new(w, h),
        };
        if out.as_ref().is_some_and(|o| o.pause) && active {
            self.screen = Screen::Pause;
        }
        let action = if self.screen != Screen::Game { menu::draw(self, &mut ui, inp, s) } else { Action::None };
        prof::mark("rendu");
        match &out {
            Some(o) => {
                let ink = self.settings.ink && std::env::var("EPICUBE_INK").map_or(true, |v| v != "0");
                let post = Post { tan_half: o.tan_half, ink: if ink { 0.88 } else { 0.0 } };
                gfx.render(&o.u, post, o.clear, o.eye, o.dir, &o.sky, &o.ents, &o.overlay, &ui.v)
            }
            None => {
                let u = Uniforms {
                    view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                    cam: [0.0; 4],
                    fog: [0.0, 0.0, 0.0, 1.0],
                    params: [1.0, 2.0, 0.0, 1.0],
                    sun: [1.0; 4],
                };
                gfx.render(&u, Post { tan_half: 1.0, ink: 0.0 }, [0.05, 0.05, 0.07], Vec3::ZERO, Vec3::Z, &[], &[], &[], &ui.v);
            }
        }
        prof::mark("boucle");
        self.act(action, gfx);
    }

    /// La souris doit-elle etre capturee ?
    fn want_grab(&self) -> bool {
        !self.showcase && self.screen == Screen::Game && self.game.as_ref().is_some_and(|g| g.want_grab())
    }
}

fn set_grab(window: &Window, on: bool) -> bool {
    if on {
        let ok = window
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
            .is_ok();
        window.set_cursor_visible(!ok);
        ok
    } else {
        let _ = window.set_cursor_grab(CursorGrabMode::None);
        window.set_cursor_visible(true);
        false
    }
}

/// macOS ne cherche plus les bibliotheques dans les dossiers Homebrew : sans cela,
/// le chargeur Vulkan (MoltenVK) reste introuvable et wgpu se rabat sur Metal.
/// La variable n'est lue qu'au lancement du processus, d'ou le redemarrage.
#[cfg(target_os = "macos")]
fn ensure_vulkan_loader() {
    use std::os::unix::process::CommandExt;
    if std::env::var_os("EPICUBE_RELAUNCHED").is_some() {
        return;
    }
    let dirs: Vec<&str> = ["/usr/local/lib", "/opt/homebrew/lib"]
        .into_iter()
        .filter(|d| std::path::Path::new(d).join("libvulkan.dylib").exists())
        .collect();
    if dirs.is_empty() {
        return;
    }
    let mut path = dirs.join(":");
    if let Ok(old) = std::env::var("DYLD_FALLBACK_LIBRARY_PATH") {
        path = format!("{path}:{old}");
    }
    if let Ok(exe) = std::env::current_exe() {
        // exec ne revient que s'il echoue ; on continue alors normalement.
        let _ = std::process::Command::new(exe)
            .args(std::env::args_os().skip(1))
            .env("DYLD_FALLBACK_LIBRARY_PATH", path)
            .env("EPICUBE_RELAUNCHED", "1")
            .exec();
    }
}

#[cfg(not(target_os = "macos"))]
fn ensure_vulkan_loader() {}

/// Le jeu lit ses sons, ses ressources et ses sauvegardes dans le dossier courant. Lance d'un double
/// clic, ce dossier depend du systeme (le dossier personnel, la racine du disque...) : s'il ne contient
/// pas les donnees du jeu, on se place a cote de l'executable, ou dans le projet qui l'a compile.
fn enter_data_dir() {
    if Path::new(audio::DIR).is_dir() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    // A cote de l'executable (jeu distribue), ou deux dossiers plus haut (target/release dans le projet).
    if let Some(dir) = exe.ancestors().skip(1).take(4).find(|d| d.join(audio::DIR).is_dir()) {
        if std::env::set_current_dir(dir).is_ok() {
            println!("Dossier du jeu : {}", dir.display());
        }
    }
}

/// Windows : sans fenetre de terminal a soi, on ecrit dans celle d'ou le jeu a ete lance, s'il y en a une
/// (pour --export-assets, --item-sheet et les messages de demarrage).
#[cfg(windows)]
fn attach_parent_console() {
    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(process: u32) -> i32;
    }
    // u32::MAX designe le processus parent. Un echec (double clic, pas de terminal) est sans consequence.
    unsafe {
        AttachConsole(u32::MAX);
    }
}

#[cfg(not(windows))]
fn attach_parent_console() {}

fn main() {
    attach_parent_console();
    enter_data_dir();
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let flag = |name: &str| args.iter().any(|a| a == name);

    // Planche de controle : toutes les icones d'objets, agrandies, dans une seule image.
    if let Some(path) = arg("--item-sheet") {
        let mut img = atlas::build();
        let shapes = items3d::build(&mut img);
        let ids = items3d::ids();
        let (cols, cell, zoom) = (12usize, 136usize, 4usize);
        let (w, h) = (cols * cell, ids.len().div_ceil(cols) * cell);
        let mut sheet = vec![0u8; w * h * 4];
        for px in sheet.chunks_mut(4) {
            px.copy_from_slice(&[72, 74, 96, 255]);
        }
        for (k, id) in ids.iter().enumerate() {
            let icon = items3d::draw_icon(&img, *id, &shapes[id]);
            let (ox, oy) = ((k % cols) * cell + 4, (k / cols) * cell + 4);
            for y in 0..items3d::N * zoom {
                for x in 0..items3d::N * zoom {
                    let c = icon[(y / zoom) * items3d::N + x / zoom];
                    if c[3] > 0 {
                        let i = ((oy + y) * w + ox + x) * 4;
                        sheet[i..i + 4].copy_from_slice(&c);
                    }
                }
            }
        }
        match assets::encode_png(&sheet, w as u32, h as u32).and_then(|png| std::fs::write(&path, png).map_err(|e| e.to_string())) {
            Ok(()) => println!("Planche des objets : {path}"),
            Err(e) => eprintln!("Planche impossible : {e}"),
        }
        return;
    }

    // Export des ressources : pas besoin de fenetre.
    if flag("--export-assets") {
        let mut img = atlas::build();
        let models = models::build(&mut img);
        let shapes = items3d::build(&mut img);
        items3d::icons(&mut img, &shapes);
        match assets::export(Path::new(assets::DIR), &img, &models).and_then(|n| items3d::export(Path::new(assets::DIR), &img, &shapes).map(|k| n + k)) {
            Ok(n) => println!("{n} fichiers ecrits dans le dossier {}", assets::DIR),
            Err(e) => {
                eprintln!("Export impossible : {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    ensure_vulkan_loader();
    // RUST_LOG=warn (ou debug) affiche les journaux de wgpu.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();

    let mut settings = Settings::load();
    match audio::init(Path::new(audio::DIR), settings.volumes()) {
        Some(report) => println!("{report}"),
        None => println!("Son : aucune sortie audio, le jeu sera muet"),
    }
    if let Some(n) = arg("--name") {
        settings.name = n.chars().take(16).collect();
    }
    settings.lan |= flag("--lan");
    let (img, models, shapes, log) = build_assets();
    for l in &log {
        println!("{l}");
    }

    let event_loop = EventLoop::new().expect("boucle d'evenements");
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Epicube Craft")
            .with_inner_size(LogicalSize::new(1280.0, 720.0))
            .build(&event_loop)
            .expect("fenetre"),
    );
    let mut gfx = Gfx::new(window.clone(), &img.0);
    gfx.set_vsync(settings.vsync);
    if settings.fullscreen {
        window.set_fullscreen(Some(Fullscreen::Borderless(None)));
    }
    let mut shell = Shell {
        applied: settings.clone(),
        settings,
        screen: Screen::Title,
        game: None,
        local: None,
        error: None,
        status: None,
        field: 0,
        drag: None,
        sound_page: false,
        perf_page: false,
        mic: None,
        mic_error: None,
        mic_echo: false,
        t: 0.0,
        models,
        shapes,
        atlas: img,
        seed_text: arg("--seed").unwrap_or_default(),
        new_mode: 0,
        new_difficulty: shared::rules::NORMAL,
        new_cheats: false,
        world: arg("--world").unwrap_or_else(|| "monde".to_string()),
        quit: false,
        showcase: false,
    };

    // Demarrage direct, sans passer par le menu.
    let demo = std::env::var("EPICUBE_DEMO").ok();
    if let Some(a) = arg("--connect") {
        let addr = if a.contains(':') { a } else { format!("{a}:{}", shared::DEFAULT_PORT) };
        shell.connect(&addr);
    } else if flag("--solo") || demo.as_deref().is_some_and(|d| d != "parametres") {
        shell.start_solo();
    }
    match demo.as_deref() {
        Some("pause") => shell.screen = Screen::Pause,
        Some("parametres") => shell.screen = Screen::Settings,
        Some(_) => shell.showcase = true,
        None => {}
    }

    let mut input = Input::default();
    let mut grabbed = false;
    let mut last = Instant::now();
    let bench: Option<f32> = std::env::var("EPICUBE_BENCH").ok().and_then(|v| v.parse().ok());
    let (started, mut bench_last, mut bench_frames) = (Instant::now(), Instant::now(), Vec::<f32>::new());

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);
            match event {
                Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta }, .. } => {
                    if grabbed {
                        input.dx += delta.0 as f32;
                        input.dy += delta.1 as f32;
                    }
                }
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => {
                        shell.settings.save();
                        shell.stop_local();
                        elwt.exit();
                    }
                    WindowEvent::Resized(s) => gfx.resize(s.width, s.height),
                    WindowEvent::Focused(false) => {
                        if shell.screen == Screen::Game && !shell.showcase {
                            shell.screen = Screen::Pause;
                        }
                        input.keys.clear();
                        input.lmb = false;
                        input.rmb = false;
                    }
                    // L'interface se dessine dans les pixels de la surface, qui peut etre plus petite que la fenetre.
                    WindowEvent::CursorMoved { position, .. } => input.mouse = (position.x as f32 * gfx.res(), position.y as f32 * gfx.res()),
                    WindowEvent::MouseWheel { delta, .. } => {
                        input.scroll += match delta {
                            MouseScrollDelta::LineDelta(_, y) => y,
                            MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                        };
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        let down = state == ElementState::Pressed;
                        match button {
                            MouseButton::Left => {
                                input.lmb = down;
                                input.lmb_click |= down;
                            }
                            MouseButton::Right => {
                                input.rmb = down;
                                input.rmb_click |= down;
                            }
                            _ => {}
                        }
                    }
                    WindowEvent::KeyboardInput { event: key, .. } => {
                        if key.state == ElementState::Pressed {
                            if key.logical_key == Key::Named(NamedKey::Backspace) {
                                input.backspace += 1;
                            } else if let Some(txt) = &key.text {
                                input.text.push_str(txt);
                            }
                        }
                        if let PhysicalKey::Code(code) = key.physical_key {
                            if key.state == ElementState::Pressed {
                                if !key.repeat {
                                    input.pressed.push(code);
                                }
                                input.keys.insert(code);
                            } else {
                                input.keys.remove(&code);
                            }
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt = (now - last).as_secs_f32().min(0.05);
                        last = now;
                        shell.frame(dt, &input, &mut gfx, &window, grabbed);
                        // Mode mesure : cadence libre, quelques secondes de chauffe, puis le compte rendu.
                        if let Some(secs) = bench {
                            if std::env::var("EPICUBE_FULL").is_ok() && window.fullscreen().is_none() {
                                window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                            }
                            gfx.set_vsync(false);
                            if let Some(k) = std::env::var("EPICUBE_SCALE").ok().and_then(|v| v.parse().ok()) {
                                gfx.set_scale(k);
                            }
                            let t = started.elapsed().as_secs_f32();
                            if t > BENCH_WARMUP {
                                if bench_frames.is_empty() {
                                    prof::start();
                                }
                                bench_frames.push((now - bench_last).as_secs_f32());
                            }
                            bench_last = now;
                            if t > BENCH_WARMUP + secs {
                                let (w, h) = gfx.size();
                                println!("Mesure ({w} x {h}, {} chunks charges, {} dessines, {} triangles de terrain)", gfx.meshes.len(), gfx.drawn.0, gfx.drawn.1);
                                print!("{}", prof::report(&mut bench_frames[1..]));
                                shell.stop_local();
                                std::process::exit(0);
                            }
                        }
                        input.pressed.clear();
                        input.text.clear();
                        input.backspace = 0;
                        input.dx = 0.0;
                        input.dy = 0.0;
                        input.scroll = 0.0;
                        input.lmb_click = false;
                        input.rmb_click = false;
                        // Limite d'images : on dort jusqu'a l'echeance, sans la depasser.
                        if shell.settings.fps_max > 0 && bench.is_none() {
                            let frame = Duration::from_secs_f64(1.0 / shell.settings.fps_max as f64);
                            let spent = now.elapsed();
                            if spent + Duration::from_micros(300) < frame {
                                std::thread::sleep(frame - spent - Duration::from_micros(300));
                            }
                            while now.elapsed() < frame {
                                std::hint::spin_loop();
                            }
                        }
                        if shell.quit {
                            shell.settings.save();
                            shell.stop_local();
                            elwt.exit();
                        }
                        let want = shell.want_grab();
                        if want != grabbed {
                            grabbed = set_grab(&window, want);
                            if !grabbed {
                                input.lmb = false;
                                input.rmb = false;
                            }
                        }
                    }
                    _ => {}
                },
                Event::AboutToWait => window.request_redraw(),
                _ => {}
            }
        })
        .expect("boucle d'evenements");
}
