//! What the string must do, measured.
//!
//! These are properties that need loops over the keyboard or the parameter
//! space, which a scenario file cannot express. Single cases — one patch, one
//! set of assertions — belong in `scenarios/` instead.

use fathom_analysis::{non_finite_report, peak, rms, voct_tracking_error_cents_by_comb};
use karplus_core::{damp_seconds, decay_seconds, ExciterKind, Params};
use karplus_harness::render::{fundamental_t60, render, render_with, span, volts_to_hz, Pluck};

const SR: f32 = 48_000.0;

#[test]
fn every_note_is_in_tune() {
    // Tuned at the fundamental to within a cent across six octaves, at the
    // sample rates Rack offers most often, dark to bright. The JUCE version
    // was out by tens of cents at the top: linear interpolation, and nothing
    // accounted for the lowpass's delay.
    for rate in [44_100.0f32, 48_000.0, 96_000.0] {
        for brightness in [0.2f32, 0.45, 1.0] {
            for volts in [-3.0f32, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0] {
                let p = Params {
                    pitch_volts: volts,
                    brightness,
                    decay: 0.9,
                    ..Params::default()
                };
                // Forty periods at least, or the lowest notes cannot be
                // resolved to a cent.
                let window = (40.0 / volts_to_hz(volts)).max(0.6);
                let out = render(&p, &[Pluck::trigger(0.0)], window + 0.1, rate, 11);
                let tail = span(&out, rate, 0.05, window + 0.05);
                let cents = voct_tracking_error_cents_by_comb(
                    tail,
                    f64::from(rate),
                    volts_to_hz(volts),
                    300.0,
                )
                .expect("nothing to measure");
                assert!(
                    cents.abs() < 1.0,
                    "{rate} Hz, bright {brightness}, {volts:+} V: {cents:+.2} cents"
                );
            }
        }
    }
}

#[test]
fn the_decay_knob_means_what_it_says() {
    // With key tracking up, every note takes the dialed time to fall 60 dB.
    for volts in [-2.0f32, 0.0, 2.0] {
        for knob in [0.3f32, 0.6, 0.8] {
            for brightness in [0.45f32, 1.0] {
                let t = f64::from(decay_seconds(knob));
                let p = Params {
                    pitch_volts: volts,
                    decay: knob,
                    brightness,
                    key_track: 1.0,
                    ..Params::default()
                };
                let out = render(&p, &[Pluck::trigger(0.0)], t + 0.5, SR, 5);
                let measured =
                    fundamental_t60(&out, SR, volts_to_hz(volts)).expect("never decayed");
                assert!(
                    (measured / t - 1.0).abs() < 0.15,
                    "{volts:+} V, bright {brightness}: dialed {t:.2} s, measured {measured:.2} s"
                );
            }
        }
    }
}

#[test]
fn without_key_tracking_low_notes_ring_longer() {
    // The default, and how a real string behaves: the knob names the decay
    // of an A4, and an octave down rings twice as long.
    let knob = 0.45;
    let t = f64::from(decay_seconds(knob));
    let at = |volts: f32| {
        let p = Params {
            pitch_volts: volts,
            decay: knob,
            ..Params::default()
        };
        let out = render(&p, &[Pluck::trigger(0.0)], 3.0 * t + 0.5, SR, 5);
        fundamental_t60(&out, SR, volts_to_hz(volts)).expect("never decayed")
    };
    let a4 = at(0.75);
    let a3 = at(-0.25);
    assert!(
        (a4 / t - 1.0).abs() < 0.15,
        "A4 dialed {t:.2}, measured {a4:.2}"
    );
    assert!(
        (a3 / a4 - 2.0).abs() < 0.3,
        "A3 {a3:.2} s against A4 {a4:.2} s"
    );
}

#[test]
fn damping_stops_the_string_when_the_gate_falls() {
    let p = Params {
        decay: 0.9,
        damp: 0.7,
        ..Params::default()
    };
    let release = f64::from(damp_seconds(0.7).expect("damp is on"));
    let held = 0.3;
    let out = render(
        &p,
        &[Pluck {
            at_s: 0.0,
            held_s: held,
        }],
        held + release + 0.3,
        SR,
        3,
    );
    // Ringing freely while the gate is up.
    let before = rms(span(&out, SR, 0.2, held));
    assert!(before > 0.01, "the string was damped while held: {before}");
    // The note sixty decibels down once the damping time has passed.
    let window = (8.0 * f64::from(SR) / volts_to_hz(0.0)) as usize;
    let note = karplus_harness::render::partial_envelope(&out, SR, volts_to_hz(0.0), window);
    let at = |t: f64| note[(t * f64::from(SR)) as usize / window];
    let fall = 20.0 * (at(held + release + 0.02) / at(held - 0.05)).log10();
    assert!(
        fall < -60.0,
        "the note only fell {fall:.1} dB in {release:.2} s"
    );
    // What is left is the loop's DC blocker letting go of its own memory, a
    // few hertz, and it must be well down too.
    let after = rms(span(&out, SR, held + release, held + release + 0.05));
    let rest = 20.0 * (after / before).log10();
    assert!(
        rest < -45.0,
        "still at {rest:.1} dB after {release:.2} s of damping"
    );
}

