struct U {
    view_proj: mat4x4<f32>,
    cam: vec4<f32>,    // xyz = camera, w = secondes ecoulees
    fog: vec4<f32>,    // rgb = couleur du brouillard, a = debut
    params: vec4<f32>, // x = lumiere du jour, y = fin du brouillard, w = lumiere minimale
    sun: vec4<f32>,    // rgb = teinte de la lumiere du ciel
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var samp_ui: sampler;

struct VO {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) light: vec3<f32>,
    @location(3) dist: f32,
};

// light.w : 1 = surface d'eau (vagues), 0.5 = vegetation (balancement).
@vertex
fn vs_main(
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) light: vec4<f32>,
) -> VO {
    var p = pos;
    let t = u.cam.w;
    if (light.w > 0.9) {
        p.y += sin(t * 1.7 + pos.x * 0.9 + pos.z * 0.6) * 0.035 + sin(t * 1.1 + pos.z * 1.3) * 0.025 - 0.03;
    } else if (light.w > 0.3) {
        p.x += sin(t * 1.6 + pos.x * 1.3 + pos.z * 1.7 + pos.y) * 0.05;
        p.z += sin(t * 1.3 + pos.x * 1.1 - pos.z * 1.5) * 0.05;
    }
    var o: VO;
    o.pos = u.view_proj * vec4<f32>(p, 1.0);
    o.uv = uv;
    o.color = color;
    o.light = light.xyz;
    o.dist = distance(p, u.cam.xyz);
    return o;
}

// light.x = lumiere du ciel, light.y = lumiere des blocs, light.z = 1 pour ignorer eclairage et brouillard.
fn shade(i: VO, t: vec4<f32>) -> vec3<f32> {
    let base = t.rgb * i.color.rgb;
    if (i.light.z > 0.5) {
        return base;
    }
    // Le ciel eclaire avec sa propre teinte (bleutee la nuit), les torches en orange chaud.
    let sky = pow(0.8, 15.0 * (1.0 - i.light.x * u.params.x)) * u.sun.rgb;
    let blk = pow(0.8, 15.0 * (1.0 - i.light.y)) * vec3<f32>(1.0, 0.85, 0.62);
    var lit = max(sky, blk);
    // Ombres en paliers, comme sur un celluloid.
    // Aplats de celluloid : on arrondit l'intensite, pas chaque couleur a part
    // (sinon la lumiere chaude des lanternes se decompose en anneaux rouges et jaunes).
    let peak = max(max(lit.r, lit.g), max(lit.b, 0.001));
    lit = mix(lit, lit * (floor(peak * 6.0 + 0.5) / 6.0 / peak), 0.75);
    lit = max(lit, vec3<f32>(u.params.w));
    let f = clamp((i.dist - u.fog.a) / (u.params.y - u.fog.a), 0.0, 1.0);
    return mix(base * lit, u.fog.rgb, f * f * (3.0 - 2.0 * f));
}

@fragment
fn fs_opaque(i: VO) -> @location(0) vec4<f32> {
    let t = textureSample(atlas, samp, i.uv);
    if (t.a < 0.3) {
        discard;
    }
    return vec4<f32>(shade(i, t), 1.0);
}

@fragment
fn fs_trans(i: VO) -> @location(0) vec4<f32> {
    let t = textureSample(atlas, samp, i.uv);
    let a = t.a * i.color.a;
    if (a < 0.01) {
        discard;
    }
    return vec4<f32>(shade(i, t), a);
}

struct UO {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_ui(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>) -> UO {
    var o: UO;
    o.pos = vec4<f32>(pos, 0.0, 1.0);
    o.uv = uv;
    o.color = color;
    return o;
}

@fragment
fn fs_ui(i: UO) -> @location(0) vec4<f32> {
    return textureSampleLevel(atlas, samp_ui, i.uv, 0.0) * i.color;
}
