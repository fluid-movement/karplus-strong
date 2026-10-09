//! The C ABI. The only thing standing between `karplus-core` and C++.
//!
//! The same shape as Fathom's `fathom-ffi`, deliberately: VCV Rack needs a
//! C++ subclass of `rack::engine::Module`, so `targets/vcv` is a thin shim
//! that drives the core through the functions below, and `cbindgen`
//! generates the header so the interface is never written twice by hand.
//!
//! # Rules for this crate
//!
//! * **Every pointer is checked for null.** A null handle is a no-op, never
//!   a crash.
//! * **Nothing panics across the boundary.** Unwinding into C++ is undefined
//!   behaviour; there is no panic path here, and the release profile aborts.
//! * **No logic.** Anything that looks like a decision belongs in the core.

#![warn(missing_docs)]
#![allow(clippy::missing_safety_doc)]

use core::slice;

use karplus_core::{ExciterKind, Params, Voice};

/// An opaque string voice. The C side only ever holds a pointer to it.
pub struct KarplusVoice {
    voice: Voice,
}

/// The burst that plucks the string. Mirrors `karplus_core::ExciterKind`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KarplusExciter {
    /// White noise.
    Noise = 0,
    /// Sparse random impulses.
    Dust = 1,
    /// A sine at a harmonic of the note.
    Sine = 2,
}

/// Per-block parameters. Mirrors `karplus_core::Params`; the knobs are
/// positions, `0..1`, with any CV already added.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KarplusParams {
    /// Pitch in volts, one volt per octave. 0 V is C4.
    pub pitch_volts: f32,
    /// Decay time knob.
    pub decay: f32,
    /// Loop lowpass knob.
    pub brightness: f32,
    /// Pluck position knob.
    pub pick: f32,
    /// Exciter lowpass knob.
    pub tone: f32,
    /// Burst length knob.
    pub length: f32,
    /// Loop saturation knob.
    pub drive: f32,
    /// Damping-on-release knob.
    pub damp: f32,
    /// Per-pluck variation knob.
    pub humanize: f32,
    /// Decay key tracking knob.
    pub key_track: f32,
    /// Velocity to brightness amount.
    pub velocity_brightness: f32,
    /// Velocity to decay amount.
    pub velocity_decay: f32,
    /// Velocity to burst length amount.
    pub velocity_length: f32,
    /// The burst.
    pub exciter: KarplusExciter,
    /// Harmonic of the sine exciter, 1..8.
    pub harmonic: u32,
    /// Velocity input, in volts.
    pub velocity_volts: f32,
    /// Non-zero if the velocity input is patched.
    pub velocity_patched: u8,
    /// Non-zero while the panel's PLUCK button is held.
    pub manual_gate: u8,
}

impl From<KarplusExciter> for ExciterKind {
    fn from(kind: KarplusExciter) -> Self {
        match kind {
            KarplusExciter::Noise => ExciterKind::Noise,
            KarplusExciter::Dust => ExciterKind::Dust,
            KarplusExciter::Sine => ExciterKind::Sine,
        }
    }
}

impl From<ExciterKind> for KarplusExciter {
    fn from(kind: ExciterKind) -> Self {
        match kind {
            ExciterKind::Noise => KarplusExciter::Noise,
            ExciterKind::Dust => KarplusExciter::Dust,
            ExciterKind::Sine => KarplusExciter::Sine,
        }
    }
}

impl From<&KarplusParams> for Params {
    fn from(p: &KarplusParams) -> Self {
        Params {
            pitch_volts: p.pitch_volts,
            decay: p.decay,
            brightness: p.brightness,
            pick: p.pick,
            tone: p.tone,
            length: p.length,
            drive: p.drive,
            damp: p.damp,
            humanize: p.humanize,
            key_track: p.key_track,
            velocity_brightness: p.velocity_brightness,
            velocity_decay: p.velocity_decay,
            velocity_length: p.velocity_length,
            exciter: p.exciter.into(),
            harmonic: p.harmonic,
            velocity_volts: p.velocity_volts,
            velocity_patched: p.velocity_patched != 0,
            manual_gate: p.manual_gate != 0,
        }
    }
}

impl From<&Params> for KarplusParams {
    fn from(p: &Params) -> Self {
        KarplusParams {
            pitch_volts: p.pitch_volts,
            decay: p.decay,
            brightness: p.brightness,
            pick: p.pick,
            tone: p.tone,
            length: p.length,
            drive: p.drive,
            damp: p.damp,
            humanize: p.humanize,
            key_track: p.key_track,
            velocity_brightness: p.velocity_brightness,
            velocity_decay: p.velocity_decay,
            velocity_length: p.velocity_length,
            exciter: p.exciter.into(),
            harmonic: p.harmonic,
            velocity_volts: p.velocity_volts,
            velocity_patched: u8::from(p.velocity_patched),
            manual_gate: u8::from(p.manual_gate),
        }
    }
}

