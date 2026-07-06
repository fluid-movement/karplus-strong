# Parameters

Read when adding, renaming, or removing an APVTS parameter.

All parameters are declared in the `apvts` initializer list in
`src/PluginProcessor.cpp` (top of the constructor). Order below matches that
file. Their atomics are cached once in the constructor; each block
`updateVoiceParameters()` fills a `KsParams` struct (`src/dsp/KsParams.h`)
and pushes it to every voice.

## Parameter table

| ID | Display name | Type | Range | Default | Defined at | Sets (voice side) |
|---|---|---|---|---|---|---|
| `excitation` | Excitation Type | Choice | Noise/Sine/Dust/Chirp/Velvet | Noise (0) | `PluginProcessor.cpp:11` | `Exciter::excitationType` |
| `excitation_length` | Excitation Length | Float | 1 – 1000 | 100 | `:14` | `Exciter::excitationLength` (clamped) |
| `pick_position` | Pick Position | Float | 0 – 0.5 | 0.2 | `:17` | `Exciter::pickPosition` |
| `pick_model` | Pick Model | Choice | Off/Comb/Two-delay | Off (0) | `:20` | `Exciter::pickModel` |
| `sine_harmonic` | Sine Harmonic | Int | 1 – 8 | 1 | `:22-23` | `Exciter::sineHarmonic` — multiplies the tracked note frequency for the Sine excitation type |
| `exciter_tone` | Exciter Tone | Float | -1 – 1 | 0.0 (clean) | `:24-26` | `Exciter::exciterTone` — bipolar: center clean, left lowpasses, right highpasses the burst (cutoff from `ks::computeCutoffHz`, same mapping as `brightness`); filter is skipped entirely within `ks::exciterFilterDeadzone` of center |
| `vel_excitation_length` | Velocity->Length | Float | 0 – 1 | 0 | `:27-29` | `Exciter::velExcitationLength` → modulates the note's burst length |
| `velvet_density` | Velvet Density | Float | 50 – 8000 (skew centre 1500) | 2000 | `:30-33` | `Exciter::velvetDensity` → grain length for the Velvet excitation type |
| `decay_time` | Decay Time | Float | 0.5 – 40 s (skew centre 8 s) | 4.0 | `:34-40` | `KsDelayLine::decayTime` → time-to-quiet in seconds |
| `key_track` | Key Track | Float | 0.0 – 1.0 | 0.0 | `:41-43` | `KsDelayLine::keyTrack` → compensation across keyboard (decay **and** stiffness) |
| `brightness` | Brightness | Float | 0 – 1 | 0.5 | `:44-46` | `KsDelayLine::brightness` → `cutoffHz` |
| `vel_brightness` | Velocity->Brightness | Float | 0 – 1 | 0 | `:47-49` | `KsDelayLine::velBrightnessAmt` |
| `vel_decay` | Velocity->Decay | Float | 0 – 1 | 0 | `:50-52` | `KsDelayLine::velDecayAmt` |
| `drive` | Drive | Float | 0 – 1 | 0 | `:53-55` | `KsDelayLine::drive` → `ks::applySaturation` amount in the feedback loop |
| `stiffness` | Stiffness | Float | 0 – 1 | 0 | `:56-58` | `KsDelayLine::stiffness` → `ks::computeAllpassCoeff` amount for the dispersion allpass; filter is skipped entirely at 0 |
| `damp_mode` | Damp Mode | Choice | Ring/Damp | Ring (0) | `:59-61` | `KsDelayLine::dampMode` — Ring is the original no-op `noteOff`; Damp arms a release envelope |
| `release_time` | Release Time | Float | 0.01 – 2 s | 0.3 | `:62-67` | `KsDelayLine::releaseTime` — only used in Damp mode |
| `humanize` | Humanize | Float | 0 – 1 | 0 | `:68-70` | `KarplusStrongDsp`/`KsDelayLine`/`Exciter` — per-note detune, brightness, and pick-position jitter |
| `output_level` | Output Level | Float | 0 – 1 | 0.8 | `:71-73` | `KsDelayLine::outputLevel` |
| `voices` | Voices | Int | 1 – 16 | 8 | `:74-75` | `PluginProcessor::currentNumVoices` (triggers `updateNumVoices`) |
| `stereo_spread` | Stereo Spread | Float | 0 – 1 | 0 | `:76-78` | **Special** — not part of `KsParams`; read directly in `updateVoicePans()`, sets `KarplusStrongVoice::pan` per voice index |
| `sympathy` | Sympathy | Float | 0 – 1 | 0 | `:79-81` | **Special** — not part of `KsParams`; read once per block in `updateSympatheticBleed()`, folded into each voice's per-sample `sympatheticInput` (see `dsp.md`) |

