//! The bridge: what all the strings share (K19–K25, refined in K27–K31).
//!
//! Strings meet at the bridge and, through it, the body of the instrument.
//! Neither belongs to one string, so neither lives in a [`crate::Voice`]: a
//! polyphonic shell owns one [`Bridge`], sums its voices into it, and plays
//! what comes out.
//!
//! ```text
//!   voices + external ─► sum ─┬──────────────────────────────────────────┐
//!                             │                                          ▼
//!                             ├─► body: 24 modes ─► pickup ─► × mix ───► (+) ─► soft knee ─► L, R
//!                             │                                          ▲
//!                             └─► sympathetic: 12 lines ─► pickup ───────┘
//! ```
//!
//! Both are linear and stable by construction, so whatever the strings do,
//! the bridge is finite; the soft knee at the end keeps it bounded.
//!
//! Several ideas here were learned from Mutable Instruments' Elements and
//! Rings (Émilie Gillet, MIT): a modal resonator whose pickup position weights
//! each mode instead of combing the signal, a second pickup that drifts slowly
//! to give a stereo image, sympathetic strings with detuned partners and a
//! soft glide when they are retuned, and state-variable filters for the modes.
//! Nothing is copied: the mode series are derived here, the laws and the
//! constants are this instrument's own, and `docs/decisions.md` (K27–K31) says
//! what was taken and what was not.

use crate::delay::DelayLine;
use crate::filter::{one_pole_response, LoopFilter};
use crate::math::{cos_turns, exp, flush_denormal, soft_knee, sqrt, unit};
use crate::params::brightness_ratio;
use crate::ramp::Ramp;
use fathom_core::{clamp, exp2, floor, sin_turns, ZERO_VOLT_HZ};

/// `log2(1000)`: sixty decibels, as a power of two.
const LOG2_1000: f32 = 9.965_784;

/// `ln(1000)`.
const LN_1000: f32 = 6.907_755;

/// Resonant modes in the body.
pub const BODY_MODES: usize = 24;

/// Strings the scenes tune.
pub const SYMPATHETIC_STRINGS: usize = 8;

/// Detuned partners of some of those strings, for the beating.
pub const SYMPATHETIC_PARTNERS: usize = 4;

/// Lines in the sympathetic bank: the strings and their partners.
pub const SYMPATHETIC_LINES: usize = SYMPATHETIC_STRINGS + SYMPATHETIC_PARTNERS;

/// Length of each sympathetic line.
pub const SYMPATHETIC_LEN: usize = 8192;

/// Which string each partner shadows, and by how many cents it is detuned. A
/// pair a cent or two apart beats slowly, which is most of what a real
/// sympathetic bank sounds like next to a bank of exact copies.
const PARTNERS: [(usize, f32); SYMPATHETIC_PARTNERS] = [(1, 1.2), (2, -0.9), (3, 1.6), (5, -1.4)];

/// How long the knobs take to settle, in seconds. State-variable modes can be
/// moved quickly without clicking, so this is short enough to feel like a
/// knob, long enough that a block-rate step is not heard.
const SETTLE_S: f32 = 0.012;

/// Slower, for the root: a new note moves the strings that follow it with a
/// glide, as if they were being retuned, not a jump.
const ROOT_SETTLE_S: f32 = 0.15;

/// Rate of the slow drift of the second pickup, in Hz.
const PICKUP_DRIFT_HZ: f32 = 0.55;

/// How far the second pickup sits from the first, and how far it drifts, in
/// fractions of the string.
const PICKUP_OFFSET: f32 = 0.08;
const PICKUP_DRIFT: f32 = 0.10;

