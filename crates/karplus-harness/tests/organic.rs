//! The organic-sound features (K19–K24): stiffness, tension, buzz, the shared
//! body, the sympathetic strings and the outboard loop.
//!
//! Single cases live in `scenarios/`. What is here needs a loop over notes,
//! corners or tunings, or a measurement a scenario cannot name (the pitch of
//! one partial).

use fathom_analysis::{non_finite_report, peak, voct_tracking_error_cents_by_comb};
use karplus_core::{
    Bridge, BridgeParams, Params, Voice, BODY_MODES, LOOP_LATENCY, SCENES, SYMPATHETIC_STRINGS,
    ZERO_VOLT_HZ,
};
use karplus_harness::render::{fundamental_t60, render, render_chain, span, volts_to_hz, Pluck};

const SR: f32 = 48_000.0;

/// The frequency of the strongest component within `cents` of `centre_hz`:
/// a Hann-windowed DFT scanned finely, then a parabola through the top.
fn peak_hz(signal: &[f32], centre_hz: f64, cents: f64) -> f64 {
    let sr = f64::from(SR);
    let n = signal.len();
    let window: Vec<f64> = (0..n)
        .map(|i| 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos())
        .collect();
    let magnitude = |hz: f64| -> f64 {
        let w = std::f64::consts::TAU * hz / sr;
        let (mut re, mut im) = (0.0, 0.0);
        for (i, (x, h)) in signal.iter().zip(&window).enumerate() {
            let phase = w * i as f64;
            re += h * f64::from(*x) * phase.cos();
            im -= h * f64::from(*x) * phase.sin();
        }
        (re * re + im * im).sqrt()
    };
    let step = 0.5;
    let count = (2.0 * cents / step) as usize;
    let grid: Vec<(f64, f64)> = (0..=count)
        .map(|i| {
            let c = -cents + i as f64 * step;
            let hz = centre_hz * 2f64.powf(c / 1200.0);
            (c, magnitude(hz))
        })
        .collect();
    let (best, _) = grid.iter().enumerate().fold(
        (0, 0.0),
        |b, (i, (_, m))| if *m > b.1 { (i, *m) } else { b },
    );
    let at = |i: usize| grid.get(i).map_or(0.0, |g| g.1);
    let (a, b, c) = (at(best.saturating_sub(1)), at(best), at(best + 1));
    let denominator = a - 2.0 * b + c;
    let offset = if denominator.abs() > 1e-12 {
        0.5 * (a - c) / denominator
    } else {
        0.0
    };
    let cents_found = grid.get(best).map_or(0.0, |g| g.0) + offset * step;
    centre_hz * 2f64.powf(cents_found / 1200.0)
}

fn cents(found: f64, expected: f64) -> f64 {
    1200.0 * (found / expected).log2()
}

fn pluck_params(volts: f32) -> Params {
    Params {
        pitch_volts: volts,
        decay: 0.9,
        key_track: 1.0,
        ..Params::default()
    }
}

// --- K19: stiffness ---------------------------------------------------------

#[test]
fn stiffness_keeps_the_fundamental_in_tune() {
    for volts in [-2.0f32, -1.0, 0.0, 1.0, 2.0] {
        for stiffness in [0.25f32, 0.5, 0.75, 1.0] {
            let p = Params {
                stiffness,
                ..pluck_params(volts)
            };
            let hz = volts_to_hz(volts);
            let out = render(&p, &[Pluck::trigger(0.0)], 1.6, SR, 3);
            let tail = span(&out, SR, 0.1, 1.5);
            let found = peak_hz(tail, hz, 40.0);
            assert!(
                cents(found, hz).abs() < 1.0,
                "{volts:+} V, stiffness {stiffness}: {:+.2} cents",
                cents(found, hz)
            );
        }
    }
}

