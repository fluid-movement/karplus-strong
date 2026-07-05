# Parameters

Read when adding, renaming, or removing an APVTS parameter.

All parameters are declared in the `apvts` initializer list in
`src/PluginProcessor.cpp:6-44`. Order below matches that file. Their atomics
are cached once in the constructor (`:46-56`); each block
`updateVoiceParameters()` (`:84`) fills a `KsParams` struct
(`src/dsp/KsParams.h`) and pushes it to every voice (`:99-101`).

## Parameter table

| ID | Display name | Type | Range | Default | Defined at | Sets (voice side) |
|---|---|---|---|---|---|---|
| `excitation` | Excitation Type | Choice | Noise/Sine/Dust | Noise (0) | `PluginProcessor.cpp:9` | `Exciter::excitationType` |
| `excitation_length` | Excitation Length | Float | 1 – 1000 | 100 | `:12` | `Exciter::excitationLength` (clamped) |
| `pick_position` | Pick Position | Float | 0 – 0.5 | 0.2 | `:15` | `Exciter::pickPosition` |
| `pick_model` | Pick Model | Choice | Off/Comb/Two-delay | Off (0) | `:18` | `Exciter::pickModel` |
| `decay_time` | Decay Time | Float | 0.5 – 40 s (skew centre 8 s) | 4.0 | `:21` | `KsDelayLine::decayTime` → time-to-quiet in seconds |
| `key_track` | Key Track | Float | 0.0 – 1.0 | 0.0 | `:28` | `KsDelayLine::keyTrack` → compensation across keyboard |
| `brightness` | Brightness | Float | 0 – 1 | 0.5 | `:31` | `KsDelayLine::brightness` → `cutoffHz` |
| `vel_brightness` | Velocity->Brightness | Float | 0 – 1 | 0 | `:34` | `KsDelayLine::velBrightnessAmt` |
| `vel_decay` | Velocity->Decay | Float | 0 – 1 | 0 | `:37` | `KsDelayLine::velDecayAmt` |
| `output_level` | Output Level | Float | 0 – 1 | 0.8 | `:40` | `KsDelayLine::outputLevel` |
| `voices` | Voices | Int | 1 – 16 | 8 | `:43` | `PluginProcessor::currentNumVoices` (triggers `updateNumVoices`) |

## How each param flows to the DSP

```
decayTimeParam->load()  (cached std::atomic<float>*)
  → updateVoiceParameters() builds KsParams   src/PluginProcessor.cpp:84
  → voice->setParameters(params)               :99-101
  → dsp.setParameters(params)                  src/dsp/KarplusStrongVoice.h:22
  → delayLine.setParameters(params)            src/dsp/KsDelayLine.h:32
    exciter.setParameters(params)              src/dsp/Exciter.h:31
```

Every parameter takes the same path; each consumer reads only the `KsParams`
fields it owns.

The `voices` parameter is special: it's read in `processBlock` (`:79`) and
passed to `updateNumVoices` (`:103`), which only toggles each preallocated
voice's `enabled` flag — see `voice-and-synth.md`.

## To add a parameter

1. **`src/PluginProcessor.cpp:6-44`** — add a `std::make_unique<juce::AudioParameterFloat/Choice/Int>(...)` to the apvts initializer. Pick the `ParameterID` string; it's the canonical id everywhere.
2. **`src/PluginProcessor.h:48-58`** + constructor (`:46-56`) — add and cache the raw-value pointer.
3. **`src/dsp/KsParams.h`** — add the field (with the APVTS default).
4. **`src/PluginProcessor.cpp:84`** — set the field in `updateVoiceParameters()`.
5. **`src/dsp/KsDelayLine.h`** or **`src/dsp/Exciter.h`** — consume the field where it matters.
6. **`src/PluginEditor.cpp:23-34`** — one `addControl(...)` line (combo vs slider is derived from the param type).
7. **`tests/test_dsp.cpp`** — add a unit test exercising the new param.
8. **Update this file.**

## To rename a parameter

The `ParameterID` string is the canonical id persisted in host state. Renaming
the id will break saved sessions. To rename safely: add the new id, keep the old
one for a release, map both. For a display-name-only change, just edit the
second arg to the param constructor — no state break.

## Defaults reference

| Param | Default | Why |
|---|---|---|
| decay_time | 4.0 s | musical pluck sustain; knob centre = 8 s (retuned — old 0.05–30 s range wasted 80% of travel on too-short decays) |
| key_track | 0.0 | no compensation by default |
| brightness | 0.5 | mid cutoff (~6100 Hz) |
| excitation_length | 100 | short noise burst |
| pick_position | 0.2 | near the bridge |
| output_level | 0.8 | leaves headroom |
| voices | 8 | standard polyphony |