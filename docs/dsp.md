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
| `DcBlocker.h` | one-pole DC blocker (~`ks::dcBlockerHz` = 5 Hz) |
| `SilenceDetector.h` | peak tracker with halving window + min-samples guard |
| `Exciter.h` | burst generation (noise/sine/dust) + pick-model routing |
| `KsDelayLine.h` | the KS feedback loop topology |
| `KarplusStrongDsp.h` | facade: one Exciter + one KsDelayLine |

## Signal flow per sample

```
KarplusStrongDsp::processSample()           src/dsp/KarplusStrongDsp.h:45
  │
  ├── exciter.isActive()? ──► exciter.processSample()  (nonzero during burst)
  │                            else 0.0f
  │
  ▼
delayLine.processSample(excitation)         src/dsp/KsDelayLine.h:60
  │
  ├── delayed = buffer.readDelayed(delaySamples)          (linear-interpolated)
  ├── if excitation != 0:  input = excitation                                (burst phase)
  │   else:                input = saturate(dcBlocker(lowpass(delayed)), drive) * feedbackGain  (ring phase)
  ├── if damping (Damp mode, after noteOff): input *= releaseGain; releaseGain *= releaseCoeff
  ├── buffer.write(input)
  ├── output = delayed * velocity * params.outputLevel * (releaseGain if damping)
  ├── silence.track(output)
  └── return output
```

`KarplusStrongDsp::noteOn` applies the `humanize` detune (see below) to `frequency`
before deriving `delaySamp` and calling into `exciter`/`delayLine`, so the
exciter's pitch tracking and the string's tuning always agree.

## Calibration — `src/dsp/KsCalibration.h`

All magic numbers live here as named constants (`ks::maxFeedbackGain`,
`ks::silenceThreshold`, `ks::minCutoffHz`, `ks::maxDelaySeconds`, …).
The two pure functions are directly unit-tested (`tests/test_dsp.cpp`,
`[calibration]` tag):

| Function | Formula |
|---|---|
| `computeCutoffHz(brightness)` | `200 + clamp(brightness, 0, 1) * 11800` |
| `computeFeedbackGain(decayTime, freq, delaySamples, keyTrack, cutoffHz, sr)` | `clamp(exp(-ln1000·effDelay/(decayTime·sr)) · sqrt(1+(freq/cutoffHz)²), 0, 0.99999)` where `effDelay = refDelay + keyTrack·(delaySamples − refDelay)`, `refDelay = sr/440` |

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
| `processSample()` | `:45` | pull excitation (0 after burst), feed to delay line, return output |

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

The generated sample is then passed through the tone filter (see below) before
pick-model routing.

### Exciter tone filter

`exciterTone` (0–1, default 1 = bypass) shapes the burst's brightness before
the pick model, independent of the string's `brightness` control — soft
fingertip (low tone) vs. hard plectrum (high tone). At `exciterTone >= 0.999`
the filter is skipped entirely (`toneFilterActive = false`) so the default
sound is bit-identical to no filter. Below that, `noteOn` sets the filter's
cutoff to `ks::computeCutoffHz(exciterTone)` (same mapping as `brightness`)
and resets it, so it starts clean each note.

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

The KS feedback loop: `CircularBuffer` → `OnePoleLowpass` → `DcBlocker` →
saturation → feedback gain. Holds a `KsParams` copy, a `SilenceDetector`, and
the per-note state (`velocity`, `delaySamples`, `feedbackGain`, `cutoffHz`,
damping envelope).

| Method | Line | Notes |
|---|---|---|
| `prepare(sampleRate)` | `:15` | sizes buffer to `sampleRate * ks::maxDelaySeconds`; prepares DC blocker |
| `getSampleRate()` | `:22` | used by `KarplusStrongDsp::noteOn` |
| `reset()` | `:24` | clears buffer, filter states, silence detector, damping envelope |
| `setParameters(const KsParams&)` | `:32` | stores the struct |
| `noteOn(freq, velocity, excLen)` | `:37` | velocity mod → `ks::computeCutoffHz`/`ks::computeFeedbackGain`; jitters brightness by `humanize` first; arms the silence detector with `delaySamples + excLen + ks::silenceGuardSamples` |
| `noteOff(allowTailOff)` | `:56` | no-op in Ring mode (`dampMode == 0`, default — unchanged behavior). In Damp mode (`dampMode == 1`) with `allowTailOff`, arms a release envelope: `releaseGain = 1`, `releaseCoeff = ks::computeReleaseCoeff(releaseTime, sampleRate)` |
| `isSilent()` | `:58` | delegates to `SilenceDetector` |
| `processSample(excitation)` | `:60` | the core loop (see signal flow above); ring-phase input passes through `ks::applySaturation(·, drive)`; while damping, both the feedback input and the output are multiplied by the decaying `releaseGain` |

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
