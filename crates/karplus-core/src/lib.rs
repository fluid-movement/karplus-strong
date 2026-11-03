//! Karplus-Strong — a plucked string for VCV Rack, and later for hardware.
//!
//! One DSP core, no platform knowledge, built the way Fathom's is: every
//! shell reads its own controls, fills [`Params`], calls [`Voice::process`]
//! and writes the result somewhere. A polyphonic shell owns one [`Voice`]
//! per channel; there is no voice allocator here, because a modular
//! instrument does not have one — the cable's channel count is the polyphony.
//!
//! # Rules
//!
//! - `no_std`, no allocator. One dependency: `fathom-core`.
//! - No `unsafe`, forbidden by the attribute below.
//! - `f32` only, never `f64`.
//! - No allocation and no panics in [`Voice::process`].

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(
    not(test),
    deny(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::todo,
        clippy::unimplemented,
    )
)]

mod bridge;
mod delay;
mod filter;
pub mod math;
mod params;
mod ramp;
mod voice;

pub use bridge::{
    Bridge, BridgeParams, BODY_MODES, SCENES, SHAPES, SYMPATHETIC_LEN, SYMPATHETIC_LINES,
    SYMPATHETIC_PARTNERS, SYMPATHETIC_STRINGS,
};
pub use delay::{DelayLine, MIN_DELAY};
pub use filter::{
    allpass_response, dc_blocker_response, fathom_dc_coefficient, one_pole_response, LoopFilter,
    Response,
};
pub use params::{
    allpass_coefficient, brightness_ratio, burst_periods, damp_seconds, decay_seconds,
    pick_fraction, stiffness_delay, tone_ratio, ExciterKind, Params, DECAY_MAX_S, DECAY_MIN_S,
    DEFAULT_BRIGHTNESS, DEFAULT_DECAY, DEFAULT_LENGTH, DRIVE_MAX_GAIN, MAX_CUTOFF,
    STIFFNESS_MAX_DELAY, STIFFNESS_MAX_FRACTION, TENSION_MAX_FRACTION,
};
pub use voice::{
    Voice, GATE_HIGH_V, GATE_LOW_V, LOOP_LATENCY, MAX_LOOP_GAIN, MIN_VELOCITY, STRING_LEN,
};

/// Re-exported so that shells and tests name the same 0 V as the core.
pub use fathom_core::ZERO_VOLT_HZ;