#[test]
fn without_damping_the_gate_falling_changes_nothing() {
    // The JUCE version's founding rule: the string rings out. A long gate and
    // a short trigger must sound identical.
    let p = Params::default();
    let a = render(&p, &[Pluck::trigger(0.0)], 1.0, SR, 3);
    let b = render(
        &p,
        &[Pluck {
            at_s: 0.0,
            held_s: 0.5,
        }],
        1.0,
        SR,
        3,
    );
    assert_eq!(a, b);
}

#[test]
fn softer_velocity_is_quieter() {
    let at = |volts: f32| {
        let p = Params {
            velocity_volts: volts,
            velocity_patched: true,
            ..Params::default()
        };
        peak(&render(&p, &[Pluck::trigger(0.0)], 0.3, SR, 3))
    };
    let loud = at(10.0);
    let soft = at(2.5);
    let ratio = 20.0 * (soft / loud).log10();
    assert!(
        (-14.0..-10.0).contains(&ratio),
        "a quarter velocity was {ratio:.1} dB"
    );
}

#[test]
fn unpatched_velocity_is_full_velocity() {
    let full = Params {
        velocity_volts: 10.0,
        velocity_patched: true,
        ..Params::default()
    };
    let unpatched = Params {
        velocity_volts: 0.0,
        velocity_patched: false,
        ..Params::default()
    };
    assert_eq!(
        render(&full, &[Pluck::trigger(0.0)], 0.3, SR, 3),
        render(&unpatched, &[Pluck::trigger(0.0)], 0.3, SR, 3)
    );
}

#[test]
fn the_same_seed_is_the_same_pluck() {
    let p = Params {
        humanize: 1.0,
        ..Params::default()
    };
    let plucks = [Pluck::trigger(0.0), Pluck::trigger(0.2)];
    let a = render(&p, &plucks, 0.5, SR, 9);
    assert_eq!(a, render(&p, &plucks, 0.5, SR, 9));
    assert_ne!(a, render(&p, &plucks, 0.5, SR, 10));
}

#[test]
fn humanize_moves_each_pluck() {
    // Two plucks of the same note, far enough apart to measure each.
    let p = Params {
        humanize: 1.0,
        decay: 0.7,
        ..Params::default()
    };
    let out = render(&p, &[Pluck::trigger(0.0), Pluck::trigger(1.0)], 2.0, SR, 21);
    let pitch = |from: f64| {
        voct_tracking_error_cents_by_comb(
            span(&out, SR, from + 0.05, from + 0.95),
            f64::from(SR),
            volts_to_hz(0.0),
            100.0,
        )
        .expect("nothing to measure")
    };
    let first = pitch(0.0);
    let second = pitch(1.0);
    assert!(
        (first - second).abs() > 0.5,
        "{first:+.2} and {second:+.2} cents"
    );
    assert!(first.abs() < 35.0 && second.abs() < 35.0);
}

#[test]
fn no_bad_settings() {
    // Every corner of the parameter space, and a seeded scatter through the
    // middle of it: finite, bounded, and not silent.
    let mut state = 0x1234_5678u32;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state >> 8) as f32 / 16_777_216.0
    };
    let corner = |bits: u32, i: u32| if bits & (1 << i) != 0 { 1.0 } else { 0.0 };
    let mut cases = Vec::new();
    for bits in 0..512u32 {
        cases.push([
            corner(bits, 0),
            corner(bits, 1),
            corner(bits, 2),
            corner(bits, 3),
            corner(bits, 4),
            corner(bits, 5),
            corner(bits, 6),
            corner(bits, 7),
            corner(bits, 8),
        ]);
    }
    for _ in 0..200 {
        cases.push([
            next(),
            next(),
            next(),
            next(),
            next(),
            next(),
            next(),
            next(),
            next(),
        ]);
    }
    for (n, k) in cases.iter().enumerate() {
        let exciter = match n % 3 {
            0 => ExciterKind::Noise,
            1 => ExciterKind::Dust,
            _ => ExciterKind::Sine,
        };
        let p = Params {
            pitch_volts: -4.0 + 9.0 * k[0],
            decay: k[1],
            brightness: k[2],
            pick: k[3],
            tone: k[4],
            length: k[5],
            drive: k[6],
            damp: k[7],
            humanize: k[8],
            key_track: k[(n + 1) % 9],
            exciter,
            harmonic: 1 + (n as u32 % 8),
            ..Params::default()
        };
        let out = render(&p, &[Pluck::trigger(0.0)], 0.25, SR, n as u32 + 1);
        let report = non_finite_report(&out);
        assert!(report.is_clean(), "case {n} {p:?}: {report:?}");
        let top = peak(&out);
        assert!(top <= 1.0, "case {n} {p:?}: peak {top}");
        assert!(top > 1e-3, "case {n} {p:?}: silent, peak {top}");
    }
}

