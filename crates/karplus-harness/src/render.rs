//! Turning parameters and gate events into samples.

use karplus_core::{Params, Voice};

/// Block size every render uses. Matches the VCV shim's.
pub const BLOCK: usize = 16;

/// One pluck: the gate goes high at `at_s` and falls `held_s` later.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pluck {
    /// When the gate rises, in seconds.
    pub at_s: f64,
    /// How long it stays up, in seconds.
    pub held_s: f64,
}

impl Pluck {
    /// A trigger at `at_s`: one millisecond of gate.
    pub fn trigger(at_s: f64) -> Self {
        Self {
            at_s,
            held_s: 0.001,
        }
    }
}

/// Render `seconds` of one voice with fixed parameters.
pub fn render(
    params: &Params,
    plucks: &[Pluck],
    seconds: f64,
    sample_rate: f32,
    seed: u32,
) -> Vec<f32> {
    render_with(plucks, seconds, sample_rate, seed, |_| *params, None)
}

/// Render with parameters that may change every block. `params_at` is given
/// the time of the block's first sample, in seconds. `audio_in`, if given, is
/// fed to the external exciter input.
pub fn render_with(
    plucks: &[Pluck],
    seconds: f64,
    sample_rate: f32,
    seed: u32,
    params_at: impl FnMut(f64) -> Params,
    audio_in: Option<&[f32]>,
) -> Vec<f32> {
    render_in_blocks(
        BLOCK,
        plucks,
        seconds,
        sample_rate,
        seed,
        params_at,
        audio_in,
    )
}

/// [`render_with`], at a block size of the caller's choosing.
pub fn render_in_blocks(
    block_size: usize,
    plucks: &[Pluck],
    seconds: f64,
    sample_rate: f32,
    seed: u32,
    mut params_at: impl FnMut(f64) -> Params,
    audio_in: Option<&[f32]>,
) -> Vec<f32> {
    let block_size = block_size.max(1);
    let len = (seconds * f64::from(sample_rate)).round().max(0.0) as usize;
    let mut voice = Voice::new(sample_rate, seed);
    let mut out = vec![0.0f32; len];
    let gate: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f64 / f64::from(sample_rate);
            let high = plucks.iter().any(|p| t >= p.at_s && t < p.at_s + p.held_s);
            if high {
                10.0
            } else {
                0.0
            }
        })
        .collect();
    for (b, (block, gate)) in out
        .chunks_mut(block_size)
        .zip(gate.chunks(block_size))
        .enumerate()
    {
        let start = b * block_size;
        let t = start as f64 / f64::from(sample_rate);
        let input = audio_in
            .and_then(|a| a.get(start..(start + block.len()).min(a.len())))
            .unwrap_or(&[]);
        voice.process(&params_at(t), gate, input, block);
    }
    out
}

/// Frequency of a pitch in volts, 0 V = C4.
pub fn volts_to_hz(volts: f32) -> f64 {
    f64::from(karplus_core::ZERO_VOLT_HZ) * 2f64.powf(f64::from(volts))
}

/// The slice from `from_s` to `to_s`, clamped to the signal.
pub fn span(signal: &[f32], sample_rate: f32, from_s: f64, to_s: f64) -> &[f32] {
    let sr = f64::from(sample_rate);
    let a = ((from_s * sr) as usize).min(signal.len());
    let b = ((to_s * sr) as usize).clamp(a, signal.len());
    &signal[a..b]
}

/// Seconds until the short-window RMS first falls `db` below its loudest
/// window, measured from the loudest window. `None` if it never does.
pub fn time_to_fall(signal: &[f32], sample_rate: f32, db: f64) -> Option<f64> {
    let window = (f64::from(sample_rate) * 0.01) as usize;
    let env = fathom_analysis::RmsEnvelope::new(signal, window);
    let (peak_i, peak) = env
        .windows
        .iter()
        .copied()
        .enumerate()
        .fold((0, 0.0f64), |b, (i, w)| if w > b.1 { (i, w) } else { b });
    let threshold = peak * 10f64.powf(-db / 20.0);
    env.windows
        .iter()
        .enumerate()
        .skip(peak_i)
        .find(|(_, w)| **w < threshold)
        .map(|(i, _)| (i - peak_i) as f64 * window as f64 / f64::from(sample_rate))
}

/// Level of one frequency over time: the magnitude of a single DFT bin at
/// `hz`, over consecutive windows of `window` samples, Hann-weighted.
pub fn partial_envelope(signal: &[f32], sample_rate: f32, hz: f64, window: usize) -> Vec<f64> {
    let window = window.max(2);
    let w = std::f64::consts::TAU * hz / f64::from(sample_rate);
    signal
        .chunks_exact(window)
        .map(|chunk| {
            let (mut re, mut im, mut norm) = (0.0f64, 0.0f64, 0.0f64);
            for (i, x) in chunk.iter().enumerate() {
                let hann = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / window as f64).cos();
                let phase = w * i as f64;
                re += hann * f64::from(*x) * phase.cos();
                im -= hann * f64::from(*x) * phase.sin();
                norm += hann;
            }
            2.0 * (re * re + im * im).sqrt() / norm
        })
        .collect()
}

/// Time for the fundamental to fall sixty decibels, in seconds, from the
/// slope of its envelope between 10 dB and 40 dB below its peak.
///
/// The fundamental alone, because that is what the DECAY knob is defined
/// against: upper partials lose more to the loop filter on every pass and
/// die sooner, as on any string, and a whole-signal level would mix the two.
pub fn fundamental_t60(signal: &[f32], sample_rate: f32, hz: f64) -> Option<f64> {
    // Eight periods, so the bin is narrower than the gap to the next
    // harmonic, and at least five milliseconds.
    let window = ((8.0 * f64::from(sample_rate) / hz).max(f64::from(sample_rate) * 0.005)) as usize;
    let env = partial_envelope(signal, sample_rate, hz, window);
    let (peak_i, top) = env
        .iter()
        .copied()
        .enumerate()
        .fold((0, 0.0f64), |b, (i, w)| if w > b.1 { (i, w) } else { b });
    if top <= 0.0 {
        return None;
    }
    let points: Vec<(f64, f64)> = env
        .iter()
        .enumerate()
        .skip(peak_i)
        .map(|(i, w)| {
            (
                i as f64 * window as f64 / f64::from(sample_rate),
                20.0 * (w / top).log10(),
            )
        })
        .skip_while(|(_, db)| *db > -10.0)
        .take_while(|(_, db)| *db > -40.0)
        .collect();
    if points.len() < 3 {
        return None;
    }
    let n = points.len() as f64;
    let mt = points.iter().map(|p| p.0).sum::<f64>() / n;
    let md = points.iter().map(|p| p.1).sum::<f64>() / n;
    let num: f64 = points.iter().map(|p| (p.0 - mt) * (p.1 - md)).sum();
    let den: f64 = points.iter().map(|p| (p.0 - mt) * (p.0 - mt)).sum();
    let slope = num / den;
    if slope < 0.0 {
        Some(-60.0 / slope)
    } else {
        None
    }
}
