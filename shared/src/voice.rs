//! Chat vocal de proximite : format de la voix et portee.
//!
//! La voix voyage en mono a 16 kHz, par tranches de 20 ms, chaque echantillon compresse sur un octet
//! par la loi mu (celle du telephone) : 16 ko par seconde et par joueur qui parle, sans aucune
//! bibliotheque externe. Le serveur ne relaie une voix qu'aux joueurs assez proches pour l'entendre.
pub const RATE: u32 = 16_000;
/// Echantillons par tranche (20 ms).
pub const FRAME: usize = 320;
/// Distance, en blocs, au-dela de laquelle une voix ne porte plus.
pub const RANGE: f64 = 48.0;
/// Taille maximale d'un envoi : quelques tranches regroupees.
pub const MAX_PACKET: usize = FRAME * 4;
/// Envois acceptes par seconde et par joueur (50 suffisent pour parler sans arret).
pub const MAX_RATE: u32 = 80;

const MU: f32 = 255.0;

/// Compresse des echantillons (-1 a 1) : un octet chacun, plus fin pour les sons faibles.
pub fn encode(samples: &[f32]) -> Vec<u8> {
    samples
        .iter()
        .map(|&x| {
            let x = x.clamp(-1.0, 1.0);
            let y = (1.0 + MU * x.abs()).ln() / (1.0 + MU).ln();
            ((y.copysign(x) * 0.5 + 0.5) * 255.0).round() as u8
        })
        .collect()
}

pub fn decode(data: &[u8]) -> Vec<f32> {
    data.iter()
        .map(|&b| {
            let y = b as f32 / 255.0 * 2.0 - 1.0;
            (((1.0 + MU).powf(y.abs()) - 1.0) / MU).copysign(y)
        })
        .collect()
}

/// Niveau sonore d'une tranche (moyenne quadratique), de 0 a 1.
pub fn level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Ramene un son capte a `rate` Hz (un canal) vers 16 kHz, en moyennant les echantillons de chaque pas.
/// `state` garde l'avancement entre deux appels : (phase, somme, nombre).
pub fn resample(input: &[f32], rate: u32, state: &mut (u32, f32, u32), out: &mut Vec<f32>) {
    for &x in input {
        state.1 += x;
        state.2 += 1;
        state.0 += RATE;
        if state.0 >= rate {
            state.0 -= rate;
            out.push(state.1 / state.2 as f32);
            (state.1, state.2) = (0.0, 0);
        }
    }
}

/// Un joueur en `a` entend-il la voix d'un joueur en `b` ?
pub fn hears(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>() <= RANGE * RANGE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_survives_the_codec() {
        // Une voyelle synthetique : deux harmoniques, a volume moyen.
        let wave: Vec<f32> = (0..FRAME * 4).map(|i| {
            let t = i as f32 / RATE as f32;
            0.3 * (t * 220.0 * std::f32::consts::TAU).sin() + 0.15 * (t * 660.0 * std::f32::consts::TAU).sin()
        }).collect();
        let data = encode(&wave);
        assert_eq!(data.len(), wave.len(), "un octet par echantillon");
        let back = decode(&data);
        let noise = level(&wave.iter().zip(&back).map(|(a, b)| a - b).collect::<Vec<_>>());
        assert!(level(&wave) / noise > 30.0, "rapport signal sur bruit trop faible : {}", level(&wave) / noise);
        // Le silence reste du silence, et rien ne deborde.
        assert!(decode(&encode(&[0.0; 16])).iter().all(|x| x.abs() < 0.005));
        assert!(decode(&encode(&[5.0, -5.0])).iter().all(|x| x.abs() <= 1.0001));
        // Un murmure passe aussi : c'est l'interet de la loi mu.
        let quiet: Vec<f32> = wave.iter().map(|x| x * 0.02).collect();
        let back = decode(&encode(&quiet));
        assert!(level(&back) > level(&quiet) * 0.7);
    }

    #[test]
    fn resampling_keeps_duration_and_pitch() {
        for rate in [44_100u32, 48_000, 16_000, 96_000] {
            // Une seconde de son a 440 Hz, livree par petits paquets comme le fait la carte son.
            let input: Vec<f32> = (0..rate).map(|i| (i as f32 / rate as f32 * 440.0 * std::f32::consts::TAU).sin()).collect();
            let (mut state, mut out) = ((0, 0.0, 0), Vec::new());
            for chunk in input.chunks(441) {
                resample(chunk, rate, &mut state, &mut out);
            }
            assert!((out.len() as i32 - RATE as i32).abs() <= 1, "{rate} Hz : {} echantillons", out.len());
            // Meme hauteur : autant de passages par zero.
            let crossings = out.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
            assert!((crossings as i32 - 440).abs() <= 2, "{rate} Hz : {crossings} periodes");
            assert!(level(&out) > 0.6);
        }
    }

    #[test]
    fn range_is_a_sphere() {
        assert!(hears([0.0, 100.0, 0.0], [47.0, 100.0, 0.0]));
        assert!(!hears([0.0, 100.0, 0.0], [49.0, 100.0, 0.0]));
        assert!(!hears([0.0, 100.0, 0.0], [30.0, 140.0, 30.0]));
    }
}
