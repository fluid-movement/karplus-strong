#pragma once

#include <cmath>
#include <cstdint>
#include <algorithm>
#include "CircularBuffer.h"
#include "KsCalibration.h"
#include "KsParams.h"

class Exciter
{
public:
    void prepare (double newSampleRate)
    {
        sampleRate = newSampleRate;
        int bufferSize = static_cast<int> (sampleRate * ks::maxDelaySeconds);
        preDelayBuffer.prepare (bufferSize);
        delay2Buffer.prepare (bufferSize);
        delay3Buffer.prepare (bufferSize);
    }

    void reset()
    {
        preDelayBuffer.clear();
        delay2Buffer.clear();
        delay3Buffer.clear();
        phase = 0.0;
        burstRemaining = 0;
    }

    void setParameters (const KsParams& newParams)
    {
        excitationType   = newParams.excitationType;
        excitationLength = std::clamp (newParams.excitationLength, 1.0f, 1000.0f);
        pickPosition     = newParams.pickPosition;
        pickModel        = newParams.pickModel;
    }

    void noteOn (float delaySamples_)
    {
        delaySamples = delaySamples_;
        reset();
        burstRemaining = static_cast<int> (excitationLength);
    }

    bool isActive() const { return burstRemaining > 0; }

    float getExcitationLength() const { return excitationLength; }

    float processSample()
    {
        if (burstRemaining <= 0)
            return 0.0f;

        float excitation = generateExcitation();
        --burstRemaining;

        switch (pickModel)
        {
            case 0:
                return excitation;

            case 1:
            {
                float pd = pickPosition * delaySamples;
                float out = preDelayBuffer.readDelayed (pd);
                preDelayBuffer.write (excitation);
                return out;
            }

            case 2:
            {
                float pos2 = pickPosition * delaySamples;
                float d2 = delaySamples + pos2;
                float d3 = std::max (1.0f, delaySamples - pos2);
                float out = 0.5f * (delay2Buffer.readDelayed (d2)
                                  + delay3Buffer.readDelayed (d3));
                delay2Buffer.write (excitation);
                delay3Buffer.write (excitation);
                return out;
            }

            default:
                return excitation;
        }
    }

private:
    float generateExcitation()
    {
        switch (excitationType)
        {
            case 0:
                return nextRandom() * 2.0f - 1.0f;

            case 1:
            {
                phase += 2.0 * ks::pi * 440.0 / sampleRate;
                if (phase > 2.0 * ks::pi)
                    phase -= 2.0 * ks::pi;
                return static_cast<float> (std::sin (phase));
            }

            case 2:
                return (nextRandomBits() & 1) ? 1.0f : -1.0f;

            default:
                return 0.0f;
        }
    }

    uint32_t nextRandomBits()
    {
        uint32_t r = rngState;
        r ^= r << 13;
        r ^= r >> 17;
        r ^= r << 5;
        rngState = r;
        return r;
    }

    float nextRandom()
    {
        return static_cast<float> (nextRandomBits()) / static_cast<float> (0xFFFFFFFFu);
    }

    double sampleRate = 44100.0;
    float  delaySamples = 100.0f;
    int    burstRemaining = 0;
    double phase = 0.0;

    int   excitationType = 0;
    float excitationLength = 100.0f;
    float pickPosition = 0.2f;
    int   pickModel = 0;

    uint32_t rngState = 123456789u;

    CircularBuffer preDelayBuffer, delay2Buffer, delay3Buffer;
};