#[test]
fn stiffness_stretches_the_upper_partials() {
    // The point of the knob: partial n sits above n × the fundamental, more
    // as n grows. And at no stiffness it does not.
    let hz = volts_to_hz(-1.0); // C3
    let sharp_of_harmonic = |stiffness: f32, n: f64| {
        let p = Params {
            stiffness,
            brightness: 0.8,
            ..pluck_params(-1.0)
        };
        let out = render(&p, &[Pluck::trigger(0.0)], 2.0, SR, 3);
        let tail = span(&out, SR, 0.1, 1.5);
        cents(peak_hz(tail, hz * n, 150.0), hz * n)
    };
    for n in [4.0, 8.0] {
        let plain = sharp_of_harmonic(0.0, n);
        let stiff = sharp_of_harmonic(1.0, n);
        assert!(
            plain.abs() < 3.0,
            "partial {n} of an ideal string: {plain:+.1} cents"
        );
        assert!(
            stiff > 8.0,
            "partial {n} of a stiff string: {stiff:+.1} cents"
        );
    }
    // More stiffness, more stretch.
    let some = sharp_of_harmonic(0.6, 8.0);
    let most = sharp_of_harmonic(1.0, 8.0);
    assert!(most > some + 3.0, "{some:+.1} then {most:+.1} cents");
}

#[test]
fn stiffness_does_not_change_how_long_a_note_rings() {
    for volts in [-1.0f32, 0.0, 1.0] {
        let hz = volts_to_hz(volts);
        let t60 = |stiffness: f32| {
            let p = Params {
                stiffness,
                decay: 0.55,
                ..pluck_params(volts)
            };
            let out = render(&p, &[Pluck::trigger(0.0)], 6.0, SR, 5);
            fundamental_t60(&out, SR, hz).expect("no decay measured")
        };
        let (plain, stiff) = (t60(0.0), t60(1.0));
        assert!(
            (stiff / plain - 1.0).abs() < 0.15,
            "{volts:+} V: {plain:.2} s without, {stiff:.2} s with"
        );
    }
}

// --- K20: tension modulation ------------------------------------------------

#[test]
fn a_hard_pluck_chirps_down_and_a_soft_one_barely_moves() {
    let hz = volts_to_hz(0.0);
    let pitch_in = |p: &Params, volts_velocity: Option<f32>, from: f64, to: f64| {
        let p = Params {
            velocity_patched: volts_velocity.is_some(),
            velocity_volts: volts_velocity.unwrap_or(10.0),
            ..*p
        };
        let out = render(&p, &[Pluck::trigger(0.0)], 3.2, SR, 7);
        voct_tracking_error_cents_by_comb(span(&out, SR, from, to), f64::from(SR), hz, 300.0)
            .expect("nothing measured")
    };
    let off = pluck_params(0.0);
    let on = Params {
        tension: 1.0,
        ..off
    };
    let early = pitch_in(&on, None, 0.1, 0.25);
    let late = pitch_in(&on, None, 1.5, 3.0);
    assert!(early > 10.0, "a hard pluck began {early:+.1} cents out");
    assert!(late.abs() < 1.5, "and ended {late:+.1} cents out");
    // The opposing constraint: with the knob down it is in tune throughout.
    let flat = pitch_in(&off, None, 0.1, 0.25);
    assert!(flat.abs() < 1.0, "no tension, yet {flat:+.1} cents");
    // And a soft pluck chirps less than a hard one.
    let soft = pitch_in(&on, Some(2.0), 0.1, 0.25);
    assert!(soft < 0.5 * early, "soft {soft:+.1}, hard {early:+.1}");
}

// --- K21: buzz ----------------------------------------------------------------

