#include <juce_audio_processors/juce_audio_processors.h>
#include <juce_audio_basics/juce_audio_basics.h>
#include "PluginProcessor.h"
#include <cmath>
#include <iostream>

int main()
{
    KarplusStrongProcessor processor;
    processor.prepareToPlay(44100.0, 512);

    juce::AudioBuffer<float> buffer(2, 512);
    juce::MidiBuffer midi;
    midi.addEvent(juce::MidiMessage::noteOn(1, 60, 0.8f), 0);

    processor.processBlock(buffer, midi);

    float maxAmp = 0.0f;
    int firstNonZero = -1;
    for (int ch = 0; ch < buffer.getNumChannels(); ++ch)
        for (int s = 0; s < buffer.getNumSamples(); ++s)
        {
            float v = std::abs(buffer.getSample(ch, s));
            if (v > maxAmp) maxAmp = v;
            if (v > 1e-6f && firstNonZero == -1) firstNonZero = s;
        }

    std::cerr << "Block 0: maxAmp=" << maxAmp
              << " firstNonZero=" << firstNonZero << std::endl;

    float maxAfter = 0.0f;
    for (int block = 1; block < 20; ++block)
    {
        buffer.clear();
        midi.clear();
        processor.processBlock(buffer, midi);
        for (int ch = 0; ch < buffer.getNumChannels(); ++ch)
            for (int s = 0; s < buffer.getNumSamples(); ++s)
                maxAfter = std::max(maxAfter, std::abs(buffer.getSample(ch, s)));
    }

    std::cerr << "Blocks 1-19: maxAmp=" << maxAfter << std::endl;
    return (maxAmp > 1e-6f) ? 0 : 1;
}