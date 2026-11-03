//! One string.
//!
//! ```text
//!   gate ─► pluck ─► burst × velocity ─┐
//!                                     (+)─► tone lowpass ─► pick comb ─► × drive gain ─┐
//!   audio in ─────────────────────────┘                                                │
//!                                                                                      ▼
//!          ┌──────────── loop lowpass ◄── DC blocker ◄── × loop gain ◄────────────── (+) ─► shape ─┐
//!          │                                                                                         │
//!          └─────────────────────────────── delay line (the string) ◄────────────────────────────────┘
//!                                                │
//!                                                └──► out
//! ```
//!
//! The order in the loop is lowpass, then DC blocker, then gain, then the
//! shaper; the diagram reads right to left along the bottom. `docs/dsp.md`
//! has the reasoning for each block. The three properties everything else
//! hangs off:
//!
//! * **Tuned.** The loop's total delay at the fundamental is exactly one
//!   period. The lowpass lags and the DC blocker leads, so the delay line is
//!   shortened or lengthened by exactly what they add.
//! * **Decays as dialed.** The loop gain is set so that an A4 falls 60 dB in
//!   the dialed time *after* what the filters take, rather than before.
//! * **Bounded.** The shaper's slope is one at zero and its output never
//!   exceeds one, so it changes nothing about a quiet string and cannot let a
//!   loud one run away, whatever is patched into the audio input.

use crate::delay::{DelayLine, MIN_DELAY};
use crate::filter::{allpass_response, dc_blocker_response, fathom_dc_coefficient, LoopFilter};
use crate::math::{flush_denormal, soft_knee, sqrt, tanh, unit, Rng};
use crate::params::{
    allpass_coefficient, brightness_ratio, burst_periods, damp_seconds, decay_seconds,
    pick_fraction, stiffness_delay, tone_ratio, ExciterKind, Params, DRIVE_MAX_GAIN, MAX_CUTOFF,
    TENSION_MAX_FRACTION,
};
use crate::ramp::Ramp;
use fathom_core::{clamp, exp2, increment_from_turns, sin_turns, DcBlocker, Phase, ZERO_VOLT_HZ};

/// Length of the string's delay line. At 48 kHz the lowest note is 5.9 Hz;
/// at 192 kHz, 23 Hz.
pub const STRING_LEN: usize = 8192;

/// Length of the pick comb's line. The pick sits at most half way along.
const COMB_LEN: usize = 4096;

/// Most gain the loop may have. Five nines: a 40 s decay at 20 Hz needs
/// 0.9996 after compensation, so this is a guard against rounding, not a
/// limit anyone will reach by ear.
pub const MAX_LOOP_GAIN: f32 = 0.999_99;

/// How far below the fundamental the loop's DC blocker sits.
///
/// Fathom's blocker follows the note at a sixty-fourth of it, chosen for
/// waveform overshoot at the output. Inside a loop the cost is different: the
/// blocker's phase lead is largest at the fundamental, so the tuning
/// arithmetic corrects it there and every harmonic above comes out flat by up
/// to the same amount — 4.3 cents at a sixty-fourth, measured. The pick comb
/// already keeps DC out of the excitation (its gain at zero is zero), so the
/// blocker is only a guard and can sit eight times lower: half a cent.
const DC_CORNER_DIVISOR: f32 = 512.0;

/// How much lower the divisor goes at full buzz. The curved bridge's bend is
/// lopsided and leaves a little DC, loudest on low notes; the blocker is
/// brought up to remove it (to a divisor of 96), and the subsonic-mode cap
/// below follows it. Elements AC-couples its bridge the same way.
const DC_CORNER_BUZZ_REACH: f32 = 416.0;

// The loop gain of the blocker's subsonic mode is `1/√(1 + 2π/divisor)`,
// whatever the note (0.99392 at a divisor of 512): see the comment on the
// divisor above. The make-up gain for the filters is capped at its inverse,
// so that this mode always dies at least as fast as the note. Left alone, a
// dark setting, where the lowpass takes more from the fundamental than the
// blocker takes from this mode, would hand the make-up gain to the subsonic
// mode, and it rang on as a slow wobble for seconds after the note had gone.

/// `log2(1000)`: sixty decibels, as a power of two.
const LOG2_1000: f32 = 9.965_784;

/// The note whose decay the DECAY knob names when key tracking is off.
const DECAY_REFERENCE_HZ: f32 = 440.0;