#[test]
fn buzz_squeezes_one_side_of_the_wave_and_leaves_the_tuning() {
    // A sitar's bridge touches the string on one side only: the positive
    // swing is cut short, the negative is not. Measured as the ratio of the
    // two peaks while the note is loud enough to reach the knee.
    let hz = volts_to_hz(-1.0);
    let render_with_buzz = |buzz: f32| {
        let p = Params {
            buzz,
            decay: 0.6,
            ..pluck_params(-1.0)
        };
        render(&p, &[Pluck::trigger(0.0)], 8.0, SR, 9)
    };
    let one_sidedness = |out: &[f32], from: f64, to: f64| {
        let tail = span(out, SR, from, to);
        let max = tail.iter().copied().fold(0.0f32, f32::max);
        let min = tail.iter().copied().fold(0.0f32, f32::min);
        f64::from(max / -min)
    };
    let plain = render_with_buzz(0.0);
    let buzzy = render_with_buzz(1.0);
    let (a, b) = (
        one_sidedness(&plain, 0.02, 0.15),
        one_sidedness(&buzzy, 0.02, 0.15),
    );
    assert!(b < 0.6 * a, "peak ratio {a:.2} without, {b:.2} with");
    // Half the knob is in between.
    let half = one_sidedness(&render_with_buzz(0.5), 0.02, 0.15);
    assert!(b < half && half < a, "{a:.2}, {half:.2}, {b:.2}");
    // It does not move the note, take its sustain, or leave DC behind.
    let found = peak_hz(span(&buzzy, SR, 0.1, 1.4), hz, 40.0);
    assert!(
        cents(found, hz).abs() < 1.0,
        "{:+.1} cents",
        cents(found, hz)
    );
    let t60 = |out: &[f32]| fundamental_t60(out, SR, hz).expect("no decay");
    assert!(
        t60(&buzzy) > 0.8 * t60(&plain),
        "{:.1} s against {:.1} s",
        t60(&buzzy),
        t60(&plain)
    );
    let mean = fathom_analysis::dc_offset_dbfs(span(&buzzy, SR, 0.0, 1.0));
    assert!(mean < -50.0, "{mean:.1} dBFS of DC");
}

// --- All three, and the outboard loop: no bad settings --------------------------

#[test]
fn no_bad_settings_with_the_loop_stages_up() {
    let mut state = 0xdead_beefu32;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state >> 8) as f32 / 16_777_216.0
    };
    for n in 0..300 {
        let corner = |i: u32| if (n >> i) & 1 == 1 { 1.0 } else { 0.0 };
        let (a, b, c) = if n < 8 {
            (corner(0), corner(1), corner(2))
        } else {
            (next(), next(), next())
        };
        let p = Params {
            pitch_volts: -4.0 + 9.0 * next(),
            stiffness: a,
            tension: b,
            buzz: c,
            drive: next(),
            decay: next(),
            brightness: next(),
            ..Params::default()
        };
        let out = render(&p, &[Pluck::trigger(0.0)], 0.3, SR, n + 1);
        let report = non_finite_report(&out);
        assert!(report.is_clean(), "case {n} {p:?}: {report:?}");
        let top = peak(&out);
        assert!(top <= 1.0, "case {n} {p:?}: peak {top}");
        assert!(top > 1e-3, "case {n} {p:?}: silent");
    }
}

// --- K22: the outboard loop ---------------------------------------------------

fn through_outboard(volts: f32, lowpass_hz: f64, trim_volts: f32) -> Vec<f32> {
    render_chain(
        16,
        &[Pluck::trigger(0.0)],
        2.2,
        SR,
        3,
        |_| Params {
            loop_trim_volts: trim_volts,
            ..pluck_params(volts)
        },
        None,
        Some(lowpass_hz),
        None,
    )
}

fn outboard_error(volts: f32, lowpass_hz: f64, trim_volts: f32) -> f64 {
    let hz = volts_to_hz(volts);
    let out = through_outboard(volts, lowpass_hz, trim_volts);
    cents(peak_hz(span(&out, SR, 0.1, 2.0), hz, 120.0), hz)
}

#[test]
fn an_outboard_loop_that_does_nothing_is_in_tune() {
    // A filter that passes everything leaves only the cables' delay, which the
    // string has already given up. In tune across the range the loop can
    // reach: the period must exceed the latency and the line's minimum.
    for volts in [-3.0f32, -2.0, -1.0, 0.0] {
        let error = outboard_error(volts, 20_000.0, 0.0);
        assert!(error.abs() < 2.5, "{volts:+} V: {error:+.2} cents");
    }
}

#[test]
fn an_outboard_filter_detunes_and_the_trim_corrects_it() {
    // A dark filter has group delay of its own: the note goes flat. That is the
    // filter's doing, not the module's, and TRIM is there for it.
    let volts = -1.0;
    let off = outboard_error(volts, 800.0, 0.0);
    assert!(
        off < -10.0,
        "a dark outboard filter should detune, got {off:+.1}"
    );
    let trim = (-off / 1200.0) as f32;
    assert!(
        trim < 1.0 / 24.0,
        "the correction must fit in the trim's range"
    );
    let fixed = outboard_error(volts, 800.0, trim);
    assert!(
        fixed.abs() < 3.0,
        "{off:+.1} cents, then {fixed:+.1} with trim"
    );
}

