# What Karplus-Strong borrowed from Fathom

The experiment was to rebuild this instrument the way Fathom is built, using
Fathom's code wherever it would go, without changing anything in Fathom. The
point was to find out which of Fathom's primitives are ready to become
separate crates for other developers, and which are not. This page is the
result.

The dependency is real, not copied: `Cargo.toml` pins Fathom at
`be4d00b` (the `main` of 2026-09-19) as a git dependency. Wherever a Fathom
API did not fit, nothing on Fathom's side was changed. Each of those places
is listed below with what got in the way.

## Summary

| Fathom piece | Used? | Ready for its own crate? |
| --- | --- | --- |
| `fathom-analysis` (measurement) | Every function needed, unchanged | **Yes.** Worked unchanged on a very different instrument. |
| `fathom-core` maths: `exp2`, `sin_turns`, `floor`, `clamp` | Yes, unchanged | **Yes.** No changes needed. |
| `fathom-core::Phase`, `increment_from_turns` | Yes (sine exciter) | **Yes.** |
| `fathom-core::DcBlocker` | Yes, with `set_corner_hz` | **Almost.** It needs to expose its coefficient and response. |
| `fathom-core::OnePole` | **No** | **No.** The cutoff law and the API both need work, explained below. |
| `fathom-core` limiter / soft-knee curve | **No**, it is private | Yes, once it is public. |
| `fathom-core::filter::flush_denormal` | **No**, it is `pub(crate)` | Yes, once it is public. |
| `fathom-harness`: `Seconds`, `Pitch`, `Automation`, `Bound`, `wav`, `Error` | Yes, unchanged | **Yes, but split out.** They are generic; the crate they live in is not. |
| `fathom-harness`: `Scenario`, metrics, runner | **No** | **No.** They are built around Fathom's oscillator. |
| `fathom-ffi` + header drift test | Pattern copied, near line for line | Template, not crate |
| `targets/vcv/controls.hpp` | Pattern and `unipolar` copied | Template, not crate |
| `targets/vcv/res/panel.py` (stroke font) | 200 lines copied wholesale | **Yes.** A small panel tool would save every module this. |
| `fathom-ui`, `fathom-sim`, `fathom-reference`, `targets/daisy` | Not needed | — |

## What worked unchanged

**`fathom-analysis`** is the clearest result. It measured tuning to a
hundredth of a cent, DC, peaks, levels, clicks and numerical health on a
plucked string, which is nothing like the oscillator it was written for.
Nothing in it needed changing. Two lessons, both already written in its own
docs:

- `measure_fundamental_hz` takes the loudest peak. A string plucked away from
  its end often has a second or third harmonic louder than the first, and it
  then reported 1200 or 1900 cents. `measure_fundamental_by_comb`, written for
  FM, got it right every time. For a general-purpose crate, the comb version
  should probably be the default.
- `discontinuity` compares each window with its neighbours. On a note that is
  fading fast, the first window looks like a click. The scenario has to be
  set up to sustain. This is documented in the crate; it just catches you out
  the first time.

**The maths** (`exp2`, `sin_turns`, `floor`, `clamp`) and **`Phase`** were
used as they are. The NaN-safe `clamp` and the always-finite `exp2` prevented
problems that would otherwise have needed guard code here.

**The scenario building blocks** in `fathom-harness` were used as they are:
`Seconds` (`4s`, `250ms`), `Pitch` (`C4`, `440 Hz`), `Automation` (sweeps
with linear, exponential and smooth shapes), `Bound` (`{ min, max }`), and
the WAV writer. The scenario files in this repository look and read like
Fathom's because of that.

## What did not fit, and why

### `OnePole`: the cutoff stops working above 7.6 kHz, and its response cannot be read

`OnePole` maps the cutoff to its coefficient as `2π·fc/fs`, clamped at 1.
That approximation is accurate well below Nyquist and not near it. The
coefficient reaches 1, and the filter turns itself off, at `fs/2π`: 7.6 kHz at
48 kHz. In Fathom's low-pass gate that does no harm. In a string loop, the
top third of the BRIGHT knob would have done nothing.

