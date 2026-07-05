# Gotchas

Read before any edit. This file documents traps we've already hit and
design decisions that must not regress. Each entry: Symptom → Root cause
→ Fix → file:line → Prevention rule.

## Traps (bug history)

### 1. Voices never added — no sound on note-on

- **Symptom:** Plugin loads, MIDI notes produce no output. Integration test
  reports `maxAmp=0`.
- **Root cause:** `currentNumVoices` was initialized to `8`. The constructor
  calls `updateNumVoices(...)`, but `updateNumVoices` early-returns when
  `newNumVoices == currentNumVoices` (`src/PluginProcessor.cpp:105`). With
  `currentNumVoices=8`, the early return skipped voice setup entirely.
- **Fix:** `src/PluginProcessor.h:46` — `int currentNumVoices = 0;`
- **Rule:** `currentNumVoices` MUST init to `0`. The constructor relies on
  the mismatch for the first `updateNumVoices` call to run. Since the
  voice-pool refactor, all 16 voices are preallocated in the constructor
  (`PluginProcessor.cpp:58-62`) and `updateNumVoices` only toggles per-voice
  `enabled` flags — the failure mode is now a wrong enabled count rather
  than silence, but the init rule stands.

### 2. Note-off killed the sound

- **Symptom:** Releasing a key immediately cut the string. No natural decay.
- **Root cause:** `stopNote` called `dsp.noteOff(allowTailOff)`, which in
  `KsDelayLine::noteOff` set `active = false` and started a `releaseGain`
  ramp (`0.999` per sample — ~50ms fade). The user wants the string to ring
  out per the `decay` feedback param, not be force-faded.
- **Fix:**
  - `src/dsp/KarplusStrongVoice.h:38-47` — `stopNote` no longer force-fades;
    `dsp.noteOff` is still called but is a no-op. (It initially also called
    `clearCurrentNote()` unconditionally — that caused trap #5; it now only
    clears on hard stop.)
  - `src/dsp/KsDelayLine.h:56` — `noteOff` is `{}`
  - output is `delayed * velocity * params.outputLevel` with no release gain
    (`:73`); the release ramp was removed entirely.
  - silence detection is peak-based only (`src/dsp/SilenceDetector.h`) —
    no early-return on any `active` flag.
- **Rule:** `noteOff` is a **no-op** by design. Do not add release-ramp logic
  back. The string rings out controlled by `feedbackGain` (from `decay`). The
  `noteOff` method is kept for future exciters (e.g. bowed string) that may
  need release behavior — extend it then, not now.
- **Tests:** `noteOff is a no-op — string rings out naturally`
  (`tests/test_dsp.cpp:165`) asserts output stays >0.01 one second after
  noteOff.

### 3. Double-write to delay line

- **Symptom:** Earlier (pre-refactor) — corrupted KS loop.
- **Root cause:** Both feedback and excitation were being written to the
  delay buffer per sample.
- **Fix:** `KsDelayLine::processSample` (`src/dsp/KsDelayLine.h:60`) writes
  `input` once per sample, where `input` is `excitation` if nonzero, else
  `dcblock(lowpass(delayed)) * feedbackGain`. `KarplusStrongDsp::processSample`
  (`src/dsp/KarplusStrongDsp.h:45`) pulls excitation from the exciter
  (0 after burst) and passes it in.
- **Rule:** One write per sample. `processSample` takes `excitation` as an
  arg and falls back to feedback when it's 0.

### 4. isSilent killed the voice before output

- **Symptom:** Earlier — voice ended immediately on noteOn.
- **Root cause:** `isSilent` returned true before the first delay-line
  output sample arrived, because `peakAmplitude` was still 0 and `active`
  was checked wrong.
- **Fix:** the `SilenceDetector` (`src/dsp/SilenceDetector.h`) is armed per
  note with `minSamples = delaySamples + excitationLen +
  ks::silenceGuardSamples` (`KsDelayLine::noteOn`,
  `src/dsp/KsDelayLine.h:52-53`) and cannot report silent before that many
  samples were tracked.
