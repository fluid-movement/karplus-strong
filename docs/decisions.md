# Karplus-Strong — Decision Log

Decisions made in the move from a JUCE plugin to a Rust core with a VCV Rack
module, with rationale. If something looks arbitrary, the reason is here.
**Do not reverse one of these without asking.** Where a decision changes
the sound of the JUCE version, it says so: those are the ones to listen to.

---

### K1 — Rust core, VCV Rack shell; VST, AU and JUCE dropped

The instrument is now built the same way as Fathom: a `no_std` Rust core with
no platform knowledge, a C ABI, and a thin C++ shim for Rack. JUCE, CMake and
the VST3/AU/Standalone builds are gone.

**Why:** asked for, and it makes this repository a test of how reusable
Fathom's primitives are (`docs/fathom-reuse.md`). It also keeps hardware open:
the core builds for the Daisy's Cortex-M7 today.

**Consequence:** the AGPL that JUCE imposed is gone. The code is now
MIT OR Apache-2.0, the same as Fathom.

---

### K2 — Depend on Fathom by pinned git revision, never copy silently

`fathom-core`, `fathom-analysis` and `fathom-harness` are git dependencies
pinned to one commit. Where Fathom code had to be copied (a private function,
the panel font), the copy says so, and where the copy could drift, a test
checks it (K11).

**Why:** a copy can't show where an API fails to fit; a dependency can.
Pinning means a change in Fathom never changes this instrument unannounced.

**Cost:** Fathom is private, so building needs GitHub credentials, and CI
needs a token (`FATHOM_READ_TOKEN`).

---

### K3 — Polyphony is the cable's

One voice per channel of whichever of V/OCT and GATE carries more, up to 16.
No voice allocator, no voice-count knob, no voice stealing. A mono cable on
any other input reaches every voice.

**Why:** that is how polyphony works in Rack. The JUCE version's allocator
existed because MIDI has no channels-per-voice; a poly cable does. Removing it
also removes the JUCE version's three worst bugs (`gotchas.md` traps 1, 4 and
5 in the old docs), which were all about allocation.

**Dropped:** `voices`, `stereo_spread`. Spread voices with a mixer.

---

### K4 — The output is what goes into the string, not what comes out

`out = shape(loop + burst)`, the sample written to the delay line, rather
than the sample read from it.

**Why:** it is bounded by construction (an interpolated read can overshoot
the samples it reads), and the pluck is heard the moment the gate rises
rather than one period later. At C1 that period is 30 ms.

---

### K5 — No config, no context menu

Every control is on the panel. There is no `Config` struct and no
`configure()`.

**Why:** Fathom's config exists for settings that change how controls
respond, and for derived state too expensive to rebuild every block. This
instrument has neither yet: velocity sensitivity and key tracking are
trimpots, and all derived state is a handful of coefficients per block.
Adding a `Config` with nothing in it would be scaffolding.

---

### K6 — BRIGHT follows the note *(sound change)*

The loop lowpass cutoff is 4× to 256× the note's frequency, not an absolute
200 Hz – 12 kHz.

**Why:** with an absolute cutoff, the filter's loss at the fundamental grows
with pitch. Past a point no loop gain can make it up without making the loop
unstable. A dark setting played a high note as a click followed by silence,
which breaks "no bad settings". Tracking the note keeps the timbre the same
across the keyboard. Mutable Instruments' string models do the same. At its
darkest it is a short, muted pluck, never silence.

**Default:** 0.45, about 26× the note: 6.8 kHz at C4, close to the JUCE
version's 6.1 kHz.

---

### K7 — Burst length is in periods of the note *(sound change)*

LENGTH is 1/16 to 4 periods, not 1 to 1000 samples. Default: one period, the
classic Karplus-Strong pluck.

**Why:** a fixed number of samples is a long, tonal burst on a high note and
a click on a low one. In periods, the pluck has the same character at every
pitch and every sample rate.

---

### K8 — The pick position is a real comb, always on *(sound change)*

`excitation − excitation[n − β·period]`, β from 0 (at the bridge) to 0.5 (the
middle). This replaces the Off / Comb / Two-delay choice.

**Why:** the JUCE "Comb" model delayed the burst without combing it, so it
had no effect on the spectrum. "Two-delay" averaged two delays. The real
comb notches every harmonic at a multiple of 1/β, which is what plucking a
real string at that point does. It also has zero gain at DC, so the
excitation never feeds DC into the loop. That is what lets the DC blocker sit
low (K11).

---

