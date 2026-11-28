//! Rendu wgpu : Vulkan en priorite, repli sur le backend natif si Vulkan est absent.
//! La scene est dessinee hors ecran, puis une passe finale ajoute les contours a l'encre
//! et l'interface.
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::atlas::AT;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vtx {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [u8; 4],
    /// [ciel, bloc, sans eclairage, mouvement (eau, vegetation)]
    pub light: [u8; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct V2 {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    pub view_proj: [[f32; 4]; 4],
    pub cam: [f32; 4],
    pub fog: [f32; 4],
    pub params: [f32; 4],
    pub sun: [f32; 4],
}

/// Reglages de la passe finale.
#[derive(Clone, Copy)]
pub struct Post {
    /// Tangente du demi-champ de vision vertical.
    pub tan_half: f32,
    /// Force des contours a l'encre (0 = aucun).
    pub ink: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PostU {
    a: [f32; 4],
    b: [f32; 4],
    /// x, y = pixels de la scene par pixel d'ecran.
    c: [f32; 4],
}

pub const NEAR: f32 = 0.08;
pub const FAR: f32 = 600.0;

#[derive(Default)]
pub struct MeshData {
    pub ov: Vec<Vtx>,
    pub oi: Vec<u32>,
    /// Faces opaques visibles des deux cotes (plantes en croix, rails, fils) : memes sommets que `oi`.
    pub oi2: Vec<u32>,
    pub tv: Vec<Vtx>,
    pub ti: Vec<u32>,
    /// Lumiere de chaque bloc du chunk : ciel dans les 4 bits hauts, blocs dans les 4 bas.
    pub light: Vec<u8>,
}

struct GpuMesh {
    vb: wgpu::Buffer,
    ib: wgpu::Buffer,
    /// Indices des faces a une seule face visible, puis de celles qui se voient des deux cotes.
    n: u32,
    n2: u32,
}

pub struct ChunkMesh {
    opaque: Option<GpuMesh>,
    trans: Option<GpuMesh>,
    /// Hauteurs entre lesquelles le maillage tient : elles bornent la boite testee contre le champ de vision.
    lo: f32,
    hi: f32,
}

/// Tampon de sommets garde d'une image a l'autre ; il ne grandit que lorsqu'il le faut.
struct Dyn {
    buf: wgpu::Buffer,
    cap: u64,
}

impl Dyn {
    fn new(device: &wgpu::Device, cap: u64) -> Dyn {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("image"),
            size: cap,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Dyn { buf, cap }
    }

    /// Copie `data` dans le tampon et renvoie le nombre de sommets.
    fn fill<T: bytemuck::Pod>(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[T]) -> u32 {
        let bytes: &[u8] = bytemuck::cast_slice(data);
        if bytes.is_empty() {
            return 0;
        }
        if bytes.len() as u64 > self.cap {
            *self = Dyn::new(device, (bytes.len() as u64).next_power_of_two());
        }
        queue.write_buffer(&self.buf, 0, bytes);
        data.len() as u32
    }
}

/// Les six plans (normale vers l'interieur, distance) de la pyramide de vue d'une matrice vue-projection.
fn frustum(m: &Mat4) -> [glam::Vec4; 6] {
    let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
    [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2]
}

/// La boite [lo, hi] touche-t-elle la pyramide de vue ?
fn in_frustum(planes: &[glam::Vec4; 6], lo: Vec3, hi: Vec3) -> bool {
    planes.iter().all(|p| {
        // Le coin de la boite le plus avance dans le sens du plan.
        let v = Vec3::new(if p.x >= 0.0 { hi.x } else { lo.x }, if p.y >= 0.0 { hi.y } else { lo.y }, if p.z >= 0.0 { hi.z } else { lo.z });
        p.x * v.x + p.y * v.y + p.z * v.z + p.w >= 0.0
    })
}

/// Cibles hors ecran, recreees a chaque changement de taille.
struct Targets {
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    post_bind: wgpu::BindGroup,
    /// Scene encree, quand elle est calculee plus petite que l'ecran, et de quoi l'agrandir.
    ink: Option<wgpu::TextureView>,
    blit_bind: wgpu::BindGroup,
}

pub struct Gfx {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    targets: Targets,
    pipe_opaque: wgpu::RenderPipeline,
    pipe_trans: wgpu::RenderPipeline,
    pipe_ui: wgpu::RenderPipeline,
    pipe_post: wgpu::RenderPipeline,
    /// Blocs pleins : les faces tournees vers l'arriere sont ecartees.
    pipe_solid: wgpu::RenderPipeline,
    pipe_blit: wgpu::RenderPipeline,
    sky_buf: Dyn,
    ent_buf: Dyn,
    over_buf: Dyn,
    ui_buf: Dyn,
    /// Derniere image : (chunks dessines, triangles de terrain).
    pub drawn: (usize, u32),
    /// Part de la resolution de la fenetre a laquelle la scene est calculee (0,25 a 1).
    scale: f32,
    /// Part de la resolution de la fenetre utilisee par la surface d'affichage, et taille de la fenetre.
    res: f32,
    win: (u32, u32),
    bind: wgpu::BindGroup,
    post_layout: wgpu::BindGroupLayout,
    ubuf: wgpu::Buffer,
    pbuf: wgpu::Buffer,
    pub meshes: HashMap<(i32, i32), ChunkMesh>,
    pub backend: String,
    /// Aucune carte graphique utilisable : Windows calcule l'image sur le processeur, tres lentement.
    pub software: bool,
    /// Premiere erreur signalee par le moteur graphique depuis la derniere lecture.
    fault: Arc<Mutex<Option<String>>>,
    atlas_tex: wgpu::Texture,
}

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const VTX_ATTRS: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Unorm8x4, 3 => Unorm8x4];
const V2_ATTRS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];

/// Niveaux de detail de l'atlas : chaque tuile de 64 px se reduit jusqu'a 1 px.
const MIPS: u32 = 7;

/// Reduit une image RGBA de moitie ; la couleur est ponderee par l'opacite pour eviter les franges sombres.
fn halve(src: &[u8], w: usize) -> Vec<u8> {
    let h = w / 2;
    let mut out = vec![0u8; h * h * 4];
    for y in 0..h {
        for x in 0..h {
            let (mut c, mut a) = ([0u32; 3], 0u32);
            for d in 0..4 {
                let i = ((y * 2 + d / 2) * w + x * 2 + d % 2) * 4;
                let al = src[i + 3] as u32;
                a += al;
                for k in 0..3 {
                    c[k] += src[i + k] as u32 * al;
                }
            }
            let o = (y * h + x) * 4;
            for k in 0..3 {
                out[o + k] = if a > 0 { (c[k] / a) as u8 } else { 0 };
            }
            out[o + 3] = (a / 4) as u8;
        }
    }
    out
}

/// Ecrit une moitie de l'atlas (un carre de AT pixels) : celle du jeu en haut, celle des mods en bas.
fn write_atlas(queue: &wgpu::Queue, tex: &wgpu::Texture, atlas: &[u8], mods: bool) {
    let mut level = atlas.to_vec();
    let mut w = AT;
    for mip in 0..MIPS {
        let origin = wgpu::Origin3d { x: 0, y: if mods { w as u32 } else { 0 }, z: 0 };
        queue.write_texture(
            wgpu::ImageCopyTexture { texture: tex, mip_level: mip, origin, aspect: wgpu::TextureAspect::All },
            &level,
            wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4 * w as u32), rows_per_image: Some(w as u32) },
            wgpu::Extent3d { width: w as u32, height: w as u32, depth_or_array_layers: 1 },
        );
        level = halve(&level, w);
        w /= 2;
    }
}

