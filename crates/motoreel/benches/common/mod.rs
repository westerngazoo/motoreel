//! The frame-time harness the R-0022 benches share: std only, no
//! statistics crate (SPEC-0022 §2.6 says why).
//!
//! Every frame of a reel's timeline is timed `rounds` times on one thread,
//! after one untimed warm-up round; a frame's time is the fastest of its
//! rounds (other load on the machine only ever adds time, so the fastest
//! round is the best estimate of the frame's own cost), and a phase's
//! figures are the median, the 95th percentile and the maximum over its
//! frames. The warm-up round also folds every
//! primitive into a fingerprint (FNV-1a over the `Debug` text, which
//! prints each `f64` exactly), so a run before a change and a run after it
//! show at a glance whether the frames are the same, bit for bit.

use std::process::Command;
use std::time::Instant;

use motoreel::Prim2;

/// Timed rounds per frame: `MOTOREEL_BENCH_ROUNDS`, 5 by default.
pub fn rounds() -> usize {
    std::env::var("MOTOREEL_BENCH_ROUNDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n: &usize| n > 0)
        .unwrap_or(5)
}

/// Whether the section `name` runs: every section with no filter, else
/// those whose name contains one of the command line's words
/// (`cargo bench -p motoreel --bench anatomy -- toy`).
pub fn selected(name: &str) -> bool {
    let words: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with("--"))
        .collect();
    words.is_empty() || words.iter().any(|w| name.contains(w.as_str()))
}

/// The machine's load, as `uptime` prints it (empty where there is none):
/// a loaded machine slows every figure, so each run records it.
pub fn load() -> String {
    Command::new("uptime")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// FNV-1a, 64 bits: small, fixed and the same on every host.
pub struct Fingerprint(u64);

impl Fingerprint {
    pub fn new() -> Fingerprint {
        Fingerprint(0xcbf2_9ce4_8422_2325)
    }

    /// Folds `prims` in, exactly: `Debug` prints the shortest text that
    /// reads back to the same `f64`.
    pub fn add(&mut self, prims: &[Prim2]) {
        for byte in format!("{prims:?}").bytes() {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }

    pub fn hex(&self) -> String {
        format!("{:016x}", self.0)
    }
}

/// Per-frame minima (ms) of `rounds` timed runs of `frame(i, &input)`
/// for every `i` in `0..count`, where `input = prepare(i)` is built
/// untimed before each run, after a warm-up round whose output `warm`
/// sees.
pub fn time_frames<P, T>(
    count: usize,
    rounds: usize,
    mut prepare: impl FnMut(usize) -> P,
    mut frame: impl FnMut(usize, &P) -> T,
    mut warm: impl FnMut(usize, &T),
) -> Vec<f64> {
    for i in 0..count {
        let input = prepare(i);
        let out = frame(i, &input);
        warm(i, &out);
    }
    let mut runs = vec![Vec::with_capacity(rounds); count];
    for _ in 0..rounds {
        for (i, run) in runs.iter_mut().enumerate() {
            let input = prepare(i);
            let start = Instant::now();
            let out = frame(i, &input);
            run.push(start.elapsed().as_secs_f64() * 1e3);
            drop(out);
        }
    }
    runs.into_iter()
        .map(|r| r.into_iter().fold(f64::INFINITY, f64::min))
        .collect()
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

fn percentile(values: &mut [f64], p: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    let k = ((values.len() as f64 * p).ceil() as usize).clamp(1, values.len());
    values[k - 1]
}

/// Prints the header of the table [`row`] fills.
pub fn header(rounds: usize) {
    println!("# load at start: {}", load());
    println!("# ms per frame, the fastest of {rounds} rounds, one thread");
    println!(
        "{:<14} {:<10} {:<9} {:>6} {:>9} {:>9} {:>9}",
        "scene", "phase", "part", "frames", "median", "p95", "max"
    );
}

/// One table row: `ms` holds the per-frame times of one phase.
pub fn row(scene: &str, phase: &str, part: &str, ms: &[f64]) {
    if ms.is_empty() {
        return;
    }
    let mut v = ms.to_vec();
    println!(
        "{:<14} {:<10} {:<9} {:>6} {:>9.2} {:>9.2} {:>9.2}",
        scene,
        phase,
        part,
        v.len(),
        median(&mut v),
        percentile(&mut v, 0.95),
        v.iter().copied().fold(0.0, f64::max)
    );
}

/// The rows of every phase of one scene, in `phases` order, then all of
/// them: `phase_of[i]` names frame `i`'s phase.
pub fn rows(scene: &str, part: &str, phases: &[&str], phase_of: &[&str], ms: &[f64]) {
    for phase in phases {
        let of: Vec<f64> = ms
            .iter()
            .zip(phase_of)
            .filter(|(_, p)| *p == phase)
            .map(|(m, _)| *m)
            .collect();
        row(scene, phase, part, &of);
    }
    row(scene, "all", part, ms);
}
