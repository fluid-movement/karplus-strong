# DSP

Read when editing the DSP, adding an exciter, or changing the KS algorithm.

All classes are header-only in `src/dsp/`, pure C++, and unit-tested without
JUCE (see `tests/test_dsp.cpp`). The composition root is `KarplusStrongDsp`
(`src/dsp/KarplusStrongDsp.h`). Small single-purpose primitives are composed
into the two processing classes.

## File map

| File | Responsibility |
|---|---|
| `KsParams.h` | plain struct of all voice parameters; passed by const ref through the whole chain |
| `KsCalibration.h` | namespace `ks`: named constants + pure functions `computeCutoffHz` / `computeFeedbackGain` |
| `CircularBuffer.h` | delay buffer: `prepare(size)`, `clear()`, `write(x)`, `readDelayed(d)` (linearly-interpolated fractional read, clamped `[1, size-1]`) |
| `OnePoleLowpass.h` | RC lowpass; `setCutoff` computes alpha once (not per sample) |
| `AllpassFilter.h` | first-order allpass (`y[n] = -g·x[n] + x[n-1] + g·y[n-1]`); unity gain, phase-only — powers the stiffness/dispersion effect |
| `DcBlocker.h` | one-pole DC blocker (~`ks::dcBlockerHz` = 5 Hz) |
| `SilenceDetector.h` | peak tracker with halving window + min-samples guard |
| `Exciter.h` | burst generation (noise/sine/dust/chirp/velvet) + pick-model routing |
| `KsDelayLine.h` | the KS feedback loop topology |
| `KarplusStrongDsp.h` | facade: one Exciter + one KsDelayLine |

## Signal flow per sample

```
KarplusStrongDsp::processSample(sympatheticInput = 0)   src/dsp/KarplusStrongDsp.h
  │
  ├── exciter.isActive()? ──► exciter.processSample()  (nonzero during burst)
  │                            else 0.0f
  │
  ▼
delayLine.processSample(excitation, sympatheticInput)   src/dsp/KsDelayLine.h
  │
  ├── delayed = buffer.readDelayed(delaySamples)          (linear-interpolated)
  ├── if excitation != 0:  input = excitation                                (burst phase)
  │   else:                filtered = lowpass(delayed); if stiffness active: filtered = allpass(filtered)
  │                         input = saturate(dcBlocker(filtered), drive) * feedbackGain  (ring phase)
  ├── if sympatheticInput != 0: input += saturate(sympatheticInput, ks::sympathySaturationAmount)
  ├── if damping (Damp mode, after noteOff): input *= releaseGain; releaseGain *= releaseCoeff
  ├── buffer.write(input)
  ├── output = delayed * velocity * params.outputLevel * (releaseGain if damping)
  ├── silence.track(output)
  └── return output
```

`KarplusStrongDsp::noteOn` applies the `humanize` detune (see below) to `frequency`
before deriving `delaySamp` and calling into `exciter`/`delayLine`, so the
exciter's pitch tracking and the string's tuning always agree.

`sympatheticInput` is a per-sample **argument**, not a `KsParams` field — see
the Sympathetic resonance section below for why, and `docs/parameters.md` for
where the `Sympathy` knob itself lives (it's a `stereo_spread`-style
processor-level concern, not part of `KsParams` either).

## Calibration — `src/dsp/KsCalibration.h`

All magic numbers live here as named constants (`ks::maxFeedbackGain`,
`ks::silenceThreshold`, `ks::minCutoffHz`, `ks::maxDelaySeconds`, …).
The two pure functions are directly unit-tested (`tests/test_dsp.cpp`,
`[calibration]` tag):

| Function | Formula |
|---|---|
| `computeCutoffHz(brightness)` | `200 + clamp(brightness, 0, 1) * 11800` |
| `computeFeedbackGain(decayTime, freq, delaySamples, keyTrack, cutoffHz, sr)` | `clamp(exp(-ln1000·effDelay/(decayTime·sr)) · sqrt(1+(freq/cutoffHz)²), 0, 0.99999)` where `effDelay = refDelay + keyTrack·(delaySamples − refDelay)`, `refDelay = sr/440` |
| `computeAllpassCoeff(stiffness, delaySamples, keyTrack, sr)` | `clamp(stiffness · maxAllpassCoeff · registerScale, -maxAllpassCoeff, maxAllpassCoeff)` where `registerScale = clamp(refDelay/effDelay, 0.25, 4)`, reusing the same `refDelay`/`effDelay` as `computeFeedbackGain` — at `keyTrack=0` the coefficient is flat across the keyboard; at `keyTrack=1` higher notes get more dispersion (matches how real stiff strings show more inharmonicity on shorter/higher strings) |

