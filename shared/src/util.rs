//! Aleatoire deterministe et bruit de valeur.

pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 32) as u32
    }
    /// Flottant dans [0, 1).
    pub fn f(&mut self) -> f32 {
        (self.next() >> 8) as f32 / 16_777_216.0
    }
    /// Entier dans [a, b).
    pub fn range(&mut self, a: i32, b: i32) -> i32 {
        a + (self.next() % ((b - a).max(1) as u32)) as i32
    }
    /// Flottant double dans [0, 1).
    pub fn d(&mut self) -> f64 {
        self.f() as f64
    }
    pub fn chance(&mut self, p: impl Into<f64>) -> bool {
        (self.f() as f64) < p.into()
    }
}

pub fn hash(x: i32, y: i32, z: i32, seed: u32) -> u32 {
    let mut h = seed
        ^ (x as u32).wrapping_mul(0x85EB_CA6B)
        ^ (y as u32).wrapping_mul(0xC2B2_AE35)
        ^ (z as u32).wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    h
}

#[inline]
fn lat(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    (hash(x, y, z, seed) & 0xFFFF) as f32 / 32767.5 - 1.0
}

#[inline]
fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Bruit de valeur 2D dans [-1, 1]. Les coordonnees sont lues en double precision : a des dizaines
/// de millions de blocs de l'origine, un flottant simple ne distingue plus deux blocs voisins et le
/// relief se figerait en marches ; la case du reseau est donc calculee en f64, seul le reste passe en f32.
pub fn noise2(x: impl Into<f64>, y: impl Into<f64>, seed: u32) -> f32 {
    let (x, y) = (x.into(), y.into());
    let (xf, yf) = (x.floor(), y.floor());
    let (xi, yi) = (xf as i64 as i32, yf as i64 as i32);
    let (tx, ty) = (smooth((x - xf) as f32), smooth((y - yf) as f32));
    let a = lerp(lat(xi, yi, 0, seed), lat(xi.wrapping_add(1), yi, 0, seed), tx);
    let b = lerp(lat(xi, yi.wrapping_add(1), 0, seed), lat(xi.wrapping_add(1), yi.wrapping_add(1), 0, seed), tx);
    lerp(a, b, ty)
}

/// Bruit de valeur 3D dans [-1, 1].
pub fn noise3(x: impl Into<f64>, y: impl Into<f64>, z: impl Into<f64>, seed: u32) -> f32 {
    let (x, y, z) = (x.into(), y.into(), z.into());
    let (xf, yf, zf) = (x.floor(), y.floor(), z.floor());
    let (xi, yi, zi) = (xf as i64 as i32, yf as i64 as i32, zf as i64 as i32);
    let (tx, ty, tz) = (smooth((x - xf) as f32), smooth((y - yf) as f32), smooth((z - zf) as f32));
    let mut r = [0.0f32; 2];
    for (k, item) in r.iter_mut().enumerate() {
        let z = zi.wrapping_add(k as i32);
        let a = lerp(lat(xi, yi, z, seed), lat(xi.wrapping_add(1), yi, z, seed), tx);
        let b = lerp(lat(xi, yi.wrapping_add(1), z, seed), lat(xi.wrapping_add(1), yi.wrapping_add(1), z, seed), tx);
        *item = lerp(a, b, ty);
    }
    lerp(r[0], r[1], tz)
}

/// Bruit fractal 2D, normalise dans [-1, 1].
pub fn fbm2(x: impl Into<f64>, y: impl Into<f64>, seed: u32, oct: u32) -> f32 {
    let (x, y) = (x.into(), y.into());
    let (mut amp, mut freq, mut sum, mut norm) = (1.0f32, 1.0f64, 0.0f32, 0.0f32);
    for o in 0..oct {
        sum += noise2(x * freq, y * freq, seed.wrapping_add(o * 131)) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}
