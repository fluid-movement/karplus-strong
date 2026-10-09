//! The loop filter, and what the loop's filters do to the fundamental.
//!
//! Fathom has a one-pole lowpass (`fathom_core::OnePole`) and it is not used
//! here, for two reasons recorded in `docs/fathom-reuse.md`:
//!
//! * Its coefficient is `2π·fc/fs`, clamped at one. That is the small-angle
//!   approximation, accurate well below Nyquist and not near it, and it
//!   reaches one — the filter switches itself off — at `fs/2π`, 7.6 kHz at
//!   48 kHz. In Fathom's low-pass gate that is harmless. In a string loop the
//!   brightness knob would stop doing anything for its top third.
//! * The coefficient is private, so nothing outside can ask what the filter
//!   does to a given frequency. A string has to know exactly that, to keep
//!   its tuning and its decay time.
//!
//! This one maps the cutoff by impulse invariance, `1 − e^(−2π·fc/fs)`, which
//! never reaches one, and answers for its own response.

use crate::math::{atan2, cos_turns, exp, flush_denormal, sqrt};
use fathom_core::{clamp, sin_turns};

/// A one-pole lowpass, `y += a·(x − y)`.
#[derive(Debug, Clone, Copy)]
pub struct LoopFilter {
    coefficient: f32,
    state: f32,
}

impl Default for LoopFilter {
    fn default() -> Self {
        Self::open()
    }
}

impl LoopFilter {
    /// A filter that passes everything.
    pub const fn open() -> Self {
        Self {
            coefficient: 1.0,
            state: 0.0,
        }
    }

    /// The coefficient for a cutoff, by impulse invariance.
    pub fn coefficient_for(cutoff_hz: f32, sample_rate: f32) -> f32 {
        let turns = clamp(cutoff_hz / sample_rate, 0.0, 10.0);
        clamp(1.0 - exp(-core::f32::consts::TAU * turns), 0.0, 1.0)
    }

    /// Set the coefficient directly. 1.0 is a wire.
    pub fn set_coefficient(&mut self, coefficient: f32) {
        self.coefficient = clamp(coefficient, 0.0, 1.0);
    }

    /// The coefficient in use.
    pub fn coefficient(&self) -> f32 {
        self.coefficient
    }

    /// Clear the memory.
    pub fn reset(&mut self) {
        self.state = 0.0;
    }

    /// The memory, for the subnormal assertion.
    pub fn state(&self) -> f32 {
        self.state
    }

    /// Filter one sample.
    pub fn process(&mut self, input: f32) -> f32 {
        self.state = flush_denormal(self.state + self.coefficient * (input - self.state));
        self.state
    }

    /// Gain and phase delay at a frequency, in turns per sample.
    pub fn response(&self, turns: f32) -> Response {
        one_pole_response(self.coefficient, turns)
    }
}

/// What a filter does to one frequency.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Response {
    /// Linear gain.
    pub gain: f32,
    /// Phase delay, in samples. Negative for a phase lead.
    pub delay: f32,
}

impl Response {
    /// Unit gain, no delay.
    pub const WIRE: Self = Self {
        gain: 1.0,
        delay: 0.0,
    };

    /// Two filters in series.
    pub fn then(self, other: Self) -> Self {
        Self {
            gain: self.gain * other.gain,
            delay: self.delay + other.delay,
        }
    }
}

/// Response of `a / (1 − b·z⁻¹)`, `b = 1 − a`, at `turns` per sample.
pub fn one_pole_response(coefficient: f32, turns: f32) -> Response {
    let b = 1.0 - coefficient;
    let c = cos_turns(turns);
    let s = sin_turns(turns);
    let denominator = sqrt(1.0 - 2.0 * b * c + b * b);
    let gain = if denominator > 0.0 {
        coefficient / denominator
    } else {
        1.0
    };
    let phase = -atan2(b * s, 1.0 - b * c);
    Response {
        gain,
        delay: phase_delay(phase, turns),
    }
}

