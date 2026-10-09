//! Render a scenario, measure it, check it.

use std::collections::BTreeMap;
use std::path::Path;

use fathom_harness::scenario::Bound;
use fathom_harness::Result;

use crate::metrics::measure;
use crate::render::{render_in_blocks, volts_to_hz};
use crate::scenario::{AudioIn, Scenario};

/// One assertion's outcome.
#[derive(Debug, Clone)]
pub struct Check {
    /// The metric.
    pub metric: String,
    /// The bound it had to meet.
    pub bound: Bound,
    /// What was measured, if anything could be.
    pub measured: Option<f64>,
}

impl Check {
    /// Whether it held.
    pub fn passed(&self) -> bool {
        self.measured.is_some_and(|v| self.bound.satisfied_by(v))
    }
}

/// One scenario's outcome.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// The scenario's name.
    pub name: String,
    /// Every metric measured, asserted on or not.
    pub metrics: BTreeMap<&'static str, f64>,
    /// The assertions.
    pub checks: Vec<Check>,
    /// The render.
    pub samples: Vec<f32>,
}

impl Outcome {
    /// Whether every assertion held.
    pub fn passed(&self) -> bool {
        self.checks.iter().all(Check::passed)
    }
}

/// Seeded noise for the audio input, independent of the voice's seed.
fn noise(len: usize, level: f64) -> Vec<f32> {
    let mut state = 0x2545_f491u32;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (level * ((state >> 8) as f64 / 8_388_608.0 - 1.0)) as f32
        })
        .collect()
}

/// Render and check one scenario.
pub fn run(s: &Scenario) -> Outcome {
    let sr = s.sample_rate as f32;
    let len = s.duration.samples(s.sample_rate);
    let audio = s.audio_in.map(|AudioIn { noise: level }| noise(len, level));
    let plucks: Vec<_> = s.plucks().iter().map(|p| p.pluck()).collect();
    let samples = render_in_blocks(
        s.block_size,
        &plucks,
        s.duration.0,
        sr,
        s.seed,
        |t| s.params_at(t),
        audio.as_deref(),
    );

    let from = s.analysis.from.0;
    let to = s.analysis.to.map(|t| t.0).unwrap_or(s.duration.0);
    let span = crate::render::span(&samples, sr, from, to);
    let expected = volts_to_hz(s.pitch_volts_at(0.5 * (from + to)) as f32);
    let metrics = measure(&samples, span, s.sample_rate, expected);
    let checks = s
        .assert
        .iter()
        .map(|(metric, bound)| Check {
            metric: metric.clone(),
            bound: *bound,
            measured: metrics.get(metric.as_str()).copied(),
        })
        .collect();
    Outcome {
        name: s.name.clone(),
        metrics,
        checks,
        samples,
    }
}

/// Load every scenario in a directory, optionally filtered by name.
pub fn load(dir: &Path, filter: Option<&str>) -> Result<Vec<Scenario>> {
    Ok(Scenario::load_dir(dir)?
        .into_iter()
        .map(|(_, s)| s)
        .filter(|s| filter.map(|f| s.name.contains(f)).unwrap_or(true))
        .collect())
}
