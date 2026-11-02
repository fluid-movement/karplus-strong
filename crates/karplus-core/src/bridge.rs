//! The bridge: what all the strings share (K19–K21).
//!
//! Strings meet at the bridge and, through it, the body of the instrument.
//! Neither belongs to one string, so neither lives in a [`crate::Voice`]: a
//! polyphonic shell owns one [`Bridge`], sums its voices into it, and plays
//! what comes out.
//!
//! ```text
//!   voices ─► sum ─┬──────────────────────────────────────────────┐
//!                  │                                              ▼
//!                  ├─► body: 12 resonant modes ──► × mix ──────► (+) ─► soft knee ─► out
//!                  │                                              ▲
//!                  └─► sympathetic strings: 8 undriven lines ─────┘
//! ```
//!
//! Both are linear and stable by construction, so whatever the strings do,
//! the bridge is finite; the soft knee at the end keeps it bounded.

use crate::delay::DelayLine;
use crate::filter::{one_pole_response, LoopFilter};
use crate::math::{cos_turns, exp, flush_denormal, soft_knee, sqrt, unit};
use crate::params::brightness_ratio;
use crate::ramp::Ramp;
use fathom_core::{clamp, exp2, floor, sin_turns, ZERO_VOLT_HZ};

/// `log2(1000)`: sixty decibels, as a power of two.
const LOG2_1000: f32 = 9.965_784;

/// Resonant modes in the body.
pub const BODY_MODES: usize = 12;

/// Strings that ring in sympathy.
pub const SYMPATHETIC_STRINGS: usize = 8;

/// Length of each sympathetic string's line.
pub const SYMPATHETIC_LEN: usize = 8192;

/// How long the knobs take to settle, in seconds. A body or a tuning that
/// stepped would click; this one is slow enough to hear as the instrument
/// being re-shaped.
const SETTLE_S: f32 = 0.03;

/// Slower, for the root: a new note moves the strings that follow it with a
/// glide rather than a jump.
const ROOT_SETTLE_S: f32 = 0.06;

/// Body mode frequencies as `log2` of their ratio to the first, for each
/// shape the SHAPE knob passes through.
/// Body shapes the SHAPE knob passes through.
pub const SHAPES: usize = 4;
const SHAPE_TABLE: [[f32; BODY_MODES]; SHAPES] = [
    // harmonic: a string's own series
    [
        0.0000,
        1.0000,
        1.5850,
        2.0000,
        2.3219,
        2.5850,
        2.8074,
        3.0000,
        3.1699,
        core::f32::consts::LOG2_10,
        3.4594,
        3.5850,
    ],
    // bar: a free bar, cos·cosh = 1
    [
        0.0000, 1.4631, 2.4340, 3.1591, 3.7381, 4.2202, 4.6331, 4.9942, 5.3151, 5.6039, 5.8664,
        6.1070,
    ],
    // membrane: a circular drum, Bessel zeros j(m,n)/j(0,1)
    [
        0.0000, 0.6717, 1.0949, 1.1985, 1.4076, 1.5445, 1.6576, 1.8074, 1.8476, 1.8667, 2.0211,
        2.0468,
    ],
    // plate: a simply supported rectangle, aspect 1.4
    [
        0.0000, 1.0101, 1.5782, 1.8887, 2.0000, 2.5082, 2.6012, 2.6547, 2.8701, 3.0097, 3.1699,
        3.1871,
    ],
];

/// How much the body hears of what strikes it.
const BODY_GAIN: f32 = 0.5;

/// Body mode levels. The lower modes carry the sound.
const MODE_LEVEL: f32 = 0.35;
const MODE_FALLOFF: f32 = 0.85;

/// Fixed-Hz body: the first mode with SIZE fully down, in Hz.
const BODY_TOP_HZ: f32 = 800.0;
/// Octaves SIZE spans.
const BODY_SIZE_OCTAVES: f32 = 5.0;
/// Tracked body: octaves above and below the note SIZE spans.
const BODY_TRACK_OCTAVES: f32 = 2.0;
/// The shortest ring a mode is given, in seconds, and the longest.
const MODE_T60_MIN: f32 = 0.004;
const MODE_T60_MAX: f32 = 30.0;

