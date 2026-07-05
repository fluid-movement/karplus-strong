#pragma once

#include <algorithm>
#include <cmath>

namespace ks
{
    constexpr float pi    = 3.14159265358979f;
    constexpr float twoPi = 2.0f * pi;

    constexpr float maxDelaySeconds     = 0.25f;
    constexpr float maxFeedbackGain     = 0.99999f;
    constexpr float minVelocity         = 0.01f;
    constexpr float silenceThreshold    = 1.0e-5f;
    constexpr int   peakDecayWindow     = 256;
    constexpr int   silenceGuardSamples = 256;
    constexpr float minCutoffHz         = 200.0f;
    constexpr float cutoffRangeHz       = 11800.0f;
    constexpr float refFrequencyHz      = 440.0f;
    constexpr float dcBlockerHz         = 5.0f;
    constexpr float ln1000              = 6.9078f;

    constexpr float driveMaxK           = 9.0f;
    constexpr float silenceEnvelope     = 1.0e-3f;

    constexpr float humanizeMaxDetune      = 0.02f;
    constexpr float humanizeMaxBrightness  = 0.1f;
    constexpr float humanizeMaxPickJitter  = 0.05f;

    inline float computeCutoffHz (float brightness)
    {
        return minCutoffHz + std::clamp (brightness, 0.0f, 1.0f) * cutoffRangeHz;
    }

    inline float applySaturation (float x, float amount)
    {
        float a = std::clamp (amount, 0.0f, 1.0f);
        if (a <= 0.0f)
            return x;

        float k = 1.0f + a * driveMaxK;
        return std::tanh (x * k) / std::tanh (k);
    }

    inline float computeReleaseCoeff (float releaseTime, float sampleRate)
    {
        float samples = std::max (releaseTime, 0.001f) * sampleRate;
        return std::pow (silenceEnvelope, 1.0f / samples);
    }

    inline float computeFeedbackGain (float decayTime, float frequency, float delaySamples,
                                      float keyTrack, float cutoffHz, float sampleRate)
    {
        float refDelay = sampleRate / refFrequencyHz;
        float effDelay = refDelay + keyTrack * (delaySamples - refDelay);

        float ratio = frequency / cutoffHz;
        float lowpassLossComp = std::sqrt (1.0f + ratio * ratio);

        float gain = std::exp (-ln1000 * effDelay / (decayTime * sampleRate));
        return std::clamp (gain * lowpassLossComp, 0.0f, maxFeedbackGain);
    }
}
