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

---

## The organic-sound round (K19–K26)

Made in one go, without a conversation, on the instruction "implement
everything, decide, and document it so I can review and play with it". Every
one of these is a decision I made for Andre; none was asked. Each says what
else could have been done, and the constants that are most likely to need
tuning by ear are listed in K26. The sound-changing ones are marked.

### K19 — Stiffness: a pair of allpass filters in the loop *(new)*

Real strings are stiff, so high partials travel faster and sit above whole
multiples of the fundamental. In a loop that is an allpass filter, whose
delay is large at low frequencies and small at high ones. Two identical
first-order stages, `(a + z⁻¹)/(1 + a·z⁻¹)`, sit after the DC blocker.

**The knob** sets the *delay of one stage at DC*, in samples. At rest it is 1
(`a = 0`, which is plain delay, so an untouched knob is the same string),
growing with the square of the knob (the first half is subtle) up to 24
samples, and never more than 20% of the period per stage (so the pair never
takes more than 40% of the loop; this limits the effect on high notes, which
have little loop to spare).

**Tuning and decay stay right** the way the lowpass and DC blocker stay right
(K12): the allpass's phase delay *at the fundamental* is computed exactly and
taken off the delay line, so the fundamental does not move; its gain is 1, so
the decay does not change. Only the upper partials move. Tests: the
fundamental is within a cent from C2 to C6 at every stiffness; partials 4 and 8
are more than 8 cents sharp at full and within 3 at none; T60 within 15%.

**Alternatives not taken:** a longer allpass cascade (more CPU for a finer
shape), and a knob that sets inharmonicity *B* directly (one number that means
the same on every note). The allpass does not give constant *B* across notes:
the same knob stretches high notes more than low ones in cents, which is also
what a real instrument does.

*Costs one extra state per stage per voice; always on, at rest bit-different
from before only by rounding.*

### K20 — Tension modulation *(new)*

A plucked string is stretched by its own swing, so a hard pluck starts sharp
and falls into tune as the swing dies. The voice keeps a running mean-square
of what it writes (averaged over three periods) and shortens the delay by
`knob × 12% × period × (energy / 0.03)`, the energy clamped to 1. It
is a chirp of up to about +14 cents on a default full-velocity pluck at full
knob (measured, C4, 100–250 ms), settling to within 0.3 cents; soft plucks
(velocity 2 V) move less than half as much.

The 12% ceiling (about 220 cents) is for the rare very hard hit; a normal
pluck uses a small part of it. If the chirp is too shy, raise
`TENSION_MAX_FRACTION` or lower `TENSION_FULL_ENERGY`, both named in the code.

**Why the follower and not the instantaneous value:** the instantaneous
swing would be frequency modulation of the loop, a different (buzzy) effect.
Averaging over three periods makes it a slow, musical pitch glide.

### K21 — Buzz: a curved bridge that bends the string, lossless *(new; reworked in K27)*

A sitar's bridge is curved, and the string touches it where it swings hard.
The touch shortens the string for a moment. The loop reads its delay line at
`delay × (1 − touch × bend)`, where `touch` is twice the swing beyond a dead
zone of 0.025, with the negative side counting 1.5 times the positive. The
knob is squared and reaches a bend of 1% of the length at full. A quiet string
never leaves the dead zone and is untouched, so a hard pluck buzzes and the
tail comes clean, as on a sitar.

**It takes nothing away.** A time warp adds partials and removes none: at full
buzz the note keeps its pitch (±0.2 cents) and its decay (T60 within 1%) at
C2, C3 and C4. The warp is lopsided, so it leaves a little DC, mostly on low
notes; the loop's DC blocker is brought up with the knob (the corner goes from
a 512th of the note to a 96th), and the make-up gain cap follows it. Measured
DC at full buzz: −73 dBFS at C4, below −100 at C2.

**The first version was a clip, and it was thrown away.** I first squeezed the
positive swings against a knee. That took 5–30% of the sustain with it, and
the variants that fixed that (a knee that follows the level; a delay
modulation normalised to the level) either left DC that climbed and collapsed
the string or detuned it by 7 cents. The lossless version is Mutable
Instruments' (K27); the dead zone, the 1.5 asymmetry and the 1% are theirs in
spirit and measured again here, not copied.

