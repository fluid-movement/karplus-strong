/*
 * controls.hpp — knobs, CV and jacks to KarplusParams.
 *
 * Free of every Rack type, so it can be compiled and tested without the SDK
 * (test/controls_test.cpp). The rest of the shim is glue that cannot be
 * wrong in an interesting way; this is the only arithmetic, so this is the
 * part that has tests. The pattern, and `unipolar` itself, are Fathom's.
 *
 * If this file starts making decisions about sound, the core's boundary is
 * in the wrong place.
 */

#ifndef KARPLUS_CONTROLS_HPP
#define KARPLUS_CONTROLS_HPP

namespace karplus {

/** Clamp, with NaN going to `low`. */
inline float clamp(float x, float low, float high) {
    if (!(x == x)) {
        return low;
    }
    return x < low ? low : (x > high ? high : x);
}

/**
 * A knob, an attenuverter and a CV input into a 0..1 knob position.
 *
 * Eurorack convention: +/-5 V is a full-scale swing, the attenuverter scales
 * it by -1..+1, the knob sets the resting point. Clamped, never wrapped.
 */
inline float unipolar(float knob, float attenuverter, float cv_volts) {
    return clamp(knob + attenuverter * cv_volts / 5.0f, 0.0f, 1.0f);
}

/** Pitch in volts: the PITCH knob's offset plus the V/oct input. */
inline float pitch_volts(float knob_volts, float voct_volts) {
    return knob_volts + voct_volts;
}

/** Eurorack audio is +/-5 V; the core works at +/-1. */
inline float audio_from_volts(float volts) {
    return volts / 5.0f;
}

/** And back. */
inline float audio_to_volts(float sample) {
    return sample * 5.0f;
}

/** A switch or snapped knob's value as a whole position in 0..last. */
inline int position(float value, int last) {
    const float clamped = clamp(value, 0.0f, (float)last);
    return (int)(clamped + 0.5f);
}

/** Most channels a Rack cable carries. */
const int MAX_CHANNELS = 16;

/**
 * How many voices to run: as many as the widest of the V/oct and gate
 * cables, and at least one so the PLUCK button and the audio input always
 * have a string to play.
 */
inline int voice_count(int voct_channels, int gate_channels) {
    int n = voct_channels > gate_channels ? voct_channels : gate_channels;
    if (n < 1) {
        n = 1;
    }
    return n > MAX_CHANNELS ? MAX_CHANNELS : n;
}

}  // namespace karplus

#endif