More importantly, the coefficient is private, so nothing outside the crate can
ask what the filter does to a given frequency. A string needs exactly that
answer to stay in tune (the filter's delay must be taken off the loop length)
and to decay for the dialed time (its loss must be made up).

This repository has `LoopFilter` instead. Its cutoff mapping is
`1 − e^(−2π·fc/fs)`, which never reaches 1, and it has `coefficient()`,
`set_coefficient()` and `response(turns) → { gain, delay }`.

**Proposal:** one shared one-pole with an accurate cutoff mapping and a
`response()` method. Fathom's LPG would get the more accurate cutoff for free.

### `DcBlocker`: good, but its coefficient is private too

It is used here through `set_corner_hz`. The tuning maths needs its phase
response, and the coefficient formula is private, so
`karplus_core::fathom_dc_coefficient` **duplicates Fathom's arithmetic**. A
test (`fathoms_dc_blocker_is_the_filter_the_tuning_assumes`) measures the
real blocker against that copy, so if Fathom's formula ever changes, the test
fails loudly instead of the strings quietly going out of tune.

A second finding: `follow()` puts the corner at a 64th of the note. That is
right for an output stage, which is what it was chosen for. Inside a feedback
loop it detunes every harmonic above the fundamental by up to 4.3 cents
(measured). Here the corner sits at a 512th of the note. Where the corner
goes depends on the job, so `set_corner_hz` is the right API and `follow` is
Fathom's own policy.

**Proposal:** add `coefficient()` and `response(turns)`. Then delete the
duplicate here.

### Private pieces that are worth sharing

- **`flush_denormal`** (`pub(crate)`). It is copied into `karplus_core::math`
  with the same threshold.
- **The soft-knee curve inside the output limiter** (`saturate.rs`, private).
  It is copied as `karplus_core::math::soft_knee`. Here it keeps the string's
  loop bounded rather than limiting an output, which shows it is useful
  beyond one job.
- **A square root and a `log2`**, each written as a helper inside a test
  module (`oversample.rs`, `modulator.rs`). Both are needed by the core
  itself, not only by tests.

### Maths Fathom does not have

A string needed `sqrt`, `atan2`, `tanh`, `exp` and a seeded noise generator.
All are in `karplus_core::math`, written to Fathom's rules: plain `f32`, no
tables, no `libm`, tested against `f64` `std` in `karplus-harness/tests/math.rs`.

### A primitive Fathom does not have: the delay line

`karplus_core::DelayLine<N>` is a fixed-size, allocation-free, panic-free
delay line with a third-order Lagrange fractional read. Any physical model,
chorus, flanger or comb filter needs one. It has exactly one user so far, so
its API is not proven yet.

### Depending on a whole oscillator to get a few primitives

`fathom-core` is the Fathom oscillator plus its building blocks. Depending on
it compiles all of `Osc` into this crate (the linker throws it away). It also
means any change to Fathom's oscillator is a reason to update the pin here,
even though nothing used here changed. The same is true of `fathom-harness`:
six generic types bring along `fathom-ui`, `embedded-graphics`,
`fathom-reference`, `clap` and the scenario runner.

### The private repository

Fathom is private, so the git dependency needs GitHub credentials
(`.cargo/config.toml` sets `git-fetch-with-cli`). A machine logged in to
GitHub builds normally. CI in this repository cannot fetch Fathom without a
token, which is why `.github/workflows/verify.yml` needs a
`FATHOM_READ_TOKEN` secret. For an ecosystem, the shared crates must be
public.

## Recommended split, when the time comes

`docs/architecture.md` in Fathom says to keep everything unpublished until
the APIs have been used for a while. This experiment is the first second
user. Based on it:

1. **A maths crate** (`no_std`, zero dependencies): `clamp`, `floor`, `exp2`,
   `exp`, `log2`, `sqrt`, `sin_turns`, `cos_turns`, `tanh`, `atan2`,
   `flush_denormal`, a seeded RNG. Fathom's half is settled. This repository's
   half is tested, but has one user.
2. **A building-blocks crate** (`no_std`): `Phase`, the DC blocker (with
   `response`), a one-pole lowpass (redesigned as above), the soft knee and
   limiter, `DelayLine`, Fathom's `Decimator`. `Phase` and the DC blocker are
   settled. The one-pole needs the redesign first. The delay line needs a
   second user.
3. **`fathom-analysis` under a neutral name.** Ready now.
4. **A scenario kit**: `Seconds`, `Pitch`, `Automation`, `Bound`, WAV, errors,
   plus a small trait for "render this instrument with these named parameters
   and these events". This repository's `karplus-harness` and Fathom's
   harness are the two designs to merge. Not settled.
5. **Templates, not crates:** the FFI crate with its header drift test, the
   `controls.hpp` pattern, the justfile, and the panel generator with its
   stroke font. The panel generator could become a small tool. The others are
   better copied than depended on, because each instrument's shim is its own.

What this experiment did not test: `fathom-ui` (no screen here), the Daisy
target, the instruction budgets, and Fathom's oversampling.