### K22 — The outboard loop: SEND and RETURN, normalled to the internal damping *(new)*

The delay line's output goes to **SEND** (polyphonic, one channel per voice).
**RETURN**, when patched, stands in for the loop's lowpass: what comes back
goes through a soft limiter, the DC blocker and the loop gain (which are not
optional: a loop through a filter you cannot see needs them, and the
limiter keeps a resonant filter from running away). Unpatched, the string is
exactly what it was; the unpatched return is ignored (a test checks bit-for-bit).

**Latency, and K17 kept.** The shim works in 16-sample blocks (K17). A trip
through the jacks costs 16 samples to play out the block, 16 to collect the
return (the core needs a whole block of it before it renders), and one sample
each way for the cables: **34 samples**, `LOOP_LATENCY`. The core shortens the
delay line by that much when the return is patched, so the loop is the right
length. Consequences:

- **The top note is about 1.2 kHz** (the loop must be longer than the
  latency plus the interpolator's minimum). Above that the note goes flat.
  Per-sample processing would take the latency down to the 2 cables' 2
  samples and put the limit near 10 kHz, but it means giving up K17's blocks
  and rewriting the shim. Not done; it is the first thing to revisit if the
  loop is liked.
- **Pitch drifts with the outboard filter's group delay.** A dark filter
  goes flat (an 800 Hz one-pole: −42 cents at C3; scenario
  `outboard_dark_is_flat`). **TRIM** (±½ semitone, centre is none) retunes the loop alone:
  `outboard_trim_fixes_it` finds it back within a cent. The module cannot know
  what is patched, so it cannot do this for you.
- **Decay is the filter's business too.** The loop gain assumes the outboard
  filter is a wire; one that loses level will shorten the note.

The harness simulates the outboard filter (a one-pole, `outboard:` in a
scenario) with the real 34-sample delay, so the loop's tuning is tested
without Rack.

### K23 — The bridge: one shared body and one set of sympathetic strings, owned by the shim *(new)*

The body and the sympathetic strings are properties of the instrument, not of
a string, so they do not live in `Voice`. `Bridge` takes the sum of all the
voices (`karplus_bridge_process` sums the 16 rows the shim already has) and
has its own output jack, **BRIDGE**, mono. The polyphonic **OUT** is
untouched: the dry strings, one channel each, as before. So the body can be
ignored, or the strings sent through something else, without losing it.

`Bridge` is *additive* and one-way in this first version: the strings drive
it and it does not drive them. A true two-way bridge (the body's response
shaping the string's loop) needs per-sample coupling (K22's latency again) and
is left for later; the effect it would add, a body that pulls the strings'
partials, is subtle next to what is here.

Controls: **SHAPE, SIZE, MATERIAL, MIX** and the **TRACK** switch, on the
panel's right half. SHAPE and SCENE take CV (±5 V sweeps the knob; there are no
attenuverters because there was no room, and `shape` and `scene` are the two
that want sweeping).

### K24 — The body: twelve resonant modes, four shapes, glass to wood *(new)*

Twelve two-pole resonators, summed. **SHAPE** morphs (geometrically, in
log-frequency) through four tables of mode ratios:

| Anchor | Ratios (first 6) | From |
| --- | --- | --- |
| string | 1 2 3 4 5 6 … | whole multiples |
| bar | 1, 2.757, 5.404, 8.933, 13.34, 18.64 … | roots of cos·cosh = 1, solved |
| drum | 1, 1.593, 2.136, 2.295, 2.653, 2.917 … | zeros of Bessel J, solved |
| plate | 1, 2.014, 2.986, 3.703, 4.0, 5.689 … | a simply supported rectangle, aspect 1.4, computed |

The bar and drum values are checked against a numerical solution of their
defining equations (they agree to the third decimal). The plate is *a*
plate: a rectangle with a made-up aspect ratio, which gives a dense
non-harmonic series; a real free plate or a guitar top would differ, and a
guitar body specifically has modes near 100 and 200 Hz that this does not
try to match (not verified; I did not find a source I trusted).

