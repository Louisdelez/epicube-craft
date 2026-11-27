//! Ressources modifiables : l'atlas en PNG et les modeles au format Blockbench (.bbmodel).
//!
//! `export` ecrit `assets/textures/atlas.png` et `assets/models/<nom>.bbmodel`.
//! Au lancement, `load` relit ces fichiers s'ils existent : on peut donc retoucher les textures
//! dans un editeur d'images et les modeles dans Blockbench, puis relancer le jeu.
use crate::atlas::{Img, AT};
use crate::models::{cell_origin, margin, Model, Part, CELL, NAMES};
use base64::Engine;
use glam::Vec3;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

pub const DIR: &str = "assets";
/// Marge, en pixels, entre le bord d'une face et la zone de texture reellement lue.
const MARGIN: f32 = 0.5;
/// Faces Blockbench correspondant aux notres (+X, -X, +Y, -Y, +Z, -Z), le modele regardant vers le nord.
const FACE_NAMES: [&str; 6] = ["west", "east", "up", "down", "north", "south"];

pub(crate) fn encode_png(rgba: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    drop(writer);
    Ok(out)
}

/// Decode un PNG en RGBA 8 bits : (pixels, largeur, hauteur).
pub(crate) fn decode_png(data: &[u8]) -> Result<(Vec<u8>, usize, usize), String> {
    let mut dec = png::Decoder::new(data);
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(info.buffer_size());
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf.chunks(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        other => return Err(format!("format de couleur non gere : {other:?}")),
    };
    Ok((rgba, info.width as usize, info.height as usize))
}

/// Identifiant stable au format UUID, derive d'un nom.
pub(crate) fn uuid(seed: &str) -> String {
    let mut h = [0x9E37_79B9u32, 0x85EB_CA6B, 0xC2B2_AE35, 0x27D4_EB2F];
    for (i, b) in seed.bytes().enumerate() {
        let k = i % 4;
        h[k] = (h[k] ^ b as u32).wrapping_mul(0x0100_0193).rotate_left(5);
        h[(k + 1) % 4] ^= h[k];
    }
    format!("{:08x}-{:04x}-4{:03x}-a{:03x}-{:08x}{:04x}", h[0], h[1] >> 16, h[1] & 0xFFF, h[2] & 0xFFF, h[3], h[2] >> 16)
}

/// Ecrit un modele et sa texture dans un fichier .bbmodel lisible par Blockbench.
/// `cell` : coin et cote de la case de texture dans l'atlas ; `lift` : hauteur ajoutee, en blocs, pour
/// poser sur le sol de Blockbench un modele centre sur l'origine.
pub(crate) fn bbmodel(name: &str, cell: (usize, usize, usize), lift: f32, model: &Model, img: &Img) -> Result<String, String> {
    let (ox, oy, side) = cell;
    let mut tex = vec![0u8; side * side * 4];
    for y in 0..side {
        let src = ((oy + y) * AT + ox) * 4;
        tex[y * side * 4..(y + 1) * side * 4].copy_from_slice(&img.0[src..src + side * 4]);
    }
    let data = base64::engine::general_purpose::STANDARD.encode(encode_png(&tex, side as u32, side as u32)?);

    let mut elements = Vec::new();
    let mut outliner = Vec::new();
    for (k, p) in model.parts.iter().enumerate() {
        let id = uuid(&format!("{name}-{k}"));
        // Blockbench compte en pixels (16 par bloc) et place l'avant au nord (-Z) : demi-tour autour de Y.
        let (lo, hi) = ((p.c - p.h) * 16.0, (p.c + p.h) * 16.0);
        let mut faces = serde_json::Map::new();
        for (f, face) in FACE_NAMES.iter().enumerate() {
            // Pixels entiers dans le fichier ; la petite marge anti-debordement est remise a la lecture.
            // (la marge d'un demi-pixel est retiree avant d'arrondir).
            let px = |v: f32, other: f32, o: usize| {
                let x = v * AT as f32 - o as f32;
                (x - MARGIN * (other - v).signum()).round()
            };
            let [u0, v0, u1, v1] = p.uv[f];
            let (a, b, c, d) = (px(u0, u1, ox), px(v0, v1, oy), px(u1, u0, ox), px(v1, v0, oy));
            // Le demi-tour retourne aussi le dessus et le dessous.
            let uv = if f == 2 || f == 3 { json!([c, d, a, b]) } else { json!([a, b, c, d]) };
            faces.insert(face.to_string(), json!({ "uv": uv, "texture": 0 }));
        }
        // Le nom garde le sens du balancement : jambe_a avance quand jambe_b recule.
        let prefix = match p.leg {
            0 => "piece",
            l if l > 0 => "jambe_a",
            _ => "jambe_b",
        };
        let pivot = p.pivot * 16.0;
        elements.push(json!({
            "name": format!("{prefix}_{k}"),
            "type": "cube",
            "uuid": id,
            "from": [-hi.x, lo.y + lift * 16.0, -hi.z],
            "to": [-lo.x, hi.y + lift * 16.0, -lo.z],
            "origin": [-pivot.x, pivot.y + lift * 16.0, -pivot.z],
            "rotation": [-p.rot.x, p.rot.y, -p.rot.z],
            "autouv": 0,
            "color": k % 8,
            "faces": faces,
        }));
        outliner.push(json!(id));
    }
    let doc = json!({
        "meta": { "format_version": "4.10", "model_format": "free", "box_uv": false },
        "name": name,
        "resolution": { "width": side, "height": side },
        "elements": elements,
        "outliner": outliner,
        "textures": [{
            "name": format!("{name}.png"),
            "id": "0",
            "uuid": uuid(&format!("{name}-texture")),
            "width": side,
            "height": side,
            "uv_width": side,
            "uv_height": side,
            "particle": false,
            "render_mode": "default",
            "visible": true,
            "mode": "bitmap",
            "source": format!("data:image/png;base64,{data}"),
        }],
    });
    serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())
}

