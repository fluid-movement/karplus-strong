#pragma once

#include "KsCalibration.h"

class DcBlocker
{
public:
    void prepare (double sampleRate)
    {
        r = 1.0f - ks::twoPi * ks::dcBlockerHz / static_cast<float> (sampleRate);
    }

    void reset()
    {
        prevIn = 0.0f;
        prevOut = 0.0f;
    }

    float process (float input)
    {
        float output = input - prevIn + r * prevOut;
        prevIn = input;
        prevOut = output;
        return output;
    }

private:
    float r = 0.999f;
    float prevIn = 0.0f;
    float prevOut = 0.0f;
};
