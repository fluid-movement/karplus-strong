#pragma once

#include <cmath>
#include <cstdint>
#include <vector>
#include <algorithm>

class KsDelayLine
{
public:
    static constexpr int maxDelaySamples = 192000;

    void prepare (double newSampleRate)
    {
        sampleRate = newSampleRate;
        delayBuffer.assign (static_cast<size_t> (maxDelaySamples), 0.0f);
    }

    double getSampleRate() const { return sampleRate; }

    void reset()
    {
        std::fill (delayBuffer.begin(), delayBuffer.end(), 0.0f);
        writePos = 0;
        lowpassPrev = 0.0f;
        peakAmplitude = 0.0f;
        peakDecayCounter = 0;
        samplesSinceNoteOn = 0;
        active = false;
        releaseGain = 1.0f;
    }

    void setParameters (float decay_, float brightness_,
                        float velBrightnessAmt_, float velDecayAmt_,
                        float outputLevel_)
    {
        decay            = decay_;
        brightness       = brightness_;
        velBrightnessAmt = velBrightnessAmt_;
        velDecayAmt      = velDecayAmt_;
        outputLevel      = outputLevel_;
    }

    void noteOn (float frequency, float velocity_, float excitationLength)
    {
        velocity     = std::max (velocity_, 0.01f);
        delaySamples = static_cast<float> (sampleRate / frequency);
        excitationLen = excitationLength;

        reset();

        active = true;

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
        else
        {
            active = false;
            releaseGain = 1.0f;
        }
    }

    bool isSilent() const
    {
        if (active)
            return false;
        int minSamples = static_cast<int> (delaySamples) + static_cast<int> (excitationLen) + 256;
        if (samplesSinceNoteOn < minSamples)
            return false;
        return peakAmplitude < 1e-5f;
    }

    float processSample (float excitation)
    {
        ++samplesSinceNoteOn;

        float delayed = readDelay (delayBuffer, writePos, delaySamples);

        float input;
        if (excitation != 0.0f)
        {
            input = excitation;
        }
        else
        {
            float filtered = processLowpass (delayed);
            input = filtered * feedbackGain;
        }

        delayBuffer[writePos] = input;
        writePos = (writePos + 1) % maxDelaySamples;

        lastOutput = delayed * velocity * outputLevel * releaseGain;

        if (! active)
        {
            releaseGain *= 0.999f;
            if (releaseGain < 0.001f)
                releaseGain = 0.0f;
        }

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
    float readDelay (const std::vector<float>& buf, int wpos, float delaySamp) const
    {
        int d = static_cast<int> (delaySamp);
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

    double sampleRate = 44100.0;
    float  delaySamples = 100.0f;
    float  feedbackGain = 0.99f;
    float  velocity = 0.0f;
    float  lastOutput = 0.0f;
    float  excitationLen = 100.0f;

    float  peakAmplitude = 0.0f;
    int    peakDecayCounter = 0;
    int    peakDecayWindow = 256;
    int    samplesSinceNoteOn = 0;
    bool   active = false;
    float  releaseGain = 1.0f;

    float decay = 0.95f;
    float brightness = 0.5f;
    float cutoffHz = 6000.0f;
    float velBrightnessAmt = 0.0f;
    float velDecayAmt = 0.0f;
    float outputLevel = 0.8f;

    float lowpassPrev = 0.0f;

    std::vector<float> delayBuffer;
    int writePos = 0;
};