# Scenarios

A scenario is a requirement written as data. To add a requirement, add a
file here, not Rust. The runner renders each file, measures it, and fails
the build if an assertion does not hold.

```
just scenarios                                          # all of them
cargo run -p karplus-harness -- run --filter pluck      # some
cargo run -p karplus-harness -- check                   # parse only
cargo run -p karplus-harness -- run --wav out/          # and write WAVs
```

The format is Fathom's, cut down to what a string needs. Durations, note
names, automation and bounds are parsed by Fathom's own `fathom-harness`
types.

## The file

```yaml
name: damp_mute                 # required, unique
description: >                  # why it exists, and what the bounds were measured at
  ...
duration: 1s                    # required
sample_rate: 48000              # default 48000
block_size: 16                  # default 16, the VCV shim's
seed: 1                         # the voice's noise; default 1
pitch: C3                       # note name or `440 Hz`; default C4 (0 V)
exciter: noise                  # noise | dust | sine
harmonic: 1                     # the sine exciter's, 1..8

params:                         # knob positions, 0..1, held for the whole render
  decay: 0.9
  damp: 0.7

automation:                     # values that move; shapes linear | exponential | smooth
  pitch_volts: { from: 0, to: 1, over: 50ms, start: 500ms }

plucks:                         # default: one trigger at 0s; [] for none
  - 0s                          # a 1 ms trigger
  - { at: 500ms, held: 300ms, velocity: 6 }   # gate length, VEL in volts

audio_in: { noise: 0.25 }       # seeded noise at the audio input, 1.0 = ±5 V

analysis: { from: 450ms, to: 1s }   # the span measured; default all of it

assert:                         # required, never empty
  rms_dbfs: { max: -60 }
```

Parameters: `pitch_volts`, `decay`, `brightness`, `pick`, `tone`, `length`,
`drive`, `damp`, `humanize`, `key_track`, `velocity_brightness`,
`velocity_decay`, `velocity_length`. Set `pitch` or `pitch_volts`, not both.
The laws behind the knob positions are in `docs/dsp.md`.

Unknown fields, parameters or metrics are errors, and so is an empty
`assert`.

## Metrics

| Metric | Meaning |
| --- | --- |
| `pitch_error_cents` | Measured fundamental against the expected pitch, by fitting a harmonic comb (a plucked string's loudest partial is often not its first). Signed. |
| `fundamental_hz` | The measured fundamental. |
| `t60_s` | Time for the fundamental to fall 60 dB, from the slope between 10 and 40 dB down. What DECAY means. |
| `peak`, `rms_dbfs` | Levels. `peak` is linear. |
| `dc_offset_dbfs` | Windowed mean. |
| `discontinuity` | How far the worst jump exceeds what the signal does around it. Catches steps. Needs a sustained signal: a fast fade reads as a click. |
| `non_finite_samples` | NaN, infinity and subnormal samples, over the whole render. |

A metric that cannot be measured fails its assertion.

## Writing a good one

Assert the opposing constraint too. Silence passes every "too loud" check,
so `damp_mute` has `damp_mute_rings_while_held`. Take bounds from a
measurement, not a guess, and write the measured value in the description.
If you find a way to pass a scenario without the sound being right, that is
a bug in the scenario: report it and add the constraint that closes it.
