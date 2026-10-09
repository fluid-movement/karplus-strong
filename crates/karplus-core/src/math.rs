//! The maths Fathom does not have.
//!
//! `fathom-core` provides `exp2`, `sin_turns`, `floor`, `clamp` and `lerp`,
//! and every one of them is used here as it is. A plucked string needs four
//! more things: a square root and an arctangent, to work out what the loop
//! filters do to the fundamental, a hyperbolic tangent for the drive, and a
//! noise source. Fathom has a private square root and no arctangent, tanh or
//! random numbers at all (`docs/fathom-reuse.md`).
//!
//! Same rules as Fathom's: plain `f32` arithmetic, no `libm`, no tables, so
//! that host and target agree bit for bit. Accuracy is tested against `f64`
//! `std` in `karplus-verify`, because this crate has no `std` to test with.

use fathom_core::{clamp, exp2, sin_turns};

/// `log2(e)`, for writing a natural exponential with Fathom's `exp2`.
pub const LOG2_E: f32 = core::f32::consts::LOG2_E;

/// `e^x`, by way of `exp2`.
pub fn exp(x: f32) -> f32 {
    exp2(x * LOG2_E)
}

/// `cos(2π·turns)`, by way of `sin_turns`.
pub fn cos_turns(turns: f32) -> f32 {
    sin_turns(turns + 0.25)
}

/// Square root. Zero for anything that is not a positive, finite number.
///
/// An exponent-halving first guess, then three Newton steps. The guess is
/// within 4% everywhere; each step squares the error, so three leave it below
/// one `f32` ulp.
pub fn sqrt(x: f32) -> f32 {
    if x.is_nan() || x <= 0.0 || x == f32::INFINITY {
        return 0.0;
    }
    let mut y = f32::from_bits((x.to_bits() >> 1) + 0x1fbd_1df5);
    y = 0.5 * (y + x / y);
    y = 0.5 * (y + x / y);
    y = 0.5 * (y + x / y);
    y
}

/// Coefficients for `atan(x)/x` in `v = x²`, `|x| ≤ 1`.
///
/// Near-minimax fit of degree six in `v` (iteratively reweighted least
/// squares on the absolute error of `atan` itself). Maximum absolute error
/// 2.5e-7 rad, about two `f32` ulps at π/4.
const ATAN_COEFFICIENTS: [f32; 7] = [
    0.999_996_1,
    -0.333_173_7,
    0.198_078_14,
    -0.132_333_38,
    0.079_623_61,
    -0.033_604_167,
    0.006_811_776,
];

/// `atan(x)` for `|x| ≤ 1`.
fn atan_unit(x: f32) -> f32 {
    let v = x * x;
    let mut acc = ATAN_COEFFICIENTS[6];
    acc = acc * v + ATAN_COEFFICIENTS[5];
    acc = acc * v + ATAN_COEFFICIENTS[4];
    acc = acc * v + ATAN_COEFFICIENTS[3];
    acc = acc * v + ATAN_COEFFICIENTS[2];
    acc = acc * v + ATAN_COEFFICIENTS[1];
    acc = acc * v + ATAN_COEFFICIENTS[0];
    x * acc
}

/// `atan2(y, x)`, in radians, `-π..=π`. Zero for `(0, 0)` and for anything
/// non-finite.
pub fn atan2(y: f32, x: f32) -> f32 {
    use core::f32::consts::{FRAC_PI_2, PI};
    if !y.is_finite() || !x.is_finite() || (x == 0.0 && y == 0.0) {
        return 0.0;
    }
    let ax = if x < 0.0 { -x } else { x };
    let ay = if y < 0.0 { -y } else { y };
    // First octant, then reflect out to the right quadrant.
    let mut angle = if ay <= ax {
        atan_unit(ay / ax)
    } else {
        FRAC_PI_2 - atan_unit(ax / ay)
    };
    if x < 0.0 {
        angle = PI - angle;
    }
    if y < 0.0 {
        -angle
    } else {
        angle
    }
}

