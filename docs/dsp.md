# DSP

One voice is one string: `karplus_core::Voice`. Everything is in
`crates/karplus-core/src`, and the reason behind each choice is in
`docs/decisions.md`.

## Signal flow

```
gate ─► pluck ─► burst × velocity ─┐
                                  (+)─► tone lowpass ─► pick comb ─► × drive gain ─┐
audio in × 0.1 ───────────────────┘                                                │
                                                                                   ▼
       ┌──── delay line ──► loop lowpass ──► DC blocker ──► × loop gain ──────► (+) ─► shape ─┬─► out
       │                                                                                      │
       └──────────────────────────────────────────────────────────────────────────────────────┘
```

Per sample (`Voice::process`):

1. A rising gate (through 1 V) **plucks**: velocity is sampled, humanize
   offsets are drawn, and a burst of `LENGTH` periods starts.
2. The burst (noise, dust or sine) is scaled by velocity, summed with the
   audio input, lowpassed by TONE, and combed by PICK.
3. The string's previous output is read from the delay line with a Lagrange
   fractional read, lowpassed (BRIGHT), DC-blocked, and scaled by the loop
   gain.
4. The two are summed and **shaped**: the safety knee at no drive, `tanh` at
   full drive, crossfaded. Both have a slope of one at zero and never exceed
   one.
5. That sample is written to the delay line and is the output.

Per block (`Voice::targets`), from the knobs and the note:

| Derived | How |
| --- | --- |
| frequency | `ZERO_VOLT_HZ · 2^(pitch + humanize detune)`, clamped so the period fits the delay line (5.9 Hz to a quarter of the sample rate at 48 kHz) |
| loop lowpass | cutoff `f · brightness_ratio(BRIGHT)`, capped at 0.45·fs; coefficient `1 − e^(−2π·fc/fs)` |
| delay length | `period − (lowpass delay + DC blocker delay at f)`: so the whole loop is exactly one period |
| loop gain | `wanted / (lowpass gain · blocker gain at f)`, capped by `SUB_MODE_GAIN` and `MAX_LOOP_GAIN` |
| wanted | `1000^(−passes / (T·fs))`: 60 dB in `T` seconds, where `passes` is the A4 period, or this note's with KEY up |
| damping | when the gate is low and DAMP > 0, `wanted` is lowered to reach 60 dB in the damping time at this note |
| comb delay | `β · period`, β = `PICK / 2` + humanize |

The delay length, loop gain and comb delay glide linearly to their new values
over each block, so nothing steps. A gate edge mid-block recomputes them for
the rest of the block.

## Knob laws

All in `crates/karplus-core/src/params.rs`. A knob is `0..1`.

| Control | Law | Default |
| --- | --- | --- |
| DECAY | `0.1 s · 400^k`: 0.1 – 40 s, 2 s at the centre (time for an A4 to fall 60 dB) | 0.616 (4 s) |
| BRIGHT | cutoff = note × `2^(2 + 6k)`: 4× – 256× | 0.45 (26×) |
| PICK | β = k / 2: bridge to middle of the string | 0.4 (β = 0.2) |
| TONE | cutoff = note × `2^(8k)`: 1× – 256×; the last thousandth of travel is no filter | 1.0 (off) |
| LENGTH | `2^(−4 + 6k)` periods: 1/16 – 4 | 0.667 (1 period) |
| DRIVE | gain into the loop `1 + 7k`; shaper crossfade `k` | 0 |
| DAMP | 0 = rings; else `3 s · (0.02/3)^k`: 3 s – 20 ms | 0 |
| HUMAN | per pluck, up to ±2% pitch, ±0.1 BRIGHT, ±0.1 PICK | 0 |
| KEY | 0: DECAY is the A4's time, low notes ring longer; 1: every note rings DECAY | 0 |
| VEL → BRIGHT / DECAY / LENGTH | how far velocity below full pulls each down: BRIGHT by up to 1, DECAY and LENGTH by up to 0.25 of travel | 0 |
| HARM | sine exciter harmonic, 1 – 8, lowered automatically to stay below Nyquist | 1 |

## Invariants the tests hold

- In tune within a cent, C1 – C7, at 44.1/48/96 kHz, BRIGHT 0.2 – 1.
- DECAY within 15% of the measured fundamental's T60, with KEY up.
- Never above full scale, never NaN, infinite or subnormal, never silent
  after a pluck, at every corner of the parameter space and 200 random
  points.
- No steps in a fast glide at 64-sample blocks. Check this by making the delay
  jump once per block: the test fails.
- The same seed gives the same output, bit for bit.
- Level within a decibel at 44.1, 96 and 192 kHz against 48.
- Fathom's DC blocker behaves as the tuning maths predicts.
