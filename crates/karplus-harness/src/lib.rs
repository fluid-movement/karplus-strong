//! Rendering and measuring the string, outside any host.
//!
//! The measurements are Fathom's (`fathom-analysis`, used unchanged). What is
//! here is the part that knows what a Karplus-Strong voice is: how to pluck
//! it, and when.

#![forbid(unsafe_code)]

pub mod render;