/// Exporte l'atlas et tous les modeles. Renvoie le nombre de fichiers ecrits.
pub fn export(dir: &Path, img: &Img, models: &[Model]) -> Result<usize, String> {
    let (tex_dir, model_dir) = (dir.join("textures"), dir.join("models"));
    fs::create_dir_all(&tex_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&model_dir).map_err(|e| e.to_string())?;
    fs::write(tex_dir.join("atlas.png"), encode_png(&img.0, AT as u32, AT as u32)?).map_err(|e| e.to_string())?;
    let mut n = 1;
    for (i, m) in models.iter().enumerate() {
        let name = NAMES[i];
        fs::write(model_dir.join(format!("{name}.bbmodel")), {
            let (ox, oy) = cell_origin(i);
            bbmodel(name, (ox, oy, CELL), 0.0, m, img)?
        }).map_err(|e| e.to_string())?;
        n += 1;
    }
    Ok(n)
}

fn vec3(v: &Value) -> Option<Vec3> {
    let a = v.as_array()?;
    Some(Vec3::new(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32))
}

/// Relit un .bbmodel : cubes (inclines ou non), texture de 256 x 256 au plus.
pub(crate) fn read_bbmodel(text: &str, cell: (usize, usize, usize), lift: f32, img: &mut Img) -> Result<Model, String> {
    let (ox, oy, side) = cell;
    let doc: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let res_w = doc["resolution"]["width"].as_f64().unwrap_or(side as f64) as f32;
    let res_h = doc["resolution"]["height"].as_f64().unwrap_or(side as f64) as f32;
    // Texture incorporee.
    let source = doc["textures"][0]["source"].as_str().ok_or("pas de texture incorporee")?;
    let b64 = source.split_once("base64,").map(|x| x.1).ok_or("texture non incorporee en base64")?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).map_err(|e| e.to_string())?;
    let (rgba, tw, th) = decode_png(&bytes)?;
    if tw > side || th > side {
        return Err(format!("texture de {tw} x {th} pixels, {side} x {side} au maximum"));
    }
    for y in 0..side {
        for x in 0..side {
            let c = if x < tw && y < th {
                let i = (y * tw + x) * 4;
                [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
            } else {
                [0; 4]
            };
            img.set_px(ox + x, oy + y, c);
        }
    }

    let mut model = Model::default();
    for el in doc["elements"].as_array().ok_or("pas d'elements")? {
        if el["type"].as_str().is_some_and(|t| t != "cube") {
            continue;
        }
        let (from, to) = (vec3(&el["from"]).ok_or("cube sans 'from'")?, vec3(&el["to"]).ok_or("cube sans 'to'")?);
        let (lo, hi) = (from.min(to) / 16.0, from.max(to) / 16.0);
        // Demi-tour inverse de celui de l'export.
        let c = Vec3::new(-(lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0 - lift, -(lo.z + hi.z) / 2.0);
        let h = ((hi - lo) / 2.0).max(Vec3::splat(0.001));
        let mut uv = [[0f32; 4]; 6];
        for (f, face) in FACE_NAMES.iter().enumerate() {
            let r = &el["faces"][*face]["uv"];
            let get = |k: usize| r[k].as_f64().unwrap_or(0.0) as f32;
            let (mut a, mut b, mut c2, mut d) = (get(0), get(1), get(2), get(3));
            if f == 2 || f == 3 {
                (a, b, c2, d) = (c2, d, a, b);
            }
            let (sx, sy) = (tw as f32 / res_w, th as f32 / res_h);
            let s = AT as f32;
            // Marge vers l'interieur du rectangle, pour ne pas deborder sur la face voisine.
            let (ex, ey) = (
                margin((c2 - a).abs() * sx) * (c2 - a).signum(),
                margin((d - b).abs() * sy) * (d - b).signum(),
            );
            uv[f] = [
                (ox as f32 + a * sx + ex) / s,
                (oy as f32 + b * sy + ey) / s,
                (ox as f32 + c2 * sx - ex) / s,
                (oy as f32 + d * sy - ey) / s,
            ];
        }
        let name = el["name"].as_str().unwrap_or("").to_lowercase();
        let leg = if name.starts_with("jambe_b") {
            -1
        } else if name.starts_with("jambe") || name.starts_with("leg") {
            1
        } else {
            0
        };
        // Inclinaison et pivot, ramenes dans notre repere par le meme demi-tour.
        let rot = vec3(&el["rotation"]).map(|r| Vec3::new(-r.x, r.y, -r.z)).unwrap_or(Vec3::ZERO);
        let pivot = vec3(&el["origin"]).map(|o| Vec3::new(-o.x, o.y, -o.z) / 16.0 - Vec3::Y * lift).unwrap_or(c);
        model.parts.push(Part { c, h, rot, pivot, leg, uv });
    }
    if model.parts.is_empty() {
        return Err("aucun cube".to_string());
    }
    Ok(model)
}

/// Applique les fichiers du dossier de ressources par-dessus ce que le code a genere.
/// Renvoie une ligne par fichier pris en compte ou refuse.
pub fn load(dir: &Path, img: &mut Img, models: &mut [Model]) -> Vec<String> {
    let mut log = Vec::new();
    let atlas = dir.join("textures").join("atlas.png");
    if let Ok(data) = fs::read(&atlas) {
        match decode_png(&data) {
            Ok((rgba, w, h)) if w == AT && h == AT => {
                img.0 = rgba;
                log.push(format!("Textures : {}", atlas.display()));
            }
            Ok((_, w, h)) => log.push(format!("{} ignore : {w} x {h} pixels au lieu de {AT} x {AT}", atlas.display())),
            Err(e) => log.push(format!("{} illisible : {e}", atlas.display())),
        }
    }
    for (i, name) in NAMES.iter().enumerate() {
        let path = dir.join("models").join(format!("{name}.bbmodel"));
        let Ok(text) = fs::read_to_string(&path) else { continue };
        let (ox, oy) = cell_origin(i);
        match read_bbmodel(&text, (ox, oy, CELL), 0.0, img) {
            Ok(m) => {
                models[i] = m;
                log.push(format!("Modele : {}", path.display()));
            }
            Err(e) => log.push(format!("{} refuse : {e}", path.display())),
        }
    }
    log
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{atlas, models};

    #[test]
    fn bbmodel_roundtrip() {
        let mut img = atlas::build();
        let built = models::build(&mut img);
        for (i, original) in built.iter().enumerate() {
            let (ox, oy) = cell_origin(i);
            let text = bbmodel(NAMES[i], (ox, oy, CELL), 0.0, original, &img).unwrap();
            let mut copy = Img(img.0.clone());
            let back = read_bbmodel(&text, (ox, oy, CELL), 0.0, &mut copy).unwrap();
            assert_eq!(copy.0, img.0, "la texture de {} doit revenir identique", NAMES[i]);
            assert_eq!(back.parts.len(), original.parts.len());
            for (a, b) in back.parts.iter().zip(&original.parts) {
                assert!(a.c.distance(b.c) < 1e-3 && a.h.distance(b.h) < 1e-3, "boite deplacee dans {}", NAMES[i]);
                assert_eq!(a.leg, b.leg);
                assert!(a.rot.distance(b.rot) < 1e-3 && a.pivot.distance(b.pivot) < 1e-3, "pivot ou inclinaison perdus");
                for f in 0..6 {
                    for k in 0..4 {
                        assert!((a.uv[f][k] - b.uv[f][k]).abs() < 1e-4, "UV modifie dans {}", NAMES[i]);
                    }
                }
            }
        }
    }

    #[test]
    fn png_roundtrip() {
        let img = atlas::build();
        let bytes = encode_png(&img.0, AT as u32, AT as u32).unwrap();
        let (back, w, h) = decode_png(&bytes).unwrap();
        assert_eq!((w, h), (AT, AT));
        assert_eq!(back, img.0);
    }
}
