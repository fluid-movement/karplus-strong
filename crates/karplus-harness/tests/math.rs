//! The core's own maths against `f64` `std`, which the core cannot use.

use karplus_core::math::{atan2, cos_turns, exp, soft_knee, sqrt, tanh};

#[test]
fn tanh_is_accurate() {
    let mut worst = 0.0f64;
    let mut x = -12.0f64;
    while x < 12.0 {
        let e = (f64::from(tanh(x as f32)) - x.tanh()).abs();
        worst = worst.max(e);
        x += 0.000_37;
    }
    assert!(worst < 3e-7, "worst error {worst}");
}

#[test]
fn atan2_is_accurate() {
    let mut worst = 0.0f64;
    for i in 0..3600 {
        let a = f64::from(i) * std::f64::consts::TAU / 3600.0 - std::f64::consts::PI + 1e-4;
        for r in [1e-3f64, 1.0, 1e3] {
            let (y, x) = (r * a.sin(), r * a.cos());
            let e = (f64::from(atan2(y as f32, x as f32)) - y.atan2(x)).abs();
            worst = worst.max(e);
        }
    }
    assert!(worst < 1e-6, "worst error {worst}");
}

#[test]
fn sqrt_is_accurate() {
    let mut x = 1e-6f64;
    while x < 1e6 {
        let e = (f64::from(sqrt(x as f32)) / x.sqrt() - 1.0).abs();
        assert!(e < 3e-7, "sqrt({x}) relative error {e}");
        x *= 1.013;
    }
}

#[test]
fn exp_and_cos_are_accurate() {
    let mut x = -20.0f64;
    while x < 20.0 {
        let e = (f64::from(exp(x as f32)) / x.exp() - 1.0).abs();
        assert!(e < 3e-6, "exp({x}) relative error {e}");
        x += 0.01;
    }
    for i in 0..1000 {
        let t = f64::from(i) / 1000.0;
        let e = (f64::from(cos_turns(t as f32)) - (std::f64::consts::TAU * t).cos()).abs();
        assert!(e < 1e-6, "cos at {t} turns: error {e}");
    }
}

#[test]
fn the_soft_knee_is_smooth_through_its_knee() {
    let k = karplus_core::math::KNEE;
    let slope = |x: f32| (soft_knee(x + 1e-3) - soft_knee(x - 1e-3)) / 2e-3;
    assert!((slope(k) - 1.0).abs() < 0.02, "slope {}", slope(k));
}