/// Body mode frequencies as `log2` of their ratio to the first, for each
/// shape the SHAPE knob passes through.
/// Body shapes the SHAPE knob passes through.
pub const SHAPES: usize = 4;
const SHAPE_TABLE: [[f32; BODY_MODES]; SHAPES] = [
    // string: whole multiples
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
        3.7004,
        3.8074,
        3.9069,
        4.0000,
        4.0875,
        4.1699,
        4.2479,
        4.3219,
        4.3923,
        4.4594,
        4.5236,
        4.5850,
    ],
    // bar: a free bar, cos·cosh = 1
    [
        0.0000, 1.4629, 2.4340, 3.1591, 3.7382, 4.2202, 4.6331, 4.9942, 5.3151, 5.6039, 5.8664,
        6.1070, 6.3291, 6.5352, 6.7277, 6.9081, 7.0779, 7.2382, 7.3901, 7.5344, 7.6718, 7.8030,
        7.9285, 8.0487,
    ],
    // drum: a circular membrane, Bessel zeros j(m,n)/j(0,1)
    [
        0.0000, 0.6721, 1.0946, 1.1988, 1.4077, 1.5446, 1.6579, 1.8074, 1.8474, 1.8669, 2.0211,
        2.0467, 2.0808, 2.2020, 2.2048, 2.2726, 2.2937, 2.3458, 2.3592, 2.4362, 2.4700, 2.4733,
        2.4985, 2.5793,
    ],
    // plate: a simply supported rectangle, aspect 1.4
    [
        0.0000, 1.0097, 1.5784, 1.8886, 2.0000, 2.5082, 2.6011, 2.6547, 2.8700, 3.0097, 3.1699,
        3.1872, 3.4505, 3.4718, 3.5065, 3.5784, 3.6808, 3.7693, 3.8485, 3.8886, 4.0000, 4.0783,
        4.1057, 4.1623,
    ],
];

/// How much the body hears of what strikes it.
const BODY_GAIN: f32 = 0.5;

/// Body mode levels. The lower modes carry the sound.
const MODE_LEVEL: f32 = 0.25;
const MODE_FALLOFF: f32 = 0.9;

/// Fixed-Hz body: the first mode with SIZE fully down, in Hz.
const BODY_TOP_HZ: f32 = 800.0;
/// Octaves SIZE spans.
const BODY_SIZE_OCTAVES: f32 = 5.0;
/// Tracked body: octaves above and below the note SIZE spans.
const BODY_TRACK_OCTAVES: f32 = 2.0;
/// The shortest ring a mode is given, in seconds, and the longest.
const MODE_T60_MIN: f32 = 0.004;
const MODE_T60_MAX: f32 = 30.0;
/// The least a high mode may ring, as a fraction of the first mode's time.
/// Without a floor the top modes of a damped body are gone before they are
/// heard, and the body sounds dull rather than damped.
const MODE_T60_FLOOR: f32 = 0.08;

/// Sensitivity of a sympathetic string to the bridge. A matching sustained
/// input builds up to this multiple of itself at LEVEL fully up.
const SYMPATHETIC_SENSITIVITY: f32 = 6.0;
/// Most gain a sympathetic loop may have.
const SYMPATHETIC_MAX_GAIN: f32 = 0.999_9;
/// How much longer each successive string rings, as a fraction of RING: the
/// higher strings sustain, which lets a bank bloom into its upper partials.
const SYMPATHETIC_RING_SPREAD: f32 = 0.1;
/// The fixed C3 the sympathetic strings sit on when they do not follow.
const FIXED_ROOT_VOLTS: f32 = -1.0;
/// Octaves ROOT spans: a quarter of the knob up and down is an octave.
const ROOT_SPAN_OCTAVES: f32 = 4.0;
/// Shortest period a sympathetic string will play, in samples.
const SYMPATHETIC_MIN_PERIOD: f32 = 8.0;
/// Where along its line a sympathetic string's second pickup sits, and how far
/// it wanders, as fractions of the string. Each string wanders at its own rate.
const STRING_PICKUP: f32 = 0.22;
const STRING_PICKUP_DRIFT: f32 = 0.12;

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
/// no body, no sympathetic strings, no width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BridgeParams {
    /// Pitch of the lead voice, in volts. What the tracked body and the
    /// following sympathetic strings sit on.
    pub root_volts: f32,
    /// Body shape: string, bar, drum, plate.
    pub body_shape: f32,
    /// Body size: low modes at one end, high at the other.
    pub body_size: f32,
    /// Body material: wood at one end, glass at the other.
    pub body_material: f32,
    /// Body pickup position: all modes at 0, the odd modes gone at 1, hollow.
    pub body_position: f32,
    /// How wide the second pickup makes the stereo image, 0 mono.
    pub body_spread: f32,
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
            body_position: 0.0,
            body_spread: 0.0,
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

