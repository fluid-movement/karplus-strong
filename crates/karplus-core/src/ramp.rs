//! A value that moves linearly to a target over a block.

/// Linear glide, one step per sample.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Ramp {
    value: f32,
    step: f32,
}

impl Ramp {
    pub(crate) const fn new() -> Self {
        Self {
            value: 0.0,
            step: 0.0,
        }
    }

    pub(crate) fn aim(&mut self, target: f32, samples: usize) {
        self.step = if samples > 0 {
            (target - self.value) / samples as f32
        } else {
            0.0
        };
        if samples == 0 {
            self.value = target;
        }
    }

    pub(crate) fn jump(&mut self, target: f32) {
        self.value = target;
        self.step = 0.0;
    }

    pub(crate) fn next(&mut self) -> f32 {
        let value = self.value;
        self.value += self.step;
        value
    }
}