/// Sensitivity of a sympathetic string to the bridge. A matching sustained
/// input builds up to this multiple of itself at LEVEL fully up.
const SYMPATHETIC_SENSITIVITY: f32 = 6.0;
/// Most gain a sympathetic loop may have.
const SYMPATHETIC_MAX_GAIN: f32 = 0.999_9;
/// The fixed C3 the sympathetic strings sit on when they do not follow.
const FIXED_ROOT_VOLTS: f32 = -1.0;
/// Octaves ROOT spans: a quarter of the knob up and down is an octave.
const ROOT_SPAN_OCTAVES: f32 = 4.0;
/// Shortest period a sympathetic string will play, in samples.
const SYMPATHETIC_MIN_PERIOD: f32 = 8.0;

/// Tunings the sympathetic strings can be put in.
pub const SCENES: usize = 6;
const SCENE_TABLE: [[f32; SYMPATHETIC_STRINGS]; SCENES] = [
    // octaves and fifths
    [
        -1.0000, 0.0000, 0.5850, 1.0000, 1.5850, 2.0000, 2.5850, 3.0000,
    ],
    // major, just
    [
        0.0000, 0.3219, 0.5850, 1.0000, 1.3219, 1.5850, 2.0000, 2.3219,
    ],
    // minor seventh, just
    [
        0.0000, 0.2630, 0.5850, 0.8480, 1.0000, 1.2630, 1.5850, 2.0000,
    ],
    // raga, Bhairav: Sa to Sa'
    [
        0.0000, 0.0931, 0.3219, 0.4150, 0.5850, 0.6781, 0.9069, 1.0000,
    ],
    // tanpura: Pa Sa Sa Sa'
    [
        -0.4150, 0.0000, 0.0000, 1.0000, 0.5850, 1.5850, -1.0000, 2.0000,
    ],
    // cluster, inharmonic
    [
        0.0000, 0.2510, 0.5160, 0.8237, 1.0909, 1.3840, 1.6645, 1.9411,
    ],
];

/// What the bridge is told every block. Knob positions are `0..1`, any CV
/// already added by the shell. Defaults leave the bridge as a plain mixer:
/// no body, no sympathetic strings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BridgeParams {
    /// Pitch of the lead voice, in volts. What the tracked body and the
    /// following sympathetic strings sit on.
    pub root_volts: f32,
    /// Body shape: harmonic, bar, membrane, plate.
    pub body_shape: f32,
    /// Body size: low modes at one end, high at the other.
    pub body_size: f32,
    /// Body material: wood at one end, glass at the other.
    pub body_material: f32,
    /// Dry against body.
    pub body_mix: f32,
    /// Whether the body's modes follow the played note.
    pub body_track: bool,
    /// Which tuning the sympathetic strings sit in; the knob passes through
    /// them in turn and morphs between.
    pub scene: f32,
    /// The root of the sympathetic tuning, an offset in octaves around the
    /// followed note or the fixed one.
    pub root: f32,
    /// How strongly the sympathetic strings listen.
    pub level: f32,
    /// How long they ring.
    pub decay: f32,
    /// Whether the root follows the played note or stays put.
    pub follow: bool,
}

impl Default for BridgeParams {
    fn default() -> Self {
        Self {
            root_volts: 0.0,
            body_shape: 0.0,
            body_size: 0.5,
            body_material: 0.5,
            body_mix: 0.0,
            body_track: false,
            scene: 0.0,
            root: 0.5,
            level: 0.0,
            decay: 0.6,
            follow: true,
        }
    }
}

/// One mode of the body: a two-pole resonator `g·x + a1·y₁ + a2·y₂`.
#[derive(Debug, Clone, Copy, Default)]
struct Mode {
    y1: f32,
    y2: f32,
    a1: f32,
    a2: f32,
    gain: f32,
}

/// One string that is never plucked, only listened to.
#[derive(Debug, Clone)]
struct Sympathetic {
    line: DelayLine<SYMPATHETIC_LEN>,
    lowpass: LoopFilter,
    delay: Ramp,
    gain: Ramp,
    input: Ramp,
}

impl Sympathetic {
    const fn new() -> Self {
        Self {
            line: DelayLine::new(),
            lowpass: LoopFilter::open(),
            delay: Ramp::new(),
            gain: Ramp::new(),
            input: Ramp::new(),
        }
    }
}

/// The shared body and sympathetic strings.
#[derive(Debug, Clone)]
pub struct Bridge {
    sample_rate: f32,
    modes: [Mode; BODY_MODES],
    strings: [Sympathetic; SYMPATHETIC_STRINGS],
    primed: bool,
    // Smoothed copies of the knobs.
    shape: f32,
    size: f32,
    material: f32,
    played_volts: f32,
    sympathetic_root: f32,
}

