# Build & Test

Read when you need to build, test, or verify the plugin.

## First-time setup

```bash
git submodule update --init --recursive   # JUCE is a submodule
cmake -B build -G Xcode                    # generate Xcode project
```

JUCE is pinned to 8.0.14 in `JUCE/` — do not modify the submodule.
CMake 3.22+ required (project uses Xcode generator).

## Build all formats

```bash
cmake --build build --config Release
```

Builds VST3, AU, and Standalone. `COPY_PLUGIN_AFTER_BUILD TRUE` is set in
`CMakeLists.txt:24`, so plugins auto-install to:

| Format | Install path |
|---|---|
| VST3 | `~/Library/Audio/Plug-Ins/VST3/Karplus-Strong.vst3` |
| AU | `~/Library/Audio/Plug-Ins/Components/Karplus-Strong.component` |
| Standalone | `build/KarplusStrong_artefacts/Release/Standalone/` |

Build output (artifacts): `build/KarplusStrong_artefacts/Release/`.

To build just one target:
```bash
cmake --build build --config Release --target KarplusStrong_VST3
cmake --build build --config Release --target KarplusStrong_Standalone
```

## Run tests

```bash
cmake --build build --config Release --target KarplusStrongTests IntegrationTest
ctest --test-dir build -C Release --output-on-failure
```

Expected output:
```
100% tests passed, 0 tests failed out of 2
Total Test time (real) = ~1-2 sec
```

### Test targets

| Target | Source | What it checks |
|---|---|---|
| `KarplusStrongTests` | `tests/test_dsp.cpp` | Catch2 unit tests, pure-DSP no JUCE — 32 cases: `KarplusStrongDsp`/`Exciter`/`KsDelayLine` behavior (`[dsp]`, `[exciter]`, `[delayline]`), primitives (`[primitives]`), tuning math (`[calibration]`) |
| `IntegrationTest` | `tests/test_integration.cpp` | Catch2, headless full `KarplusStrongProcessor` + MIDI — 3 cases: sound on noteOn, released tail survives next note, single-voice stealing |

### Integration test quick run

```bash
cmake --build build --config Release --target IntegrationTest
build/Release/IntegrationTest
# Expected: "All tests passed (N assertions in 3 test cases)"
```

If "Processor produces sound on noteOn" fails, see `gotchas.md` — the usual
cause is the `currentNumVoices` init bug (trap #1).

## Host rescan

After rebuilding, **rescan plugins in Ableton Live** (or restart Live) to
pick up the new binary. AU plugins sometimes cache and need a full restart.

## CMake test wiring

Tests are defined in `CMakeLists.txt`:
- Catch2 v3.5.0 via `FetchContent` — `:54-59`
- `KarplusStrongTests` — `:61-72`
- `IntegrationTest` — `:76-92`, links `PluginProcessor.cpp` + `PluginEditor.cpp`
  + Catch2 + JUCE modules
- `add_test` + `include(CTest)` registers both with ctest — `:73`, `:94`

## Updating JUCE

Do not update JUCE without a deliberate reason. The submodule is pinned. If
required:
```bash
cd JUCE && git checkout <tag> && cd ..
git add JUCE
cmake -B build -G Xcode
```
Note the AGPLv3 license implications before updating — see root `AGENTS.md`.