The `sqrt(1+(f/fc)²)` factor cancels the lowpass's loss at the fundamental so
dialed decay seconds match perceived decay (see `gotchas.md` trap #6). The DC
blocker in the ring phase prevents the loop holding DC forever when
`feedbackGain` approaches 1.

Velocity modulation happens in `KsDelayLine::noteOn` (`src/dsp/KsDelayLine.h:37`)
before calling these: `modBright = brightness + velBrightness·(vel−0.5)`,
`modT = decayTime · (1 + velDecay·(vel−0.5))`. `Exciter::noteOn` applies the
analogous `modLength = excitationLength · (1 + velExcitationLength·(vel−0.5))`.

Two more pure helpers support the drive/damp features, both `[calibration]`-tagged:

| Function | Formula |
|---|---|
| `applySaturation(x, amount)` | bypass at `amount<=0`; else `tanh(x·k)/tanh(k)` where `k = 1 + amount·driveMaxK` — unity-gain at `x=1`, boosts small signals (added harmonics) |
| `computeReleaseCoeff(releaseTime, sampleRate)` | `silenceEnvelope^(1/(releaseTime·sampleRate))` — the per-sample multiplier that decays a `1.0` envelope to `ks::silenceEnvelope` (1e-3) over `releaseTime` seconds |

Humanize jitter constants: `humanizeMaxDetune` (±2% frequency), `humanizeMaxBrightness` (±0.1 on the 0–1 brightness scale), `humanizeMaxPickJitter` (±0.05 on pick position). All are `humanize` (0–1) scaled and applied via a per-class xorshift RNG seeded once at construction (not reseeded per note) — each `noteOn` draws fresh jitter without needing new state.

Sympathetic-resonance safety constants: `sympathyMaxGain` (0.15 — caps the `Sympathy` 0–1 knob's effective injection well below unity coupled-loop-gain even at max polyphony/drive/decay) and `sympathySaturationAmount` (1.0 — the fixed `applySaturation` amount always applied to the injected cross-voice signal, independent of the `Drive` knob; see the Sympathetic resonance section below for why this is load-bearing, not optional). `chirpSweepRatio` (6.0) sets how many times above the fundamental the Chirp exciter starts its downward sweep.

## KarplusStrongDsp — `src/dsp/KarplusStrongDsp.h`

Facade composing `Exciter` + `KsDelayLine`. No JUCE dependency.

| Method | Line | Purpose |
|---|---|---|
| `prepare(sampleRate)` | `:10` | forwards to both children |
| `reset()` | `:16` | forwards to both children |
| `setParameters(const KsParams&)` | `:22` | forwards the struct to both children |
| `noteOn(freq, velocity)` | `:28` | applies `humanize` detune to `freq`; `delaySamp = delayLine.getSampleRate() / freq`; seed exciter, init delay line |
| `noteOff(allowTailOff)` | `:35` | forwards to `delayLine.noteOff` — no-op unless Damp mode (see below and gotchas) |
| `isSilent()` | `:40` | returns `delayLine.isSilent()` |
| `processSample(sympatheticInput = 0)` | `:45` | pull excitation (0 after burst), feed to delay line along with `sympatheticInput`, return output |

Key: `noteOn` computes delay length from the **delay line's** sample rate
(not a hardcoded constant). Don't reintroduce `44100.0`.

## Exciter — `src/dsp/Exciter.h`

Generates the excitation burst and routes it through a pick model. Owns three
`CircularBuffer`s used only by pick models 1 and 2, each sized
`sampleRate * ks::maxDelaySeconds` in `prepare` (`:13`), plus a `OnePoleLowpass`
tone filter.

### Excitation types (`excitationType` int)

| Value | Name | Generator |
|---|---|---|
| 0 | Noise | xorshift32 RNG → `[-1, 1]` |
| 1 | Sine | `sin(phase)`, phase increments by `2π · frequency · sineHarmonic / sampleRate` — tracks the played note, `sineHarmonic` (1–8) selects which harmonic |
| 2 | Dust | xorshift32 → `±1` impulse |
| 3 | Chirp | `sin(phase)` with a time-varying instantaneous frequency: `instFreq = frequency · chirpSweepRatio^(1 - progress)`, `progress = 1 - burstRemaining/noteExcitationLength`. Sweeps from `frequency·6` down to `frequency` over the burst — no new param, reuses `excitation_length` for duration and the existing pitch tracking for range |
| 4 | Velvet | Sparse velvet-noise impulses: time is divided into grains of `sampleRate/velvetDensity` samples; each grain emits exactly one pseudo-random-position, pseudo-random-sign `±1` pulse, zero elsewhere. `velvetDensity` (new param, pulses/sec) controls grain length; recomputed in `reset()` |

The generated sample is then passed through the tone filter (see below) before
pick-model routing.

### Exciter tone filter

`exciterTone` (−1 to 1, default 0 = clean) is a bipolar tone knob shaping the
burst's brightness before the pick model, independent of the string's
`brightness` control: center is unfiltered, turning it left engages a
lowpass (soft fingertip), turning it right engages a highpass (hard
plectrum). At `|exciterTone| <= ks::exciterFilterDeadzone` (0.01) the filter
is skipped entirely (`toneFilterActive = false`), so the default is
bit-identical to no filter.

Away from center, `noteOn` sets `toneFilterIsHighpass = exciterTone > 0` and
picks the cutoff from the same `ks::computeCutoffHz` mapping used by
`brightness`:
- left half: `computeCutoffHz(1 + exciterTone)` — cutoff falls from ~12 kHz
  near center to `ks::minCutoffHz` (200 Hz) at full left (heavier lowpass).
- right half: `computeCutoffHz(exciterTone)` — cutoff rises from ~200 Hz near
  center to ~12 kHz at full right (heavier highpass).

There's only one one-pole filter (`toneFilter`, a `OnePoleLowpass`). The
highpass side doesn't need a second filter class: it reuses the same lowpass
and takes the complementary output, `excitation - toneFilter.process
(excitation)` — the standard one-pole "leaky differentiator" highpass, which
shares its −3 dB point exactly with the lowpass at the same cutoff.

### Velocity → excitation length

`noteOn(delaySamples, frequency, velocity)` computes
`modLength = excitationLength · (1 + velExcitationLength·(vel−0.5))`, clamps
to `[1, 1000]`, and uses that as the note's actual burst length
(`getExcitationLength()` returns this per-note value, not the raw param —
call `noteOn` before reading it).

### Per-note humanization (pick position)

If `humanize > 0`, `noteOn` jitters the pick position used for that note by
up to `± humanize · ks::humanizeMaxPickJitter`, clamped to `[0, 0.5]`. The raw
`pickPosition` param is untouched; only the note's effective position moves.

### Pick models (`pickModel` int) — in `processSample()` (`:50`)

| Value | Name | Behavior |
|---|---|---|
| 0 | Off | returns raw excitation |
| 1 | Comb | pre-delay by `pickPositionForNote * delaySamples` samples |
| 2 | Two-delay | avg of two paths: `delaySamples+pos2` and `max(1, delaySamples-pos2)` |

`setParameters(const KsParams&)` reads the exciter-side fields and clamps
`excitationLength` to `[1, 1000]`. `delaySamples` is the **string period in
samples**, passed in via `noteOn(delaySamples, frequency, velocity)` from
`KarplusStrongDsp::noteOn`.

## KsDelayLine — `src/dsp/KsDelayLine.h`

The KS feedback loop: `CircularBuffer` → `OnePoleLowpass` → `AllpassFilter`
(if stiffness active) → `DcBlocker` → saturation → feedback gain →
sympathetic input. Holds a `KsParams` copy, a `SilenceDetector`, and the
per-note state (`velocity`, `delaySamples`, `feedbackGain`, `cutoffHz`,
allpass coefficient, damping envelope).

| Method | Line | Notes |
|---|---|---|
| `prepare(sampleRate)` | `:15` | sizes buffer to `sampleRate * ks::maxDelaySeconds`; prepares DC blocker |
| `getSampleRate()` | `:22` | used by `KarplusStrongDsp::noteOn` |
| `reset()` | `:24` | clears buffer, filter states (including allpass), silence detector, damping envelope |
| `setParameters(const KsParams&)` | `:32` | stores the struct |
| `noteOn(freq, velocity, excLen)` | `:37` | velocity mod → `ks::computeCutoffHz`/`ks::computeFeedbackGain`; jitters brightness by `humanize` first; sets the allpass coefficient via `ks::computeAllpassCoeff` if `stiffness > 0`; arms the silence detector with `delaySamples + excLen + ks::silenceGuardSamples` |
| `noteOff(allowTailOff)` | `:56` | no-op in Ring mode (`dampMode == 0`, default — unchanged behavior). In Damp mode (`dampMode == 1`) with `allowTailOff`, arms a release envelope: `releaseGain = 1`, `releaseCoeff = ks::computeReleaseCoeff(releaseTime, sampleRate)` |
| `isSilent()` | `:58` | delegates to `SilenceDetector` |
| `processSample(excitation, sympatheticInput = 0)` | `:60` | the core loop (see signal flow above); ring-phase input passes through the allpass (if active) and `ks::applySaturation(·, drive)`; `sympatheticInput` (if nonzero) is separately saturated with a fixed `ks::sympathySaturationAmount` and added; while damping, both the feedback input and the output are multiplied by the decaying `releaseGain` |

### Stiffness / inharmonicity dispersion

`stiffness` (0–1, default 0) drives a single `AllpassFilter` inserted between
the lowpass and the DC blocker in the ring phase. At `stiffness == 0` the
filter is skipped entirely (`allpassActive = false`), so the default sound is
bit-identical to before this feature — **not** just "coefficient happens to be
zero": a first-order allpass with coefficient 0 reduces to `y[n] = x[n-1]`, a
pure one-sample delay, not identity, so the bypass has to be an explicit
branch (see `AllpassFilter::process`). The coefficient comes from
`ks::computeAllpassCoeff`, which reuses `keyTrack` the same way
`computeFeedbackGain` already does (see Calibration above). Because an
allpass is unity-gain by construction, no `feedbackGain` recompensation is
needed the way the lowpass required (trap #6 in `gotchas.md`) — stiffness
only changes phase, not decay time.

### Sympathetic resonance

`sympatheticInput` is deliberately a `processSample` **argument**, not a
`KsParams` field — same category as `excitation` (trap #3 in gotchas.md): a
different value every sample, computable only by the processor (it needs
every other voice's freshly-rendered audio, which no per-voice `KsParams`
snapshot could ever contain). It's injected unconditionally (burst and ring
phase alike — a sympathetically-coupled string keeps picking up neighbor
energy even mid-pluck) and, when nonzero, is passed through
`ks::applySaturation(sympatheticInput, ks::sympathySaturationAmount)` before
being added. That saturation call is **load-bearing, not optional**: the
injected signal bypasses each voice's own `drive`-controlled saturation
entirely (it's added after that stage), so without its own fixed bound the
cross-voice coupling is pure linear feedback between up to 16 voices with no
limiter — verified experimentally to diverge (samples reaching >1000 within
a few hundred blocks) at high `Sympathy` + `Drive` + `decay_time` + polyphony
before this fix was added. With the fixed saturation, per-sample writes are
bounded regardless of how many voices are bleeding into each other; see
`tests/test_integration.cpp` ("Sympathy stays bounded and finite…") for the
regression test. The `Sympathy` knob itself (0–1) is read once per block in
`PluginProcessor::updateSympatheticBleed` and folded into the per-sample
`sympatheticInput` value there — `KsDelayLine`/`KarplusStrongDsp` never know
what "Sympathy" is, they just add a number. See `docs/architecture.md` and
`docs/voice-and-synth.md` for the per-voice scratch-buffer/one-block-latency
mechanics that produce that number.

### Ring vs. Damp mode

`dampMode` (0 = Ring, 1 = Damp) and `releaseTime` (seconds, only meaningful in
Damp mode) are new `KsParams` fields. Ring mode preserves the original
"noteOff is a no-op, string rings out per `decay`" design exactly (see
gotchas.md). Damp mode is additive: it layers a release envelope on top of
the normal feedback loop rather than replacing any existing decay logic, and
only engages on a soft release (`allowTailOff == true`); a hard stop (voice
steal / all-notes-off) still goes through `KarplusStrongVoice`'s existing
`dsp.reset()` path.

### Silence semantics — `src/dsp/SilenceDetector.h`

```
isSilent() = (samplesTracked >= minSamples) AND (peakAmplitude < ks::silenceThreshold)
```

- `minSamples` (set per note) prevents silence-reporting before the first
  output even appears (string period + burst length + guard).
- `peakAmplitude` decays by half every `ks::peakDecayWindow` (256) samples,
  so once the feedback loop has quieted the signal, the voice ends.

## Adding a new excitation type

1. Add to the `excitation` choice param in `PluginProcessor.cpp:9` — the
   editor combo picks up the new choice automatically.
2. Add a `case` in `Exciter::generateExcitation()` — `src/dsp/Exciter.h:89`.
3. Add a unit test in `tests/test_dsp.cpp`.

## Adding a new pick model

1. Add to the `pick_model` choice param (`PluginProcessor.cpp:18`) — combo
   updates automatically.
2. Add a `case` in `Exciter::processSample()` — `src/dsp/Exciter.h:50`.
3. Pick models read `pickPosition * delaySamples` for comb spacing.
