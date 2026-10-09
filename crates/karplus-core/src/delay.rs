//! A fixed-size delay line with a fractional read.
//!
//! The string itself. No allocator, so the length is a type parameter and the
//! storage is an array the voice owns. A power of two, so that wrapping is a
//! mask and every index is in range by construction: the reads go through
//! `get()` anyway, because the rule is no panics in the audio path, not "no
//! panics we can think of".
//!
//! # Why Lagrange
//!
//! The JUCE version read with linear interpolation. Linear interpolation is a
//! lowpass whose depth depends on the fractional part of the delay, so two
//! notes a few cents apart had audibly different brightness, and a pitch that
//! glided swept that difference with it. Third-order Lagrange is flat to
//! within a fraction of a decibel up to a quarter of the sample rate and its
//! phase delay is the requested delay at low frequencies, which is what the
//! tuning arithmetic assumes. It is also passive — its gain never exceeds one
//! for delays between one and two taps from its centre — so it cannot make a
//! loop unstable. Allpass interpolation would be flatter still, but it rings
//! when the delay moves, and on a modular V/oct the delay is always moving.

/// The shortest delay [`DelayLine::read`] can produce, in samples: the four
/// taps must all be in the past.
pub const MIN_DELAY: f32 = 2.0;

/// A delay line of `N` samples. `N` must be a power of two.
#[derive(Debug, Clone)]
pub struct DelayLine<const N: usize> {
    buffer: [f32; N],
    write: usize,
}

impl<const N: usize> Default for DelayLine<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> DelayLine<N> {
    const MASK: usize = {
        assert!(N.is_power_of_two() && N >= 8, "N must be a power of two");
        N - 1
    };

    /// The longest delay [`DelayLine::read`] can produce, in samples.
    pub const MAX_DELAY: f32 = (N - 3) as f32;

    /// An empty line.
    pub const fn new() -> Self {
        Self {
            buffer: [0.0; N],
            write: 0,
        }
    }

    /// Silence the line.
    pub fn clear(&mut self) {
        self.buffer = [0.0; N];
        self.write = 0;
    }

    /// Append a sample.
    pub fn write(&mut self, sample: f32) {
        if let Some(slot) = self.buffer.get_mut(self.write & Self::MASK) {
            *slot = sample;
        }
        self.write = (self.write + 1) & Self::MASK;
    }

    /// The sample written `age` samples ago; `age = 1` is the newest.
    fn tap(&self, age: usize) -> f32 {
        self.buffer
            .get(self.write.wrapping_sub(age) & Self::MASK)
            .copied()
            .unwrap_or(0.0)
    }

    /// The signal `delay` samples ago, clamped to
    /// [`MIN_DELAY`]`..=`[`Self::MAX_DELAY`].
    pub fn read(&self, delay: f32) -> f32 {
        let delay = fathom_core::clamp(delay, MIN_DELAY, Self::MAX_DELAY);
        let whole = fathom_core::floor(delay);
        let f = delay - whole;
        let i = whole as usize;
        // Taps at ages i−1, i, i+1, i+2; the read point sits f past age i.
        let xm1 = self.tap(i.saturating_sub(1));
        let x0 = self.tap(i);
        let x1 = self.tap(i + 1);
        let x2 = self.tap(i + 2);
        let fm1 = f - 1.0;
        let fm2 = f - 2.0;
        let fp1 = f + 1.0;
        let hm1 = -f * fm1 * fm2 * (1.0 / 6.0);
        let h0 = fp1 * fm1 * fm2 * 0.5;
        let h1 = -fp1 * f * fm2 * 0.5;
        let h2 = fp1 * f * fm1 * (1.0 / 6.0);
        hm1 * xm1 + h0 * x0 + h1 * x1 + h2 * x2
    }

    /// Every sample, for the subnormal assertion.
    pub fn contents(&self) -> &[f32] {
        &self.buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_delays_are_exact() {
        let mut line = DelayLine::<64>::new();
        for n in 0..40 {
            line.write(n as f32);
        }
        // Newest is 39. A delay of d reads the sample written d ago.
        assert_eq!(line.read(1.0 + 1.0), 38.0);
        assert_eq!(line.read(10.0), 30.0);
    }

    #[test]
    fn a_ramp_is_interpolated_exactly() {
        // Cubic interpolation reproduces any cubic, so a ramp comes back
        // exactly at any fractional delay.
        let mut line = DelayLine::<64>::new();
        for n in 0..40 {
            line.write(n as f32);
        }
        for d in [2.0f32, 2.25, 5.5, 10.75, 20.1] {
            let expected = 40.0 - d;
            assert!((line.read(d) - expected).abs() < 1e-4, "delay {d}");
        }
    }

    #[test]
    fn out_of_range_delays_clamp() {
        let mut line = DelayLine::<16>::new();
        for n in 0..16 {
            line.write(n as f32);
        }
        assert_eq!(line.read(-5.0), line.read(MIN_DELAY));
        assert_eq!(line.read(1e9), line.read(DelayLine::<16>::MAX_DELAY));
        assert_eq!(line.read(f32::NAN), line.read(MIN_DELAY));
    }
}