#[test]
fn the_audio_input_makes_a_resonator_that_stays_bounded() {
    // Full-scale noise into a string at full decay: the worst case for a
    // loop with gain this close to one. It must ring at its pitch and never
    // leave full scale.
    let mut state = 77u32;
    let noise: Vec<f32> = (0..(SR as usize * 2))
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state >> 8) as f32 / 8_388_608.0 - 1.0
        })
        .collect();
    let p = Params {
        decay: 1.0,
        brightness: 1.0,
        ..Params::default()
    };
    let out = render_with(&[], 2.0, SR, 1, |_| p, Some(&noise));
    assert!(peak(&out) <= 1.0);
    assert!(peak(&out) > 0.3, "barely rang: {}", peak(&out));
    assert!(non_finite_report(&out).is_clean());
    // And far past anything a cable carries, still bounded.
    let absurd: Vec<f32> = noise.iter().map(|x| x * 1e6).collect();
    let loud = render_with(&[], 0.5, SR, 1, |_| p, Some(&absurd));
    assert!(peak(&loud) <= 1.0 && non_finite_report(&loud).is_clean());
    let cents = voct_tracking_error_cents_by_comb(
        span(&out, SR, 1.0, 2.0),
        f64::from(SR),
        volts_to_hz(0.0),
        300.0,
    )
    .expect("nothing to measure");
    assert!(cents.abs() < 1.5, "resonated {cents:+.2} cents off");
}

#[test]
fn a_glide_has_no_steps() {
    // An octave in a twentieth of a second while the string rings, at the
    // hardware's block size of 64. The delay length ramps across each block;
    // stepped once per block instead, each step is a click riding on the
    // note, and this test fails — checked by making exactly that change.
    // Dark, so that the note is close to a sine and the click detector is
    // at its sharpest.
    let p = |t: f64| Params {
        pitch_volts: ((t as f32 - 0.5) * 20.0).clamp(0.0, 1.0),
        brightness: 0.3,
        decay: 1.0,
        key_track: 1.0,
        ..Params::default()
    };
    let out =
        karplus_harness::render::render_in_blocks(64, &[Pluck::trigger(0.0)], 1.0, SR, 1, p, None);
    let report = fathom_analysis::discontinuity(span(&out, SR, 0.4, 0.65)).expect("too short");
    assert!(report.excess < 0.5, "{report:?}");
}

#[test]
fn fathoms_dc_blocker_is_the_filter_the_tuning_assumes() {
    // The tuning arithmetic predicts the blocker's response from a copy of
    // Fathom's private coefficient formula. If Fathom ever changes it, this
    // is where it shows — not as a string that has drifted out of tune.
    for (corner, hz) in [(1.0f32, 100.0f32), (0.5, 261.6), (8.0, 4_000.0)] {
        let mut blocker = fathom_core::DcBlocker::new(SR);
        blocker.set_corner_hz(corner);
        let turns = hz / SR;
        let mut energy = 0.0f64;
        let mut count = 0.0f64;
        let mut cross = 0.0f64;
        for i in 0..(SR as usize * 4) {
            let x = fathom_core::sin_turns(turns * i as f32);
            let y = blocker.process(x);
            if i >= SR as usize * 2 {
                energy += f64::from(y) * f64::from(y);
                // Correlate with the input's quadrature to read the phase.
                cross += f64::from(y) * f64::from(fathom_core::sin_turns(turns * i as f32 + 0.25));
                count += 1.0;
            }
        }
        let measured_gain = (2.0 * energy / count).sqrt();
        let measured_phase = (2.0 * cross / count / measured_gain).asin();
        let predicted = karplus_core::dc_blocker_response(
            karplus_core::fathom_dc_coefficient(corner, SR),
            turns,
        );
        let predicted_phase =
            -f64::from(predicted.delay) * std::f64::consts::TAU * f64::from(turns);
        assert!(
            (measured_gain - f64::from(predicted.gain)).abs() < 1e-3,
            "{corner} Hz corner at {hz} Hz: gain {measured_gain} vs {}",
            predicted.gain
        );
        assert!(
            (measured_phase - predicted_phase).abs() < 1e-3,
            "{corner} Hz corner at {hz} Hz: phase {measured_phase} vs {predicted_phase}"
        );
    }
}