/// Fill a `KarplusParams` with the core's defaults, which are the panel's.
///
/// # Safety
///
/// `out` must be null or a valid, writable `KarplusParams`.
#[no_mangle]
pub unsafe extern "C" fn karplus_params_default(out: *mut KarplusParams) {
    if let Some(out) = out.as_mut() {
        *out = KarplusParams::from(&Params::default());
    }
}

/// Create a voice. `seed` picks its noise: give each channel its own.
///
/// Every voice owns all of its state, so a host may create as many as it
/// likes.
#[no_mangle]
pub extern "C" fn karplus_voice_new(sample_rate: f32, seed: u32) -> *mut KarplusVoice {
    Box::into_raw(Box::new(KarplusVoice {
        voice: Voice::new(sample_rate, seed),
    }))
}

/// Destroy a voice. Null is accepted and ignored.
///
/// # Safety
///
/// `voice` must be null, or a pointer from [`karplus_voice_new`] not already
/// freed.
#[no_mangle]
pub unsafe extern "C" fn karplus_voice_free(voice: *mut KarplusVoice) {
    if !voice.is_null() {
        drop(Box::from_raw(voice));
    }
}

/// Silence a voice and forget its last pluck. Cheap, allocation-free and safe
/// on the audio thread.
///
/// # Safety
///
/// `voice` must be null or valid.
#[no_mangle]
pub unsafe extern "C" fn karplus_voice_reset(voice: *mut KarplusVoice) {
    if let Some(voice) = voice.as_mut() {
        voice.voice.reset();
    }
}

/// Render `count` samples into `out`.
///
/// `gate` is the gate input in volts and `audio_in` the external exciter at
/// full scale ±1, each `count` samples long. Either may be null, which reads
/// as silence: an unpatched jack.
///
/// # Safety
///
/// `voice` and `params` must be null or valid. `out` must be null or point to
/// `count` writable `f32`s; `gate` and `audio_in` must each be null or point
/// to `count` readable `f32`s. Nothing is written if `voice`, `params` or
/// `out` is null.
#[no_mangle]
pub unsafe extern "C" fn karplus_voice_process(
    voice: *mut KarplusVoice,
    params: *const KarplusParams,
    gate: *const f32,
    audio_in: *const f32,
    out: *mut f32,
    count: usize,
) {
    let (Some(voice), Some(params)) = (voice.as_mut(), params.as_ref()) else {
        return;
    };
    if out.is_null() || count == 0 {
        return;
    }
    let out = slice::from_raw_parts_mut(out, count);
    let gate: &[f32] = if gate.is_null() {
        &[]
    } else {
        slice::from_raw_parts(gate, count)
    };
    let audio_in: &[f32] = if audio_in.is_null() {
        &[]
    } else {
        slice::from_raw_parts(audio_in, count)
    };
    voice
        .voice
        .process(&Params::from(params), gate, audio_in, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_round_trip() {
        let mut p = core::mem::MaybeUninit::<KarplusParams>::uninit();
        // SAFETY: a valid, writable pointer.
        unsafe { karplus_params_default(p.as_mut_ptr()) };
        // SAFETY: just written in full.
        let p = unsafe { p.assume_init() };
        assert_eq!(Params::from(&p), Params::default());
    }

    #[test]
    fn null_is_never_a_crash() {
        // SAFETY: every pointer here is null, which each function accepts.
        unsafe {
            karplus_params_default(core::ptr::null_mut());
            karplus_voice_free(core::ptr::null_mut());
            karplus_voice_reset(core::ptr::null_mut());
            karplus_voice_process(
                core::ptr::null_mut(),
                core::ptr::null(),
                core::ptr::null(),
                core::ptr::null(),
                core::ptr::null_mut(),
                64,
            );
        }
    }

    #[test]
    fn the_abi_plays_the_same_string_as_the_core() {
        let p = Params::default();
        let cp = KarplusParams::from(&p);
        let gate = [10.0f32; 64];
        let mut through_abi = [0.0f32; 64];
        let mut direct = [0.0f32; 64];
        let handle = karplus_voice_new(48_000.0, 3);
        let mut voice = Voice::new(48_000.0, 3);
        for _ in 0..50 {
            // SAFETY: a live handle, and arrays of the stated length.
            unsafe {
                karplus_voice_process(
                    handle,
                    &cp,
                    gate.as_ptr(),
                    core::ptr::null(),
                    through_abi.as_mut_ptr(),
                    64,
                );
            }
            voice.process(&p, &gate, &[], &mut direct);
            assert_eq!(through_abi, direct);
        }
        // SAFETY: from karplus_voice_new, freed once.
        unsafe { karplus_voice_free(handle) };
    }
}
