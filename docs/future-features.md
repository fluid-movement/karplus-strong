# Future Features

Read when… you are deciding what to build next, or you want the musical rationale behind a planned feature. This is a roadmap of ideas; rows marked **✅ Implemented** have shipped — see `dsp.md` / `parameters.md` for the current behavior and this file only for the original rationale. Everything else is still just an idea.

Goal: make the plugin a great-sounding, *musical* instrument that leans into the organic-but-synthetic character of Karplus-Strong. Features are grouped by theme; each lists the musical payoff, where it hooks into the current code, and rough effort (S/M/L).

## Implemented in the 2026-07 feature pass

Fractional delay interpolation, pitch-tracked sine exciter + harmonic control,
exciter tone filter, velocity→excitation-length, feedback-loop saturation
(drive), note-off damping (Ring/Damp mode), per-note humanization, and true
stereo voice spread. Details in `dsp.md` and `parameters.md`.

## Implemented in the second 2026-07 pass

Stiffness/inharmonicity dispersion, two new exciter types (Chirp, Velvet),
sympathetic resonance, and factory presets. Details in `dsp.md` and
`parameters.md`.

Not implemented — each needs a product/design decision this pass didn't make
(bend range and glide-time UX, which new exciter types and their controls,
detune amount and stereo image for coupled strings, body-filter design, strum
stagger amount, MPE/aftertouch mapping, randomize bounds): pitch bend/glide,
continuous-bow exciter, coupled detuned string pairs, body resonator, strum
mode, MPE/aftertouch, and randomize. These stay below as-is. The FX chain row
(drive → chorus → reverb) has been dropped from the roadmap entirely — not
deferred, removed by explicit decision.

## 1. Tuning & pitch (foundations)

These unlock almost everything else. Build first.

| Feature | Musical payoff | Hook | Effort |
|---|---|---|---|
| ✅ Fractional delay interpolation | Delay length is currently integer-truncated (`CircularBuffer::readDelayed`, `src/dsp/CircularBuffer.h:28`), so high notes are audibly out of tune. Allpass or Lagrange interpolation fixes intonation and is the prerequisite for vibrato, glide, and detune. Implemented once in `CircularBuffer`, both the string and the pick-model combs get it. | `src/dsp/CircularBuffer.h` | M |
| Pitch bend + glide/portamento | Expressive slides and bends — a huge part of real string playing. Smoothly ramp `delaySamples` instead of jumping. | `pitchWheelMoved` is empty at `src/dsp/KarplusStrongVoice.h:65` | M |
| ✅ Stiffness / inharmonicity | A single allpass dispersion filter in the feedback loop stretches upper partials: piano-, bell-, and sitar-like tones from one knob, scaling with `key_track`. | Ring phase in `KsDelayLine::processSample` | S |

## 2. Excitation (the pluck is half the sound)

| Feature | Musical payoff | Hook | Effort |
|---|---|---|---|
| ✅ Pitch-tracked sine exciter | The sine exciter is hardcoded to 440 Hz (`src/dsp/Exciter.h:96-102`). Tracking the played note (plus a harmonic-number control) turns it from a test tone into a playable timbre. | `Exciter::generateExcitation` | S |
| ✅ Exciter color filter | Lowpass/highpass on the burst = soft fingertip vs. hard plectrum. Cheap, immediately audible. | Filter the burst in `Exciter::processSample` | S |
| ✅ Chirp + Velvet exciter types | Chirp sweep (bouncing pick, pitch-tracked, reuses `excitation_length`) and velvet noise (breathier pluck, new `velvet_density` control). Continuous "bow" excitation is **not** included — it needs the exciter to sustain while the key is held, which reopens the "noteOff is a no-op" design (see `docs/gotchas.md`) and is a real architecture change, not a quick add. | `Exciter::generateExcitation` switch | S–M |
| ✅ Velocity → excitation length | Hard hits get longer bursts. Extends the existing velocity-mod pattern at `src/dsp/KsDelayLine.h:42-43`. (Color modulation not done — brightness-by-velocity already existed via `vel_brightness`; only length was new.) | `KarplusStrongDsp::noteOn` | S |

