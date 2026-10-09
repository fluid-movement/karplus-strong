# Karplus-Strong

A polyphonic plucked string for VCV Rack. A Karplus-Strong loop, tuned to
within a cent across the keyboard, with a pick position, drive, damping, and
an audio input that turns the string into a resonator.

The sound is made by a Rust core (`crates/karplus-core`) that knows nothing
about Rack, so the same code can run on hardware later. The Rack module is a
thin C++ shim around it. The structure, and several of the building blocks,
come from [Fathom](https://github.com/fluid-movement/Fathom).
`docs/fathom-reuse.md` covers what was reused and what was not.

This replaces the earlier JUCE VST3/AU plugin, which is in the git history
up to `e08f83c`.

## The panel

| Control | What it does |
| --- | --- |
| **DECAY** | How long the string rings: 0.1 s to 40 s for an A4 to fall 60 dB |
| **BRIGHT** | How much of the high end survives each pass. Follows the note, so it means the same timbre everywhere |
| **PICK** | Where the string is plucked: thin and nasal at the bridge, hollow in the middle |
| **DRIVE** | How hard the pluck hits a saturator inside the loop: louder, squarer, overtones that bloom as it dies |
| CV + trimpot under each | ±5 V moves the knob through its whole range, scaled by the trimpot |
| **PITCH** | ±4 octaves, added to V/OCT |
| **TONE** | Lowpass on the pluck itself: soft fingertip to hard plectrum. Fully up is no filter |
| **LENGTH** | Pluck burst, 1/16 to 4 periods of the note |
| **DAMP** | Fully down, the string rings out. Turned up, it is stopped when the gate falls, 3 s down to 20 ms |
| HUMAN | Per-pluck variation in tuning, brightness and pick position |
| KEY | 0: low notes ring longer, as on a real string. 1: every note rings for DECAY |
| HARM | The sine exciter's harmonic, 1–8 |
| NOISE / DUST / SINE | The pluck: white noise, sparse impulses, or a sine on the chosen harmonic |
| VELOCITY: BRIGHT / DECAY / LENGTH | How much a soft velocity darkens, shortens or thins the note |
| PLUCK | Plucks every active voice by hand |
| **V/OCT, GATE** | Polyphonic: one string per channel, up to 16. A rising gate plucks |
| **VEL** | 0–10 V, read when the gate rises. Unpatched is full velocity |
| **IN** | Audio added to the pluck. With no gates, the string rings with whatever comes in |
| **OUT** | Polyphonic, ±5 V |

## Build

You need Rust (via [rustup](https://rustup.rs)), a C++ compiler, `make`, and
[`just`](https://github.com/casey/just). Fathom is a private repository and a
git dependency, so the machine must be signed in to GitHub with access to it.

```
just vcv        # fetch the Rack SDK, build, install into Rack's plugin folder
just verify     # the full check: tests, scenarios, lints, embedded build
just render     # every scenario and listening patch as a WAV in listening/out/
```

Then restart Rack and find **Karplus-Strong** in the module browser.

## Licence

MIT OR Apache-2.0, at your option. With JUCE gone, the AGPL it required no
longer applies.