#[test]
fn an_outboard_loop_rings_for_about_as_long_as_dialed() {
    let hz = volts_to_hz(-1.0);
    let knob = 0.55;
    let out = render_chain(
        16,
        &[Pluck::trigger(0.0)],
        8.0,
        SR,
        3,
        |_| Params {
            decay: knob,
            ..pluck_params(-1.0)
        },
        None,
        Some(20_000.0),
        None,
    );
    let t60 = fundamental_t60(&out, SR, hz).expect("no decay");
    let wanted = f64::from(karplus_core::decay_seconds(knob));
    assert!(
        (t60 / wanted - 1.0).abs() < 0.25,
        "{t60:.2} s, wanted {wanted:.2} s"
    );
}

#[test]
fn an_unpatched_return_is_the_internal_loop() {
    // Whatever arrives on the return jack is ignored until the jack is patched.
    let p = pluck_params(0.0);
    let gate = [10.0f32; 16];
    let noise: Vec<f32> = (0..16).map(|i| (i as f32 * 1.7).sin()).collect();
    let mut plain = Voice::new(SR, 1);
    let mut loose = Voice::new(SR, 1);
    let (mut a, mut b, mut send) = ([0.0f32; 16], [0.0f32; 16], [0.0f32; 16]);
    for _ in 0..100 {
        plain.process(&p, &gate, &[], &mut a);
        loose.process_loop(&p, &gate, &[], &noise, Some(&mut send), &mut b);
        assert_eq!(a, b);
    }
}

#[test]
fn the_send_is_what_leaves_the_delay_line() {
    let p = pluck_params(0.0);
    let mut voice = Voice::new(SR, 1);
    let (mut out, mut send) = ([0.0f32; 16], [0.0f32; 16]);
    let mut heard = 0.0f32;
    for _ in 0..40 {
        voice.process_loop(&p, &[10.0; 16], &[], &[], Some(&mut send), &mut out);
        heard = heard.max(peak(&send) as f32);
    }
    assert!(heard > 0.05, "nothing on the send");
    assert!(heard <= 1.0 + 1e-3, "send {heard}");
}

#[test]
fn the_latency_constant_is_what_the_shim_adds() {
    // 16 samples to play out, 16 to collect, one cable each way: see K22. If
    // the shim's block changes, this and the constant change together.
    assert_eq!(LOOP_LATENCY, 2.0 * 16.0 + 2.0);
}

// --- K19–K21 on the bridge: the body and the sympathetic strings ---------------

fn bridge_run(p: &BridgeParams, input: &[f32], seconds: f64) -> Vec<f32> {
    let mut bridge = Bridge::new(SR);
    let total = (seconds * f64::from(SR)) as usize;
    let mut out = vec![0.0f32; total];
    for (b, block) in out.chunks_mut(16).enumerate() {
        let start = b * 16;
        let end = (start + block.len()).min(input.len());
        let slice = input.get(start..end).unwrap_or(&[]);
        bridge.process(p, slice, block);
    }
    out
}

fn noise(len: usize, level: f32, seed: u32) -> Vec<f32> {
    let mut state = seed | 1;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            level * ((state >> 8) as f32 / 8_388_608.0 - 1.0)
        })
        .collect()
}

#[test]
fn a_bridge_with_nothing_turned_up_is_a_wire() {
    let input = noise(4_800, 0.5, 1);
    let out = bridge_run(&BridgeParams::default(), &input, 0.1);
    for (x, y) in input.iter().zip(&out) {
        assert_eq!(x, y);
    }
}

#[test]
fn the_bridge_is_bounded_at_every_corner() {
    // Sixteen full-scale voices at once, then silence. Finite, inside full
    // scale, and nothing left subnormal afterwards.
    let loud = noise(24_000, 16.0, 7);
    for bits in 0..1024u32 {
        let bit = |i: u32| if bits & (1 << i) != 0 { 1.0f32 } else { 0.0 };
        let p = BridgeParams {
            root_volts: 4.0 * bit(9) - 2.0,
            body_shape: bit(0),
            body_size: bit(1),
            body_material: bit(2),
            body_mix: bit(3),
            body_track: bit(4) > 0.5,
            scene: bit(5),
            root: bit(6),
            level: 1.0,
            decay: bit(7),
            follow: bit(8) > 0.5,
        };
        let mut bridge = Bridge::new(SR);
        let mut block = [0.0f32; 16];
        for b in 0..(24_000 / 16 + 2_000) {
            let start = b * 16;
            let input = loud.get(start..start + 16).unwrap_or(&[]);
            bridge.process(&p, input, &mut block);
            for &x in &block {
                assert!(x.is_finite() && x.abs() <= 1.0, "corner {bits}: {x}");
            }
        }
    }
}

