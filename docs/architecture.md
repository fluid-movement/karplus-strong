# Architecture

Read when you need to understand how modules fit together or trace the
MIDI-to-audio data flow.

## Layer graph

```
MIDI in
  │
  ▼
PluginProcessor (src/PluginProcessor.h:7)
  ├── apvts (AudioProcessorValueTreeState)  src/PluginProcessor.h:42
  ├── cached param atomics                  src/PluginProcessor.h:48-58
  ├── synth (juce::Synthesiser)             src/PluginProcessor.h:45
  │     ├── KarplusStrongSound              src/dsp/KarplusStrongVoice.h:7
  │     └── KarplusStrongVoice[16]          src/dsp/KarplusStrongVoice.h:14
  │           └── dsp (KarplusStrongDsp)    src/dsp/KarplusStrongVoice.h:68
  │                 ├── exciter  (Exciter)      3× CircularBuffer
  │                 └── delayLine (KsDelayLine) CircularBuffer + OnePoleLowpass
  │                                             + DcBlocker + SilenceDetector
  └── PluginEditor (src/PluginEditor.h)  — owns controls + attachments, not DSP
```

Shared plumbing: `KsParams` (one struct carried through the whole parameter
chain) and `KsCalibration.h` (constants + pure tuning functions). See
`dsp.md` for the DSP file map.

## Ownership rules

| Owner | Owns | Notes |
|---|---|---|
| `PluginProcessor` | `synth`, `apvts`, cached param pointers, `currentNumVoices` | `synth` is private; `apvts` is public so the editor can attach |
| `juce::Synthesiser` | voices (OwnedArray), sounds (ReferenceCountedArray) | All 16 voices are preallocated in the constructor; the `voices` param only toggles each voice's `enabled` flag |
| `KarplusStrongVoice` | `dsp` (by value) | One DSP instance per voice — no shared DSP state |
| `KarplusStrongDsp` | `exciter`, `delayLine` (by value) | Thin facade; no JUCE dependency |
| `Exciter` / `KsDelayLine` | `CircularBuffer`s (by value) | Buffers sized `sampleRate * ks::maxDelaySeconds` (0.25 s) in `prepare` |

## MIDI → audio path (one block)

1. Host calls `processBlock(buffer, midi)` — `src/PluginProcessor.cpp`
2. `buffer.clear()`, then `updateNumVoices` (enabled-flag toggle only, no
   allocation)
3. `updateVoiceParameters()` builds one `KsParams` from the cached atomics and
   pushes it to every voice; `updateVoicePans()` computes each voice's
   `stereo_spread`-derived pan the same way
4. `clearVoiceOutputScratch(numSamples)` zeroes every voice's `outputScratch`
   buffer (see `voice-and-synth.md` — Sympathetic resonance)
5. `synth.renderNextBlock(buffer, midi, 0, N)`
6. Synthesiser splits block at MIDI event positions, calls `noteOn`/`noteOff`
   (JUCE internals — see `voice-and-synth.md` for our hooks)
7. For each sub-block, `renderVoices()` calls each voice's `renderNextBlock`
8. Each `KarplusStrongVoice::renderNextBlock` pulls
   `dsp.processSample(sympatheticInput[idx])` per sample (the bleed value
   computed from the *previous* block), stores its own mono output into
   `outputScratch[idx]`, and adds it to both channels scaled by that voice's
   pan gains — at the default `stereo_spread == 0` both gains are `1.0`,
   identical to the original mono-copy behavior
9. Voice calls `clearCurrentNote()` + breaks when `dsp.isSilent()`
10. Back in `processBlock`, `updateSympatheticBleed(numSamples)` sums every
    voice's freshly-written `outputScratch` and writes each voice's
    "everyone else's sum, scaled by `Sympathy`" into its `sympatheticInput`
    buffer for step 8 to consume **next** block

## Parameter → DSP binding

`PluginProcessor` reads the cached APVTS atomics each block, fills a
`KsParams`, and calls `voice->setParameters(params)` — `updateVoiceParameters()`
in `src/PluginProcessor.cpp`. The struct is forwarded by const ref:
Voice → `KarplusStrongDsp` → `Exciter` + `KsDelayLine`. Each consumer reads
only the fields it owns. See `parameters.md` for the per-param mapping.

## Real-time-safety rules

- No allocation, locks, or buffer resizing in `processBlock` or anything it
  calls. Voices are preallocated (`maxVoices = 16`, `src/PluginProcessor.h`);
  buffers (including the sympathetic-resonance scratch buffers) are sized
  once in `prepare`.
- Parameter access is via cached `std::atomic<float>*` — no per-block string
  lookups.

## Module summaries

| Module | File | One-line responsibility |
|---|---|---|
| PluginProcessor | `src/PluginProcessor.h:7` | AudioProcessor, APVTS, Synthesiser owner, per-block param push |
| PluginEditor | `src/PluginEditor.h` | GUI: data-driven controls in two GroupComponent panels (see `ui.md`) |
| KarplusStrongVoice | `src/dsp/KarplusStrongVoice.h:14` | SynthesiserVoice wrapper: startNote/stopNote/renderNextBlock + enabled flag |
| KarplusStrongDsp | `src/dsp/KarplusStrongDsp.h` | Thin composition of Exciter + KsDelayLine (no JUCE dep) |
| Exciter | `src/dsp/Exciter.h` | Excitation generation (noise/sine/dust) + pick model routing |
| KsDelayLine | `src/dsp/KsDelayLine.h` | KS feedback loop topology |
| KsParams | `src/dsp/KsParams.h` | plain param struct passed through the chain |
| KsCalibration | `src/dsp/KsCalibration.h` | named constants + pure tuning functions |
| CircularBuffer / OnePoleLowpass / DcBlocker / SilenceDetector | `src/dsp/*.h` | single-purpose primitives, individually unit-tested |
