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

use karplus_core::{Bridge, BridgeParams, ExciterKind, Params, Voice};

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
    /// Stiffness knob: dispersion in the loop.
    pub stiffness: f32,
    /// Tension modulation knob.
    pub tension: f32,
    /// Buzz knob: a one-sided clip in the loop.
    pub buzz: f32,
    /// Non-zero if the loop return jack is patched.
    pub loop_return: u8,
    /// Tuning trim of the outboard loop, in volts.
    pub loop_trim_volts: f32,
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
            stiffness: p.stiffness,
            tension: p.tension,
            buzz: p.buzz,
            loop_return: p.loop_return != 0,
            loop_trim_volts: p.loop_trim_volts,
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
            stiffness: p.stiffness,
            tension: p.tension,
            buzz: p.buzz,
            loop_return: u8::from(p.loop_return),
            loop_trim_volts: p.loop_trim_volts,
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

/// Render `count` samples with the loop brought out (K22).
///
/// As [`karplus_voice_process`], plus `loop_return`, what comes back from the
/// outboard chain (null reads as silence), and `send`, which is filled with
/// the signal leaving the delay line (null skips it). Both are `count` long.
///
/// # Safety
///
/// As [`karplus_voice_process`]; `loop_return` must be null or point to
/// `count` readable `f32`s, and `send` null or `count` writable ones.
#[no_mangle]
pub unsafe extern "C" fn karplus_voice_process_loop(
    voice: *mut KarplusVoice,
    params: *const KarplusParams,
    gate: *const f32,
    audio_in: *const f32,
    loop_return: *const f32,
    send: *mut f32,
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
    let read = |p: *const f32| -> &[f32] {
        if p.is_null() {
            &[]
        } else {
            slice::from_raw_parts(p, count)
        }
    };
    let send = if send.is_null() {
        None
    } else {
        Some(slice::from_raw_parts_mut(send, count))
    };
    voice.voice.process_loop(
        &Params::from(params),
        read(gate),
        read(audio_in),
        read(loop_return),
        send,
        out,
    );
}

/// An opaque bridge: the body and the sympathetic strings the voices share.
pub struct KarplusBridge {
    bridge: Bridge,
}

/// What the bridge is told every block. Mirrors `karplus_core::BridgeParams`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KarplusBridgeParams {
    /// Pitch of the lead voice, in volts.
    pub root_volts: f32,
    /// Body shape knob.
    pub body_shape: f32,
    /// Body size knob.
    pub body_size: f32,
    /// Body material knob.
    pub body_material: f32,
    /// Dry against body.
    pub body_mix: f32,
    /// Non-zero if the body follows the played note.
    pub body_track: u8,
    /// Sympathetic tuning knob.
    pub scene: f32,
    /// Sympathetic root knob.
    pub root: f32,
    /// Sympathetic level knob.
    pub level: f32,
    /// Sympathetic ring knob.
    pub decay: f32,
    /// Non-zero if the sympathetic root follows the played note.
    pub follow: u8,
}

impl From<&KarplusBridgeParams> for BridgeParams {
    fn from(p: &KarplusBridgeParams) -> Self {
        BridgeParams {
            root_volts: p.root_volts,
            body_shape: p.body_shape,
            body_size: p.body_size,
            body_material: p.body_material,
            body_mix: p.body_mix,
            body_track: p.body_track != 0,
            scene: p.scene,
            root: p.root,
            level: p.level,
            decay: p.decay,
            follow: p.follow != 0,
        }
    }
}

impl From<&BridgeParams> for KarplusBridgeParams {
    fn from(p: &BridgeParams) -> Self {
        KarplusBridgeParams {
            root_volts: p.root_volts,
            body_shape: p.body_shape,
            body_size: p.body_size,
            body_material: p.body_material,
            body_mix: p.body_mix,
            body_track: u8::from(p.body_track),
            scene: p.scene,
            root: p.root,
            level: p.level,
            decay: p.decay,
            follow: u8::from(p.follow),
        }
    }
}

/// Fill a `KarplusBridgeParams` with the core's defaults.
///
/// # Safety
///
/// `out` must be null or a valid, writable `KarplusBridgeParams`.
#[no_mangle]
pub unsafe extern "C" fn karplus_bridge_params_default(out: *mut KarplusBridgeParams) {
    if let Some(out) = out.as_mut() {
        *out = KarplusBridgeParams::from(&BridgeParams::default());
    }
}

/// Create a bridge.
#[no_mangle]
pub extern "C" fn karplus_bridge_new(sample_rate: f32) -> *mut KarplusBridge {
    Box::into_raw(Box::new(KarplusBridge {
        bridge: Bridge::new(sample_rate),
    }))
}

/// Destroy a bridge. Null is accepted and ignored.
///
/// # Safety
///
/// `bridge` must be null, or from [`karplus_bridge_new`] and not yet freed.
#[no_mangle]
pub unsafe extern "C" fn karplus_bridge_free(bridge: *mut KarplusBridge) {
    if !bridge.is_null() {
        drop(Box::from_raw(bridge));
    }
}

/// Silence a bridge. Allocation-free, but not cheap: it clears the sympathetic
/// strings' lines.
///
/// # Safety
///
/// `bridge` must be null or valid.
#[no_mangle]
pub unsafe extern "C" fn karplus_bridge_reset(bridge: *mut KarplusBridge) {
    if let Some(bridge) = bridge.as_mut() {
        bridge.bridge.reset();
    }
}