## 3. String character & realism

| Feature | Musical payoff | Hook | Effort |
|---|---|---|---|
| ✅ Feedback-loop saturation | Gentle `tanh` drive inside the loop makes overtones bloom and shift as notes decay — gut-string / sitar flavor. Cheapest big character win in the whole list. | Ring phase, `src/dsp/KsDelayLine.h:66-67` | S |
| Coupled / detuned string pairs | Two delay lines per note with slight detune and stereo spread: 12-string shimmer, piano-unison beating, chorus without a chorus pedal. | Second `KsDelayLine` in `src/dsp/KarplusStrongDsp.h` | M |
| ✅ Sympathetic resonance | Held/ringing voices receive a small unweighted bleed of every other voice's output and ring along — open-tuning drone magic, instant "instrument in a room" feel. One block of latency for the cross-voice reaction. | Per-voice output scratch + `KarplusStrongProcessor::updateSympatheticBleed`, injected into `KsDelayLine::processSample` | L |
| Body resonator | A small fixed comb/modal filter bank after the string (guitar/violin body) turns "string in a vacuum" into an instrument. A body-size/type control is very musical. | Post-synth processing in `processBlock` | M–L |

## 4. Performance & expressivity

| Feature | Musical payoff | Hook | Effort |
|---|---|---|---|
| ✅ Damping on note-off (Ring/Damp mode) | Notes currently always ring out (`stopNote` at `src/dsp/KarplusStrongVoice.h:38-47`; deliberate design, see `docs/gotchas.md`). A Damp mode with a release-damping time gives palm mutes and tight staccato playing. | `KsDelayLine::noteOff` `src/dsp/KsDelayLine.h:56` | M |
| ✅ Per-note humanization | Tiny random detune, brightness, and pick-position variance per `noteOn` kills the machine-gun effect on repeated notes. One "Human" knob. | `KarplusStrongDsp::noteOn` | S |
| Strum mode | Millisecond-scale onset stagger across simultaneous chord notes — chords stop sounding quantized. | MIDI handling in `processBlock` | M |
| MPE / poly aftertouch → brightness & damping | Pressing into a note to brighten or choke it. The damping filter is already per-voice, so this is a natural fit. | `controllerMoved` / aftertouch, `src/dsp/KarplusStrongVoice.h:66` | M–L |

## 5. Space & output

| Feature | Musical payoff | Hook | Effort |
|---|---|---|---|
| ✅ True stereo (per-voice pan spread) | Output is currently mono copied to both channels (`src/dsp/KarplusStrongVoice.h:54-55`). Spreading voices across the field makes chords wide and alive. | `KarplusStrongVoice::renderNextBlock` | S |

## 6. Workflow

| Feature | Musical payoff | Hook | Effort |
|---|---|---|---|
| ✅ Factory presets + browser | Sounds people can play in the first ten seconds. State save/load already works via APVTS XML; presets are curated states applied via `setValueNotifyingHost`. | `src/Presets.h`, preset `ComboBox` in `PluginEditor.cpp` | S |
| Randomize ("new string") button | Musically-bounded randomization for sound discovery — this synth's parameter space is full of happy accidents. | Editor button writing to `apvts` | S |

## Suggested order

1. **Fractional delay** — foundation; nothing pitch-related works well without it.
2. **Loop saturation + pitch-tracked sine exciter** — two small changes, big character win.
3. **Note-off damping + humanization** — playability.
4. **True stereo** — width and richness.
5. **Dispersion + sympathetic resonance + new exciter types** — deep realism tier.
6. **Presets** — polish and shareability.

Keep the doc updated: mark features implemented (and move their details into `dsp.md` / `parameters.md`) rather than deleting them, so the rationale history stays.
