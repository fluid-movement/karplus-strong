#include <catch2/catch_test_macros.hpp>
#include <juce_audio_processors/juce_audio_processors.h>
#include <juce_audio_basics/juce_audio_basics.h>
#include "PluginProcessor.h"
#include <cmath>

namespace
{
    float blockMax (const juce::AudioBuffer<float>& buffer)
    {
        float m = 0.0f;
        for (int ch = 0; ch < buffer.getNumChannels(); ++ch)
            for (int s = 0; s < buffer.getNumSamples(); ++s)
                m = std::max (m, std::abs (buffer.getSample (ch, s)));
        return m;
    }

    void renderBlocks (KarplusStrongProcessor& processor, juce::AudioBuffer<float>& buffer, int numBlocks)
    {
        juce::MidiBuffer midi;
        for (int i = 0; i < numBlocks; ++i)
        {
            buffer.clear();
            processor.processBlock (buffer, midi);
        }
    }

    void sendMidi (KarplusStrongProcessor& processor, juce::AudioBuffer<float>& buffer,
                   const juce::MidiMessage& message)
    {
        juce::MidiBuffer midi;
        midi.addEvent (message, 0);
        buffer.clear();
        processor.processBlock (buffer, midi);
    }

    void setVoices (KarplusStrongProcessor& processor, int numVoices)
    {
        auto* param = processor.apvts.getParameter ("voices");
        param->setValueNotifyingHost (param->convertTo0to1 (static_cast<float> (numVoices)));
    }
}

TEST_CASE ("Processor produces sound on noteOn", "[integration]")
{
    KarplusStrongProcessor processor;
    processor.prepareToPlay (44100.0, 512);
    juce::AudioBuffer<float> buffer (2, 512);

    sendMidi (processor, buffer, juce::MidiMessage::noteOn (1, 60, 0.8f));

    float maxAmp = blockMax (buffer);
    juce::MidiBuffer midi;
    for (int block = 1; block < 20; ++block)
    {
        buffer.clear();
        processor.processBlock (buffer, midi);
        maxAmp = std::max (maxAmp, blockMax (buffer));
    }

    REQUIRE (maxAmp > 1e-6f);
}

TEST_CASE ("Released note's tail survives the next noteOn", "[integration]")
{
    KarplusStrongProcessor processor;
    processor.prepareToPlay (44100.0, 512);
    juce::AudioBuffer<float> buffer (2, 512);

    sendMidi (processor, buffer, juce::MidiMessage::noteOn (1, 60, 0.8f));
    renderBlocks (processor, buffer, 19);
    float tailBefore = blockMax (buffer);
    REQUIRE (tailBefore > 1e-4f);

    sendMidi (processor, buffer, juce::MidiMessage::noteOff (1, 60));
    sendMidi (processor, buffer, juce::MidiMessage::noteOn (1, 96, 0.01f));

    float tailAfter = 0.0f;
    for (int block = 0; block < 3; ++block)
    {
        renderBlocks (processor, buffer, 1);
        tailAfter = std::max (tailAfter, blockMax (buffer));
    }

    REQUIRE (tailAfter > tailBefore * 0.3f);
}

TEST_CASE ("With a single voice, the next note steals the ringing tail", "[integration]")
{
    KarplusStrongProcessor processor;
    processor.prepareToPlay (44100.0, 512);
    juce::AudioBuffer<float> buffer (2, 512);

    setVoices (processor, 1);
    renderBlocks (processor, buffer, 1);

    sendMidi (processor, buffer, juce::MidiMessage::noteOn (1, 60, 0.8f));
    renderBlocks (processor, buffer, 19);
    float tailBefore = blockMax (buffer);
    REQUIRE (tailBefore > 1e-4f);

    sendMidi (processor, buffer, juce::MidiMessage::noteOff (1, 60));
    sendMidi (processor, buffer, juce::MidiMessage::noteOn (1, 96, 0.01f));

    float tailAfter = 0.0f;
    for (int block = 0; block < 3; ++block)
    {
        renderBlocks (processor, buffer, 1);
        tailAfter = std::max (tailAfter, blockMax (buffer));
    }

    REQUIRE (tailAfter < tailBefore * 0.3f);
}
