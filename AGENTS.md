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
├── PluginEditor.h/cpp       — GUI (rotary sliders, combo boxes, labels)
└── dsp/
    ├── KarplusStrongDsp.h    — Pure KS algorithm (no JUCE dependency, testable)
    └── KarplusStrongVoice.h — SynthesiserVoice wrapper around KarplusStrongDsp
tests/
└── test_dsp.cpp             — Catch2 unit tests for KarplusStrongDsp
```

## Architecture

- **PluginProcessor** owns a `juce::Synthesiser` populated with
  `KarplusStrongVoice` instances. Parameters are managed via
  `AudioProcessorValueTreeState` (APVTS) — 10 parameters exposed to the host.
  `processBlock()` checks for voice-count changes and updates all voice
  parameters each block.

- **KarplusStrongVoice** extends `juce::SynthesiserVoice`. On `startNote()`:
  computes delay length from MIDI note frequency, resets excitation burst
  counter, applies velocity modulation to brightness/decay, sets lowpass
  cutoff. In `renderNextBlock()`: generates excitation (noise/sine/dust),
  routes through pick model (off/comb/two-delay), processes the KS feedback
  loop (delay → lowpass → gain → back into delay), outputs to stereo buffer.

- **PluginEditor** creates rotary sliders and combo boxes bound to APVTS
  parameters via `SliderAttachment` / `ComboBoxAttachment`. Dark background
  (`0xff1a1a2e`), 5-column grid layout.

## Key JUCE APIs used

| Purpose | JUCE class |
|---|---|
| Polyphony / voice allocation | `juce::Synthesiser` + `juce::SynthesiserVoice` |
| Delay line | `juce::dsp::DelayLine<float>` |
| Lowpass filter | `juce::dsp::FirstOrderTPTFilter<float>` |
| Random noise | `juce::Random` |
| Parameter management | `juce::AudioProcessorValueTreeState` |
| Parameter types | `AudioParameterFloat`, `AudioParameterChoice`, `AudioParameterInt` |
| UI controls | `juce::Slider` (rotary), `juce::ComboBox`, `juce::Label` |
| UI layout | Manual `setBounds()` in `resized()` |

## Parameters (APVTS)

| ID | Name | Type | Range |
|---|---|---|---|
| `decay` | Decay | Float | 0.50 – 0.999 |
| `brightness` | Brightness | Float | 0 – 1 |
| `excitation` | Excitation Type | Choice | Noise/Sine/Dust |
| `excitation_length` | Excitation Length | Float | 1 – 1000 |
| `pick_position` | Pick Position | Float | 0 – 0.5 |
| `pick_model` | Pick Model | Choice | Off/Comb/Two-delay |
| `vel_brightness` | Velocity->Brightness | Float | 0 – 1 |
| `vel_decay` | Velocity->Decay | Float | 0 – 1 |
| `voices` | Voices | Int | 1 – 16 |
| `output_level` | Output Level | Float | 0 – 1 |

## Conventions

- All source files in `src/`. DSP is header-only in `src/dsp/`.
- No comments in source files unless explicitly requested.
- JUCE module includes use the `<juce_module_name/juce_module_name.h>` form.
- The `JUCE/` directory is a git submodule pinned to 8.0.14 — do not modify it.
- Do not run `npm run dev` — not applicable. This is a C++/CMake project.