- **Rule:** Any `isSilent` check must include the min-samples guard. The
  voice must produce output for at least `delaySamples + burstLen + 256`
  samples before it can be considered silent.

### 5. Sequential notes always cut off the previous note's tail

- **Symptom:** Playing notes in sequence silenced the previous note's ringing
  tail the instant the next note started, regardless of the `voices` param.
- **Root cause:** The trap #2 fix made `stopNote` call `clearCurrentNote()`
  **unconditionally**. A released-but-ringing voice reported inactive, so
  `juce::Synthesiser::findFreeVoice` handed the same voice to the next
  noteOn, whose `KsDelayLine::noteOn` → `reset()` zeroed the delay buffer.
- **Fix:** `src/dsp/KarplusStrongVoice.h:38-47` — `stopNote` clears the note
  (and resets the DSP) only when `allowTailOff == false` (steal /
  all-notes-off). On normal release the voice stays busy; `renderNextBlock`
  (`:49-63`) frees it via `isSilent()` once the tail decays.
- **Rule:** A released voice MUST stay busy (`getCurrentlyPlayingNote() >= 0`)
  until `isSilent()`. Never `clearCurrentNote()` on a tail-off release.
  `voices` bounds simultaneous ringing tails by design.
- **Tests:** tail-preservation check in `tests/test_integration.cpp` —
  releases note 60, starts a near-silent note 96, asserts the tail keeps
  ≥30% of its amplitude.

### 6. Decay knob: bottom 80% of travel was musically useless

- **Symptom:** Everything below ~80% of the Decay Time knob's travel sounded
  too short to be musical; the usable zone was squeezed into the top 20%.
- **Root cause:** Three compounding factors.
  1. Range `0.05–30 s` with skew centre 1.5 s put half the travel below
     1.5 s.
  2. `feedbackGain` targeted −60 dB using the feedback multiplier alone,
     ignoring the energy the damping lowpass eats every pass — actual decay
     was substantially shorter than dialed.
  3. (Found while fixing) with feedback near 1, the loop held the excitation
     burst's DC component almost forever, since the lowpass passes DC — the
     voice never went silent at dark settings.
- **Fix:**
  - `src/PluginProcessor.cpp:20-26` — range `0.5–40 s`, skew centre 8 s,
    default 4.0 s.
  - `ks::computeFeedbackGain` (`src/dsp/KsCalibration.h`) multiplies the
    gain by `sqrt(1+(f/cutoffHz)²)` to cancel the lowpass loss at the
    fundamental (clamp `ks::maxFeedbackGain` still applies). Directly
    unit-tested (`[calibration]` tag).
  - `DcBlocker` (`src/dsp/DcBlocker.h`, ~5 Hz) in the feedback path
    (`src/dsp/KsDelayLine.h:67`) kills the DC hold.
- **Rule:** Any per-pass loss added to the feedback loop (filters, drive)
  must either be compensated in `feedbackGain` or accepted as a documented
  decay-shortener. Never remove the DC blocker while `feedbackGain` can
  approach 1. Changing the `decay_time` `NormalisableRange` remaps values in
  saved sessions — flag it in release notes.
- **Tests:** `Dialed decay time roughly matches time to silence`
  (`tests/test_dsp.cpp`).

## Design decisions (do not regress)

### noteOff is a no-op

Already covered in trap #2. `KsDelayLine::noteOff` (`src/dsp/KsDelayLine.h:56`)
and `KarplusStrongDsp::noteOff` (`src/dsp/KarplusStrongDsp.h:35`) are empty.
The string rings out per `decay`. Future exciters needing release behavior
(e.g. a damp/palm-mute mode) are expected to extend these — flag this in the
commit if you do. The old `releaseGain` dead-code field was deleted in the
2026-07 refactor; reintroduce release state fresh with the feature.