#[test]
fn a_long_silence_leaves_no_subnormals_in_the_bridge() {
    let p = BridgeParams {
        body_mix: 1.0,
        body_material: 1.0,
        level: 1.0,
        decay: 1.0,
        ..BridgeParams::default()
    };
    let mut bridge = Bridge::new(SR);
    let mut block = [0.0f32; 16];
    let burst = noise(480, 1.0, 3);
    bridge.process(&p, &burst[..16], &mut block);
    for _ in 0..(48_000 * 90 / 16) {
        bridge.process(&p, &[], &mut block);
    }
    let smallest = bridge.smallest_nonzero_state();
    assert!(
        smallest == 0.0 || smallest >= karplus_core::math::DENORMAL_FLOOR,
        "state of {smallest}"
    );
}

#[test]
fn the_same_input_gives_the_same_bridge_bit_for_bit() {
    let p = BridgeParams {
        body_mix: 0.7,
        level: 0.8,
        scene: 0.4,
        ..BridgeParams::default()
    };
    let input = noise(9_600, 1.0, 5);
    assert_eq!(bridge_run(&p, &input, 0.5), bridge_run(&p, &input, 0.5));
    let mut a = Bridge::new(SR);
    let mut scratch = [0.0f32; 16];
    a.process(&p, &input[..16], &mut scratch);
    a.reset();
    let mut b = Bridge::new(SR);
    let (mut x, mut y) = ([0.0f32; 16], [0.0f32; 16]);
    a.process(&p, &input[..16], &mut x);
    b.process(&p, &input[..16], &mut y);
    assert_eq!(x, y);
}

#[test]
fn the_body_rings_at_its_modes() {
    // One strike, glass, the harmonic shape: modes at whole multiples of the
    // first, which sits where SIZE puts it.
    let size = 0.5;
    let first = 800.0 * 2f64.powf(-5.0 * f64::from(size));
    let p = BridgeParams {
        body_mix: 1.0,
        body_material: 1.0,
        body_size: size,
        body_shape: 0.0,
        ..BridgeParams::default()
    };
    let mut strike = vec![0.0f32; 16];
    strike[0] = 1.0;
    let out = bridge_run(&p, &strike, 3.0);
    let tail = span(&out, SR, 0.2, 2.2);
    for n in [1.0f64, 2.0, 3.0] {
        let found = peak_hz(tail, first * n, 60.0);
        assert!(
            cents(found, first * n).abs() < 1.5,
            "mode {n}: {:+.2} cents",
            cents(found, first * n)
        );
    }
}

#[test]
fn a_tracked_body_follows_the_note_and_a_fixed_one_does_not() {
    let mut strike = vec![0.0f32; 16];
    strike[0] = 1.0;
    let first_mode = |volts: f32, track: bool| {
        let p = BridgeParams {
            root_volts: volts,
            body_mix: 1.0,
            body_material: 1.0,
            body_track: track,
            ..BridgeParams::default()
        };
        let out = bridge_run(&p, &strike, 3.0);
        let tail = span(&out, SR, 0.2, 2.2);
        let centre = f64::from(ZERO_VOLT_HZ) * 2f64.powf(f64::from(volts));
        // Tracked at the middle of SIZE the first mode is the note.
        peak_hz(tail, if track { centre } else { 141.4 }, 80.0)
    };
    let (c4, c5) = (first_mode(0.0, true), first_mode(1.0, true));
    assert!((c5 / c4 - 2.0).abs() < 0.01, "{c4:.1} Hz then {c5:.1} Hz");
    let (a, b) = (first_mode(0.0, false), first_mode(1.0, false));
    assert!((a / b - 1.0).abs() < 0.01, "{a:.1} Hz then {b:.1} Hz");
}