**SIZE** scales all frequencies together. Fixed in Hz (TRACK down) the first
mode runs from 800 Hz (small) down to 25 Hz (big), 141 Hz at the middle.
Tracked (TRACK up) the first mode sits at the *lead voice's pitch* (channel
1) times 4× at the small end to ¼× at the big, 1× at the middle, so a harmonic
body at the middle is a resonator on the note. The lead voice only: in a
chord the body follows the first note.

**MATERIAL** sets how long each mode rings, in two parts: the first mode's
T60 (30 ms at wood to 3.8 s at glass), and how much faster the higher modes
die (T60 ∝ ratio^−α, α from 1 at wood to 0.2 at glass). Wood is short and
dull; glass is a bell. The body is *normalised* for how much it hears: each
mode's gain is between unit gain at its own frequency and equal power for
white noise, three-quarters of the way to the latter. At first (unit
gain) a long ring was almost silent when struck by a pluck's broadband burst.

**MIX** crossfades the strings against the body in the BRIDGE output
(`dry·(1 − mix) + body·mix`); sympathetic strings add on top. The knobs are
smoothed at block rate (30 ms) so turning them is audible as the instrument
changing shape, not as a click (a test sweeps SIZE and SHAPE while ringing).

### K25 — The sympathetic strings: eight undriven lines, six tunings, a morph *(new)*

Eight delay lines with a lowpass, tuned and decaying like the main strings
(Lagrange read, delay compensated for the lowpass), never plucked: they hear
the sum of the voices, scaled by `(1 − loop gain) × 6 × LEVEL`. That scaling
makes a *sustained* input at a string's own pitch build to 6 × LEVEL times
itself however long RING is, which is what lets a long ring be audible and
a short one not blow up. Each is a linear loop with gain below 1, so
there is no way for them to run away; the BRIDGE output ends in a soft knee.

**SCENE** passes through six tunings (log-ratio tables in `bridge.rs`):

1. octaves and fifths (½, 1, 3/2, 2, 3, 4, 6, 8)
2. major chord, just intonation
3. minor seventh, just
4. raga (Bhairav: Sa, komal Re, Ga, Ma, Pa, komal Dha, Ni, Sa′)
5. tanpura (Pa, Sa, Sa, Sa′ …)
6. an inharmonic cluster