/// Gate thresholds, in volts. Rack's Schmitt trigger convention.
pub const GATE_HIGH_V: f32 = 1.0;
/// See [`GATE_HIGH_V`].
pub const GATE_LOW_V: f32 = 0.1;

/// What the audio input is scaled by before it joins the burst.
///
/// A string driven at its own resonance builds up by `1/(1 − gain)`, which at
/// a long decay is tens of thousands. A tenth puts a full-scale ±5 V signal
/// at roughly full scale out once the string has rung up, with the shaper
/// only grazed; DRIVE then pushes it into saturation like it does a pluck.
const EXTERNAL_INPUT_GAIN: f32 = 0.1;

/// Softest velocity. Zero would be a pluck that makes no sound.
pub const MIN_VELOCITY: f32 = 0.01;

/// Humanize at full: up to ±2% of frequency (±34 cents), in volts.
const HUMANIZE_DETUNE_V: f32 = 0.028_57;
/// Humanize at full: up to ±0.1 of brightness travel.
const HUMANIZE_BRIGHTNESS: f32 = 0.1;
/// Humanize at full: up to ±0.1 of pick travel (±5% of the string).
const HUMANIZE_PICK: f32 = 0.1;

/// How far a soft velocity can pull the decay and length knobs back.
const VELOCITY_KNOB_REACH: f32 = 0.25;

/// Probability per sample of a dust impulse, as a fraction of `u32::MAX`.
/// One in eight.
const DUST_THRESHOLD: u32 = u32::MAX / 8;

/// The rate noise and dust are drawn at, in Hz, whatever the sample rate.
///
/// A burst of white noise one period long puts `σ/√P` into each harmonic,
/// and `P` doubles with the sample rate: drawn afresh every sample, the
/// same pluck came out 3 dB quieter at 96 kHz than at 48, and 6 dB at 192.
/// Held to a 48 kHz grid instead, the burst has the same spectrum in the
/// audio band at every rate; what holding adds above 24 kHz the loop filter
/// takes off on the first pass. Below 48 kHz there is nothing to hold.
const NOISE_REFERENCE_HZ: f32 = 48_000.0;

/// Longest burst, in samples, whatever the knob and the note say.
const MAX_BURST: f32 = 65_536.0;

/// The shortest period the string will play, in samples. Shorter and the
/// Lagrange taps and the filters' delay no longer fit inside one period.
const MIN_PERIOD: f32 = 4.0;

/// Samples between a sample leaving SEND and its answer arriving at RETURN
/// when the loop goes out through two cables and the shim's 16-sample blocks
/// (K22): one block to play out, one cable out, one cable back, and one block
/// to collect.
pub const LOOP_LATENCY: f32 = 34.0;

/// The buzz is a curved bridge: the string touches it where it swings hard,
/// and the touch shortens the string for a moment. Not a clip, so it takes no
/// energy: the note keeps its pitch and its decay and gains partials. The idea
/// is Mutable Instruments' (Elements and Rings, `String`); the numbers here are
/// measured on this loop (K21).
///
/// A swing smaller than this does not touch the bridge, so a hard-plucked note
/// buzzes and the quiet tail comes clean.
const BUZZ_DEAD_ZONE: f32 = 0.025;

/// Touching on the negative side bends the string this much more than on the
/// positive. The asymmetry is what makes the buzz a buzz and not vibrato.
const BUZZ_ASYMMETRY: f32 = 1.5;

/// The most the string's length is bent, as a fraction, per unit of touch, at
/// full buzz. The knob is squared, so the first half of its travel is gentle.
const BUZZ_BEND: f32 = 0.01;

/// How long the string's energy is averaged for tension modulation, in
/// periods. Short enough to follow a pluck, long enough not to follow the
/// waveform.
const TENSION_PERIODS: f32 = 3.0;

/// Mean-square level that counts as a fully driven string for tension
/// modulation. A default pluck at full velocity sits near this for its first tenth of a second.
const TENSION_FULL_ENERGY: f32 = 0.03;

/// First-order allpass `(a + z⁻¹)/(1 + a·z⁻¹)`.
#[derive(Debug, Clone, Copy, Default)]
struct Allpass {
    x1: f32,
    y1: f32,
}

impl Allpass {
    fn process(&mut self, a: f32, x: f32) -> f32 {
        let y = flush_denormal(a * x + self.x1 - a * self.y1);
        self.x1 = x;
        self.y1 = y;
        y
    }
}

