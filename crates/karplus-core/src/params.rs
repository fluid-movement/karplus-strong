//! What the voice is told every block, and what the knobs mean.
//!
//! Every control arrives as a knob position, `0.0..=1.0`, with any CV already
//! added by the shell. The mapping from position to seconds or hertz lives
//! here, once, so that the panel, the scenarios and the firmware all mean the
//! same thing by "decay at 0.6". The laws are in `docs/parameters.md`.

use crate::math::unit;
use fathom_core::exp2;

/// The burst that plucks the string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExciterKind {
    /// White noise: the classic Karplus-Strong pluck.
    #[default]
    Noise,
    /// Sparse random impulses: a scrape, a brush, a bowed-on-gravel pluck.
    Dust,
    /// A sine at a harmonic of the note: a soft, pitched mallet.
    Sine,
}

/// Per-block parameters. Defaults are the panel's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    /// Pitch in volts, one volt per octave. 0 V is C4.
    pub pitch_volts: f32,
    /// Decay time knob. See [`decay_seconds`].
    pub decay: f32,
    /// Loop lowpass knob. See [`brightness_hz`].
    pub brightness: f32,
    /// Pluck position along the string, 0 at the bridge, 1 at the middle.
    pub pick: f32,
    /// Exciter lowpass knob; fully clockwise is no filter at all.
    pub tone: f32,
    /// Burst length knob. See [`burst_periods`].
    pub length: f32,
    /// Saturation inside the loop, 0 clean to 1 driven.
    pub drive: f32,
    /// Damping when the gate falls. Zero lets the string ring.
    pub damp: f32,
    /// Per-pluck variation in tuning, brightness and pick position.
    pub humanize: f32,
    /// Decay key tracking: 0 low notes ring longer, 1 every note rings the
    /// same time.
    pub key_track: f32,
    /// How far a soft velocity darkens the note.
    pub velocity_brightness: f32,
    /// How far a soft velocity shortens the note.
    pub velocity_decay: f32,
    /// How far a soft velocity shortens the burst.
    pub velocity_length: f32,
    /// The burst.
    pub exciter: ExciterKind,
    /// Which harmonic the sine exciter sits on, 1..=8.
    pub harmonic: u32,
    /// Velocity input, in volts, 0..10. Sampled when the gate rises.
    pub velocity_volts: f32,
    /// Whether anything is patched into the velocity input. Unpatched is
    /// full velocity, so that a bare gate plays at full strength.
    pub velocity_patched: bool,
    /// A gate held from the panel: the PLUCK button. Combined with the gate
    /// input, so either can play the string.
    pub manual_gate: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            pitch_volts: 0.0,
            decay: DEFAULT_DECAY,
            brightness: DEFAULT_BRIGHTNESS,
            pick: 0.4,
            tone: 1.0,
            length: DEFAULT_LENGTH,
            drive: 0.0,
            damp: 0.0,
            humanize: 0.0,
            key_track: 0.0,
            velocity_brightness: 0.0,
            velocity_decay: 0.0,
            velocity_length: 0.0,
            exciter: ExciterKind::Noise,
            harmonic: 1,
            velocity_volts: 10.0,
            velocity_patched: false,
            manual_gate: false,
        }
    }
}

/// Knob position for a cutoff of 26× the note: 6.8 kHz at C4, near the
/// JUCE version's default of 6.1 kHz.
pub const DEFAULT_BRIGHTNESS: f32 = 0.45;

/// Shortest decay, in seconds.
pub const DECAY_MIN_S: f32 = 0.1;
/// Longest decay, in seconds.
pub const DECAY_MAX_S: f32 = 40.0;
/// `log2(DECAY_MAX_S / DECAY_MIN_S)`.
const DECAY_OCTAVES: f32 = 8.643_856;
/// Knob position for four seconds, the JUCE version's default.
pub const DEFAULT_DECAY: f32 = 0.615_68;

/// Time for an A4 to fall by 60 dB, in seconds. Exponential: equal turns of
/// the knob are equal ratios of time, 0.1 s to 40 s, 2 s at the centre.
pub fn decay_seconds(knob: f32) -> f32 {
    DECAY_MIN_S * exp2(unit(knob) * DECAY_OCTAVES)
}

/// Loop lowpass cutoff as a multiple of the note's frequency: 4× to 256×,
/// exponentially.
///
/// Relative to the note rather than absolute, as Mutable Instruments' string
/// models do it. The JUCE version used an absolute cutoff, and the loop
/// filter's loss at the fundamental then grew with pitch until it was more
/// than any loop gain could make up: a dark setting played a high note as a
/// click. Tracking the note makes BRIGHT mean the same timbre everywhere, and
/// at its darkest it is a short, muted pluck rather than nothing.
pub fn brightness_ratio(knob: f32) -> f32 {
    exp2(2.0 + 6.0 * unit(knob))
}

/// Exciter lowpass cutoff as a multiple of the note's frequency, 1× to
/// 256×, or `None` for no filter. The last thousandth of travel removes the
/// filter entirely, so the default pluck is the raw burst.
pub fn tone_ratio(knob: f32) -> Option<f32> {
    let k = unit(knob);
    if k >= 0.999 {
        None
    } else {
        Some(exp2(8.0 * k))
    }
}

/// Highest cutoff either filter is given, as a fraction of the sample rate.
pub const MAX_CUTOFF: f32 = 0.45;

/// Knob position for a burst of one period.
pub const DEFAULT_LENGTH: f32 = 4.0 / 6.0;

/// Burst length in periods of the note: a sixteenth to four, exponentially.
///
/// In periods rather than samples or milliseconds so that the pluck has the
/// same character at every pitch. The JUCE version counted samples, which
/// made the same setting a long, tonal burst on a high note and a click on a
/// low one.
pub fn burst_periods(knob: f32) -> f32 {
    exp2(-4.0 + 6.0 * unit(knob))
}

/// Pluck position as a fraction of the string, 0..0.5.
pub fn pick_fraction(knob: f32) -> f32 {
    0.5 * unit(knob)
}

/// The time the string takes to fall 60 dB once the gate has dropped, or
/// `None` if it is left to ring. 3 s down to 20 ms.
pub fn damp_seconds(knob: f32) -> Option<f32> {
    let k = unit(knob);
    if k <= 0.0 {
        None
    } else {
        // 3 · (0.02 / 3)^k
        Some(3.0 * exp2(-7.228_819 * k))
    }
}

/// Gain into the drive stage, 1 to [`DRIVE_MAX_GAIN`].
pub const DRIVE_MAX_GAIN: f32 = 8.0;