The knob is a morph, with a flat 20% around each scene and a smoothstep
between, so a tuning is easy to land on and the glide between is audible
while the strings ring (they glide, per block, with the lines' delays).
A test checks every string of every scene is within 1.5 cents of the table.

**ROOT** is an offset of ±2 octaves around the root, which is either the
**lead voice's pitch** (FOLLOW up) or a fixed C3 (FOLLOW down, tanpura-style).
The followed root is smoothed (60 ms), so each new note re-tunes the strings
with a glide, not a jump. *(It follows channel 1 only.)* **RING** sets the T60
of all eight, 0.5 s to 20 s.

**Not done, deliberately:** coupling between the strings, and from the strings
back to the voices. "Total coupling gain below 1" is satisfied trivially by
having none.

### K26 — Other things decided, and the numbers most likely to need tuning by ear

- **The panel is 32 HP.** The left half is the old panel, unchanged. The right
  is **LOOP** (STIFF, TENSION, BUZZ, TRIM), **BODY** (SHAPE, SIZE, MATERIAL,
  MIX, a TRACK switch) and **STRINGS** (SCENE, ROOT, LEVEL, RING, a FOLLOW
  switch), and the jacks SEND, RETURN, SHAPE CV, SCENE CV and BRIDGE.
  The alternative, a second module that connects to this one, would have
  meant an expander protocol and was not worth it.
- **The shim got more logic than K1 would like:** the return buffers
  (two blocks of latency) and the bridge call. It is all plumbing; the one
  new arithmetic is `trim_volts()` in `controls.hpp`, tested by `just shim`.
- **The Rack plugin has not been compiled against the SDK in this round** (the
  SDK download is blocked in this environment, HTTP 403). The shim was written
  against the headers' names as in the existing code; `just vcv` is the first
  thing to run. The C ABI, the header, the controls test and the panel are all
  checked by `just verify`.
- **No new dependencies; Fathom untouched.** The new maths (`allpass_response`,
  the body's coefficients) uses `fathom_core::exp2` and `sin_turns` and this
  crate's own `cos_turns`, `sqrt`, `exp`, `atan2`.
- **Defaults:** every new knob is at 0 or at its neutral position, and the
  bridge defaults to a wire, so an untouched module is still the module of
  K18 (`pluck_default` and the other 17 old scenarios pass unchanged).

**Constants to adjust by ear, in `voice.rs` and `bridge.rs`:**

| Constant | Now | Raise it to |
| --- | --- | --- |
| `TENSION_MAX_FRACTION` | 12% | let hard plucks chirp further |
| `TENSION_FULL_ENERGY` | 0.03 | (lower) make softer plucks chirp too |
| `BUZZ_KNEE` | 0.12 | (lower) squeeze more, even at low levels |
| `STIFFNESS_MAX_DELAY` | 24 samples | stretch the partials further |
| `BODY_GAIN`, `MODE_FALLOFF` | 0.5, 0.85 | louder body; brighter body (toward 1) |
| `SYMPATHETIC_SENSITIVITY` | 6 | wake the strings more easily |
| `SETTLE_S` | 30 ms | slower, smoother knob response |

---

## What Elements and Rings taught the bridge (K27–K31)

Andre's instruction: Mutable Instruments' Elements and Rings are excellent
modelling modules and open source (MIT, Émilie Gillet); learn from them, do
not copy. I read the DSP of both (`elements/dsp/resonator.cc`, `string.cc`,
`part.cc`; `rings/dsp/resonator.cc`, `string.cc`, `part.cc`). What follows is
what they do that this module did not, what was taken, in this instrument's
own numbers and code, and what was left alone and why. Nothing is a line
for line port; the mode series, the laws and every constant were rederived
or remeasured here, and the tests are ours.

### K27 — The curved bridge, lossless *(taken)*

Their `String` reads its delay line at a length bent by a dead-zone, lopsided
rectification of the signal, and mixes in an AC-blocked copy to keep DC out.
This is the thing my clip (K21's first version) was trying to be, without its
cost. See K21 for what it measured here. **Not taken:** their *dispersion
noise* (random modulation of the length that grows with the knob), which makes
a stiff string metallic and noisy; and tying buzz and stiffness to one bipolar
knob (negative one way is sitar, positive the other is piano). The panel has
separate knobs, and two knobs are easier to understand than one that does two
things.

### K28 — Modes as state-variable band-passes, twenty-four of them *(taken)*

Their resonators are a bank of state-variable band-pass filters, up to 64,
with the coefficients refreshed at control rate. The body was twelve two-pole
direct-form filters. Direct-form resonators glitch when their coefficients
move while they ring (I smoothed the knobs over 30 ms to hide it); the
state-variable (topology-preserving) form does not. So the modes are now that
form, 24 of them, the knobs settle in 12 ms (it feels like a knob now, not a
slow fade), and the coefficients are recomputed every block. Twenty-four
rather than their 64 because the body is one shared bank at Rack's sample
rate, not one per voice on a 168 MHz microcontroller, and twenty-four is where
the series stops mattering to the ear at the levels the strings drive it; the
count is `BODY_MODES`, and `just verify` has a test that the twentieth mode
rings in tune. Modes above 0.45 of the sample rate are skipped, not folded.
The mode tables were extended to 24 entries by solving the same equations
(bar, drum) and enumerating the same rectangle (plate).

**Damping.** Theirs is one recurrence over the mode index, which saturates
so the highest partials do not die at once. Ours stays a power law over the
mode's *frequency ratio* (T60 ∝ ratio^−α), which is right for inharmonic
shapes where index and frequency are not the same thing; but I took the
saturation: a high mode now rings at least 8% as long as the first
(`MODE_T60_FLOOR`), so a damped body is dull and not just short.

**Not taken:** their Q ∝ frequency baseline with a loss per mode, their
separate DAMPING and BRIGHTNESS knobs (our MATERIAL is one knob doing both, which
was a decision of K24 and costs one knob of panel), and their geometry axis,
which is one continuous generative law (a stretched or compressed harmonic
series) where ours morphs between four physically derived series. Theirs is
smoother as a knob; ours has recognisable places to stop (bar, drum, plate).
If the SHAPE morph feels stepped by ear, theirs is the thing to try next.

### K29 — Pickup position as a weight on each mode *(taken)*

The most useful idea in either module and the one this one lacked. A pickup at
a point along the string hears mode *n* at `cos(π·n·position)`. They apply
that as a weight on each mode, not as a comb filter on the signal, because a
comb that moves flanges and smears the attack. **POSITION** does the same: at
0 every mode is heard, at 1 the odd ones are gone and the sound is hollow.
It is a real timbre control that costs nothing (twenty-four weights a block,
ramped per sample, because the position is extremely sensitive to zipper
noise, as they note). A test checks the odd modes fall below 5% and the even
ones do not move.

### K30 — Stereo from a second pickup that drifts *(taken)*

Both modules take a second pickup, put it a little off the first, and move it
slowly with a low-frequency oscillator; the difference between the two is a
side signal, scaled by a width control and added to the left and subtracted from
the right. It gives the image life without any chorus or reverb. Here: the
**BRIDGE** output is now **L** and **R**; **SPREAD** is the width, at 0 the two
are identical to the bit (a test), and the mean of the two is the mono signal
whatever the spread. The second body pickup sits 0.08 away and drifts ±0.1 at
0.55 Hz; each sympathetic string has its own second pickup, a tap on its own
line wandering at its own slow rate. **Not taken:** Rings' split of the modes
into odd and even outputs (what you get for free from a pickup at the middle),
and their per-module randomised drift and offset (a seed derived from the
hardware: a property of a unit, not of a design; ours are fixed constants).

### K31 — Sympathetic strings: partners, glide, and a longer reach *(taken, partly)*

Rings' sympathetic strings differ from the first version here in three ways
that matter to the sound:

- **Detuned partners.** Half the strings are doubled a cent or two off, so the
  bank beats slowly instead of sounding like copies. Four of the eight are
  doubled (strings 2, 3, 4 and 6) at +1.2, −0.9, +1.6 and −1.4 cents: twelve
  lines in all (`SYMPATHETIC_LINES`), the eight the scenes tune and four partners.
  A tuned string with a partner measures as the pair, and the tuning test allows
  for it.
- **A slow retune.** When a new note moves the followed root, they glide the
  strings' pitch, as if retuning. The root now settles in 150 ms, up from 60.
- **Higher strings ring longer.** Each string's ring time is longer by 10% of
  RING per step up the table, so a struck bank blooms into its upper strings.

**Not taken:** their chord table (eleven equal-tempered chords, selected by a
separate knob; ours are six just-intoned scenes and a morph, K25), and the
fixed tonic string that always sounds beside the played notes. That one is a
good idea (the drone of a sitar's chikari or a tanpura's Sa, independent of the
FOLLOW switch) and is in `TODO.md`, not built. Also **not taken** and noted:
their strings are one-way, driven only by string 0; ours are driven by the sum
of all the voices. That is the module's polyphony, not a difference of
principle, and the coupling stays one-way (K25).

### K32 — External audio into the body, and what that leaves for later *(new)*

Elements is a resonator first: you send it sound. The **BODY IN** jack sends
audio straight into the shared bridge (the body and the sympathetic strings),
through the same input sum as the strings. A drum loop, a noise source or a
sampled pluck through the modal body is Elements' signature use, and it costs
one jack. **Left for later, noted in `TODO.md`:** Elements' bowed modes (banded
waveguides with a friction table), its breath-driven tube exciter, its reverb
(SPACE), and Rings' fixed tonic string.

### K33 — A summary of the constants and knobs this round changed

| What | Before | Now |
| --- | --- | --- |
| Body modes | 12, direct-form | 24, state-variable |
| Body knobs | SHAPE SIZE MATERIAL MIX TRACK | + POSITION, SPREAD |
| Bridge output | one mono jack | L and R |
| Settling of body knobs | 30 ms | 12 ms |
| Root glide | 60 ms | 150 ms |
| Sympathetic lines | 8 | 12 (8 + 4 partners) |
| High-mode ring floor | none | 8% of the first |
| Buzz | positive clip (lossy) | curved bridge (lossless) |
| New jack | | BODY IN |

The panel's right half moved to fit: POSITION and SPREAD sit beside the TRACK
switch, the SHAPE and SCENE CV jacks beside their sections, and the bottom
row is SEND, RETURN, BODY IN and the BRIDGE L and R pair.
