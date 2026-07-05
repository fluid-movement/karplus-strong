#pragma once

#include <cmath>
#include <cstdint>
#include <vector>
#include <algorithm>

class Exciter
{
public:
    static constexpr int maxDelaySamples = 192000;

    void prepare (double newSampleRate)
    {
        sampleRate = newSampleRate;
        preDelayBuffer.assign (static_cast<size_t> (maxDelaySamples), 0.0f);
        delay2Buffer.assign (static_cast<size_t> (maxDelaySamples), 0.0f);
        delay3Buffer.assign (static_cast<size_t> (maxDelaySamples), 0.0f);
    }

    void reset()
    {
        std::fill (preDelayBuffer.begin(), preDelayBuffer.end(), 0.0f);
        std::fill (delay2Buffer.begin(),   delay2Buffer.end(),   0.0f);
        std::fill (delay3Buffer.begin(),   delay3Buffer.end(),   0.0f);
        preWritePos = 0;
        d2WritePos  = 0;
        d3WritePos  = 0;
        phase       = 0.0;
        burstRemaining = 0;
    }

    void setParameters (int excitationType_, float excitationLength_,
                        float pickPosition_, int pickModel_)
    {
        excitationType   = excitationType_;
        excitationLength = std::clamp (excitationLength_, 1.0f, 1000.0f);
        pickPosition     = pickPosition_;
        pickModel        = pickModel_;
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
                preDelayBuffer[preWritePos] = excitation;
                float out = readDelay (preDelayBuffer, preWritePos, pd);
                preWritePos = (preWritePos + 1) % maxDelaySamples;
                return out;
            }

            case 2:
            {
                float pos2 = pickPosition * delaySamples;
                float d2 = delaySamples + pos2;
                float d3 = std::max (1.0f, delaySamples - pos2);
                delay2Buffer[d2WritePos] = excitation;
                delay3Buffer[d3WritePos] = excitation;
                float out = 0.5f * (readDelay (delay2Buffer, d2WritePos, d2)
                                  + readDelay (delay3Buffer, d3WritePos, d3));
                d2WritePos = (d2WritePos + 1) % maxDelaySamples;
                d3WritePos = (d3WritePos + 1) % maxDelaySamples;
                return out;
            }

            default:
                return excitation;
        }
    }

private:
    float readDelay (const std::vector<float>& buf, int wpos, float delaySamp) const
    {
        int d = static_cast<int> (delaySamp);
        if (d < 1) d = 1;
        if (d > maxDelaySamples - 1) d = maxDelaySamples - 1;
        int readPos = wpos - d;
        if (readPos < 0) readPos += maxDelaySamples;
        return buf[static_cast<size_t> (readPos)];
    }

    float generateExcitation()
    {
        switch (excitationType)
        {
            case 0:
            {
                uint32_t r = rngState;
                r ^= r << 13;
                r ^= r >> 17;
                r ^= r << 5;
                rngState = r;
                return static_cast<float> (r) / static_cast<float> (0xFFFFFFFFu) * 2.0f - 1.0f;
            }
            case 1:
            {
                phase += 2.0 * 3.14159265358979 * 440.0 / sampleRate;
                if (phase > 2.0 * 3.14159265358979)
                    phase -= 2.0 * 3.14159265358979;
                return static_cast<float> (std::sin (phase));
            }
            case 2:
            {
                uint32_t r = rngState;
                r ^= r << 13;
                r ^= r >> 17;
                r ^= r << 5;
                rngState = r;
                return (r & 1) ? 1.0f : -1.0f;
            }
            default:
                return 0.0f;
        }
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

    std::vector<float> preDelayBuffer, delay2Buffer, delay3Buffer;
    int preWritePos = 0;
    int d2WritePos  = 0;
    int d3WritePos  = 0;
};