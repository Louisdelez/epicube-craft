//! Mesure du temps passe dans chaque etape d'une image.
//!
//! `mark("etape")` clot l'etape en cours et ouvre la suivante ; tant que la mesure n'est pas lancee
//! (`start`), un appel ne coute qu'un test. Sert au mode `EPICUBE_BENCH=<secondes>`.
use std::cell::RefCell;
use std::time::Instant;

#[derive(Default)]
struct Prof {
    on: bool,
    phase: Option<(&'static str, Instant)>,
    /// (etape, secondes cumulees), dans l'ordre de premiere apparition.
    total: Vec<(&'static str, f64)>,
}

thread_local! {
    static PROF: RefCell<Prof> = RefCell::new(Prof::default());
}

pub fn start() {
    PROF.with(|p| p.borrow_mut().on = true);
}

pub fn mark(name: &'static str) {
    PROF.with(|p| {
        let mut p = p.borrow_mut();
        if !p.on {
            return;
        }
        let now = Instant::now();
        if let Some((prev, since)) = p.phase.replace((name, now)) {
            let dt = (now - since).as_secs_f64();
            match p.total.iter_mut().find(|(n, _)| *n == prev) {
                Some(slot) => slot.1 += dt,
                None => p.total.push((prev, dt)),
            }
        }
    });
}

/// Compte rendu : cadence moyenne, pires images, et millisecondes par image de chaque etape.
pub fn report(frames: &mut [f32]) -> String {
    let n = frames.len().max(1);
    let sum: f32 = frames.iter().sum();
    frames.sort_by(|a, b| b.total_cmp(a));
    let worst = |part: usize| frames[..(n / part).max(1).min(frames.len())].iter().sum::<f32>() / (n / part).max(1) as f32;
    let mut out = format!(
        "{} images : {:.0} i/s en moyenne ({:.2} ms), 1 % les plus lentes {:.2} ms ({:.0} i/s), 0,1 % {:.2} ms, pire {:.2} ms\n",
        n,
        n as f32 / sum.max(1e-6),
        sum / n as f32 * 1000.0,
        worst(100) * 1000.0,
        1.0 / worst(100).max(1e-6),
        worst(1000) * 1000.0,
        frames.first().copied().unwrap_or(0.0) * 1000.0
    );
    PROF.with(|p| {
        for (name, secs) in &p.borrow().total {
            out += &format!("  {name:<12} {:.3} ms\n", secs * 1000.0 / n as f64);
        }
    });
    out
}
