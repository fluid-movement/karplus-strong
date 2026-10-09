/*
 * Tests for the only part of the VCV shim that contains arithmetic.
 *
 * Built and run by `just verify` with nothing but a C++ compiler: the Rack
 * SDK is not needed.
 */

#include <cmath>
#include <cstdio>
#include <limits>

#include "../src/controls.hpp"

static int failures = 0;

static void check(bool condition, const char* what) {
    if (!condition) {
        std::printf("FAIL  %s\n", what);
        failures++;
    }
}

static void close_to(float measured, float expected, const char* what) {
    if (std::fabs(measured - expected) > 1e-6f) {
        std::printf("FAIL  %s: got %f, wanted %f\n", what, measured, expected);
        failures++;
    }
}

int main() {
    using namespace karplus;
    const float nan = std::numeric_limits<float>::quiet_NaN();

    close_to(unipolar(0.3f, 0.0f, 5.0f), 0.3f, "cv ignored at zero attenuation");
    close_to(unipolar(0.0f, 1.0f, 5.0f), 1.0f, "five volts is full scale");
    close_to(unipolar(1.0f, -1.0f, 5.0f), 0.0f, "inverted cv");
    close_to(unipolar(0.5f, 1.0f, 10.0f), 1.0f, "clamped at the top");
    close_to(unipolar(0.5f, 1.0f, -10.0f), 0.0f, "clamped at the bottom");
    close_to(unipolar(0.5f, 1.0f, nan), 0.0f, "a NaN cv is the bottom, not a NaN");

    close_to(pitch_volts(1.0f, -2.0f), -1.0f, "knob and jack add");

    close_to(audio_from_volts(5.0f), 1.0f, "five volts in is full scale");
    close_to(audio_to_volts(1.0f), 5.0f, "full scale out is five volts");
    close_to(audio_to_volts(audio_from_volts(-3.3f)), -3.3f, "round trip");

    check(position(0.0f, 2) == 0, "switch bottom");
    check(position(1.0f, 2) == 1, "switch middle");
    check(position(2.0f, 2) == 2, "switch top");
    check(position(1.49f, 2) == 1, "rounds down below the half");
    check(position(1.51f, 2) == 2, "rounds up above the half");
    check(position(7.0f, 2) == 2, "clamped above");
    check(position(-3.0f, 2) == 0, "clamped below");
    check(position(nan, 7) == 0, "NaN is the first position");

    check(voice_count(0, 0) == 1, "nothing patched is one voice");
    check(voice_count(4, 1) == 4, "a poly pitch cable sets the count");
    check(voice_count(1, 6) == 6, "so does a poly gate cable");
    check(voice_count(99, 0) == MAX_CHANNELS, "never more than Rack carries");

    if (failures) {
        std::printf("%d failed\n", failures);
        return 1;
    }
    std::printf("controls: all passed\n");
    return 0;
}
