# Karplus-Strong VST3/AU Plugin

## Project overview

A polyphonic Karplus-Strong plucked-string synthesizer built with JUCE 8 and
CMake. Builds as VST3, AU, and Standalone for macOS (arm64).

## Tech stack

- **Framework:** JUCE 8.0.14 (git submodule in `JUCE/`)
- **Build system:** CMake 3.22+ with Xcode generator
- **Language:** C++17
- **License:** JUCE AGPLv3 (open-source only, no commercial)

## Build commands

```bash
# First-time setup (already done — JUCE submodule is committed)
git submodule update --init --recursive

# Generate Xcode project
cmake -B build -G Xcode

# Build all formats (VST3 + AU + Standalone)
cmake --build build --config Release
```

Plugins auto-install to system folders via `COPY_PLUGIN_AFTER_BUILD TRUE`.

## Running tests

```bash
cmake --build build --config Release --target KarplusStrongTests
ctest --test-dir build -C Release --output-on-failure
```

## Project structure

```
src/
├── PluginProcessor.h/cpp    — AudioProcessor + APVTS parameters + Synthesiser
├── PluginEditor.h/cpp       — GUI (two-panel: Exciter / Delay Line GroupComponents)
└── dsp/
    ├── Exciter.h            — Excitation generation (noise/sine/dust) + pick model routing
    ├── KsDelayLine.h        — KS delay buffer + lowpass + feedback + release ramp + silence detection
    ├── KarplusStrongDsp.h   — Thin composition of Exciter + KsDelayLine (no JUCE dependency, testable)
    └── KarplusStrongVoice.h — SynthesiserVoice wrapper around KarplusStrongDsp
tests/
├── test_dsp.cpp             — Catch2 unit tests for KarplusStrongDsp, Exciter, KsDelayLine
└── test_integration.cpp     — Headless integration test (full AudioProcessor + MIDI)
```

## Architecture

- **PluginProcessor** owns a `juce::Synthesiser` populated with
  `KarplusStrongVoice` instances. Parameters are managed via
  `AudioProcessorValueTreeState` (APVTS) — 10 parameters exposed to the host.
  `processBlock()` checks for voice-count changes and updates all voice
  parameters each block.

- **KarplusStrongDsp** is a thin facade composing **Exciter** (excitation
  generation + pick model routing) and **KsDelayLine** (delay buffer +
  first-order lowpass + feedback gain + release ramp + silence detection).
  `noteOn()` computes delay length from frequency using the delay line's
  sample rate, seeds the exciter burst, and initializes the delay line.
  `processSample()` pulls excitation from the exciter (zero after burst),
  feeds it to the delay line which falls back to feedback when excitation
  is zero.

- **KarplusStrongVoice** extends `juce::SynthesiserVoice`. On `startNote()`:
  calls `dsp.noteOn(freq, velocity)`. In `renderNextBlock()`: calls
  `dsp.processSample()` per sample, writes to stereo buffer, breaks and
  clears current note when `dsp.isSilent()`.

- **PluginEditor** creates rotary sliders and combo boxes bound to APVTS
  parameters via `SliderAttachment` / `ComboBoxAttachment`. Dark background
  (`0xff1a1a2e`), two-panel layout with GroupComponents (Exciter left,
  Delay Line right).

## Key JUCE APIs used

| Purpose | JUCE class |
|---|---|
| Polyphony / voice allocation | `juce::Synthesiser` + `juce::SynthesiserVoice` |
| Delay line | `std::vector<float>` circular buffer |
| Lowpass filter | First-order RC (custom) |
| Random noise | xorshift32 (custom) |
| Parameter management | `juce::AudioProcessorValueTreeState` |
| Parameter types | `AudioParameterFloat`, `AudioParameterChoice`, `AudioParameterInt` |
| UI controls | `juce::Slider` (rotary), `juce::ComboBox`, `juce::Label` |
| UI layout | Manual `setBounds()` in `resized()` |

## Parameters (APVTS)

| ID | Name | Type | Range |
|---|---|---|---|
| `excitation` | Excitation Type | Choice | Noise/Sine/Dust |
| `excitation_length` | Excitation Length | Float | 1 – 1000 |
| `pick_position` | Pick Position | Float | 0 – 0.5 |
| `pick_model` | Pick Model | Choice | Off/Comb/Two-delay |
| `decay` | Decay | Float | 0.50 – 0.999 |
| `brightness` | Brightness | Float | 0 – 1 |
| `vel_brightness` | Velocity->Brightness | Float | 0 – 1 |
| `vel_decay` | Velocity->Decay | Float | 0 – 1 |
| `output_level` | Output Level | Float | 0 – 1 |
| `voices` | Voices | Int | 1 – 16 |

## Conventions

- All source files in `src/`. DSP is header-only in `src/dsp/`.
- No comments in source files unless explicitly requested.
- JUCE module includes use the `<juce_module_name/juce_module_name.h>` form.
- The `JUCE/` directory is a git submodule pinned to 8.0.14 — do not modify it.
- Do not run `npm run dev` — not applicable. This is a C++/CMake project.