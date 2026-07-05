#pragma once

#include <algorithm>
#include <cmath>
#include <cstdint>
#include "CircularBuffer.h"
#include "OnePoleLowpass.h"
#include "DcBlocker.h"
#include "SilenceDetector.h"
#include "KsCalibration.h"
#include "KsParams.h"

class KsDelayLine
{
public:
    void prepare (double newSampleRate)
    {
        sampleRate = newSampleRate;
        buffer.prepare (static_cast<int> (sampleRate * ks::maxDelaySeconds));
        dcBlocker.prepare (sampleRate);
    }

    double getSampleRate() const { return sampleRate; }

    void reset()
    {
        buffer.clear();
        lowpass.reset();
        dcBlocker.reset();
        silence.reset();
        damping = false;
        releaseGain = 1.0f;
    }

    void setParameters (const KsParams& newParams)
    {
        params = newParams;
    }

    void noteOn (float frequency, float velocity_, float excitationLength)
    {
        velocity     = std::max (velocity_, ks::minVelocity);
        delaySamples = static_cast<float> (sampleRate / frequency);

        reset();

        float modBright = params.brightness + params.velBrightness * (velocity - 0.5f);
        if (params.humanize > 0.0f)
        {
            float jitter = (nextRandom() * 2.0f - 1.0f) * params.humanize * ks::humanizeMaxBrightness;
            modBright += jitter;
        }
        float modT = params.decayTime * (1.0f + params.velDecay * (velocity - 0.5f));

        cutoffHz     = ks::computeCutoffHz (modBright);
        feedbackGain = ks::computeFeedbackGain (modT, frequency, delaySamples, params.keyTrack,
                                                cutoffHz, static_cast<float> (sampleRate));

        lowpass.setCutoff (cutoffHz, static_cast<float> (sampleRate));
        silence.noteOn (static_cast<int> (delaySamples) + static_cast<int> (excitationLength)
                        + ks::silenceGuardSamples);
    }

    void noteOff (bool allowTailOff)
    {
        if (allowTailOff && params.dampMode == 1)
        {
            damping = true;
            releaseGain = 1.0f;
            releaseCoeff = ks::computeReleaseCoeff (params.releaseTime, static_cast<float> (sampleRate));
        }
    }

    bool isSilent() const { return silence.isSilent(); }

    float processSample (float excitation)
    {
        float delayed = buffer.readDelayed (delaySamples);

        float input;
        if (excitation != 0.0f)
            input = excitation;
        else
            input = ks::applySaturation (dcBlocker.process (lowpass.process (delayed)), params.drive)
                  * feedbackGain;

        if (damping)
        {
            input *= releaseGain;
            releaseGain *= releaseCoeff;
        }

        buffer.write (input);

        float output = delayed * velocity * params.outputLevel;
        if (damping)
            output *= releaseGain;
        silence.track (output);
        return output;
    }

private:
    float nextRandom()
    {
        uint32_t r = rngState;
        r ^= r << 13;
        r ^= r >> 17;
        r ^= r << 5;
        rngState = r;
        return static_cast<float> (r) / static_cast<float> (0xFFFFFFFFu);
    }

    double sampleRate = 44100.0;
    float  delaySamples = 100.0f;
    float  feedbackGain = 0.99f;
    float  cutoffHz = 6000.0f;
    float  velocity = 0.0f;

    bool   damping = false;
    float  releaseGain = 1.0f;
    float  releaseCoeff = 1.0f;

    uint32_t rngState = 555555555u;

    KsParams params;
    CircularBuffer buffer;
    OnePoleLowpass lowpass;
    DcBlocker dcBlocker;
    SilenceDetector silence;
};