/// Everything derived from [`Params`] for one block.
#[derive(Debug, Clone, Copy)]
struct Targets {
    delay: f32,
    gain: f32,
    comb_delay: f32,
    loop_coefficient: f32,
    tone_coefficient: f32,
    drive_gain: f32,
    drive_mix: f32,
    sine_increment: i32,
    dispersion: f32,
    tension: f32,
    energy_rate: f32,
    buzz: f32,
    dc_corner: f32,
}

/// One Karplus-Strong string.
#[derive(Debug, Clone)]
pub struct Voice {
    sample_rate: f32,
    seed: u32,
    string: DelayLine<STRING_LEN>,
    comb: DelayLine<COMB_LEN>,
    loop_filter: LoopFilter,
    tone_filter: LoopFilter,
    dc: DcBlocker,
    dispersion_stages: [Allpass; 2],
    dispersion: Ramp,
    tension: Ramp,
    buzz: Ramp,
    touch: f32,
    energy: f32,
    energy_rate: f32,
    delay: Ramp,
    gain: Ramp,
    comb_delay: Ramp,
    primed: bool,
    drive_gain: f32,
    drive_mix: f32,
    sine_increment: i32,
    gate_high: bool,
    burst_left: u32,
    noise_rate: f32,
    noise_clock: f32,
    noise_held: f32,
    sine_phase: Phase,
    noise: Rng,
    human: Rng,
    velocity: f32,
    detune_volts: f32,
    brightness_offset: f32,
    pick_offset: f32,
}

impl Voice {
    /// A silent string at a sample rate, in Hz.
    ///
    /// `seed` picks the noise. Give every voice of a polyphonic instrument its
    /// own, or a chord plucked at once is the same pluck at several pitches.
    pub fn new(sample_rate: f32, seed: u32) -> Self {
        let rate = if sample_rate.is_finite() && sample_rate >= 1_000.0 {
            sample_rate
        } else {
            48_000.0
        };
        let mut voice = Self {
            sample_rate: rate,
            seed,
            string: DelayLine::new(),
            comb: DelayLine::new(),
            loop_filter: LoopFilter::open(),
            tone_filter: LoopFilter::open(),
            dc: DcBlocker::new(rate),
            dispersion_stages: [Allpass::default(); 2],
            dispersion: Ramp::default(),
            tension: Ramp::default(),
            buzz: Ramp::default(),
            touch: 0.0,
            energy: 0.0,
            energy_rate: 0.01,
            delay: Ramp::default(),
            gain: Ramp::default(),
            comb_delay: Ramp::default(),
            primed: false,
            drive_gain: 1.0,
            drive_mix: 0.0,
            sine_increment: 0,
            gate_high: false,
            burst_left: 0,
            noise_rate: clamp(NOISE_REFERENCE_HZ / rate, 0.0, 1.0),
            noise_clock: 1.0,
            noise_held: 0.0,
            sine_phase: Phase::new(),
            noise: Rng::new(seed),
            human: Rng::new(seed),
            velocity: 1.0,
            detune_volts: 0.0,
            brightness_offset: 0.0,
            pick_offset: 0.0,
        };
        voice.reset();
        voice
    }

    /// Back to exactly the state [`Voice::new`] made: silent, unplucked, same
    /// seed. Rebuilds nothing large, so it is cheap enough for the audio
    /// thread.
    pub fn reset(&mut self) {
        self.string.clear();
        self.comb.clear();
        self.loop_filter = LoopFilter::open();
        self.tone_filter = LoopFilter::open();
        self.dc = DcBlocker::new(self.sample_rate);
        self.dispersion_stages = [Allpass::default(); 2];
        self.dispersion = Ramp::default();
        self.tension = Ramp::default();
        self.buzz = Ramp::default();
        self.touch = 0.0;
        self.energy = 0.0;
        self.energy_rate = 0.01;
        self.delay = Ramp::default();
        self.gain = Ramp::default();
        self.comb_delay = Ramp::default();
        self.primed = false;
        self.drive_gain = 1.0;
        self.drive_mix = 0.0;
        self.sine_increment = 0;
        self.gate_high = false;
        self.burst_left = 0;
        self.noise_clock = 1.0;
        self.noise_held = 0.0;
        self.sine_phase = Phase::new();
        self.noise = Rng::new(self.seed);
        // A second stream, so that turning HUMANIZE up does not change the
        // noise of every pluck that follows.
        self.human = Rng::new(self.seed.rotate_left(16) ^ 0x5bd1_e995);
        self.velocity = 1.0;
        self.detune_volts = 0.0;
        self.brightness_offset = 0.0;
        self.pick_offset = 0.0;
    }

