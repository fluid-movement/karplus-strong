#include <catch2/catch_test_macros.hpp>
#include <juce_audio_processors/juce_audio_processors.h>
#include <juce_audio_basics/juce_audio_basics.h>
#include "PluginProcessor.h"
#include "Presets.h"
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

    void setFloat (KarplusStrongProcessor& p, const char* id, float value)
    {
        auto* param = p.apvts.getParameter (id);
        param->setValueNotifyingHost (param->convertTo0to1 (value));
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

TEST_CASE ("Sympathy extends how long simultaneous notes take to quiet down", "[integration]")
{
    KarplusStrongProcessor noSympathy;
    noSympathy.prepareToPlay (44100.0, 512);
    juce::AudioBuffer<float> bufferA (2, 512);
    setFloat (noSympathy, "decay_time", 0.5f);

    KarplusStrongProcessor withSympathy;
    withSympathy.prepareToPlay (44100.0, 512);
    juce::AudioBuffer<float> bufferB (2, 512);
    setFloat (withSympathy, "decay_time", 0.5f);
    setFloat (withSympathy, "sympathy", 1.0f);

    juce::MidiBuffer midiA, midiB;
    midiA.addEvent (juce::MidiMessage::noteOn (1, 40, 1.0f), 0);
    midiA.addEvent (juce::MidiMessage::noteOn (1, 44, 1.0f), 0);
    midiB.addEvent (juce::MidiMessage::noteOn (1, 40, 1.0f), 0);
    midiB.addEvent (juce::MidiMessage::noteOn (1, 44, 1.0f), 0);
    bufferA.clear();
    noSympathy.processBlock (bufferA, midiA);
    bufferB.clear();
    withSympathy.processBlock (bufferB, midiB);

    auto blocksUntilQuiet = [] (KarplusStrongProcessor& p, juce::AudioBuffer<float>& buf)
    {
        juce::MidiBuffer empty;
        int blocks = 0;
        while (blocks < 2000)
        {
            buf.clear();
            p.processBlock (buf, empty);
            ++blocks;
            if (blockMax (buf) < 1e-4f)
                break;
        }
        return blocks;
    };

    int blocksA = blocksUntilQuiet (noSympathy, bufferA);
    int blocksB = blocksUntilQuiet (withSympathy, bufferB);

    REQUIRE (blocksB > blocksA);
}

TEST_CASE ("Sympathy stays bounded and finite at extreme drive/decay/polyphony settings", "[integration]")
{
    KarplusStrongProcessor processor;
    processor.prepareToPlay (44100.0, 512);
    juce::AudioBuffer<float> buffer (2, 512);

    setVoices (processor, 16);
    setFloat (processor, "drive", 1.0f);
    setFloat (processor, "decay_time", 40.0f);
    setFloat (processor, "sympathy", 1.0f);

    juce::MidiBuffer midi;
    for (int note = 40; note < 56; ++note)
        midi.addEvent (juce::MidiMessage::noteOn (1, note, 1.0f), 0);
    buffer.clear();
    processor.processBlock (buffer, midi);

    for (int block = 0; block < 300; ++block)
    {
        renderBlocks (processor, buffer, 1);
        for (int ch = 0; ch < buffer.getNumChannels(); ++ch)
            for (int s = 0; s < buffer.getNumSamples(); ++s)
            {
                float v = buffer.getSample (ch, s);
                REQUIRE (std::isfinite (v));
                REQUIRE (std::abs (v) < 50.0f);
            }
    }
}

TEST_CASE ("Applying a factory preset changes parameters", "[integration]")
{
    KarplusStrongProcessor processor;
    processor.prepareToPlay (44100.0, 512);

    auto& presets = getFactoryPresets();
    REQUIRE (presets.size() > 1);

    auto& guitar = presets[1];
    applyPreset (processor.apvts, guitar);

    for (auto& idValue : guitar.values)
    {
        auto* param = processor.apvts.getParameter (idValue.first);
        REQUIRE (param != nullptr);
        REQUIRE (std::abs (param->getValue() - param->convertTo0to1 (idValue.second)) < 1e-4f);
    }
}

TEST_CASE ("Init preset resets every parameter to its default", "[integration]")
{
    KarplusStrongProcessor processor;
    processor.prepareToPlay (44100.0, 512);

    setFloat (processor, "decay_time", 20.0f);
    setFloat (processor, "brightness", 0.9f);

    auto& presets = getFactoryPresets();
    applyPreset (processor.apvts, presets[0]);

    REQUIRE (std::abs (processor.apvts.getRawParameterValue ("decay_time")->load() - 4.0f) < 1e-3f);
    REQUIRE (std::abs (processor.apvts.getRawParameterValue ("brightness")->load() - 0.5f) < 1e-3f);
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
