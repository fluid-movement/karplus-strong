# Feature Docs

Karplus-Strong VST3/AU plugin. Polyphonic plucked-string synth (JUCE 8 + CMake).

Read the root `AGENTS.md` first for build commands and conventions. Then read
the specific doc you need from the table below.

## Index

| File | Read when… | Covers |
|---|---|---|
| `architecture.md` | you need to understand how modules fit together or trace data flow | layer graph, ownership, MIDI→audio path |
| `dsp.md` | you are editing the DSP, adding an exciter, or changing the KS algorithm | Exciter, KsDelayLine, KarplusStrongDsp: signal flow, methods, formulas |
| `parameters.md` | you are adding, renaming, or removing an APVTS parameter | all 10 params: id, type, range, default, file:line, what each sets |
| `voice-and-synth.md` | you are changing note lifecycle, polyphony, or release behavior | Synthesiser, Voice, noteOn/noteOff path, "rings out" design |
| `ui.md` | you are editing the GUI layout or controls | panel geometry, GroupComponents, attachments, constants |
| `build-and-test.md` | you need to build, test, or verify the plugin | verbatim commands, integration test, expected output, install paths |
| `gotchas.md` | before any edit — known traps and design decisions that must not regress | bug history with root causes, "do not" rules |
| `future-features.md` | you are deciding what to build next or want the musical rationale for a planned feature | roadmap by theme: tuning, excitation, string character, expressivity, space, workflow |
| `known-issues.md` | the plugin misbehaves during play, or before picking a bug to fix | open bugs with confirmed root causes and proposed fixes |

## Conventions for these docs

- `file_path:line` references are clickable in most agents — use them.
- Tables carry structured data; prose is kept minimal to save tokens.
- Docs are living — update the relevant file when code changes.