/// Response of the normalised DC blocker `g·(1 − z⁻¹)/(1 − R·z⁻¹)`,
/// `g = (1 + R)/2`, which is the form `fathom_core::DcBlocker` uses.
pub fn dc_blocker_response(coefficient: f32, turns: f32) -> Response {
    let r = coefficient;
    let c = cos_turns(turns);
    let s = sin_turns(turns);
    let denominator = sqrt(1.0 - 2.0 * r * c + r * r);
    // |1 − e^(−jω)| = 2·sin(ω/2), and its phase is π/2 − ω/2.
    let numerator = 2.0 * sin_turns(0.5 * turns);
    let gain = if denominator > 0.0 {
        0.5 * (1.0 + r) * numerator / denominator
    } else {
        0.0
    };
    let phase =
        core::f32::consts::FRAC_PI_2 - core::f32::consts::PI * turns - atan2(r * s, 1.0 - r * c);
    Response {
        gain,
        delay: phase_delay(phase, turns),
    }
}

/// The coefficient `fathom_core::DcBlocker::set_corner_hz` arrives at for a
/// corner frequency.
///
/// **A copy of Fathom's private arithmetic**, because the blocker does not
/// expose its coefficient (`docs/fathom-reuse.md`). `karplus-harness`
/// measures the real blocker's response against [`dc_blocker_response`] of
/// this, so if Fathom's formula ever moves, that test fails rather than the
/// tuning quietly drifting.
pub fn fathom_dc_coefficient(corner_hz: f32, sample_rate: f32) -> f32 {
    clamp(
        1.0 - core::f32::consts::TAU * clamp(corner_hz, 0.0, 1_000.0) / sample_rate,
        0.0,
        0.999_999,
    )
}

/// Phase in radians to phase delay in samples.
fn phase_delay(phase: f32, turns: f32) -> f32 {
    let omega = core::f32::consts::TAU * turns;
    if omega > 0.0 {
        -phase / omega
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// Amplitude of a settled sine through the filter, from its RMS: the
    /// sampled peak of a high sine misses the real one.
    fn measured_gain(filter: &mut LoopFilter, hz: f32) -> f32 {
        let mut energy = 0.0f32;
        let mut count = 0.0f32;
        for i in 0..48_000 {
            let y = filter.process(sin_turns(hz * i as f32 / SR));
            if i >= 24_000 {
                energy += y * y;
                count += 1.0;
            }
        }
        sqrt(2.0 * energy / count)
    }

    #[test]
    fn the_predicted_gain_is_the_measured_gain() {
        for cutoff in [200.0f32, 1_000.0, 5_000.0, 15_000.0] {
            for hz in [55.0f32, 440.0, 2_000.0, 8_000.0] {
                let mut filter = LoopFilter::open();
                filter.set_coefficient(LoopFilter::coefficient_for(cutoff, SR));
                let predicted = filter.response(hz / SR).gain;
                let measured = measured_gain(&mut filter, hz);
                assert!(
                    (predicted - measured).abs() < 0.01,
                    "cutoff {cutoff}, {hz} Hz: predicted {predicted}, measured {measured}"
                );
            }
        }
    }

    #[test]
    fn it_never_switches_itself_off() {
        // The failing of `OnePole` this filter exists to avoid.
        let a = LoopFilter::coefficient_for(12_000.0, SR);
        assert!(a < 0.85, "12 kHz at 48 kHz is already a wire: {a}");
        assert!(LoopFilter::coefficient_for(15_000.0, SR) > a);
    }

    #[test]
    fn a_wire_has_no_delay() {
        let filter = LoopFilter::open();
        let r = filter.response(0.01);
        assert!((r.gain - 1.0).abs() < 1e-6);
        assert!(r.delay.abs() < 1e-4);
    }

    #[test]
    fn the_dc_blocker_leads_and_a_lowpass_lags() {
        let lead = dc_blocker_response(fathom_dc_coefficient(440.0 / 64.0, SR), 440.0 / SR);
        assert!(lead.delay < 0.0, "{lead:?}");
        assert!(lead.gain < 1.0 && lead.gain > 0.999, "{lead:?}");
        let lag = one_pole_response(LoopFilter::coefficient_for(2_000.0, SR), 440.0 / SR);
        assert!(lag.delay > 0.0, "{lag:?}");
    }

    #[test]
    fn the_state_never_holds_a_subnormal() {
        let mut filter = LoopFilter::open();
        filter.set_coefficient(0.01);
        filter.process(1.0);
        for _ in 0..1_000_000 {
            filter.process(0.0);
        }
        assert_eq!(filter.state.classify(), core::num::FpCategory::Zero);
    }
}
