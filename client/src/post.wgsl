// Passe finale : contours a l'encre (ruptures de profondeur et aretes) et couleurs plus vives.
struct P {
    a: vec4<f32>, // x = plan proche, y = plan lointain, z = tan(champ / 2), w = rapport largeur / hauteur
    b: vec4<f32>, // x = 1 / largeur, y = 1 / hauteur (de la scene), z = force de l'encre, w = epaisseur en pixels
    c: vec4<f32>, // x, y = pixels de la scene par pixel d'ecran
};

@group(0) @binding(0) var<uniform> p: P;
@group(0) @binding(1) var scene: texture_2d<f32>;
// Lue comme une texture ordinaire : OpenGL ne sait pas lire un pixel precis d'une texture de profondeur.
@group(0) @binding(2) var depth: texture_2d<f32>;

@vertex
fn vs_post(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    // Un seul triangle qui couvre tout l'ecran.
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    return vec4<f32>(x, y, 0.0, 1.0);
}

fn clampc(c: vec2<i32>) -> vec2<i32> {
    let size = vec2<i32>(textureDimensions(depth));
    return clamp(c, vec2<i32>(0, 0), size - vec2<i32>(1, 1));
}

// Distance a la camera, en blocs.
fn lin(c: vec2<i32>) -> f32 {
    let d = textureLoad(depth, clampc(c), 0).r;
    return p.a.x * p.a.y / (p.a.y - d * (p.a.y - p.a.x));
}

// Position dans le repere de la camera.
fn vpos(c: vec2<i32>, z: f32) -> vec3<f32> {
    let ndc = (vec2<f32>(c) + vec2<f32>(0.5, 0.5)) * p.b.xy * 2.0 - vec2<f32>(1.0, 1.0);
    return vec3<f32>(ndc.x * p.a.z * p.a.w * z, -ndc.y * p.a.z * z, -z);
}

// Agrandissement d'une scene calculee plus petite que l'ecran : chaque pixel d'ecran lit celui qu'il recouvre.
@fragment
fn fs_blit(@builtin(position) fc: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(textureLoad(scene, clampc(vec2<i32>(fc.xy * p.c.xy)), 0).rgb, 1.0);
}

@fragment
fn fs_post(@builtin(position) fc: vec4<f32>) -> @location(0) vec4<f32> {
    let c = vec2<i32>(fc.xy);
    var col = textureLoad(scene, c, 0).rgb;
    let d0 = textureLoad(depth, c, 0).r;
    if (p.b.z > 0.0 && d0 < 0.99999) {
        let t = i32(p.b.w);
        let dx = vec2<i32>(t, 0);
        let dy = vec2<i32>(0, t);
        let z = lin(c);
        let zr = lin(c + dx);
        let zl = lin(c - dx);
        let zd = lin(c + dy);
        let zu = lin(c - dy);
        // Rupture de profondeur : silhouette d'un objet devant un autre.
        let lap = abs(zr + zl - 2.0 * z) + abs(zd + zu - 2.0 * z);
        var e = smoothstep(0.025, 0.07, lap / z);
        // Arete : la surface change d'orientation.
        let p0 = vpos(c, z);
        let n0 = normalize(cross(vpos(c + dx, zr) - p0, vpos(c + dy, zd) - p0));
        let n1 = normalize(cross(p0 - vpos(c - dx, zl), p0 - vpos(c - dy, zu)));
        e = max(e, smoothstep(0.3, 0.7, 1.0 - dot(n0, n1)));
        // Les traits s'effacent au loin pour ne pas noircir l'horizon.
        let fade = 1.0 - smoothstep(28.0, 85.0, z);
        col = mix(col, vec3<f32>(0.07, 0.05, 0.13), e * fade * p.b.z);
    }
    let grey = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    col = mix(vec3<f32>(grey), col, 1.08);
    return vec4<f32>(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