### The DSP layer has no JUCE dependency

Everything in `src/dsp/` except `KarplusStrongVoice.h` is pure C++ (only
`<cmath>`, `<vector>`, etc.): the primitives (`CircularBuffer.h`,
`OnePoleLowpass.h`, `DcBlocker.h`, `SilenceDetector.h`), `KsParams.h`,
`KsCalibration.h`, `Exciter.h`, `KsDelayLine.h`, `KarplusStrongDsp.h`.
This keeps the unit tests in `tests/test_dsp.cpp` fast and JUCE-free. Do not
`#include` any `juce_*` header in these files. The JUCE boundary is
`KarplusStrongVoice.h` (`src/dsp/KarplusStrongVoice.h:3`).

### `noteOn` computes delayLength from the delay line's sample rate

`src/dsp/KarplusStrongDsp.h:30`:
```cpp
float delaySamp = static_cast<float> (delayLine.getSampleRate()) / frequency;
```
Never `44100.0 / frequency` or any other hardcoded sample rate. The delay
line's sample rate is set in `prepare` (`KsDelayLine.h:15`) from the host's
`prepareToPlay`.

### Parameters travel as one `KsParams` struct

`src/dsp/KsParams.h` is the single carrier for all voice parameters:
processor fills it, everything downstream takes `const KsParams&`. Never
add positional parameter arguments back to any `setParameters` — adding a
field to the struct is the whole contract. Keep `KsParams` defaults in sync
with the APVTS defaults in `PluginProcessor.cpp:6-44`.

### Magic numbers live in `KsCalibration.h`

Thresholds, clamps, and tuning formulas are named constants/functions in
namespace `ks` (`src/dsp/KsCalibration.h`) and are used by both code and
tests. Don't inline a new magic number into the DSP — add it there.

### No comments in source

Project convention (root `AGENTS.md`). Don't add inline comments unless the
user explicitly asks. Docs go in `docs/`, not in `//`.

### Pick models use `delaySamples` = string period

In `Exciter::processSample` (`src/dsp/Exciter.h:50`), `delaySamples` is
the string period in samples (`sampleRate/freq`), passed in via
`Exciter::noteOn` (`:39`) from `KarplusStrongDsp::noteOn` (`:28`). It is
NOT the excitation length — that's `excitationLength` / `burstRemaining`.
Pick model comb filters scale relative to this string period.

### UI controls are data-driven

The editor builds each control from its parameter: one
`addControl(group, paramId, label)` line per param
(`src/PluginEditor.cpp:23-34`); choice params become combos whose items come
from the parameter's `choices`. Don't hand-build sliders/combos/attachments
as members — see `ui.md`.

## Things likely to bite you

- **Don't init `currentNumVoices` to a nonzero value** — see trap #1.
- **Don't allocate in `processBlock`** — voices are preallocated
  (`maxVoices = 16`); `updateNumVoices` only flips `enabled` flags. Never
  reintroduce `clearVoices()`/`addVoice()` on the audio thread.
- **Don't add release-ramp code to `noteOff`** — see trap #2 and decision.
- **Don't `#include` JUCE in the DSP headers** — see decision above.
- **Don't remove the `DcBlocker` from the feedback path** — see trap #6.
- **Don't call `exciter.processSample()` twice per sample** — once produces
  the burst; a second call returns 0. `KarplusStrongDsp::processSample`
  (`src/dsp/KarplusStrongDsp.h:45-48`) guards with
  `exciter.isActive() ? exciter.processSample() : 0.0f`.
- **Don't forget `synth.addSound`** — without a `KarplusStrongSound`,
  `Synthesiser::noteOn` finds no sounds that `appliesToNote` and never
  starts a voice (`PluginProcessor.cpp:58`).
- **Build BEFORE testing in the host** — ctest catches DSP issues; the host
  catches voice/synth issues. Run both.