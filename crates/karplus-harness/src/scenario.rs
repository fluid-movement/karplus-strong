//! A scenario: a requirement written as data.
//!
//! The format is Fathom's, cut to what a plucked string needs: what to
//! render, when to pluck, what must be true of the result. Everything is
//! checked at load time — unknown fields, unknown parameter names, unknown
//! metrics and an empty `assert` block are all errors — because a scenario
//! that silently asserts nothing is worse than none.
//!
//! Durations, note names, automation curves and bounds are Fathom's own
//! types, used unchanged from `fathom-harness`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fathom_harness::pitch::Pitch;
use fathom_harness::scenario::{Automation, Bound, Seconds};
use fathom_harness::{Error, Result};
use karplus_core::{ExciterKind, Params};
use serde::Deserialize;

use crate::metrics::KNOWN_METRICS;
use crate::render::Pluck;

/// Every parameter a scenario may set or automate.
pub const KNOWN_PARAMETERS: &[&str] = &[
    "pitch_volts",
    "decay",
    "brightness",
    "pick",
    "tone",
    "length",
    "drive",
    "damp",
    "humanize",
    "key_track",
    "velocity_brightness",
    "velocity_decay",
    "velocity_length",
];

/// The burst, as written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExciterSpec {
    /// White noise.
    #[default]
    Noise,
    /// Sparse impulses.
    Dust,
    /// A sine on a harmonic.
    Sine,
}

impl From<ExciterSpec> for ExciterKind {
    fn from(spec: ExciterSpec) -> Self {
        match spec {
            ExciterSpec::Noise => ExciterKind::Noise,
            ExciterSpec::Dust => ExciterKind::Dust,
            ExciterSpec::Sine => ExciterKind::Sine,
        }
    }
}

/// One pluck: a time, or a time with a hold and a velocity.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum PluckSpec {
    /// `0.5s`: a one-millisecond trigger.
    At(Seconds),
    /// `{ at: 0.5s, held: 1s, velocity: 6 }`.
    Full {
        /// When the gate rises.
        at: Seconds,
        /// How long it stays up. Default one millisecond.
        #[serde(default)]
        held: Option<Seconds>,
        /// Velocity in volts, 0..10, from this pluck on. Default unpatched.
        #[serde(default)]
        velocity: Option<f64>,
    },
}

impl PluckSpec {
    fn at(&self) -> f64 {
        match self {
            PluckSpec::At(s) | PluckSpec::Full { at: s, .. } => s.0,
        }
    }

    /// The gate this pluck makes.
    pub fn pluck(&self) -> Pluck {
        match self {
            PluckSpec::At(at) => Pluck::trigger(at.0),
            PluckSpec::Full { at, held, .. } => Pluck {
                at_s: at.0,
                held_s: held.map(|h| h.0).unwrap_or(0.001),
            },
        }
    }

    fn velocity(&self) -> Option<f64> {
        match self {
            PluckSpec::Full { velocity, .. } => *velocity,
            PluckSpec::At(_) => None,
        }
    }
}

/// Something fed to the audio input.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioIn {
    /// Seeded white noise at this peak level; 1.0 is full scale, ±5 V.
    pub noise: f64,
}

/// Which part of the render is measured.
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Analysis {
    /// Start of the measured span. Default the start.
    #[serde(default)]
    pub from: Seconds,
    /// End of the measured span. Default the end.
    #[serde(default)]
    pub to: Option<Seconds>,
}

fn default_sample_rate() -> f64 {
    48_000.0
}

fn default_block_size() -> usize {
    crate::render::BLOCK
}

fn default_seed() -> u32 {
    1
}

/// One requirement, as written by the author.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// Unique name.
    pub name: String,
    /// Why it exists.
    #[serde(default)]
    pub description: String,
    /// Length of the render.
    pub duration: Seconds,
    /// Sample rate, Hz.
    #[serde(default = "default_sample_rate")]
    pub sample_rate: f64,
    /// Samples per call to the core.
    #[serde(default = "default_block_size")]
    pub block_size: usize,
    /// The voice's seed.
    #[serde(default = "default_seed")]
    pub seed: u32,
    /// The note. Default C4, 0 V. Exclusive with `params.pitch_volts`.
    #[serde(default)]
    pub pitch: Option<Pitch>,
    /// The burst.
    #[serde(default)]
    pub exciter: ExciterSpec,
    /// The sine exciter's harmonic.
    #[serde(default)]
    pub harmonic: Option<u32>,
    /// Values held for the whole render.
    #[serde(default)]
    pub params: BTreeMap<String, f64>,
    /// Values that move.
    #[serde(default)]
    pub automation: BTreeMap<String, Automation>,
    /// When the string is plucked. Default once, at the start.
    #[serde(default)]
    pub plucks: Option<Vec<PluckSpec>>,
    /// The audio input.
    #[serde(default)]
    pub audio_in: Option<AudioIn>,
    /// Which part is measured.
    #[serde(default)]
    pub analysis: Analysis,
    /// What must be true. Never empty.
    pub assert: BTreeMap<String, Bound>,
}

