# Architecture

One core, thin shells, as in Fathom. The core knows nothing about Rack,
hardware, files or the screen. A shell reads its controls, fills `Params`,
calls `Voice::process`, and writes the output.

## Workspace

```
crates/karplus-core/     no_std, no allocator, forbid(unsafe). One dependency: fathom-core.
crates/karplus-ffi/      staticlib. C ABI for the shim; committed cbindgen header.
crates/karplus-harness/  std. Rendering, scenario runner, property tests.
targets/vcv/             The only C++. Rack module shim, panel generator.
scenarios/               YAML requirements. The spec.
listening/               Patches for listening, same format.
```

Fathom crates come in as git dependencies pinned to one commit
(`Cargo.toml`). `docs/fathom-reuse.md` lists what is used and what is not.

## Core API

```rust
Voice::new(sample_rate: f32, seed: u32) -> Self
Voice::reset(&mut self)                     // silent, as new; audio-thread safe
Voice::process(&mut self, p: &Params, gate: &[f32], audio_in: &[f32], out: &mut [f32])
```

`gate` is in volts, `audio_in` at ±1 full scale. Either can be shorter than
`out`, or empty: missing samples count as zero. A `Voice` holds its delay
lines inline (about 48 KB), so a polyphonic shell keeps one per channel,
allocated once.

There is no `configure()`/`Config` (decision K5).

## The VCV shim

`targets/vcv/src/KarplusStrong.cpp` collects 16 samples of gate and audio per
channel, renders them through `karplus_voice_process`, and plays them out
while collecting the next 16 (K17). The channel count is the wider of V/OCT
and GATE (K3). A voice that comes back into use is reset first, so it
doesn't resume an old note.

The only arithmetic in the shim is in `controls.hpp` (knob plus attenuated
CV, volts in and out, switch positions, channel count). It has no Rack
dependency and is tested by `just shim`. Panel coordinates come from
`res/panel.py`, which generates `res/KarplusStrong.svg`.

## Rules for the core

Fathom's, unchanged: `no_std`, no allocator, `#![forbid(unsafe_code)]`, `f32`
only, no panics in the audio path (clippy denies indexing, `unwrap` and
friends outside tests), its own maths (no `libm`), deterministic (seeded
noise only), denormals flushed in every filter state. `just rules` checks the
mechanical ones.

## Verification

`just verify` is the gate, about 30 s warm:

| Step | What |
| --- | --- |
| `fmt`, `lint` | rustfmt, clippy with warnings as errors |
| `test` | unit tests in each crate; property tests in `karplus-harness/tests` |
| `nostd` | the core builds for `thumbv7em-none-eabihf` |
| `rules` | no `f64`, `no_std`, `forbid(unsafe_code)`, no dependency but `fathom-core` |
| `shim` | builds the staticlib, compiles and runs `controls_test.cpp` |
| `scenarios` | renders and checks every file in `scenarios/` |
| `listening-check` | parses every file in `listening/` |

The header drift test (`karplus-ffi/tests/header.rs`) runs under `test`. It
also compiles the header as C and as C++.

Not in the gate: building against the real Rack SDK (`just vcv`, which needs
the SDK and installs into Rack).
