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
| `CircularBuffer.h` | delay buffer: `prepare(size)`, `clear()`, `write(x)`, `readDelayed(d)` (integer read, clamped `[1, size-1]`) |
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
  ├── delayed = buffer.readDelayed(delaySamples)
  ├── if excitation != 0:  input = excitation            (burst phase)
  │   else:                input = dcBlocker(lowpass(delayed)) * feedbackGain  (ring phase)
  ├── buffer.write(input)
  ├── output = delayed * velocity * params.outputLevel
  ├── silence.track(output)
  └── return output
```

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
`modT = decayTime · (1 + velDecay·(vel−0.5))`.

## KarplusStrongDsp — `src/dsp/KarplusStrongDsp.h`

Facade composing `Exciter` + `KsDelayLine`. No JUCE dependency.

| Method | Line | Purpose |
|---|---|---|
| `prepare(sampleRate)` | `:10` | forwards to both children |
| `reset()` | `:16` | forwards to both children |
| `setParameters(const KsParams&)` | `:22` | forwards the struct to both children |
| `noteOn(freq, velocity)` | `:28` | `delaySamp = delayLine.getSampleRate() / freq`; seed exciter, init delay line |
| `noteOff(allowTailOff)` | `:35` | forwards to `delayLine.noteOff` — **currently a no-op** (see gotchas) |
| `isSilent()` | `:40` | returns `delayLine.isSilent()` |
| `processSample()` | `:45` | pull excitation (0 after burst), feed to delay line, return output |

Key: `noteOn` computes delay length from the **delay line's** sample rate
(not a hardcoded constant). Don't reintroduce `44100.0`.

## Exciter — `src/dsp/Exciter.h`

Generates the excitation burst and routes it through a pick model. Owns three
`CircularBuffer`s used only by pick models 1 and 2, each sized
`sampleRate * ks::maxDelaySeconds` in `prepare` (`:13`).

### Excitation types (`excitationType` int)

| Value | Name | Generator |
|---|---|---|
| 0 | Noise | xorshift32 RNG → `[-1, 1]` |
| 1 | Sine | `sin(phase)`, 440 Hz fixed (`:96-102` — pitch tracking is a future feature) |
| 2 | Dust | xorshift32 → `±1` impulse |

### Pick models (`pickModel` int) — in `processSample()` (`:50`)

| Value | Name | Behavior |
|---|---|---|
| 0 | Off | returns raw excitation |
| 1 | Comb | pre-delay by `pickPosition * delaySamples` samples (`:63-69`) |
| 2 | Two-delay | avg of two paths: `delaySamples+pos2` and `max(1, delaySamples-pos2)` (`:71-81`) |

`setParameters(const KsParams&)` (`:31`) reads the exciter-side fields and
clamps `excitationLength` to `[1, 1000]`. `delaySamples` is the **string
period in samples**, passed in via `noteOn(delaySamples)` (`:39`) from
`KarplusStrongDsp::noteOn`.

## KsDelayLine — `src/dsp/KsDelayLine.h`

The KS feedback loop: `CircularBuffer` → `OnePoleLowpass` → `DcBlocker` →
feedback gain. Holds a `KsParams` copy, a `SilenceDetector`, and the per-note
state (`velocity`, `delaySamples`, `feedbackGain`, `cutoffHz`).

| Method | Line | Notes |
|---|---|---|
| `prepare(sampleRate)` | `:15` | sizes buffer to `sampleRate * ks::maxDelaySeconds`; prepares DC blocker |
| `getSampleRate()` | `:22` | used by `KarplusStrongDsp::noteOn` |
| `reset()` | `:24` | clears buffer, filter states, silence detector |
| `setParameters(const KsParams&)` | `:32` | stores the struct |
| `noteOn(freq, velocity, excLen)` | `:37` | velocity mod → `ks::computeCutoffHz`/`ks::computeFeedbackGain`; arms the silence detector with `delaySamples + excLen + ks::silenceGuardSamples` |
| `noteOff(allowTailOff)` | `:56` | **no-op** (kept for future exciters — see gotchas) |
| `isSilent()` | `:58` | delegates to `SilenceDetector` |
| `processSample(excitation)` | `:60` | the core loop (see signal flow above) |

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
