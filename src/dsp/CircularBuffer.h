#pragma once

#include <algorithm>
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
        int d = std::clamp (static_cast<int> (delaySamples), 1, size - 1);
        int readPos = writePos - d;
        if (readPos < 0)
            readPos += size;
        return buffer[static_cast<size_t> (readPos)];
    }

    int getSize() const { return size; }

private:
    std::vector<float> buffer;
    int writePos = 0;
    int size = 2;
};