### K9 — Drive pushes the pluck into a unit-slope tanh *(sound change)*

DRIVE raises the burst's gain into the loop up to 8×, and crossfades the
loop's shaper from the safety knee to `tanh`. Both curves have a slope of
exactly 1 at zero.

**Why:** the JUCE version used `tanh(kx)/tanh(k)`, whose slope at zero is
above 1. At high drive and long decays, that made the loop gain above 1 for
quiet signals: the string never died and DECAY stopped working. Now the quiet
tail decays as dialed, and the loud start is compressed and grows harmonics:
the bloom, without the runaway.

---

### K10 — One DAMP knob instead of a Ring/Damp switch and a release time

DAMP at zero lets the string ring, as the JUCE version's Ring mode did. Above
zero, when the gate falls, the string is stopped within 3 s down to 20 ms, at
every pitch.

**Why:** a mode switch plus a knob that only works in one mode is two
controls for one idea. Damping lowers the loop gain rather than fading the
output, so it sounds like a hand on the string, not a VCA.

---

### K11 — The loop's DC blocker sits at a 512th of the note

Fathom's `DcBlocker`, placed at `f/512` with `set_corner_hz`, rather than at
Fathom's own `f/64`.

**Why:** inside a loop, a DC blocker's phase lead is largest at the
fundamental. The tuning maths corrects for it there, which leaves the
harmonics flat by up to the same amount: 4.3 cents at `f/64`, measured. At
`f/512` it is half a cent. The pick comb (K8) keeps DC out of the excitation,
so the blocker is only a safeguard.

**Second-order effect:** the blocker gives the loop a weak subsonic mode at
about `f/57`. The loop gain is capped so that mode always dies at least as
fast as the note (`SUB_MODE_GAIN` in `voice.rs`).

---

### K12 — Third-order Lagrange interpolation, every loop delay compensated

The delay line reads with four-point Lagrange interpolation. The loop
lowpass's and the DC blocker's delays at the fundamental are subtracted from
the loop length, and their loss is added to the loop gain.

**Why:** tuning. The JUCE version was tens of cents out at the top of the
keyboard. Every note from C1 to C7 is now within a cent at 44.1, 48 and
96 kHz, dark to bright (`every_note_is_in_tune`). Allpass interpolation was
rejected: it rings when the delay moves, and on V/oct the delay is always
moving.

---

### K13 — Velocity: unpatched is full, and it scales the pluck

With nothing in VEL, every pluck is full velocity. A softer velocity makes a
smaller burst rather than a quieter output, so drive and saturation respond to
it as they would on a real string. The velocity amounts only darken, shorten
or thin a note *below* full velocity, so a bare gate sounds the same whatever
they are set to.

---

### K14 — Noise is drawn at 48 kHz at every sample rate

Noise and dust bursts are held to a 48 kHz grid.

**Why:** a one-period noise burst puts `σ/√period` into each harmonic, and
the period in samples doubles with the sample rate. Drawn every sample, the
pluck came out 3 dB quieter at 96 kHz and 6 dB at 192 kHz. Held to a fixed
rate, the level stays within a decibel across rates
(`the_sample_rate_does_not_change_the_level`).

---

### K15 — Dust means sparse impulses *(sound change)*

One impulse in eight samples (at 48 kHz), random sign. The JUCE "Dust" was
±1 every sample: binary noise, close to the noise exciter.

---

### K16 — An audio input, scaled by a tenth

A new jack: anything patched in is added to the burst, through the tone
filter and the pick comb. With no plucks, the string becomes a tuned
resonator. A string driven at its own pitch builds up by `1/(1 − gain)`, tens
of thousands of times. The tenth puts a ±5 V input at about full scale once
the string has rung up.

---

### K17 — 16-sample blocks in the shim

Rack calls the module once per sample. The shim collects 16 samples of gate
and audio, renders them, and plays them out while it collects the next 16.

**Why:** the core finds gate edges to the sample and works out its
coefficients once per block. 16 samples is a third of a millisecond of
latency at 48 kHz. The delay length and loop gain glide across each block, so
the block size cannot be heard in a pitch sweep (`glide` scenario,
`a_glide_has_no_steps`).

---

### K18 — DECAY runs from 0.1 s to 40 s

The JUCE version stopped at 0.5 s because the bottom of its range was useless
(old trap 6). That was mostly a decay that did not match the dial, now fixed
(K12). Short plucks are useful on a modular, so the range goes down to 0.1 s.
The knob is exponential: 2 s at the centre, 4 s (the JUCE default) at 0.62.