/// Below this, `tanh` is evaluated by series rather than by exponential, which
/// would cancel catastrophically near zero.
const TANH_SERIES_LIMIT: f32 = 0.25;

/// Beyond this `tanh` is 1.0 in `f32`.
const TANH_SATURATED: f32 = 9.0;

/// Hyperbolic tangent. Non-finite input gives 0.0.
///
/// The drive stage sits inside the feedback loop and must have a slope of
/// exactly one at zero, or the quiet tail of every note would decay at a rate
/// set by rounding. Hence the series near zero: `1 − 2/(e^2x + 1)` is right
/// in value there but loses most of its digits.
pub fn tanh(x: f32) -> f32 {
    if !x.is_finite() {
        return if x.is_nan() { 0.0 } else { x.signum() };
    }
    let ax = if x < 0.0 { -x } else { x };
    let magnitude = if ax < TANH_SERIES_LIMIT {
        let v = ax * ax;
        // x − x³/3 + 2x⁵/15 − 17x⁷/315 + 62x⁹/2835. Truncation error below
        // 2e-9 at the limit.
        let mut acc = 62.0 / 2835.0;
        acc = acc * v - 17.0 / 315.0;
        acc = acc * v + 2.0 / 15.0;
        acc = acc * v - 1.0 / 3.0;
        acc = acc * v + 1.0;
        ax * acc
    } else if ax > TANH_SATURATED {
        1.0
    } else {
        1.0 - 2.0 / (exp(2.0 * ax) + 1.0)
    };
    if x < 0.0 {
        -magnitude
    } else {
        magnitude
    }
}

/// Where the safety curve stops being the identity.
pub const KNEE: f32 = 0.9;

/// A curve that is exactly the identity up to [`KNEE`] and approaches ±1
/// asymptotically beyond it, continuous in value and slope.
///
/// This is the curve inside Fathom's output limiter (`saturate.rs`, there
/// with a knee of 0.98), which is private to that crate. Here it is not a
/// limiter but the thing that keeps the loop bounded with the drive at zero:
/// a string fed from the external input at its own resonance would otherwise
/// grow without limit.
pub fn soft_knee(x: f32) -> f32 {
    let ax = if x < 0.0 { -x } else { x };
    if ax <= KNEE {
        return x;
    }
    let span = 1.0 - KNEE;
    let magnitude = KNEE + span * (1.0 - exp(-(ax - KNEE) / span));
    if x < 0.0 {
        -magnitude
    } else {
        magnitude
    }
}

/// Below this a decaying value is replaced with zero.
///
/// Fathom has this exact function (`filter::flush_denormal`), but it is
/// `pub(crate)`. Same threshold, same reason: subnormals are slow on x86 and
/// differ on the M7, and a filter that has swallowed a NaN keeps returning
/// NaN for ever.
pub const DENORMAL_FLOOR: f32 = 1.0e-20;

/// Zero if `value` is non-finite or too small to keep, `value` otherwise.
pub fn flush_denormal(value: f32) -> f32 {
    if !value.is_finite() || (value > -DENORMAL_FLOOR && value < DENORMAL_FLOOR) {
        0.0
    } else {
        value
    }
}

/// A seeded xorshift32 generator. The only randomness in the crate.
///
/// Determinism is a hard rule: the seed is chosen by whoever builds the voice,
/// and the same seed always gives the same pluck.
#[derive(Debug, Clone, Copy)]
pub struct Rng {
    state: u32,
}

impl Rng {
    /// A generator from a seed. Zero, which xorshift cannot leave, is
    /// replaced with a fixed non-zero value.
    pub const fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 0x9e37_79b9 } else { seed },
        }
    }

    /// The next 32 random bits.
    pub fn next_u32(&mut self) -> u32 {
        let mut r = self.state;
        r ^= r << 13;
        r ^= r >> 17;
        r ^= r << 5;
        self.state = r;
        r
    }

    /// Uniform in `0.0..1.0`. Twenty-four bits: all an `f32` mantissa holds.
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// Uniform in `-1.0..1.0`.
    pub fn bipolar(&mut self) -> f32 {
        2.0 * self.unit() - 1.0
    }
}