#[test]
fn the_shape_knob_moves_the_modes_apart() {
    // Harmonic puts the second mode at 2×; the bar puts it at 2.76×.
    let ring = |shape: f32| {
        let p = BridgeParams {
            body_mix: 1.0,
            body_material: 1.0,
            body_shape: shape,
            ..BridgeParams::default()
        };
        let mut strike = vec![0.0f32; 16];
        strike[0] = 1.0;
        let out = bridge_run(&p, &strike, 3.0);
        let tail = span(&out, SR, 0.3, 2.5);
        let first = 800.0 * 2f64.powf(-2.5);
        peak_hz(tail, 2.4 * first, 450.0) / first
    };
    let harmonic = ring(0.0);
    let bar = ring(1.0 / 3.0);
    assert!(
        (harmonic - 2.0).abs() < 0.02,
        "harmonic second mode at {harmonic:.3}×"
    );
    assert!((bar - 2.757).abs() < 0.03, "bar second mode at {bar:.3}×");
}

#[test]
fn material_sets_how_long_the_body_rings() {
    let tail_level = |material: f32| {
        let p = BridgeParams {
            body_mix: 1.0,
            body_material: material,
            ..BridgeParams::default()
        };
        let mut strike = vec![0.0f32; 16];
        strike[0] = 1.0;
        let out = bridge_run(&p, &strike, 2.0);
        fathom_analysis::rms_dbfs(span(&out, SR, 0.5, 1.5))
    };
    let (wood, glass) = (tail_level(0.0), tail_level(1.0));
    assert!(wood < -100.0, "wood still ringing at {wood:.1} dBFS");
    assert!(glass > -75.0, "glass already silent at {glass:.1} dBFS");
}

#[test]
fn turning_the_body_while_it_rings_does_not_click() {
    let mut bridge = Bridge::new(SR);
    let mut strike = [0.0f32; 16];
    strike[0] = 1.0;
    let mut out = Vec::new();
    let mut block = [0.0f32; 16];
    for b in 0..(48_000 * 2 / 16) {
        let t = b as f32 * 16.0 / SR;
        let p = BridgeParams {
            body_mix: 1.0,
            body_material: 1.0,
            body_shape: (t * 0.5).min(1.0),
            body_size: 0.2 + 0.6 * (t * 0.5).min(1.0),
            ..BridgeParams::default()
        };
        bridge.process(&p, if b == 0 { &strike } else { &[] }, &mut block);
        out.extend_from_slice(&block);
    }
    let report =
        fathom_analysis::discontinuity(span(&out, SR, 0.3, 1.9)).expect("too short to measure");
    assert!(report.excess < 0.5, "{report:?}");
}

/// The tuning of string `index` in scene `scene`, as the root's multiple.
fn scene_ratio(scene: usize, index: usize) -> f64 {
    let table: [[f64; 8]; SCENES] = [
        [0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0],
        [1.0, 5.0 / 4.0, 3.0 / 2.0, 2.0, 5.0 / 2.0, 3.0, 4.0, 5.0],
        [
            1.0,
            6.0 / 5.0,
            3.0 / 2.0,
            9.0 / 5.0,
            2.0,
            12.0 / 5.0,
            3.0,
            4.0,
        ],
        [
            1.0,
            16.0 / 15.0,
            5.0 / 4.0,
            4.0 / 3.0,
            3.0 / 2.0,
            8.0 / 5.0,
            15.0 / 8.0,
            2.0,
        ],
        [0.75, 1.0, 1.0, 2.0, 1.5, 3.0, 0.5, 4.0],
        [1.0, 1.19, 1.43, 1.77, 2.13, 2.61, 3.17, 3.84],
    ];
    table
        .get(scene)
        .and_then(|row| row.get(index))
        .copied()
        .unwrap_or(1.0)
}