fn make_targets(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    size: (u32, u32),
    layout: &wgpu::BindGroupLayout,
    pbuf: &wgpu::Buffer,
) -> Targets {
    let tex = |label: &str, format: wgpu::TextureFormat| {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    };
    let color = tex("scene", config.format);
    let depth = tex("profondeur", DEPTH_FORMAT);
    let post_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("passe finale"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: pbuf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&color) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&depth) },
        ],
    });
    // Scene plus petite que l'ecran : une image intermediaire recoit les contours, puis est agrandie.
    let ink = (size != (config.width, config.height)).then(|| tex("encre", config.format));
    let blit_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("agrandissement"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: pbuf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(ink.as_ref().unwrap_or(&color)) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&depth) },
        ],
    });
    Targets { color, depth, post_bind, ink, blit_bind }
}

impl Gfx {
    pub fn new(window: Arc<Window>, atlas: &[u8]) -> Gfx {
        let size = window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));

        // Vulkan d'abord (MoltenVK sur macOS), puis le moteur natif du systeme, puis OpenGL : les cartes
        // d'avant 2013 n'ont ni Vulkan ni DirectX 12, mais font tourner OpenGL. Le rendu logiciel de
        // Windows ne sert qu'en dernier recours. EPICUBE_BACKEND=auto, dx12, metal ou gl impose un moteur.
        let asked = std::env::var("EPICUBE_BACKEND").unwrap_or_default();
        let tries: &[wgpu::Backends] = match asked.as_str() {
            "metal" => &[wgpu::Backends::METAL, wgpu::Backends::PRIMARY],
            "gl" => &[wgpu::Backends::GL],
            "dx12" => &[wgpu::Backends::DX12],
            "" | "vulkan" => &[wgpu::Backends::VULKAN, wgpu::Backends::PRIMARY, wgpu::Backends::GL],
            _ => &[wgpu::Backends::PRIMARY, wgpu::Backends::GL],
        };
        let (mut found, mut fallback) = (None, None);
        for &backends in tries {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends, ..Default::default() });
            let Ok(surface) = instance.create_surface(window.clone()) else { continue };
            let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }));
            let Some(adapter) = adapter else { continue };
            // Un rendu calcule par le processeur : on le garde de cote, une vraie carte peut suivre.
            if adapter.get_info().device_type == wgpu::DeviceType::Cpu {
                fallback.get_or_insert((surface, adapter));
                continue;
            }
            found = Some((surface, adapter));
            break;
        }
        let found = found.or(fallback);
        let (surface, adapter) = found.expect("aucune carte graphique compatible trouvee");
        let info = adapter.get_info();
        let backend = format!("{:?} - {}", info.backend, info.name);
        println!("Rendu : {backend}");
        // Tout ce que le systeme propose : utile pour comprendre un mauvais choix ou un pilote manquant.
        for a in wgpu::Instance::new(wgpu::InstanceDescriptor::default()).enumerate_adapters(wgpu::Backends::all()) {
            let i = a.get_info();
            println!("  carte : {:?} - {} ({:?}, pilote {} {})", i.backend, i.name, i.device_type, i.driver, i.driver_info);
        }
        let software = info.device_type == wgpu::DeviceType::Cpu;
        if software {
            println!("Rendu logiciel : aucun pilote de carte graphique utilisable, le jeu sera tres lent");
        }
        if std::env::var("EPICUBE_BENCH").is_ok() {
            println!("Modes de presentation : {:?}", surface.get_capabilities(&adapter).present_modes);
        }

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                // OpenGL n'offre pas toutes les limites habituelles : on prend ce que la carte sait faire.
                required_limits: if info.backend == wgpu::Backend::Gl {
                    wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits())
                } else {
                    wgpu::Limits::default()
                },
                // En rendu logiciel la memoire est celle de l'ordinateur : on la reserve par petits blocs.
                memory_hints: if software { wgpu::MemoryHints::MemoryUsage } else { wgpu::MemoryHints::Performance },
            },
            None,
        ))
        .expect("impossible de creer le peripherique graphique");
        // Sans cela, la moindre erreur du moteur graphique ferme le jeu : on la garde pour l'afficher.
        let fault = Arc::new(Mutex::new(None::<String>));
        let seen = fault.clone();
        device.on_uncaptured_error(Box::new(move |e| {
            eprintln!("Erreur graphique : {e}");
            if let Ok(mut first) = seen.lock() {
                first.get_or_insert_with(|| e.to_string());
            }
        }));

        // Format non sRGB : les couleurs sont ecrites telles qu'on les voit.
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: w,
            height: h,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Atlas de textures.
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("atlas"),
            // Deux carres l'un sous l'autre : les textures du jeu, puis celles des mods.
            size: wgpu::Extent3d { width: AT as u32, height: 2 * AT as u32, depth_or_array_layers: 1 },
            mip_level_count: MIPS,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        write_atlas(&queue, &tex, atlas, false);
        let tex_view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            // Pixel art : chaque pixel reste net de pres ; de loin, le melange entre niveaux
            // supprime le scintillement sans deborder sur la tuile voisine.
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // L'interface est en pixel art elle aussi : aucun lissage.
        let ui_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let uniform_buffer = |label: &str, size: usize| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let ubuf = uniform_buffer("uniformes", std::mem::size_of::<Uniforms>());
        let pbuf = uniform_buffer("passe finale", std::mem::size_of::<PostU>());

        let uniform_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let texture_entry = |binding: u32, sample_type: wgpu::TextureSampleType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                uniform_entry(0),
                texture_entry(1, wgpu::TextureSampleType::Float { filterable: true }),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&tex_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&ui_sampler) },
            ],
        });
        let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("passe finale"),
            entries: &[
                uniform_entry(0),
                texture_entry(1, wgpu::TextureSampleType::Float { filterable: false }),
                texture_entry(2, wgpu::TextureSampleType::Float { filterable: false }),
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let post_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("passe finale"),
            source: wgpu::ShaderSource::Wgsl(include_str!("post.wgsl").into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let post_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&post_layout],
            push_constant_ranges: &[],
        });

        let world_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vtx>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &VTX_ATTRS,
        };
        let ui_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<V2>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &V2_ATTRS,
        };

        // `depth` : None = pas de tampon de profondeur ; Some(ecrit) sinon.
        let make = |label: &str,
                    module: &wgpu::ShaderModule,
                    layout: &wgpu::PipelineLayout,
                    vs: &str,
                    fs: &str,
                    buffers: &[wgpu::VertexBufferLayout],
                    blend: Option<wgpu::BlendState>,
                    depth: Option<bool>,
                    cull: Option<wgpu::Face>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState { module, entry_point: vs, buffers, compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: fs,
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState { cull_mode: cull, ..Default::default() },
                depth_stencil: depth.map(|write| wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: write,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        let alpha = Some(wgpu::BlendState::ALPHA_BLENDING);
        let world = std::slice::from_ref(&world_layout);
        let pipe_opaque = make("opaque", &shader, &pl, "vs_main", "fs_opaque", world, None, Some(true), None);
        let pipe_solid = make("blocs pleins", &shader, &pl, "vs_main", "fs_opaque", world, None, Some(true), Some(wgpu::Face::Back));
        let pipe_trans = make("transparent", &shader, &pl, "vs_main", "fs_trans", world, alpha, Some(false), None);
        let pipe_ui = make("interface", &shader, &pl, "vs_ui", "fs_ui", &[ui_layout], alpha, None, None);
        let pipe_post = make("passe finale", &post_shader, &post_pl, "vs_post", "fs_post", &[], None, None, None);
        let pipe_blit = make("agrandissement", &post_shader, &post_pl, "vs_post", "fs_blit", &[], None, None, None);
        let dynbuf = || Dyn::new(&device, 1 << 16);
        let (sky_buf, ent_buf, over_buf, ui_buf) = (dynbuf(), dynbuf(), dynbuf(), dynbuf());

        let targets = make_targets(&device, &config, (w, h), &post_layout, &pbuf);
        Gfx {
            software,
            fault,
            surface,
            device,
            queue,
            config,
            targets,
            pipe_opaque,
            pipe_trans,
            pipe_ui,
            pipe_post,
            pipe_solid,
            pipe_blit,
            sky_buf,
            ent_buf,
            over_buf,
            ui_buf,
            drawn: (0, 0),
            scale: 1.0,
            res: 1.0,
            win: (w, h),
            bind,
            post_layout,
            ubuf,
            pbuf,
            meshes: HashMap::new(),
            backend,
            atlas_tex: tex,
        }
    }

    /// Remplace les textures (rechargement des ressources).
    /// Erreur graphique survenue depuis le dernier appel, s'il y en a une.
    pub fn take_fault(&self) -> Option<String> {
        self.fault.lock().ok().and_then(|mut f| f.take())
    }

    pub fn set_atlas(&mut self, atlas: &[u8]) {
        write_atlas(&self.queue, &self.atlas_tex, atlas, false);
    }

    /// Textures des mods de la partie : un carre de AT pixels, range sous l'atlas du jeu.
    pub fn set_mod_atlas(&mut self, tiles: &[u8]) {
        write_atlas(&self.queue, &self.atlas_tex, tiles, true);
    }

    /// Taille, en pixels, de l'image de la scene avant agrandissement.
    fn scene_size(&self) -> (u32, u32) {
        (((self.config.width as f32 * self.scale).round() as u32).max(1), ((self.config.height as f32 * self.scale).round() as u32).max(1))
    }

    /// Change la resolution a laquelle la scene est calculee ; l'interface reste a la resolution de la fenetre.
    pub fn set_scale(&mut self, scale: f32) {
        let scale = scale.clamp(0.25, 1.0);
        if (scale - self.scale).abs() > 1e-3 {
            self.scale = scale;
            self.targets = make_targets(&self.device, &self.config, self.scene_size(), &self.post_layout, &self.pbuf);
        }
    }

    pub fn set_vsync(&mut self, on: bool) {
        let free = match std::env::var("EPICUBE_PRESENT").as_deref() {
            Ok("immediate") => wgpu::PresentMode::Immediate,
            Ok("mailbox") => wgpu::PresentMode::Mailbox,
            _ => wgpu::PresentMode::AutoNoVsync,
        };
        let mode = if on { wgpu::PresentMode::Fifo } else { free };
        if self.config.present_mode != mode {
            self.config.present_mode = mode;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn size(&self) -> (f32, f32) {
        (self.config.width as f32, self.config.height as f32)
    }

    /// Part de la resolution de la fenetre a laquelle tout est dessine (0,25 a 1) ; le systeme agrandit le reste.
    /// A 0,5 sur un ecran a haute densite, on dessine quatre fois moins de pixels.
    ///
    /// Sur macOS, c'est la surface d'affichage elle-meme qui rapetisse : le systeme l'etire a la taille
    /// de la fenetre, et l'on gagne aussi sur la presentation de l'image. Windows et Linux n'etirent pas
    /// une surface plus petite que sa fenetre : la, seule la scene 3D est calculee en petit, puis agrandie
    /// par le jeu, et l'interface reste a la resolution de l'ecran.
    /// `whole` : rapetisser la surface entiere (interface comprise), ce que seul macOS sait etirer.
    pub fn set_res(&mut self, res: f32, whole: bool) {
        let res = res.clamp(0.25, 1.0);
        let surface = if whole && cfg!(target_os = "macos") { res } else { 1.0 };
        if (surface - self.res).abs() > 1e-3 {
            self.res = surface;
            let (w, h) = self.win;
            self.resize(w, h);
        }
        // La scene est calculee a `res` fois la fenetre : dans une surface deja rapetissee, il n'y a plus rien a reduire.
        self.set_scale((res / surface).min(1.0));
    }

    pub fn res(&self) -> f32 {
        self.res
    }

    /// `w`, `h` : taille de la fenetre en pixels de l'ecran.
    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.win = (w, h);
        self.config.width = ((w as f32 * self.res).round() as u32).max(1);
        self.config.height = ((h as f32 * self.res).round() as u32).max(1);
        self.surface.configure(&self.device, &self.config);
        self.targets = make_targets(&self.device, &self.config, self.scene_size(), &self.post_layout, &self.pbuf);
    }

    /// `i2` : triangles a dessiner des deux cotes, ranges a la suite de `i` dans le meme tampon.
    fn gpu_mesh(&self, v: &[Vtx], i: &[u32], i2: &[u32]) -> Option<GpuMesh> {
        if i.is_empty() && i2.is_empty() {
            return None;
        }
        let joined: Vec<u32>;
        let (i, n, n2) = if i2.is_empty() {
            (i, i.len() as u32, 0)
        } else {
            joined = [i, i2].concat();
            (&joined[..], i.len() as u32, i2.len() as u32)
        };
        Some(GpuMesh {
            vb: self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(v),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            ib: self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(i),
                usage: wgpu::BufferUsages::INDEX,
            }),
            n,
            n2,
        })
    }

    pub fn upload(&mut self, key: (i32, i32), m: &MeshData) {
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for v in m.ov.iter().chain(m.tv.iter()) {
            lo = lo.min(v.pos[1]);
            hi = hi.max(v.pos[1]);
        }
        let mesh = ChunkMesh { opaque: self.gpu_mesh(&m.ov, &m.oi, &m.oi2), trans: self.gpu_mesh(&m.tv, &m.ti, &[]), lo, hi };
        self.meshes.insert(key, mesh);
    }

    /// `sky`, `ents`, `overlay` (transparent) et `ui` sont des listes de triangles non indexees.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        u: &Uniforms,
        post: Post,
        clear: [f32; 3],
        cam: Vec3,
        fwd: Vec3,
        sky: &[Vtx],
        ents: &[Vtx],
        overlay: &[Vtx],
        ui: &[V2],
    ) {
        crate::prof::mark("gpu:image");
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Err(_) => return,
        };
        crate::prof::mark("gpu:prepare");
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let (w, h) = self.size();
        self.queue.write_buffer(&self.ubuf, 0, bytemuck::bytes_of(u));
        let (sw, sh) = self.scene_size();
        let (sw, sh) = (sw as f32, sh as f32);
        let pu = PostU {
            a: [NEAR, FAR, post.tan_half, w / h],
            // Trait plus epais sur les ecrans a haute densite.
            b: [1.0 / sw, 1.0 / sh, post.ink, (sh / 800.0).round().max(1.0)],
            c: [sw / w, sh / h, 0.0, 0.0],
        };
        self.queue.write_buffer(&self.pbuf, 0, bytemuck::bytes_of(&pu));

        // Chunks visibles : seuls ceux qui coupent la pyramide de vue sont dessines, du plus proche au plus lointain.
        let planes = frustum(&Mat4::from_cols_array_2d(&u.view_proj));
        let _ = fwd;
        let mut visible: Vec<(f32, &ChunkMesh)> = self
            .meshes
            .iter()
            .filter_map(|(&(cx, cz), m)| {
                let lo = Vec3::new(cx as f32 * 16.0, m.lo, cz as f32 * 16.0);
                let hi = Vec3::new(lo.x + 16.0, m.hi, lo.z + 16.0);
                if !in_frustum(&planes, lo, hi) {
                    return None;
                }
                let d = (lo + hi) * 0.5 - cam;
                Some((d.x * d.x + d.z * d.z, m))
            })
            .collect();
        visible.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        self.drawn = (visible.len(), visible.iter().map(|(_, m)| m.opaque.as_ref().map_or(0, |g| g.n + g.n2) + m.trans.as_ref().map_or(0, |g| g.n)).sum::<u32>() / 3);

        // Geometrie de l'image (ciel, creatures, effets, interface) : des tampons gardes d'une image a l'autre.
        let sky_n = self.sky_buf.fill(&self.device, &self.queue, sky);
        let ent_n = self.ent_buf.fill(&self.device, &self.queue, ents);
        let over_n = self.over_buf.fill(&self.device, &self.queue, overlay);
        let ui_n = self.ui_buf.fill(&self.device, &self.queue, ui);
        let color = |c: [f32; 3]| wgpu::Color { r: c[0] as f64, g: c[1] as f64, b: c[2] as f64, a: 1.0 };
        let scaled = self.targets.ink.is_some();

        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            // Passe 1 : la scene, hors ecran.
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.targets.color,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(color(clear)), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.targets.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.bind, &[]);

            // Ciel d'abord, sans ecrire la profondeur : le terrain passe par-dessus.
            if sky_n > 0 {
                pass.set_pipeline(&self.pipe_trans);
                pass.set_vertex_buffer(0, self.sky_buf.buf.slice(..));
                pass.draw(0..sky_n, 0..1);
            }

            // Blocs pleins, du plus proche au plus lointain : les faces tournees vers l'arriere ne sont pas dessinees,
            // et ce qui est cache derriere un bloc deja trace est ecarte avant d'etre colorie.
            pass.set_pipeline(&self.pipe_solid);
            for (_, m) in visible.iter() {
                if let Some(g) = m.opaque.as_ref().filter(|g| g.n > 0) {
                    pass.set_vertex_buffer(0, g.vb.slice(..));
                    pass.set_index_buffer(g.ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..g.n, 0, 0..1);
                }
            }
            // Formes vues des deux cotes (plantes, rails, fils), puis creatures et objets.
            pass.set_pipeline(&self.pipe_opaque);
            for (_, m) in visible.iter() {
                if let Some(g) = m.opaque.as_ref().filter(|g| g.n2 > 0) {
                    pass.set_vertex_buffer(0, g.vb.slice(..));
                    pass.set_index_buffer(g.ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(g.n..g.n + g.n2, 0, 0..1);
                }
            }
            if ent_n > 0 {
                pass.set_vertex_buffer(0, self.ent_buf.buf.slice(..));
                pass.draw(0..ent_n, 0..1);
            }

            // Transparents du plus loin au plus proche.
            pass.set_pipeline(&self.pipe_trans);
            for (_, m) in visible.iter().rev() {
                if let Some(g) = &m.trans {
                    pass.set_vertex_buffer(0, g.vb.slice(..));
                    pass.set_index_buffer(g.ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..g.n, 0, 0..1);
                }
            }
            if over_n > 0 {
                pass.set_vertex_buffer(0, self.over_buf.buf.slice(..));
                pass.draw(0..over_n, 0..1);
            }
        }
        if let Some(ink) = &self.targets.ink {
            // Scene calculee en petit : les contours le sont aussi, avant l'agrandissement.
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("encre"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: ink,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(color(clear)), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipe_post);
            pass.set_bind_group(0, &self.targets.post_bind, &[]);
            pass.draw(0..3, 0..1);
        }
        {
            // Passe finale, a l'ecran : contours (ou simple agrandissement), puis l'interface par-dessus.
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("finale"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(color(clear)), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if scaled {
                pass.set_pipeline(&self.pipe_blit);
                pass.set_bind_group(0, &self.targets.blit_bind, &[]);
            } else {
                pass.set_pipeline(&self.pipe_post);
                pass.set_bind_group(0, &self.targets.post_bind, &[]);
            }
            pass.draw(0..3, 0..1);
            if ui_n > 0 {
                pass.set_pipeline(&self.pipe_ui);
                pass.set_bind_group(0, &self.bind, &[]);
                pass.set_vertex_buffer(0, self.ui_buf.buf.slice(..));
                pass.draw(0..ui_n, 0..1);
            }
        }
        crate::prof::mark("gpu:envoi");
        self.queue.submit(Some(enc.finish()));
        crate::prof::mark("gpu:present");
        frame.present();
    }
}