impl Bridge {
    /// A silent bridge at a sample rate, in Hz.
    pub fn new(sample_rate: f32) -> Self {
        let rate = if sample_rate.is_finite() && sample_rate >= 1_000.0 {
            sample_rate
        } else {
            48_000.0
        };
        Self {
            sample_rate: rate,
            modes: [Mode {
                y1: 0.0,
                y2: 0.0,
                a1: 0.0,
                a2: 0.0,
                gain: 0.0,
            }; BODY_MODES],
            strings: [const { Sympathetic::new() }; SYMPATHETIC_STRINGS],
            primed: false,
            shape: 0.0,
            size: 0.5,
            material: 0.5,
            played_volts: 0.0,
            sympathetic_root: FIXED_ROOT_VOLTS,
        }
    }

    /// Silent, as new.
    pub fn reset(&mut self) {
        *self = Self::new(self.sample_rate);
    }

    /// The sample rate, in Hz.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Render a block. `input` is the sum of the strings; missing samples
    /// are zero. The output is bounded by one whatever goes in.
    pub fn process(&mut self, p: &BridgeParams, input: &[f32], out: &mut [f32]) {
        let len = out.len();
        if len == 0 {
            return;
        }
        self.settle(p, len);
        self.tune_body(p);
        self.tune_strings(p, len);

        let mix = unit(p.body_mix);
        let dry = 1.0 - mix;
        let level_gain = unit(p.level);
        let inputs = input.iter().copied().chain(core::iter::repeat(0.0));
        for (sample, x) in out.iter_mut().zip(inputs) {
            let x = if x.is_finite() { x } else { 0.0 };

            let mut body = 0.0;
            let mut weight = MODE_LEVEL;
            for mode in &mut self.modes {
                let y = flush_denormal(mode.gain * x + mode.a1 * mode.y1 + mode.a2 * mode.y2);
                mode.y2 = mode.y1;
                mode.y1 = y;
                body += weight * y;
                weight *= MODE_FALLOFF;
            }

            let mut strings = 0.0;
            for string in &mut self.strings {
                let heard = string.line.read(string.delay.next());
                let y = string.lowpass.process(heard);
                let drive = string.input.next() * level_gain * x;
                let now = flush_denormal(string.gain.next() * y + drive);
                string.line.write(now);
                strings += now;
            }

            *sample = soft_knee(dry * x + mix * body + strings);
        }
    }

    /// Move the smoothed knobs towards where they were put.
    fn settle(&mut self, p: &BridgeParams, len: usize) {
        let blend = |rate: f32| 1.0 - exp(-(len as f32) / (rate * self.sample_rate));
        let (k, kr) = (blend(SETTLE_S), blend(ROOT_SETTLE_S));
        let shape = unit(p.body_shape);
        let size = unit(p.body_size);
        let material = unit(p.body_material);
        let volts = clamp(p.root_volts, -10.0, 10.0);
        if self.primed {
            self.shape += k * (shape - self.shape);
            self.size += k * (size - self.size);
            self.material += k * (material - self.material);
            self.played_volts += kr * (volts - self.played_volts);
        } else {
            self.shape = shape;
            self.size = size;
            self.material = material;
            self.played_volts = volts;
        }
        let anchor = if p.follow {
            self.played_volts
        } else {
            FIXED_ROOT_VOLTS
        };
        let target = anchor + (unit(p.root) - 0.5) * ROOT_SPAN_OCTAVES;
        if self.primed {
            self.sympathetic_root += kr * (target - self.sympathetic_root);
        } else {
            self.sympathetic_root = target;
        }
    }

