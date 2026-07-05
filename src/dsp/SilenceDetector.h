#pragma once

#include <cmath>
#include <algorithm>
#include "KsCalibration.h"

class SilenceDetector
{
public:
    void reset()
    {
        peakAmplitude = 0.0f;
        windowCounter = 0;
        samplesTracked = 0;
    }

    void noteOn (int minSamples_)
    {
        reset();
        minSamples = minSamples_;
    }

    void track (float sample)
    {
        ++samplesTracked;
        ++windowCounter;
        if (windowCounter >= ks::peakDecayWindow)
        {
            peakAmplitude *= 0.5f;
            windowCounter = 0;
        }
        peakAmplitude = std::max (peakAmplitude, std::abs (sample));
    }

    bool isSilent() const
    {
        return samplesTracked >= minSamples && peakAmplitude < ks::silenceThreshold;
    }

private:
    float peakAmplitude = 0.0f;
    int windowCounter = 0;
    int samplesTracked = 0;
    int minSamples = 456;
};
