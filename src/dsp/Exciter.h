#pragma once

#include <cmath>
#include <cstdint>
#include <algorithm>
#include "CircularBuffer.h"
#include "KsCalibration.h"
#include "KsParams.h"
#include "OnePoleLowpass.h"

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
        toneFilter.reset();
    }

    void setParameters (const KsParams& newParams)
    {
        excitationType      = newParams.excitationType;
        excitationLength    = std::clamp (newParams.excitationLength, 1.0f, 1000.0f);
        pickPosition        = newParams.pickPosition;
        pickModel           = newParams.pickModel;
        sineHarmonic        = std::max (newParams.sineHarmonic, 1);
        exciterTone         = std::clamp (newParams.exciterTone, -1.0f, 1.0f);
        velExcitationLength = newParams.velExcitationLength;
        humanize            = std::clamp (newParams.humanize, 0.0f, 1.0f);
    }

    void noteOn (float delaySamples_, float frequency_, float velocity_)
    {
        delaySamples = delaySamples_;
        frequency = frequency_;
        reset();

        float notePickPosition = pickPosition;
        if (humanize > 0.0f)
        {
            float jitter = (nextRandom() * 2.0f - 1.0f) * humanize * ks::humanizeMaxPickJitter;
            notePickPosition = std::clamp (pickPosition + jitter, 0.0f, 0.5f);
        }
        pickPositionForNote = notePickPosition;

        float modLength = excitationLength * (1.0f + velExcitationLength * (velocity_ - 0.5f));
        noteExcitationLength = std::clamp (modLength, 1.0f, 1000.0f);
        burstRemaining = static_cast<int> (noteExcitationLength);

        toneFilterActive = std::abs (exciterTone) > ks::exciterFilterDeadzone;
        if (toneFilterActive)
        {
            toneFilterIsHighpass = exciterTone > 0.0f;
            float cutoffHz = toneFilterIsHighpass
                ? ks::computeCutoffHz (exciterTone)
                : ks::computeCutoffHz (1.0f + exciterTone);
            toneFilter.setCutoff (cutoffHz, static_cast<float> (sampleRate));
        }
    }

    bool isActive() const { return burstRemaining > 0; }

    float getExcitationLength() const { return noteExcitationLength; }

    float processSample()
    {
        if (burstRemaining <= 0)
            return 0.0f;

        float excitation = generateExcitation();
        if (toneFilterActive)
        {
            float lowpassed = toneFilter.process (excitation);
            excitation = toneFilterIsHighpass ? (excitation - lowpassed) : lowpassed;
        }
        --burstRemaining;

        switch (pickModel)
        {
            case 0:
                return excitation;

            case 1:
            {
                float pd = pickPositionForNote * delaySamples;
                float out = preDelayBuffer.readDelayed (pd);
                preDelayBuffer.write (excitation);
                return out;
            }

            case 2:
            {
                float pos2 = pickPositionForNote * delaySamples;
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
                phase += 2.0 * ks::pi * static_cast<double> (frequency) * sineHarmonic / sampleRate;
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
    float  frequency = 440.0f;
    int    burstRemaining = 0;
    double phase = 0.0;

    int   excitationType = 0;
    float excitationLength = 100.0f;
    float noteExcitationLength = 100.0f;
    float pickPosition = 0.2f;
    float pickPositionForNote = 0.2f;
    int   pickModel = 0;
    int   sineHarmonic = 1;
    float exciterTone = 0.0f;
    bool  toneFilterActive = false;
    bool  toneFilterIsHighpass = false;
    float velExcitationLength = 0.0f;
    float humanize = 0.0f;

    uint32_t rngState = 123456789u;

    OnePoleLowpass toneFilter;
    CircularBuffer preDelayBuffer, delay2Buffer, delay3Buffer;
};
