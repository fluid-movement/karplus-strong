# Voice & Synth

Read when changing note lifecycle, polyphony, or release behavior.

## Ownership

`PluginProcessor` owns a `juce::Synthesiser synth` (`src/PluginProcessor.h:45`).
The synth owns voices (`KarplusStrongVoice`, `src/dsp/KarplusStrongVoice.h:14`)
via `OwnedArray` and one `KarplusStrongSound` (`src/dsp/KarplusStrongVoice.h:7`)
via reference counting. Each voice owns a `KarplusStrongDsp` instance by value
(`:68`) — no DSP state is shared between voices.

## Voice pool — preallocated, flag-toggled

All `maxVoices = 16` voices (`src/PluginProcessor.h:10`) are added to the
synth **once** in the constructor (`PluginProcessor.cpp:58-62`) and prepared
in `prepareToPlay`. The `voices` param never allocates:

- `updateNumVoices(n)` (`PluginProcessor.cpp:103`) only calls
  `voice->setEnabled(i < n)` on each voice.
- `KarplusStrongVoice::canPlaySound` returns the `enabled` flag
  (`src/dsp/KarplusStrongVoice.h:29`), so JUCE's `findFreeVoice` and
  `findVoiceToSteal` both skip disabled voices.
- A voice disabled while ringing keeps rendering until silent — it just
  can't take new notes.
- **No allocation in `processBlock`** — this is a real-time-safety rule
  (see `architecture.md`). Never reintroduce `clearVoices()`/`addVoice()`
  on the audio thread.
- `currentNumVoices` **must** init to `0` — `src/PluginProcessor.h:46`. The
  constructor relies on the mismatch for the first `updateNumVoices` call to
  run (see `gotchas.md` trap #1).

## Note-on path

```
MIDI noteOn
  → juce::Synthesiser::noteOn (JUCE internal)
  → findFreeVoice(sound, channel, note, steal=true)
  → startVoice(voice, sound, channel, note, velocity)
  → voice->startNote(note, velocity, sound, pitchWheel)
```

Our `KarplusStrongVoice::startNote` — `src/dsp/KarplusStrongVoice.h:31`:
```
auto freq = juce::MidiMessage::getMidiNoteInHertz(midiNoteNumber);
dsp.noteOn(freq, velocity);
```

`KarplusStrongDsp::noteOn` — `src/dsp/KarplusStrongDsp.h:28`:
```
delaySamp = delayLine.getSampleRate() / frequency;
exciter.noteOn(delaySamp);                              // seeds burst
delayLine.noteOn(frequency, velocity, exciter.getExcitationLength());
```

## Note-off path — "rings out" design

**Key behavior:** releasing a key does NOT stop the sound. The string rings
out naturally per the `decay` feedback parameter. The voice only ends when
`isSilent()` reports true.

Our `KarplusStrongVoice::stopNote` — `src/dsp/KarplusStrongVoice.h:38`:
```cpp
void stopNote (float, bool allowTailOff) override
{
    dsp.noteOff (allowTailOff);   // no-op (see below)

    if (! allowTailOff)           // hard stop only (steal / all-notes-off)
    {
        dsp.reset();
        clearCurrentNote();
    }
}
```

On a normal release (`allowTailOff == true`) the voice stays **busy** while it
rings; `renderNextBlock` frees it via `isSilent()` once decayed. Do NOT call
`clearCurrentNote()` on normal release — that put the still-ringing voice back
in the free pool, and the next noteOn reused it and reset its buffer, cutting
the tail (see `gotchas.md` trap #5). A hard stop (`allowTailOff == false`,
from voice stealing or all-notes-off) resets the DSP and frees the slot
immediately.

`KsDelayLine::noteOff` — `src/dsp/KsDelayLine.h:56` is a no-op **in Ring mode**
(`dampMode == 0`, the default) — unchanged from the original design. In Damp
mode (`dampMode == 1`), a soft release (`allowTailOff == true`) now arms a
release-gain envelope so the string fades over `releaseTime` seconds instead
of ringing indefinitely. **Do not add unconditional release-ramp logic here**
— any new release behavior must stay gated behind an explicit mode, matching
the Damp-mode pattern; see `gotchas.md` for the history of why the old
always-on release ramp was removed.

## Render path

`KarplusStrongVoice::renderNextBlock` — `src/dsp/KarplusStrongVoice.h:49`:
```
for each sample in block:
    out = dsp.processSample()
    add out * leftGain to channel 0, out * rightGain to channel 1
    if dsp.isSilent():
        clearCurrentNote()
        break
```

`leftGain`/`rightGain` come from the voice's `pan` (`-1..1`, set by
`PluginProcessor::updateVoicePans()` from the `stereo_spread` param): linear
pan law where `pan == 0` gives `leftGain == rightGain == 1.0` (the original
always-mono-copy behavior, still the default since `stereo_spread` defaults
to 0), `pan == -1` gives `(1, 0)`, `pan == 1` gives `(0, 1)`. Output is added
(not copied) — `addSample` — so polyphonic voices sum in the buffer before
`processBlock` returns.

## isSilent trigger

`KarplusStrongDsp::isSilent` — `src/dsp/KarplusStrongDsp.h:40` — delegates to
`KsDelayLine::isSilent` (`src/dsp/KsDelayLine.h:58`), which delegates to
`SilenceDetector` (`src/dsp/SilenceDetector.h`):
```
isSilent() = (samplesTracked >= minSamples) AND (peakAmplitude < ks::silenceThreshold)
```

- The `minSamples` guard (armed per note in `KsDelayLine::noteOn`) prevents
  premature silence before the first delay output appears (string period +
  burst length + `ks::silenceGuardSamples`).
- `peakAmplitude` decays by half every `ks::peakDecayWindow` (256) samples,
  so once the feedback loop has quieted the signal, the voice ends.
- There is no forced release ramp. Decay time is entirely governed by
  `feedbackGain` (derived from `decay` + velocity).

## Polyphony and voice stealing

- `juce::Synthesiser` is built with `setNoteStealingEnabled(true)` by default
  (`JUCE/modules/juce_audio_basics/synthesisers/juce_Synthesiser.h:639`).
- Because released voices stay busy until silent, the `voices` param bounds
  the number of simultaneously **ringing** tails, not just held keys.
- When all voices are ringing and a new note arrives, the synth steals a
  non-protected voice. The victim gets `stopNote(0.0f, false)` → our hard-stop
  path (`dsp.reset()` + `clearCurrentNote()`), then `startNote` →
  `dsp.noteOn`. This cuts that string short — unavoidable with finite
  polyphony; raise `voices` if audible.
- Retriggering the same note: `Synthesiser::noteOn` first calls
  `stopVoice(1.0f, allowTailOff=true)` on any voice already playing that note
  (`juce_Synthesiser.cpp:319-321`) — with our stopNote that voice keeps
  ringing — then starts the new pluck on a free voice. Repeated hits on one
  key therefore layer tails, like a re-plucked string with sustain.

## Adding a new SynthesiserSound

`KarplusStrongSound` (`src/dsp/KarplusStrongVoice.h:6`) returns true for all
notes/channels. To make a keymap, subclass `SynthesiserSound`, override
`appliesToNote`/`appliesToChannel`, and add instances via `synth.addSound`.
`findFreeVoice` checks `voice->canPlaySound(sound)` (`KarplusStrongVoice.h:30`
returns true unconditionally) — override it if you add distinct sound types.