/// Sum `channels` voices and run them through the bridge.
///
/// `voices` points to `channels` consecutive blocks of `stride` samples, of
/// which the first `count` of each are used: the layout of a C array
/// `float v[channels][stride]`. Null reads as silence. `out` receives `count`
/// samples.
///
/// # Safety
///
/// `bridge` and `params` must be null or valid. `out` must be null or point to
/// `count` writable `f32`s. `voices` must be null or point to
/// `channels * stride` readable `f32`s, with `count <= stride`.
#[no_mangle]
pub unsafe extern "C" fn karplus_bridge_process(
    bridge: *mut KarplusBridge,
    params: *const KarplusBridgeParams,
    voices: *const f32,
    channels: usize,
    stride: usize,
    out: *mut f32,
    count: usize,
) {
    let (Some(bridge), Some(params)) = (bridge.as_mut(), params.as_ref()) else {
        return;
    };
    if out.is_null() || count == 0 {
        return;
    }
    let out = slice::from_raw_parts_mut(out, count);
    let mut sum = [0.0f32; MAX_BLOCK];
    let count_used = count.min(MAX_BLOCK);
    if !voices.is_null() && stride >= count {
        let all = slice::from_raw_parts(voices, channels * stride);
        for channel in all.chunks_exact(stride.max(1)) {
            for (acc, x) in sum.iter_mut().zip(channel.iter().take(count_used)) {
                *acc += *x;
            }
        }
    }
    let input = sum.get(..count_used).unwrap_or(&[]);
    bridge
        .bridge
        .process(&BridgeParams::from(params), input, out);
}

/// Longest block [`karplus_bridge_process`] sums on its own stack.
const MAX_BLOCK: usize = 256;

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
            karplus_voice_process_loop(
                core::ptr::null_mut(),
                core::ptr::null(),
                core::ptr::null(),
                core::ptr::null(),
                core::ptr::null(),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                64,
            );
            karplus_bridge_params_default(core::ptr::null_mut());
            karplus_bridge_free(core::ptr::null_mut());
            karplus_bridge_reset(core::ptr::null_mut());
            karplus_bridge_process(
                core::ptr::null_mut(),
                core::ptr::null(),
                core::ptr::null(),
                4,
                16,
                core::ptr::null_mut(),
                16,
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

    #[test]
    fn the_bridge_abi_sums_the_voices_and_plays_the_same_bridge_as_the_core() {
        let bp = BridgeParams {
            body_mix: 0.6,
            level: 0.8,
            ..BridgeParams::default()
        };
        let cbp = KarplusBridgeParams::from(&bp);
        let mut voices = [[0.0f32; 16]; 3];
        for (c, row) in voices.iter_mut().enumerate() {
            for (i, x) in row.iter_mut().enumerate() {
                *x = 0.1 * (c as f32 + 1.0) * ((i + c) as f32).sin();
            }
        }
        let mut sum = [0.0f32; 16];
        for row in &voices {
            for (a, x) in sum.iter_mut().zip(row) {
                *a += x;
            }
        }
        let handle = karplus_bridge_new(48_000.0);
        let mut bridge = Bridge::new(48_000.0);
        let (mut through_abi, mut direct) = ([0.0f32; 16], [0.0f32; 16]);
        for _ in 0..20 {
            // SAFETY: a live handle and a 3 x 16 array of f32s.
            unsafe {
                karplus_bridge_process(
                    handle,
                    &cbp,
                    voices.as_ptr().cast(),
                    3,
                    16,
                    through_abi.as_mut_ptr(),
                    16,
                );
            }
            bridge.process(&bp, &sum, &mut direct);
            assert_eq!(through_abi, direct);
        }
        // SAFETY: from karplus_bridge_new, freed once.
        unsafe { karplus_bridge_free(handle) };
    }

    #[test]
    fn the_loop_abi_plays_the_same_string_as_the_core() {
        let p = Params {
            loop_return: true,
            stiffness: 0.5,
            ..Params::default()
        };
        let cp = KarplusParams::from(&p);
        let gate = [10.0f32; 16];
        let returned: Vec<f32> = (0..16).map(|i| 0.1 * (i as f32).sin()).collect();
        let (mut a, mut b) = ([0.0f32; 16], [0.0f32; 16]);
        let (mut send_a, mut send_b) = ([0.0f32; 16], [0.0f32; 16]);
        let handle = karplus_voice_new(48_000.0, 3);
        let mut voice = Voice::new(48_000.0, 3);
        for _ in 0..30 {
            // SAFETY: a live handle and arrays of the stated length.
            unsafe {
                karplus_voice_process_loop(
                    handle,
                    &cp,
                    gate.as_ptr(),
                    core::ptr::null(),
                    returned.as_ptr(),
                    send_a.as_mut_ptr(),
                    a.as_mut_ptr(),
                    16,
                );
            }
            voice.process_loop(&p, &gate, &[], &returned, Some(&mut send_b), &mut b);
            assert_eq!(a, b);
            assert_eq!(send_a, send_b);
        }
        // SAFETY: from karplus_voice_new, freed once.
        unsafe { karplus_voice_free(handle) };
    }
}
