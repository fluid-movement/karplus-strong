//! From samples to the numbers a scenario asserts on. Every measurement is
//! `fathom-analysis`'s except the decay time, which is a string's question.

use std::collections::BTreeMap;

use crate::render::fundamental_t60;

/// Every metric a scenario may assert on.
pub const KNOWN_METRICS: &[&str] = &[
    "peak",
    "rms_dbfs",
    "dc_offset_dbfs",
    "non_finite_samples",
    "pitch_error_cents",
    "fundamental_hz",
    "t60_s",
    "discontinuity",
];

/// How far either side of the expected pitch the comb search looks.
pub const PITCH_SEARCH_CENTS: f64 = 300.0;

/// Measure everything that can be measured. A metric that cannot be — too
/// little signal, too short a span — is absent, and an assertion on an
/// absent metric fails.
pub fn measure(
    whole: &[f32],
    span: &[f32],
    sample_rate: f64,
    expected_hz: f64,
) -> BTreeMap<&'static str, f64> {
    let mut m = BTreeMap::new();
    m.insert("peak", fathom_analysis::peak(span));
    m.insert("rms_dbfs", fathom_analysis::rms_dbfs(span));
    m.insert("dc_offset_dbfs", fathom_analysis::dc_offset_dbfs(span));
    m.insert(
        "non_finite_samples",
        fathom_analysis::non_finite_report(whole).total() as f64,
    );
    if let Some(hz) = fathom_analysis::measure_fundamental_by_comb(
        span,
        sample_rate,
        expected_hz,
        PITCH_SEARCH_CENTS,
    ) {
        m.insert("fundamental_hz", hz);
        m.insert("pitch_error_cents", 1200.0 * (hz / expected_hz).log2());
    }
    if let Some(t) = fundamental_t60(span, sample_rate as f32, expected_hz) {
        m.insert("t60_s", t);
    }
    if let Some(d) = fathom_analysis::discontinuity(span) {
        m.insert("discontinuity", d.excess);
    }
    m
}
