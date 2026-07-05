#pragma once

#include "KsCalibration.h"

class OnePoleLowpass
{
public:
    void setCutoff (float cutoffHz, float sampleRate)
    {
        float dt = 1.0f / sampleRate;
        float rc = 1.0f / (ks::twoPi * cutoffHz);
        alpha = dt / (rc + dt);
    }

    void reset() { prev = 0.0f; }

    float process (float input)
    {
        prev = prev + alpha * (input - prev);
        return prev;
    }

private:
    float alpha = 1.0f;
    float prev = 0.0f;
};
