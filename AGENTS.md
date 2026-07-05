# Karplus-Strong VST3/AU Plugin

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
cmake --build build --config Release --target KarplusStrongTests IntegrationTest
ctest --test-dir build -C Release --output-on-failure
```

## Feature docs

Detailed documentation lives in `docs/`. Start at `docs/README.md` for an
index. Read the specific file you need:

| Read… | When… |
|---|---|
| `docs/README.md` | first — index of all docs |
| `docs/architecture.md` | you need to understand how modules fit together |
| `docs/dsp.md` | editing the DSP, adding an exciter, changing the KS algorithm |
| `docs/parameters.md` | adding/renaming/removing an APVTS parameter |
| `docs/voice-and-synth.md` | changing note lifecycle, polyphony, or release |
| `docs/ui.md` | editing the GUI layout or controls |
| `docs/build-and-test.md` | building, testing, or verifying the plugin |
| `docs/gotchas.md` | before any edit — known traps and design decisions |

**Read `docs/gotchas.md` before making changes** — it documents bugs we've
already hit (e.g. `currentNumVoices` must init to `0`, `noteOff` is a no-op)
and rules that must not regress.

## Project structure

```
src/
├── PluginProcessor.h/cpp    — AudioProcessor + APVTS parameters + Synthesiser
├── PluginEditor.h/cpp       — GUI (two-panel: Exciter / Delay Line GroupComponents)
└── dsp/
    ├── Exciter.h            — Excitation generation (noise/sine/dust) + pick model routing
    ├── KsDelayLine.h        — KS delay buffer + lowpass + feedback + silence detection
    ├── KarplusStrongDsp.h   — Thin composition of Exciter + KsDelayLine (no JUCE dependency, testable)
    └── KarplusStrongVoice.h — SynthesiserVoice wrapper around KarplusStrongDsp
tests/
├── test_dsp.cpp             — Catch2 unit tests for KarplusStrongDsp, Exciter, KsDelayLine
└── test_integration.cpp     — Headless integration test (full AudioProcessor + MIDI)
docs/
└── README.md + per-feature docs
```

## Conventions

- All source files in `src/`. DSP is header-only in `src/dsp/`.
- No comments in source files unless explicitly requested.
- JUCE module includes use the `<juce_module_name/juce_module_name.h>` form.
- The `JUCE/` directory is a git submodule pinned to 8.0.14 — do not modify it.
- Do not run `npm run dev` — not applicable. This is a C++/CMake project.