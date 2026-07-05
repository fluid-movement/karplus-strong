#pragma once

#include <algorithm>
#include <cstdint>
#include "Exciter.h"
#include "KsDelayLine.h"
#include "KsCalibration.h"
#include "KsParams.h"

class KarplusStrongDsp
{
public:
    void prepare (double sampleRate)
    {
        exciter.prepare (sampleRate);
        delayLine.prepare (sampleRate);
    }

    void reset()
    {
        exciter.reset();
        delayLine.reset();
    }

    void setParameters (const KsParams& params)
    {
        humanize = std::clamp (params.humanize, 0.0f, 1.0f);
        delayLine.setParameters (params);
        exciter.setParameters (params);
    }

    void noteOn (float frequency, float velocity)
    {
        float noteFrequency = frequency;
        if (humanize > 0.0f)
        {
            float jitter = (nextRandom() * 2.0f - 1.0f) * humanize * ks::humanizeMaxDetune;
            noteFrequency = frequency * (1.0f + jitter);
        }

        float delaySamp = static_cast<float> (delayLine.getSampleRate()) / noteFrequency;
        exciter.noteOn (delaySamp, noteFrequency, velocity);
        delayLine.noteOn (noteFrequency, velocity, exciter.getExcitationLength());
    }

    void noteOff (bool allowTailOff)
    {
        delayLine.noteOff (allowTailOff);
    }

    bool isSilent() const
    {
        return delayLine.isSilent();
    }

    float processSample()
    {
        float excitation = exciter.isActive() ? exciter.processSample() : 0.0f;
        return delayLine.processSample (excitation);
    }

    Exciter exciter;
    KsDelayLine delayLine;

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

    float humanize = 0.0f;
    uint32_t rngState = 987654321u;
};