    /// The body's coefficients for this block.
    fn tune_body(&mut self, p: &BridgeParams) {
        let rate = self.sample_rate;
        let base_hz = if p.body_track {
            ZERO_VOLT_HZ * exp2(self.played_volts + BODY_TRACK_OCTAVES * (1.0 - 2.0 * self.size))
        } else {
            BODY_TOP_HZ * exp2(-BODY_SIZE_OCTAVES * self.size)
        };
        let position = self.shape * (SHAPES - 1) as f32;
        let lower = clamp(floor(position), 0.0, (SHAPES - 2) as f32);
        let t = clamp(position - lower, 0.0, 1.0);
        let a = lower as usize;
        // Material: the first mode's ring, and how much faster the high
        // ones die. Wood is short and falls off with pitch; glass rings.
        let m = self.material;
        let first_t60 = 0.03 * exp2(7.0 * m);
        let falloff = 1.0 - 0.8 * m;
        for (i, mode) in self.modes.iter_mut().enumerate() {
            let at = |row: usize| {
                SHAPE_TABLE
                    .get(row)
                    .and_then(|r| r.get(i))
                    .copied()
                    .unwrap_or(0.0)
            };
            let l = at(a) + t * (at(a + 1) - at(a));
            let hz = clamp(base_hz * exp2(l), 1.0, 0.45 * rate);
            let t60 = clamp(first_t60 * exp2(-falloff * l), MODE_T60_MIN, MODE_T60_MAX);
            let r = clamp(exp2(-LOG2_1000 / (t60 * rate)), 0.0, 0.999_999);
            let turns = hz / rate;
            mode.a1 = 2.0 * r * cos_turns(turns);
            mode.a2 = -r * r;
            // Between unit gain at the mode's own frequency (a gain of
            // `2·(1−r)·sin θ`), which leaves a long ring almost silent when
            // a pluck's broadband burst strikes it, and equal power for
            // white noise (`√(1−r)` more), which makes a long ring roar
            // when a partial sits on it. Three quarters of the way to the
            // second.
            let bandwidth = 1.0 - r;
            mode.gain = BODY_GAIN * 2.0 * sin_turns(turns) * sqrt(bandwidth);
        }
    }

    /// The sympathetic strings' delays, gains and sensitivities.
    fn tune_strings(&mut self, p: &BridgeParams, len: usize) {
        let rate = self.sample_rate;
        let position = unit(p.scene) * (SCENES - 1) as f32;
        let lower = clamp(floor(position), 0.0, (SCENES - 2) as f32);
        // A flat stretch round each scene, so that it is easy to land on, and
        // a smooth step between: the strings glide while they ring.
        let t = smoothstep(clamp((position - lower - 0.2) / 0.6, 0.0, 1.0));
        let a = lower as usize;
        let ring = 0.5 * exp2(5.322 * unit(p.decay)); // 0.5 s to 20 s
        let primed = self.primed;
        for (i, string) in self.strings.iter_mut().enumerate() {
            let at = |row: usize| {
                SCENE_TABLE
                    .get(row)
                    .and_then(|r| r.get(i))
                    .copied()
                    .unwrap_or(0.0)
            };
            let l = at(a) + t * (at(a + 1) - at(a));
            let hz = ZERO_VOLT_HZ * exp2(clamp(self.sympathetic_root + l, -10.0, 10.0));
            let period = clamp(
                rate / hz,
                SYMPATHETIC_MIN_PERIOD,
                DelayLine::<SYMPATHETIC_LEN>::MAX_DELAY - 8.0,
            );
            let turns = 1.0 / period;
            let coefficient =
                LoopFilter::coefficient_for(brightness_ratio(0.35) * rate * turns, rate);
            string.lowpass.set_coefficient(coefficient);
            let response = one_pole_response(coefficient, turns);
            let delay = period - response.delay;
            let wanted = exp2(-LOG2_1000 * period / (ring * rate));
            let gain = clamp(
                wanted
                    / if response.gain > 0.0 {
                        response.gain
                    } else {
                        1.0
                    },
                0.0,
                SYMPATHETIC_MAX_GAIN,
            );
            // What goes in is scaled so that a sustained input at the
            // string's own pitch builds up to a fixed multiple of itself,
            // however long it rings.
            let input = (1.0 - gain) * SYMPATHETIC_SENSITIVITY;
            if primed {
                string.delay.aim(delay, len);
                string.gain.aim(gain, len);
                string.input.aim(input, len);
            } else {
                string.delay.jump(delay);
                string.gain.jump(gain);
                string.input.jump(input);
            }
        }
        self.primed = true;
    }

    /// The smallest nonzero magnitude held anywhere, for the subnormal test.
    pub fn smallest_nonzero_state(&self) -> f32 {
        let mut smallest = 0.0f32;
        let mut consider = |x: f32| {
            let m = if x < 0.0 { -x } else { x };
            if m > 0.0 && (smallest == 0.0 || m < smallest) {
                smallest = m;
            }
        };
        for mode in &self.modes {
            consider(mode.y1);
            consider(mode.y2);
        }
        for string in &self.strings {
            string
                .line
                .contents()
                .iter()
                .copied()
                .for_each(&mut consider);
            consider(string.lowpass.state());
        }
        smallest
    }
}

fn smoothstep(x: f32) -> f32 {
    x * x * (3.0 - 2.0 * x)
}