/// One mode of the body: a band-pass in the topology-preserving
/// state-variable form, which stays well behaved when its frequency and
/// damping are moved while it rings.
#[derive(Debug, Clone, Copy)]
struct Mode {
    ic1: f32,
    ic2: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    gain: f32,
    active: bool,
    centre: Ramp,
    side: Ramp,
}

impl Mode {
    const fn new() -> Self {
        Self {
            ic1: 0.0,
            ic2: 0.0,
            a1: 0.0,
            a2: 0.0,
            a3: 0.0,
            gain: 0.0,
            active: false,
            centre: Ramp::new(),
            side: Ramp::new(),
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = flush_denormal(2.0 * v1 - self.ic1);
        self.ic2 = flush_denormal(2.0 * v2 - self.ic2);
        self.gain * v1
    }
}

/// One string that is never plucked, only listened to.
#[derive(Debug, Clone)]
struct Sympathetic {
    line: DelayLine<SYMPATHETIC_LEN>,
    lowpass: LoopFilter,
    delay: Ramp,
    gain: Ramp,
    input: Ramp,
    drift: f32,
    drift_rate: f32,
}

impl Sympathetic {
    const fn new(rate_hz: f32) -> Self {
        Self {
            line: DelayLine::new(),
            lowpass: LoopFilter::open(),
            delay: Ramp::new(),
            gain: Ramp::new(),
            input: Ramp::new(),
            drift: 0.0,
            drift_rate: rate_hz,
        }
    }
}

/// The shared body and sympathetic strings.
#[derive(Debug, Clone)]
pub struct Bridge {
    sample_rate: f32,
    modes: [Mode; BODY_MODES],
    strings: [Sympathetic; SYMPATHETIC_LINES],
    primed: bool,
    drift: f32,
    // Smoothed copies of the knobs.
    shape: f32,
    size: f32,
    material: f32,
    position: f32,
    played_volts: f32,
    sympathetic_root: f32,
}

/// The drift of the sympathetic pickups, in Hz, one per line: slow, and no
/// two alike, so the comb of one is never the comb of another.
const fn drift_rate(index: usize) -> f32 {
    0.23 + 0.071 * index as f32
}

impl Bridge {
    /// A silent bridge at a sample rate, in Hz.
    pub fn new(sample_rate: f32) -> Self {
        let rate = if sample_rate.is_finite() && sample_rate >= 1_000.0 {
            sample_rate
        } else {
            48_000.0
        };
        let strings = {
            let mut i = 0;
            [(); SYMPATHETIC_LINES].map(|()| {
                let s = Sympathetic::new(drift_rate(i) / rate);
                i += 1;
                s
            })
        };
        Self {
            sample_rate: rate,
            modes: [const { Mode::new() }; BODY_MODES],
            strings,
            primed: false,
            drift: 0.0,
            shape: 0.0,
            size: 0.5,
            material: 0.5,
            position: 0.0,
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

    /// Render a block, left channel only: the mono signal the stereo pair
    /// shares at no spread. See [`Bridge::process`].
    pub fn process_mono(&mut self, p: &BridgeParams, input: &[f32], out: &mut [f32]) {
        self.process_into(p, input, out, None);
    }

    /// Render a block into a stereo pair. `input` is the sum of the strings
    /// and anything patched into the body; missing samples are zero. The
    /// outputs are bounded by one whatever goes in, and equal at no spread.
    /// `right` may be shorter than `left`: the extra is not written.
    pub fn process(
        &mut self,
        p: &BridgeParams,
        input: &[f32],
        left: &mut [f32],
        right: &mut [f32],
    ) {
        self.process_into(p, input, left, Some(right));
    }

    fn process_into(
        &mut self,
        p: &BridgeParams,
        input: &[f32],
        left: &mut [f32],
        mut right: Option<&mut [f32]>,
    ) {
        let len = left.len();
        if len == 0 {
            return;
        }
        self.settle(p, len);
        self.tune_body(p, len);
        self.tune_strings(p, len);

        let mix = unit(p.body_mix);
        let dry = 1.0 - mix;
        let level_gain = unit(p.level);
        let spread = unit(p.body_spread);
        let inputs = input.iter().copied().chain(core::iter::repeat(0.0));
        for (i, (l, x)) in left.iter_mut().zip(inputs).enumerate() {
            let x = if x.is_finite() { x } else { 0.0 };

            let (mut body_centre, mut body_side) = (0.0, 0.0);
            for mode in self.modes.iter_mut().filter(|m| m.active) {
                let y = mode.process(x);
                body_centre += mode.centre.next() * y;
                body_side += mode.side.next() * y;
            }
            // The side pickup, as its difference from the centre one: what
            // makes left and right differ, and nothing when spread is zero.
            body_side -= body_centre;

            let (mut centre, mut side) = (0.0, 0.0);
            for string in &mut self.strings {
                let delay = string.delay.next();
                let heard = string.line.read(delay);
                let y = string.lowpass.process(heard);
                let drive = string.input.next() * level_gain * x;
                let now = flush_denormal(string.gain.next() * y + drive);
                string.line.write(now);
                string.drift += string.drift_rate;
                if string.drift >= 1.0 {
                    string.drift -= 1.0;
                }
                let wave = if string.drift > 0.5 {
                    1.0 - string.drift
                } else {
                    string.drift
                };
                let tap = string
                    .line
                    .read(delay * (STRING_PICKUP + STRING_PICKUP_DRIFT * 2.0 * wave));
                centre += now;
                side += tap;
            }
            side -= centre;

            let shared = dry * x + mix * body_centre + centre;
            let width = spread * (mix * body_side + side);
            *l = soft_knee(shared + width);
            if let Some(slot) = right.as_deref_mut().and_then(|r| r.get_mut(i)) {
                *slot = soft_knee(shared - width);
            }
        }
    }

    /// Move the smoothed knobs towards where they were put.
    fn settle(&mut self, p: &BridgeParams, len: usize) {
        let blend = |rate: f32| 1.0 - exp(-(len as f32) / (rate * self.sample_rate));
        let (k, kr) = (blend(SETTLE_S), blend(ROOT_SETTLE_S));
        let shape = unit(p.body_shape);
        let size = unit(p.body_size);
        let material = unit(p.body_material);
        let position = unit(p.body_position);
        let volts = clamp(p.root_volts, -10.0, 10.0);
        if self.primed {
            self.shape += k * (shape - self.shape);
            self.size += k * (size - self.size);
            self.material += k * (material - self.material);
            self.position += k * (position - self.position);
            self.played_volts += kr * (volts - self.played_volts);
        } else {
            self.shape = shape;
            self.size = size;
            self.material = material;
            self.position = position;
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
        // The second pickup's drift, a triangle at a fraction of a hertz.
        self.drift += PICKUP_DRIFT_HZ * len as f32 / self.sample_rate;
        if self.drift >= 1.0 {
            self.drift -= 1.0;
        }
    }

    /// The body's coefficients and pickup weights for this block.
    fn tune_body(&mut self, p: &BridgeParams, len: usize) {
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
        // Material: the first mode's ring, and how much faster the higher
        // ones die. Wood is short and falls off with pitch; glass rings.
        let m = self.material;
        let first_t60 = 0.03 * exp2(7.0 * m);
        let falloff = 1.0 - 0.8 * m;
        // The pickups: the first at the knob, the second beside it and
        // drifting, each as a fraction of the string.
        let wave = if self.drift > 0.5 {
            1.0 - self.drift
        } else {
            self.drift
        };
        let first = 0.5 * self.position;
        let second = clamp(first + PICKUP_OFFSET + PICKUP_DRIFT * 2.0 * wave, 0.0, 0.5);
        let primed = self.primed;
        let mut level = MODE_LEVEL;
        for (i, mode) in self.modes.iter_mut().enumerate() {
            let at = |row: usize| {
                SHAPE_TABLE
                    .get(row)
                    .and_then(|r| r.get(i))
                    .copied()
                    .unwrap_or(0.0)
            };
            let l = at(a) + t * (at(a + 1) - at(a));
            let hz = base_hz * exp2(l);
            mode.active = hz < 0.45 * rate;
            let hz = clamp(hz, 1.0, 0.45 * rate);
            let t60 = clamp(
                first_t60 * exp2(-falloff * l),
                MODE_T60_FLOOR * first_t60,
                MODE_T60_MAX,
            );
            let t60 = clamp(t60, MODE_T60_MIN, MODE_T60_MAX);
            let turns = hz / rate;
            // Damping as the state-variable filter wants it: k = 1/Q, with Q
            // set by the ring time (60 dB in `t60` seconds is a decay of
            // ln(1000)/t60 per second, and a band-pass decays at ω₀/2Q).
            let k = clamp(
                2.0 * LN_1000 / (t60 * core::f32::consts::TAU * hz),
                1.0e-4,
                2.0,
            );
            let g = sin_turns(0.5 * turns) / cos_turns(0.5 * turns);
            let a1 = 1.0 / (1.0 + g * (g + k));
            mode.a1 = a1;
            mode.a2 = g * a1;
            mode.a3 = g * g * a1;
            // Between unit gain at the mode's own frequency, which leaves a
            // long ring almost silent when a pluck's broadband burst strikes
            // it, and equal power for white noise, which makes a long ring
            // roar when a partial sits on it, this is the second: `k` over
            // the square root of the decay per sample.
            let decay_per_sample = LN_1000 / (t60 * rate);
            mode.gain = BODY_GAIN * k / sqrt(decay_per_sample);
            // Each pickup hears mode n at cos(π·n·position): a comb in
            // frequency, applied as a weight so that moving it does not
            // flange or smear the attack.
            let n = (i + 1) as f32;
            let on = if mode.active { level } else { 0.0 };
            let (wc, ws) = (
                on * cos_turns(0.5 * first * n),
                on * cos_turns(0.5 * second * n),
            );
            if primed {
                mode.centre.aim(wc, len);
                mode.side.aim(ws, len);
            } else {
                mode.centre.jump(wc);
                mode.side.jump(ws);
            }
            level *= MODE_FALLOFF;
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
        for (line, string) in self.strings.iter_mut().enumerate() {
            let (index, cents) = match line.checked_sub(SYMPATHETIC_STRINGS) {
                None => (line, 0.0),
                Some(partner) => PARTNERS.get(partner).copied().unwrap_or((0, 0.0)),
            };
            let at = |row: usize| {
                SCENE_TABLE
                    .get(row)
                    .and_then(|r| r.get(index))
                    .copied()
                    .unwrap_or(0.0)
            };
            let l = at(a) + t * (at(a + 1) - at(a));
            let hz = ZERO_VOLT_HZ
                * exp2(clamp(
                    self.sympathetic_root + l + cents / 1200.0,
                    -10.0,
                    10.0,
                ));
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
            let this_ring = ring * (1.0 + SYMPATHETIC_RING_SPREAD * index as f32);
            let wanted = exp2(-LOG2_1000 * period / (this_ring * rate));
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
            consider(mode.ic1);
            consider(mode.ic2);
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
