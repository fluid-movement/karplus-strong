//! Rendering, measuring and checking the string, outside any host.
//!
//! The measurements are Fathom's (`fathom-analysis`), and so are the
//! scenario format's building blocks (`fathom-harness`: durations, note
//! names, automation, bounds, WAV). What is here is the part that knows what
//! a plucked string is: how to pluck it, when, and how long it rings.

#![forbid(unsafe_code)]

pub mod metrics;
pub mod render;
pub mod run;
pub mod scenario;
