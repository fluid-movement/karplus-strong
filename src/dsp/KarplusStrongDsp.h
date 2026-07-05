#pragma once

#include <cmath>
#include <algorithm>
#include "Exciter.h"
#include "KsDelayLine.h"

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

    void setParameters (float decay, float brightness, int excitationType,
                        float excitationLength, float pickPosition, int pickModel,
                        float velBrightnessAmt, float velDecayAmt, float outputLevel)
    {
        delayLine.setParameters (decay, brightness, velBrightnessAmt, velDecayAmt, outputLevel);
        exciter.setParameters (excitationType, excitationLength, pickPosition, pickModel);
    }

    void noteOn (float frequency, float velocity)
    {
        float delaySamp = delayLine.getSampleRate() / frequency;
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