#pragma once

#include <cmath>
#include <cstdint>
#include <vector>
#include <algorithm>

class KarplusStrongDsp
{
public:
    static constexpr int maxDelaySamples = 192000;

    void prepare (double newSampleRate)
    {
        sampleRate = newSampleRate;
        delayBuffer.assign (static_cast<size_t> (maxDelaySamples), 0.0f);
        preDelayBuffer.assign (static_cast<size_t> (maxDelaySamples), 0.0f);
        delay2Buffer.assign (static_cast<size_t> (maxDelaySamples), 0.0f);
        delay3Buffer.assign (static_cast<size_t> (maxDelaySamples), 0.0f);
    }

    void reset()
    {
        std::fill (delayBuffer.begin(),   delayBuffer.end(),   0.0f);
        std::fill (preDelayBuffer.begin(), preDelayBuffer.end(), 0.0f);
        std::fill (delay2Buffer.begin(),   delay2Buffer.end(),   0.0f);
        std::fill (delay3Buffer.begin(),   delay3Buffer.end(),   0.0f);
        writePos = 0;
        preWritePos = 0;
        d2WritePos = 0;
        d3WritePos = 0;
        lowpassPrev = 0.0f;
        phase = 0.0;
        burstRemaining = 0;
        peakAmplitude = 0.0f;
        peakDecayCounter = 0;
    }

    void setParameters (float decay_, float brightness_, int excitationType_,
                        float excitationLength_, float pickPosition_, int pickModel_,
                        float velBrightnessAmt_, float velDecayAmt_, float outputLevel_)
    {
        decay            = decay_;
        brightness       = brightness_;
        excitationType   = excitationType_;
        excitationLength = std::clamp (excitationLength_, 1.0f, 1000.0f);
        pickPosition     = pickPosition_;
        pickModel        = pickModel_;
        velBrightnessAmt = velBrightnessAmt_;
        velDecayAmt      = velDecayAmt_;
        outputLevel      = outputLevel_;
    }

    void setDecay (float d)     { decay = d; }
    void setBrightness (float b) { brightness = b; }
    float getDecay() const     { return decay; }
    float getBrightness() const { return brightness; }
    float getOutputLevel() const { return outputLevel; }

    void noteOn (float frequency, float velocity_)
    {
        velocity = std::max (velocity_, 0.01f);
        delaySamples = static_cast<float> (sampleRate / frequency);

        reset();

        burstRemaining = static_cast<int> (excitationLength);

        float modDecay  = decay + velDecayAmt * (velocity - 0.5f);
        float modBright = brightness + velBrightnessAmt * (velocity - 0.5f);

        feedbackGain = std::clamp (modDecay, 0.50f, 0.999f);
        cutoffHz     = 200.0f + std::clamp (modBright, 0.0f, 1.0f) * 11800.0f;
    }

    void noteOff (bool allowTailOff)
    {
        if (! allowTailOff)
        {
            reset();
            feedbackGain = 0.0f;
        }
    }

    bool isSilent() const
    {
        return burstRemaining <= 0 && peakAmplitude < 1e-5f;
    }

    float processSample()
    {
        float delayed = readDelay (delayBuffer, writePos, delaySamples);

        float input;
        if (burstRemaining > 0)
        {
            float excitation = generateExcitation();
            --burstRemaining;

            switch (pickModel)
            {
                case 0:
                    input = excitation;
                    break;
                case 1:
                {
                    float pd = pickPosition * delaySamples;
                    preDelayBuffer[preWritePos] = excitation;
                    input = readDelay (preDelayBuffer, preWritePos, pd);
                    preWritePos = (preWritePos + 1) % maxDelaySamples;
                    break;
                }
                case 2:
                {
                    float pos2 = pickPosition * delaySamples;
                    float d2 = delaySamples + pos2;
                    float d3 = std::max (1.0f, delaySamples - pos2);
                    delay2Buffer[d2WritePos] = excitation;
                    delay3Buffer[d3WritePos] = excitation;
                    input = 0.5f * (readDelay (delay2Buffer, d2WritePos, d2)
                                  + readDelay (delay3Buffer, d3WritePos, d3));
                    d2WritePos = (d2WritePos + 1) % maxDelaySamples;
                    d3WritePos = (d3WritePos + 1) % maxDelaySamples;
                    break;
                }
                default:
                    input = excitation;
                    break;
            }
        }
        else
        {
            float filtered = processLowpass (delayed);
            input = filtered * feedbackGain;
        }

        delayBuffer[writePos] = input;
        writePos = (writePos + 1) % maxDelaySamples;

        lastOutput = delayed * velocity * outputLevel;

        peakDecayCounter++;
        if (peakDecayCounter >= peakDecayWindow)
        {
            peakAmplitude *= 0.5f;
            peakDecayCounter = 0;
        }
        peakAmplitude = std::max (peakAmplitude, std::abs (lastOutput));

        return lastOutput;
    }

private:
    float readDelay (const std::vector<float>& buf, int wpos, float delaySamples_) const
    {
        int d = static_cast<int> (delaySamples_);
        if (d < 1) d = 1;
        if (d > maxDelaySamples - 1) d = maxDelaySamples - 1;
        int readPos = wpos - d;
        if (readPos < 0) readPos += maxDelaySamples;
        return buf[static_cast<size_t> (readPos)];
    }

    float processLowpass (float input)
    {
        float dt = 1.0f / static_cast<float> (sampleRate);
        float rc = 1.0f / (2.0f * 3.14159265358979f * cutoffHz);
        float alpha = dt / (rc + dt);
        lowpassPrev = lowpassPrev + alpha * (input - lowpassPrev);
        return lowpassPrev;
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
    float  feedbackGain = 0.99f;
    float  velocity = 0.0f;
    float  lastOutput = 0.0f;
    float  peakAmplitude = 0.0f;
    int    peakDecayCounter = 0;
    int    peakDecayWindow = 256;
    int    burstRemaining = 0;
    double phase = 0.0;

    float decay = 0.95f;
    float brightness = 0.5f;
    float cutoffHz = 6000.0f;
    int   excitationType = 0;
    float excitationLength = 100.0f;
    float pickPosition = 0.2f;
    int   pickModel = 0;
    float velBrightnessAmt = 0.0f;
    float velDecayAmt = 0.0f;
    float outputLevel = 0.8f;

    float lowpassPrev = 0.0f;

    uint32_t rngState = 123456789u;

    std::vector<float> delayBuffer, preDelayBuffer, delay2Buffer, delay3Buffer;
    int writePos   = 0;
    int preWritePos = 0;
    int d2WritePos  = 0;
    int d3WritePos  = 0;
};