## How each param flows to the DSP

```
decayTimeParam->load()  (cached std::atomic<float>*)
  → updateVoiceParameters() builds KsParams   src/PluginProcessor.cpp:144
  → voice->setParameters(params)               :168-169
  → dsp.setParameters(params)                  src/dsp/KarplusStrongVoice.h
  → delayLine.setParameters(params)            src/dsp/KsDelayLine.h
    exciter.setParameters(params)              src/dsp/Exciter.h
```

Every parameter takes the same path; each consumer reads only the `KsParams`
fields it owns.

The `voices` parameter is special: it's read in `processBlock` and passed to
`updateNumVoices`, which only toggles each preallocated voice's `enabled`
flag — see `voice-and-synth.md`. `stereo_spread` is likewise special and
**not** part of `KsParams`: it's read in `processBlock` and passed to
`updateVoicePans()` (`src/PluginProcessor.cpp`), which computes a pan
(`-1..1`) per voice index — spread across the fixed `maxVoices` denominator,
not the current `voices` count, so changing polyphony doesn't reshuffle the
stereo image — and calls `voice->setPan(pan)` directly
(`src/dsp/KarplusStrongVoice.h`). `KarplusStrongVoice::renderNextBlock` then
applies linear left/right gains derived from that pan; at `pan == 0` both
gains are `1.0`, so `stereo_spread == 0` (the default) is bit-identical to
the old always-mono-copy behavior.

`sympathy` is the same "special" category, one step further removed: it's
read once per block in `updateSympatheticBleed()` and combined with every
voice's previous-block output (captured via `KarplusStrongVoice::getOutputScratch()`)
to compute a per-sample `sympatheticInput` array pushed into each voice with
`getSympatheticInputBuffer()`, consumed by `KsDelayLine::processSample`'s
second argument — never touching `KsParams` at all. See `dsp.md` and
`voice-and-synth.md` for the full mechanics and why it can't be a `KsParams`
field.

## To add a parameter

1. **`src/PluginProcessor.cpp`** (apvts initializer, top of the constructor) — add a `std::make_unique<juce::AudioParameterFloat/Choice/Int>(...)`. Pick the `ParameterID` string; it's the canonical id everywhere.
2. **`src/PluginProcessor.h`** + constructor — add and cache the raw-value pointer.
3. **`src/dsp/KsParams.h`** — add the field (with the APVTS default).
4. **`src/PluginProcessor.cpp:84`** — set the field in `updateVoiceParameters()`.
5. **`src/dsp/KsDelayLine.h`** or **`src/dsp/Exciter.h`** — consume the field where it matters.
6. **`src/PluginEditor.cpp:23-34`** — one `addControl(...)` line (combo vs slider is derived from the param type).
7. **`tests/test_dsp.cpp`** — add a unit test exercising the new param.
8. **Update this file.**

If the param needs processor-level knowledge a single voice's `KsParams`
can't carry (e.g. other voices' audio, like `sympathy`; or a voice index,
like `stereo_spread`), skip steps 3–5 and instead consume it directly in
`PluginProcessor.cpp` (see `updateVoicePans()` / `updateSympatheticBleed()`
for the pattern) — do not force it into `KsParams` just for consistency.

## Preset content

`src/Presets.h` holds the factory preset list (`getFactoryPresets()`) and
`applyPreset()`, which sets every parameter explicitly via
`setValueNotifyingHost` — each preset lists **every** param, not just the
ones that differ from default, so switching presets never leaves a stray
value behind from whatever was selected before. Adding a parameter above
means adding one more entry to every preset in `src/Presets.h` too.

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
| sine_harmonic | 1 | fundamental by default |
| exciter_tone | 0.0 | dead-center — filter fully off, identical to pre-feature sound |
| vel_excitation_length | 0 | no length modulation by default |
| drive | 0 | bypass — `applySaturation` is a no-op at 0 |
| damp_mode | Ring (0) | preserves the original "rings out per decay" design |
| release_time | 0.3 s | only audible in Damp mode |
| humanize | 0 | fully deterministic by default |
| stereo_spread | 0 | mono-centered, matching the pre-feature always-mono-copy output |
| output_level | 0.8 | leaves headroom |
| voices | 8 | standard polyphony |
| velvet_density | 2000 | typical velvet-noise pulse rate from the literature; only audible when `excitation` = Velvet |
| stiffness | 0 | bypass — allpass is skipped entirely, identical to pre-feature sound |
| sympathy | 0 | no cross-voice bleed by default |