    /// The sample rate, in Hz.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Render one block.
    ///
    /// `gate` is in volts, one value per output sample; a rising edge
    /// through [`GATE_HIGH_V`] plucks, a fall through [`GATE_LOW_V`] damps if
    /// DAMP is up. `audio_in` is the external exciter, full scale ±1, scaled
    /// down and summed with the burst before the tone filter. Either may be shorter than
    /// `out`, empty included; missing samples are zero.
    pub fn process(&mut self, p: &Params, gate: &[f32], audio_in: &[f32], out: &mut [f32]) {
        self.process_loop(p, gate, audio_in, &[], None, out);
    }

    /// [`Voice::process`] with the loop brought out to the panel (K22).
    ///
    /// `send`, if given, is filled with what leaves the delay line: the signal
    /// an outboard filter should treat. `loop_return` is what comes back, and
    /// is used only when [`Params::loop_return`] says something is patched;
    /// then it stands in for the loop's own lowpass, and the delay line is
    /// shortened by [`LOOP_LATENCY`] so that the trip through the cables is
    /// part of the string. Missing samples count as zero.
    pub fn process_loop(
        &mut self,
        p: &Params,
        gate: &[f32],
        audio_in: &[f32],
        loop_return: &[f32],
        mut send: Option<&mut [f32]>,
        out: &mut [f32],
    ) {
        let len = out.len();
        if len == 0 {
            return;
        }
        let targets = self.targets(p);
        self.apply(&targets, len);

        let gates = gate.iter().copied().chain(core::iter::repeat(0.0));
        let inputs = audio_in.iter().copied().chain(core::iter::repeat(0.0));
        let returns = loop_return.iter().copied().chain(core::iter::repeat(0.0));
        let manual = if p.manual_gate { 10.0 } else { 0.0 };

        for (i, (sample, ((gate_v, input), returned))) in out
            .iter_mut()
            .zip(gates.zip(inputs).zip(returns))
            .enumerate()
        {
            let level = if gate_v > manual { gate_v } else { manual };
            if !self.gate_high && level >= GATE_HIGH_V {
                self.gate_high = true;
                self.pluck(p);
                let targets = self.targets(p);
                self.apply(&targets, len - i);
            } else if self.gate_high && level <= GATE_LOW_V {
                self.gate_high = false;
                let targets = self.targets(p);
                self.apply(&targets, len - i);
            }

            let burst = self.burst(p.exciter) * self.velocity;
            let excitation = self
                .tone_filter
                .process(burst + EXTERNAL_INPUT_GAIN * finite(input));
            let picked = 0.5 * (excitation - self.comb.read(self.comb_delay.next()));
            self.comb.write(excitation);

            // Tension: a string stretched by its own swing runs sharp, and
            // comes back down as the swing dies.
            let stretch = self.tension.next() * self.energy_unit();
            let bend = 1.0 - self.touch * self.buzz.next();
            let delayed = self.string.read((self.delay.next() - stretch) * bend);
            // How hard the string is touching the bridge now, for the next
            // sample's length: twice the swing beyond the dead zone, negative
            // on the other side.
            let over = delayed.abs() - BUZZ_DEAD_ZONE;
            let side = if delayed > 0.0 { 1.0 } else { -BUZZ_ASYMMETRY };
            self.touch = (over.abs() + over) * side;
            if let Some(slot) = send.as_deref_mut().and_then(|s| s.get_mut(i)) {
                *slot = delayed;
            }
            // The internal lowpass keeps running when the return is patched,
            // so that unplugging it does not meet a stale filter.
            let internal = self.loop_filter.process(delayed);
            let damped = if p.loop_return {
                soft_knee(finite(returned))
            } else {
                internal
            };
            let filtered = self.dc.process(damped);
            let dispersed = self.disperse(filtered);
            let x = self.gain.next() * dispersed + self.drive_gain * picked;
            let written = flush_denormal(shape(x, self.drive_mix));
            self.energy =
                flush_denormal(self.energy + self.energy_rate * (written * written - self.energy));
            self.string.write(written);
            // The output is what goes into the line, not what comes out of
            // it: bounded by the shaper, where an interpolated read can
            // overshoot it slightly, and heard the moment the string is
            // plucked rather than a period later.
            *sample = written;
        }
    }