#[test]
fn the_sympathetic_strings_sit_in_the_tuning_of_their_scene() {
    assert_eq!(SYMPATHETIC_STRINGS, 8);
    // A burst, then silence: whatever rings afterwards is the strings alone.
    // Every string of every scene must ring within a cent of root × ratio.
    let root_hz = f64::from(ZERO_VOLT_HZ); // C4, followed
    let burst = noise(2_400, 1.0, 11);
    for scene in 0..SCENES {
        let p = BridgeParams {
            root_volts: 0.0,
            scene: scene as f32 / (SCENES - 1) as f32,
            level: 1.0,
            decay: 1.0,
            ..BridgeParams::default()
        };
        let out = bridge_run(&p, &burst, 4.0);
        let tail = span(&out, SR, 1.0, 3.5);
        for index in 0..SYMPATHETIC_STRINGS {
            let expected = root_hz * scene_ratio(scene, index);
            let found = peak_hz(tail, expected, 25.0);
            assert!(
                cents(found, expected).abs() < 1.5,
                "scene {scene}, string {index}: {:+.2} cents at {expected:.1} Hz",
                cents(found, expected)
            );
        }
    }
}

#[test]
fn sympathetic_strings_follow_the_note_or_stay_where_they_are() {
    let burst = noise(2_400, 1.0, 11);
    let first_string = |volts: f32, follow: bool| {
        let p = BridgeParams {
            root_volts: volts,
            scene: 1.0 / (SCENES - 1) as f32,
            level: 1.0,
            decay: 1.0,
            follow,
            ..BridgeParams::default()
        };
        let out = bridge_run(&p, &burst, 4.0);
        let tail = span(&out, SR, 1.0, 3.5);
        let expected = if follow {
            f64::from(ZERO_VOLT_HZ) * 2f64.powf(f64::from(volts))
        } else {
            f64::from(ZERO_VOLT_HZ) / 2.0
        };
        cents(peak_hz(tail, expected, 25.0), expected)
    };
    for volts in [0.0f32, 0.5, 1.0] {
        assert!(first_string(volts, true).abs() < 1.5);
        assert!(first_string(volts, false).abs() < 1.5);
    }
}

#[test]
fn a_matching_note_wakes_the_strings_and_a_foreign_one_does_not() {
    // The sympathetic strings listen. A sustained note at one of their pitches
    // builds them up; one that is in none of their harmonic series does not.
    let level_after = |hz: f64| {
        let p = BridgeParams {
            scene: 1.0 / (SCENES - 1) as f32, // major
            level: 1.0,
            decay: 0.6,
            ..BridgeParams::default()
        };
        let input: Vec<f32> = (0..(48_000 * 2))
            .map(|i| 0.2 * (std::f64::consts::TAU * hz * i as f64 / f64::from(SR)).sin() as f32)
            .collect();
        let out = bridge_run(&p, &input, 2.0);
        // What the strings add to the input, at the input's own frequency:
        // the output minus the wire.
        let added: Vec<f32> = out.iter().zip(&input).map(|(o, i)| o - i).collect();
        fathom_analysis::rms_dbfs(span(&added, SR, 1.0, 2.0))
    };
    let root = f64::from(ZERO_VOLT_HZ);
    let matching = level_after(root * 1.25); // the major third string
    let foreign = level_after(root * 1.25 * 2f64.powf(0.5 / 12.0 * 3.0)); // a stray
    assert!(
        matching > foreign + 6.0,
        "{matching:.1} dBFS against {foreign:.1} dBFS"
    );
}

#[test]
fn morphing_between_scenes_does_not_click() {
    let mut bridge = Bridge::new(SR);
    let burst = noise(2_400, 0.5, 11);
    let mut out = Vec::new();
    let mut block = [0.0f32; 16];
    for b in 0..(48_000 * 4 / 16) {
        let t = b as f32 * 16.0 / SR;
        let p = BridgeParams {
            scene: (t / 3.0).min(1.0),
            level: 1.0,
            decay: 1.0,
            ..BridgeParams::default()
        };
        let start = b * 16;
        bridge.process(&p, burst.get(start..start + 16).unwrap_or(&[]), &mut block);
        out.extend_from_slice(&block);
    }
    let report =
        fathom_analysis::discontinuity(span(&out, SR, 0.6, 3.8)).expect("too short to measure");
    assert!(report.excess < 0.5, "{report:?}");
}

#[test]
fn the_tables_have_the_shape_the_code_assumes() {
    assert_eq!(BODY_MODES, 12);
    assert_eq!(SYMPATHETIC_STRINGS, 8);
}
