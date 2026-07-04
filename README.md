# Karplus-Strong — VST3/AU Synth Plugin

A polyphonic Karplus-Strong plucked-string synthesizer built with JUCE 8.

## Build

```bash
cmake -B build -G Xcode
cmake --build build --config Release
```

Plugins auto-install to:
- VST3: `~/Library/Audio/Plug-Ins/VST3/Karplus-Strong.vst3`
- AU: `~/Library/Audio/Plug-Ins/Components/Karplus-Strong.component`
- Standalone: `build/KarplusStrong_artefacts/Release/Standalone/Karplus-Strong.app`

## Use in Ableton Live

1. Build the plugin (above).
2. In Live: Options → Preferences → File Folder → "Rescan Plug-Ins".
3. Find "Karplus-Strong" in the Plug-Ins browser under VST or Audio Units.
4. Drag onto a MIDI track. Play.

## Parameters

| Parameter | Range | Description |
|---|---|---|
| Decay | 0.50 – 0.999 | Feedback gain (higher = longer sustain) |
| Brightness | 0 – 1 | Lowpass cutoff in feedback loop |
| Excitation Type | Noise / Sine / Dust | Source of the pluck burst |
| Excitation Length | 1 – 1000 samples | Duration of the excitation burst |
| Pick Position | 0 – 0.5 | Fraction of delay length for pick modeling |
| Pick Model | Off / Comb / Two-delay | Pick-position implementation |
| Velocity→Brightness | 0 – 1 | How much velocity modulates brightness |
| Velocity→Decay | 0 – 1 | How much velocity modulates decay time |
| Voices | 1 – 16 | Polyphony count |
| Output Level | 0 – 1 | Final output gain |

All parameters are exposed to the host for automation.

## Project structure

```
karplus-strong/
├── CMakeLists.txt
├── JUCE/                      # git submodule (8.0.14)
├── src/
│   ├── PluginProcessor.h/cpp   # AudioProcessor + APVTS + Synthesiser
│   ├── PluginEditor.h/cpp      # GUI (rotary sliders, combo boxes)
│   └── dsp/
│       ├── KarplusStrongDsp.h    # Pure KS algorithm (no JUCE, testable)
│       └── KarplusStrongVoice.h # SynthesiserVoice wrapper
├── tests/
│   └── test_dsp.cpp            # Catch2 unit tests
├── README.md
└── AGENTS.md
```

## Editing

Edit the C++ source in `src/`, then rebuild:
```bash
cmake --build build --config Release
```

No code generation step — the compiler is the source of truth.