    /// The string's recent energy as a fraction of a fully driven string.
    fn energy_unit(&self) -> f32 {
        clamp(self.energy / TENSION_FULL_ENERGY, 0.0, 1.0)
    }

    /// Two dispersion allpasses in series.
    fn disperse(&mut self, x: f32) -> f32 {
        let a = self.dispersion.next();
        let [first, second] = &mut self.dispersion_stages;
        let y = first.process(a, x);
        second.process(a, y)
    }

    /// The current pitch, in Hz, after clamping and humanize.
    fn frequency(&self, p: &Params) -> f32 {
        let hz = ZERO_VOLT_HZ * exp2(clamp(p.pitch_volts, -10.0, 10.0) + self.detune_volts);
        let period = clamp(
            self.sample_rate / hz,
            MIN_PERIOD,
            DelayLine::<STRING_LEN>::MAX_DELAY - 8.0,
        );
        self.sample_rate / period
    }

    fn targets(&self, p: &Params) -> Targets {
        let rate = self.sample_rate;
        let frequency = self.frequency(p);
        let period = rate / frequency;
        let turns = frequency / rate;
        let softness = 1.0 - self.velocity;

        let brightness =
            p.brightness + self.brightness_offset - unit(p.velocity_brightness) * softness;
        let ceiling_hz = MAX_CUTOFF * rate;
        let loop_hz = clamp(frequency * brightness_ratio(brightness), 0.0, ceiling_hz);
        let loop_coefficient = LoopFilter::coefficient_for(loop_hz, rate);

        let decay =
            decay_seconds(p.decay - VELOCITY_KNOB_REACH * unit(p.velocity_decay) * softness);

        // What the loop's filters do to the fundamental. With an outboard
        // loop the lowpass is not in the circuit; whatever the outboard
        // filter does to level and phase is its owner's to correct (the trim).
        let divisor = DC_CORNER_DIVISOR - DC_CORNER_BUZZ_REACH * unit(p.buzz);
        let dc_corner = frequency / divisor;
        let blocker = dc_blocker_response(fathom_dc_coefficient(dc_corner, rate), turns);
        let response = if p.loop_return {
            blocker
        } else {
            crate::filter::one_pole_response(loop_coefficient, turns).then(blocker)
        };

        // The dispersion allpass: two identical stages whose DC delay is the
        // knob's. At rest each is one sample of plain delay, which the line
        // gives up, so a stiffness of nothing is the same string.
        let stage_delay = stiffness_delay(p.stiffness, period);
        let dispersion = allpass_coefficient(stage_delay);
        let dispersion_delay = 2.0 * allpass_response(dispersion, turns).delay;
        // The trim retunes the loop's delay and nothing else.
        let trim = exp2(clamp(p.loop_trim_volts, -1.0, 1.0));
        // The trip through the cables is part of the string.
        let latency = if p.loop_return { LOOP_LATENCY } else { 0.0 };
        let delay = period / trim - response.delay - dispersion_delay - latency;

        // Tension: the swing's energy moves the delay by up to this many
        // samples, scaled by the knob.
        let tension = unit(p.tension) * TENSION_MAX_FRACTION * period;
        let energy_rate = clamp(1.0 / (TENSION_PERIODS * period), 0.0, 1.0);

        // Passes of the loop per decay time, measured at the reference note
        // unless key tracking says to measure at this one.
        let reference = rate / DECAY_REFERENCE_HZ;
        let passes = reference + unit(p.key_track) * (period - reference);
        let mut wanted = exp2(-LOG2_1000 * passes / (decay * rate));
        // Damping is a time to silence at every note, so it always counts
        // this note's own passes.
        if !self.gate_high {
            if let Some(damp) = damp_seconds(p.damp) {
                let damped = exp2(-LOG2_1000 * period / (damp * rate));
                if damped < wanted {
                    wanted = damped;
                }
            }
        }
        // Make up what the filters take from the fundamental, but never so
        // much that the DC blocker's own mode outlives the note.
        let sub_mode = 1.0 / sqrt(1.0 + core::f32::consts::TAU / divisor);
        let loss = if response.gain > sub_mode {
            response.gain
        } else {
            sub_mode
        };
        let gain = clamp(wanted / loss, 0.0, MAX_LOOP_GAIN);

        let comb_delay = clamp(
            pick_fraction(p.pick + self.pick_offset) * period,
            MIN_DELAY,
            DelayLine::<COMB_LEN>::MAX_DELAY,
        );

        let tone_coefficient = match tone_ratio(p.tone) {
            Some(ratio) => {
                LoopFilter::coefficient_for(clamp(frequency * ratio, 0.0, ceiling_hz), rate)
            }
            None => 1.0,
        };

        let drive_mix = unit(p.drive);
        // The highest requested harmonic still below Nyquist, so that the
        // sine exciter is never asked for a pitch it can only alias.
        let ceiling = fathom_core::floor(0.45 * period);
        let harmonic = clamp(p.harmonic as f32, 1.0, 8.0);
        let harmonic = if harmonic > ceiling && ceiling >= 1.0 {
            ceiling
        } else {
            harmonic
        };

        Targets {
            delay,
            gain,
            comb_delay,
            loop_coefficient,
            tone_coefficient,
            drive_gain: 1.0 + (DRIVE_MAX_GAIN - 1.0) * drive_mix,
            drive_mix,
            sine_increment: increment_from_turns(harmonic * turns),
            dispersion,
            tension,
            energy_rate,
            buzz: unit(p.buzz) * unit(p.buzz) * BUZZ_BEND,
            dc_corner,
        }
    }

