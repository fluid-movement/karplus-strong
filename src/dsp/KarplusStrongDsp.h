#pragma once

#include "Exciter.h"
#include "KsDelayLine.h"
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
        delayLine.setParameters (params);
        exciter.setParameters (params);
    }

    void noteOn (float frequency, float velocity)
    {
        float delaySamp = static_cast<float> (delayLine.getSampleRate()) / frequency;
        exciter.noteOn (delaySamp);
        delayLine.noteOn (frequency, velocity, exciter.getExcitationLength());
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
};