impl Scenario {
    /// Parse and validate.
    pub fn from_yaml(text: &str) -> Result<Self> {
        let scenario: Scenario =
            serde_yaml::from_str(text).map_err(|e| Error::new(e.to_string()))?;
        scenario.validate()?;
        Ok(scenario)
    }

    /// Load one file.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Self::from_yaml(&text).map_err(|e| e.in_file(path))
    }

    /// Every `.yaml` in a directory, sorted by path.
    pub fn load_dir(dir: &Path) -> Result<Vec<(PathBuf, Scenario)>> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
            .collect();
        paths.sort();
        let mut out = Vec::new();
        let mut names = std::collections::BTreeSet::new();
        for path in paths {
            let s = Self::load(&path)?;
            if !names.insert(s.name.clone()) {
                return Err(Error::new(format!("two scenarios are called `{}`", s.name)));
            }
            out.push((path, s));
        }
        Ok(out)
    }

    fn validate(&self) -> Result<()> {
        for name in self.params.keys().chain(self.automation.keys()) {
            if !KNOWN_PARAMETERS.contains(&name.as_str()) {
                return Err(Error::new(format!(
                    "unknown parameter `{name}`; known: {}",
                    KNOWN_PARAMETERS.join(", ")
                )));
            }
        }
        let pitch_set =
            self.params.contains_key("pitch_volts") || self.automation.contains_key("pitch_volts");
        if self.pitch.is_some() && pitch_set {
            return Err(Error::new("set `pitch` or `pitch_volts`, not both"));
        }
        if self.assert.is_empty() {
            return Err(Error::new(
                "`assert` is empty: a scenario must check something",
            ));
        }
        for metric in self.assert.keys() {
            if !KNOWN_METRICS.contains(&metric.as_str()) {
                return Err(Error::new(format!(
                    "unknown metric `{metric}`; known: {}",
                    KNOWN_METRICS.join(", ")
                )));
            }
        }
        if self.duration.0 <= 0.0 {
            return Err(Error::new("`duration` must be positive"));
        }
        if !(8_000.0..=384_000.0).contains(&self.sample_rate) {
            return Err(Error::new(
                "`sample_rate` must be between 8 kHz and 384 kHz",
            ));
        }
        if self.block_size == 0 || self.block_size > 4096 {
            return Err(Error::new("`block_size` must be between 1 and 4096"));
        }
        if let Some(h) = self.harmonic {
            if !(1..=8).contains(&h) {
                return Err(Error::new("`harmonic` must be 1 to 8"));
            }
        }
        for p in self.plucks() {
            if let Some(v) = p.velocity() {
                if !(0.0..=10.0).contains(&v) {
                    return Err(Error::new("a pluck's `velocity` is volts, 0 to 10"));
                }
            }
        }
        Ok(())
    }

    /// The plucks, defaulting to one at the start.
    pub fn plucks(&self) -> Vec<PluckSpec> {
        self.plucks
            .clone()
            .unwrap_or_else(|| vec![PluckSpec::At(Seconds(0.0))])
    }

    /// The pitch in volts at a time.
    pub fn pitch_volts_at(&self, t: f64) -> f64 {
        if let Some(a) = self.automation.get("pitch_volts") {
            return a.value_at(t, self.duration.0);
        }
        if let Some(v) = self.params.get("pitch_volts") {
            return *v;
        }
        self.pitch.unwrap_or(Pitch::C4).volts(440.0)
    }

    /// The parameters at a time.
    pub fn params_at(&self, t: f64) -> Params {
        let mut p = Params {
            exciter: self.exciter.into(),
            harmonic: self.harmonic.unwrap_or(1),
            ..Params::default()
        };
        let value = |name: &str, fallback: f32| -> f32 {
            if let Some(a) = self.automation.get(name) {
                a.value_at(t, self.duration.0) as f32
            } else if let Some(v) = self.params.get(name) {
                *v as f32
            } else {
                fallback
            }
        };
        p.pitch_volts = self.pitch_volts_at(t) as f32;
        p.decay = value("decay", p.decay);
        p.brightness = value("brightness", p.brightness);
        p.pick = value("pick", p.pick);
        p.tone = value("tone", p.tone);
        p.length = value("length", p.length);
        p.drive = value("drive", p.drive);
        p.damp = value("damp", p.damp);
        p.humanize = value("humanize", p.humanize);
        p.key_track = value("key_track", p.key_track);
        p.velocity_brightness = value("velocity_brightness", p.velocity_brightness);
        p.velocity_decay = value("velocity_decay", p.velocity_decay);
        p.velocity_length = value("velocity_length", p.velocity_length);

        // Velocity is a voltage that each pluck may set, and holds until the
        // next one changes it.
        let plucks = self.plucks();
        if plucks.iter().any(|p| p.velocity().is_some()) {
            p.velocity_patched = true;
            p.velocity_volts = 10.0;
            // A whole block ahead: the core reads velocity in the block the
            // gate rises in, and `t` is that block's first sample.
            let block = self.block_size as f64 / self.sample_rate;
            for pluck in plucks.iter().filter(|q| q.at() < t + block) {
                if let Some(v) = pluck.velocity() {
                    p.velocity_volts = v as f32;
                }
            }
        }
        p
    }
}