    fn apply(&mut self, t: &Targets, samples: usize) {
        self.loop_filter.set_coefficient(t.loop_coefficient);
        self.tone_filter.set_coefficient(t.tone_coefficient);
        self.dc.set_corner_hz(t.dc_corner);
        self.drive_gain = t.drive_gain;
        self.drive_mix = t.drive_mix;
        self.sine_increment = t.sine_increment;
        self.energy_rate = t.energy_rate;
        if self.primed {
            self.dispersion.aim(t.dispersion, samples);
            self.tension.aim(t.tension, samples);
            self.buzz.aim(t.buzz, samples);
            self.delay.aim(t.delay, samples);
            self.gain.aim(t.gain, samples);
            self.comb_delay.aim(t.comb_delay, samples);
        } else {
            // The first block has nothing to glide from.
            self.dispersion.jump(t.dispersion);
            self.tension.jump(t.tension);
            self.buzz.jump(t.buzz);
            self.delay.jump(t.delay);
            self.gain.jump(t.gain);
            self.comb_delay.jump(t.comb_delay);
            self.primed = true;
        }
    }

    /// The gate has risen: latch what a pluck latches and start the burst.
    fn pluck(&mut self, p: &Params) {
        self.velocity = if p.velocity_patched {
            clamp(p.velocity_volts / 10.0, MIN_VELOCITY, 1.0)
        } else {
            1.0
        };

        let human = unit(p.humanize);
        self.detune_volts = human * HUMANIZE_DETUNE_V * self.human.bipolar();
        self.brightness_offset = human * HUMANIZE_BRIGHTNESS * self.human.bipolar();
        self.pick_offset = human * HUMANIZE_PICK * self.human.bipolar();

        let period = self.sample_rate / self.frequency(p);
        let length =
            p.length - VELOCITY_KNOB_REACH * unit(p.velocity_length) * (1.0 - self.velocity);
        let samples = clamp(burst_periods(length) * period + 0.5, 1.0, MAX_BURST);
        self.burst_left = samples as u32;
        self.noise_clock = 1.0;
        // From the crest, so that even a one-sample burst is a strike.
        self.sine_phase = Phase::new();
        self.sine_phase.set_turns(0.25);
    }

    /// One sample of the burst, or zero once it is over.
    fn burst(&mut self, kind: ExciterKind) -> f32 {
        if self.burst_left == 0 {
            return 0.0;
        }
        self.burst_left -= 1;
        match kind {
            ExciterKind::Noise | ExciterKind::Dust => {
                self.noise_clock += self.noise_rate;
                if self.noise_clock >= 1.0 {
                    self.noise_clock -= 1.0;
                    self.noise_held = self.draw(kind);
                }
                self.noise_held
            }
            ExciterKind::Sine => {
                let s = sin_turns(self.sine_phase.turns());
                self.sine_phase.advance(self.sine_increment);
                s
            }
        }
    }

