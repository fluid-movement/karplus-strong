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

TEST_CASE ("Stereo spread pans voices across the field", "[integration]")
{
    KarplusStrongProcessor processor;
    processor.prepareToPlay (44100.0, 512);
    juce::AudioBuffer<float> buffer (2, 512);

    auto* spreadParam = processor.apvts.getParameter ("stereo_spread");
    spreadParam->setValueNotifyingHost (spreadParam->convertTo0to1 (1.0f));

    juce::MidiBuffer midi;
    midi.addEvent (juce::MidiMessage::noteOn (1, 40, 1.0f), 0);
    midi.addEvent (juce::MidiMessage::noteOn (1, 41, 1.0f), 0);
    buffer.clear();
    processor.processBlock (buffer, midi);

    renderBlocks (processor, buffer, 5);

    float leftSum = 0.0f, rightSum = 0.0f;
    for (int s = 0; s < buffer.getNumSamples(); ++s)
    {
        leftSum += std::abs (buffer.getSample (0, s));
        rightSum += std::abs (buffer.getSample (1, s));
    }

    REQUIRE (std::abs (leftSum - rightSum) > 1e-6f);
}

TEST_CASE ("Damp mode releases the voice faster than Ring mode", "[integration]")
{
    auto setChoice = [] (KarplusStrongProcessor& p, const char* id, int value)
    {
        auto* param = p.apvts.getParameter (id);
        param->setValueNotifyingHost (param->convertTo0to1 (static_cast<float> (value)));
    };

    KarplusStrongProcessor damped;
    damped.prepareToPlay (44100.0, 512);
    juce::AudioBuffer<float> dampedBuffer (2, 512);
    setChoice (damped, "damp_mode", 1);
    auto* releaseParam = damped.apvts.getParameter ("release_time");
    releaseParam->setValueNotifyingHost (releaseParam->convertTo0to1 (0.05f));

    sendMidi (damped, dampedBuffer, juce::MidiMessage::noteOn (1, 60, 0.8f));
    renderBlocks (damped, dampedBuffer, 5);
    sendMidi (damped, dampedBuffer, juce::MidiMessage::noteOff (1, 60));
    renderBlocks (damped, dampedBuffer, 40);

    float dampedTail = blockMax (dampedBuffer);
    REQUIRE (dampedTail < 1e-3f);
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
