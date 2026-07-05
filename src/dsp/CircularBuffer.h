#pragma once

#include <algorithm>
#include <cmath>
#include <vector>

class CircularBuffer
{
public:
    void prepare (int sizeSamples)
    {
        size = std::max (sizeSamples, 2);
        buffer.assign (static_cast<size_t> (size), 0.0f);
        writePos = 0;
    }

    void clear()
    {
        std::fill (buffer.begin(), buffer.end(), 0.0f);
        writePos = 0;
    }

    void write (float sample)
    {
        buffer[static_cast<size_t> (writePos)] = sample;
        writePos = (writePos + 1) % size;
    }

    float readDelayed (float delaySamples) const
    {
        float d = std::clamp (delaySamples, 1.0f, static_cast<float> (size - 1));
        float readPosF = static_cast<float> (writePos) - d;

        int i0 = static_cast<int> (std::floor (readPosF));
        float frac = readPosF - static_cast<float> (i0);

        int idx0 = i0 % size;
        if (idx0 < 0)
            idx0 += size;
        int idx1 = (idx0 + 1) % size;

        return buffer[static_cast<size_t> (idx0)] * (1.0f - frac)
             + buffer[static_cast<size_t> (idx1)] * frac;
    }

    int getSize() const { return size; }

private:
    std::vector<float> buffer;
    int writePos = 0;
    int size = 2;
};