    /// A fresh noise or dust value.
    fn draw(&mut self, kind: ExciterKind) -> f32 {
        if kind == ExciterKind::Dust {
            let bits = self.noise.next_u32();
            if bits < DUST_THRESHOLD {
                if bits & 1 == 0 {
                    1.0
                } else {
                    -1.0
                }
            } else {
                0.0
            }
        } else {
            self.noise.bipolar()
        }
    }

    /// The largest magnitude anywhere in the voice's memory that is below
    /// the subnormal floor and not zero, for the subnormal assertion. Zero
    /// means none.
    pub fn smallest_nonzero_state(&self) -> f32 {
        let mut smallest = 0.0f32;
        let mut consider = |x: f32| {
            let m = if x < 0.0 { -x } else { x };
            if m > 0.0 && (smallest == 0.0 || m < smallest) {
                smallest = m;
            }
        };
        self.string
            .contents()
            .iter()
            .copied()
            .for_each(&mut consider);
        self.comb.contents().iter().copied().for_each(&mut consider);
        consider(self.loop_filter.state());
        consider(self.tone_filter.state());
        consider(self.dc.state());
        consider(self.energy);
        for stage in &self.dispersion_stages {
            consider(stage.x1);
            consider(stage.y1);
        }
        smallest
    }
}

/// Zero for anything non-finite: a NaN on a patch cable must not get into
/// the loop, where it would stay for good.
fn finite(x: f32) -> f32 {
    if x.is_finite() {
        x
    } else {
        0.0
    }
}

/// The loop's nonlinearity: the safety knee at no drive, a tanh at full,
/// crossfaded between. Both have unit slope at zero and never exceed one, so
/// neither does the mix.
fn shape(x: f32, mix: f32) -> f32 {
    if mix <= 0.0 {
        soft_knee(x)
    } else {
        (1.0 - mix) * soft_knee(x) + mix * tanh(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;
    const BLOCK: usize = 64;

    fn pluck_and_run(p: &Params, blocks: usize, mut each: impl FnMut(&[f32])) -> Voice {
        let mut voice = Voice::new(SR, 1);
        let mut out = [0.0f32; BLOCK];
        let mut gate = [10.0f32; BLOCK];
        for b in 0..blocks {
            if b == 1 {
                gate = [0.0; BLOCK];
            }
            voice.process(p, &gate, &[], &mut out);
            each(&out);
        }
        voice
    }

    #[test]
    fn nothing_sounds_without_a_pluck() {
        let mut voice = Voice::new(SR, 1);
        let mut out = [1.0f32; BLOCK];
        for _ in 0..100 {
            voice.process(&Params::default(), &[], &[], &mut out);
            assert!(out.iter().all(|&x| x == 0.0));
        }
    }

    #[test]
    fn a_pluck_sounds_and_stays_bounded() {
        let mut peak = 0.0f32;
        pluck_and_run(&Params::default(), 750, |out| {
            for &x in out {
                assert!(x.is_finite() && x.abs() <= 1.0, "{x}");
                peak = peak.max(x.abs());
            }
        });
        assert!(peak > 0.3, "peak {peak}");
    }

    #[test]
    fn reset_is_new() {
        let mut voice = pluck_and_run(&Params::default(), 10, |_| {});
        voice.reset();
        let mut fresh = Voice::new(SR, 1);
        let p = Params::default();
        let gate = [10.0f32; BLOCK];
        let mut a = [0.0f32; BLOCK];
        let mut b = [0.0f32; BLOCK];
        for _ in 0..20 {
            voice.process(&p, &gate, &[], &mut a);
            fresh.process(&p, &gate, &[], &mut b);
            assert_eq!(a, b);
        }
    }

    #[test]
    fn a_long_decay_never_leaves_a_subnormal() {
        let p = Params {
            decay: 0.0,
            ..Params::default()
        };
        let voice = pluck_and_run(&p, 48_000 * 20 / BLOCK, |_| {});
        let smallest = voice.smallest_nonzero_state();
        assert!(
            smallest == 0.0 || smallest >= crate::math::DENORMAL_FLOOR,
            "a state value of {smallest}"
        );
    }

    #[test]
    fn a_nan_on_the_input_does_not_get_in() {
        let mut voice = Voice::new(SR, 1);
        let mut out = [0.0f32; BLOCK];
        voice.process(&Params::default(), &[], &[f32::NAN; BLOCK], &mut out);
        voice.process(&Params::default(), &[], &[], &mut out);
        assert!(out.iter().all(|x| x.is_finite()));
    }
}
