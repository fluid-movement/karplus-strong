#pragma once

class AllpassFilter
{
public:
    void setCoefficient (float newCoeff) { coeff = newCoeff; }

    void reset()
    {
        x1 = 0.0f;
        y1 = 0.0f;
    }

    float process (float input)
    {
        float output = -coeff * input + x1 + coeff * y1;
        x1 = input;
        y1 = output;
        return output;
    }

private:
    float coeff = 0.0f;
    float x1 = 0.0f;
    float y1 = 0.0f;
};
