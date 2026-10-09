# Karplus-Strong

A plucked-string module for VCV Rack. One DSP core in Rust, built the way
Fathom is built, and partly out of Fathom's own crates.

Personal project, open source, no commercial goals, no deadline. Quality
over speed.

**Read `docs/` before implementing anything.** `docs/decisions.md` records
every decision and its reason. If something looks arbitrary, the reason is
there. Do not reverse a logged decision without asking.

- `docs/architecture.md` — workspace, core API, the shim, verification
- `docs/dsp.md` — signal flow, knob laws, invariants
- `docs/decisions.md` — K1… and why
- `docs/fathom-reuse.md` — what comes from Fathom, what does not fit, and why
- `TODO.md` — what is next

## Hard rules

1. **`karplus-core` is `no_std`, no allocator, one dependency: `fathom-core`.**
   It must build for `thumbv7em-none-eabihf` and the host.
2. **`#![forbid(unsafe_code)]` in the core.** No exceptions.
3. **`f32` only in the core.** Never `f64`.
4. **No allocation and no panics in the audio path.** Slice indexing panics:
   use `get()`, masks, iterators.
5. **Determinism.** No wall clock, no unseeded randomness, no platform maths.
6. **The core owns its maths.** Fathom's, or our own in `math.rs`, tested
   against `f64` `std`. Never `libm`.
7. **Do not change Fathom from here.** It is a pinned git dependency. Where
   its API does not fit, work around it here and write it up in
   `docs/fathom-reuse.md`.
8. **Do not add a dependency to any crate without asking.**

## Layout

```
crates/karplus-core/     no_std. The string.
crates/karplus-ffi/      staticlib. C ABI for the VCV shim.
crates/karplus-harness/  std. Scenario runner, property tests.
targets/vcv/             The only C++. A shim with no logic.
scenarios/               YAML requirement files. The spec.
listening/               Patches to listen to.
```

`targets/vcv` is a build artefact, not a codebase. If it needs real changes,
the core's boundary is wrong. Say so instead of growing the shim.

## Verification

`just verify` is the gate. Keep it under 90 seconds. **Never claim work is
done without running it.**

```
just verify     # fmt, clippy, tests, no_std build, rules, shim, scenarios
just render     # listening/ and scenarios/ as WAVs in listening/out/
just vcv        # build the Rack plugin against the SDK and install it
just panel      # regenerate the panel SVG
just header     # regenerate the C header
```

Adding a requirement means adding a scenario file in `scenarios/`, not new
test scaffolding. Properties that need a loop over the keyboard or the
parameter space go in `crates/karplus-harness/tests/`.

## Design principles

The same as Fathom's.

- **No menu diving.** Every sound is reachable from the panel.
- **Analog feel.** No zipper noise, no clicks, graceful at extremes.
- **No bad settings.** Every point in parameter space gives finite, bounded,
  non-silent output.

## Working style

- Andre does not write code. He writes scenarios and listens. Explain
  trade-offs in plain terms; do not assume Rust or DSP fluency.
- Propose a plan before large changes.
- **Never optimise a metric by degrading the sound.**
- **If you find a way to pass a test degenerately, that is a bug in the test
  suite.** Report it and add the opposing constraint.
- Keep `TODO.md` current.