/// Clamp a knob position into `0..=1`, NaN to zero.
pub fn unit(x: f32) -> f32 {
    clamp(x, 0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqrt_of_squares_is_exact_enough() {
        for n in 1..2000 {
            let x = n as f32 * 0.37;
            let r = sqrt(x * x);
            assert!((r - x).abs() <= x * 2.0e-7, "sqrt({}) = {r}", x * x);
        }
        assert_eq!(sqrt(0.0), 0.0);
        assert_eq!(sqrt(-1.0), 0.0);
        assert_eq!(sqrt(f32::NAN), 0.0);
    }

    #[test]
    fn atan2_finds_its_quadrants() {
        use core::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};
        assert!((atan2(1.0, 1.0) - FRAC_PI_4).abs() < 1e-6);
        assert!((atan2(1.0, -1.0) - 3.0 * FRAC_PI_4).abs() < 1e-6);
        assert!((atan2(-1.0, -1.0) + 3.0 * FRAC_PI_4).abs() < 1e-6);
        assert!((atan2(-1.0, 1.0) + FRAC_PI_4).abs() < 1e-6);
        assert!((atan2(1.0, 0.0) - FRAC_PI_2).abs() < 1e-6);
        assert!((atan2(0.0, -1.0) - PI).abs() < 1e-6);
        assert_eq!(atan2(0.0, 0.0), 0.0);
    }

    #[test]
    fn tanh_has_unit_slope_at_zero() {
        // The property the loop depends on: no gain or loss for small signals.
        for x in [1e-7f32, 1e-5, 1e-3, 0.01] {
            assert!((tanh(x) / x - 1.0).abs() < 1e-4, "tanh({x}) = {}", tanh(x));
        }
        assert_eq!(tanh(0.0), 0.0);
        assert_eq!(tanh(100.0), 1.0);
        assert_eq!(tanh(-100.0), -1.0);
        assert_eq!(tanh(f32::NAN), 0.0);
    }

    #[test]
    fn tanh_is_continuous_where_its_method_changes() {
        for edge in [TANH_SERIES_LIMIT, TANH_SATURATED] {
            let below = tanh(edge * (1.0 - 1e-6));
            let above = tanh(edge * (1.0 + 1e-6));
            assert!((above - below).abs() < 1e-6, "step at {edge}");
        }
    }

    #[test]
    fn soft_knee_is_the_identity_below_its_knee_and_bounded_above() {
        for x in [0.0f32, 0.3, -0.5, 0.9, -0.9] {
            assert_eq!(soft_knee(x), x);
        }
        for x in [0.95f32, 1.0, 2.0, 10.0, 1e9] {
            let y = soft_knee(x);
            assert!(y > KNEE && y <= 1.0, "{x} gave {y}");
            assert_eq!(soft_knee(-x), -y);
        }
    }

    #[test]
    fn rng_is_deterministic_and_spread() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        let mut sum = 0.0f32;
        for _ in 0..10_000 {
            let x = a.bipolar();
            assert_eq!(x, b.bipolar());
            assert!((-1.0..1.0).contains(&x));
            sum += x;
        }
        assert!((sum / 10_000.0).abs() < 0.03, "mean {}", sum / 10_000.0);
        let mut zero = Rng::new(0);
        assert_ne!(zero.next_u32(), 0);
    }

    #[test]
    fn flush_removes_subnormals_and_nans() {
        assert_eq!(flush_denormal(1e-30), 0.0);
        assert_eq!(flush_denormal(f32::NAN), 0.0);
        assert_eq!(flush_denormal(0.5), 0.5);
    }
}
