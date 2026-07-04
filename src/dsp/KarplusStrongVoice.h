#pragma once

#include <juce_audio_basics/juce_audio_basics.h>
#include "KarplusStrongDsp.h"

class KarplusStrongSound : public juce::SynthesiserSound
{
public:
    bool appliesToNote (int) override { return true; }
    bool appliesToChannel (int) override { return true; }
};

class KarplusStrongVoice : public juce::SynthesiserVoice
{
public:
    void prepare (double sampleRate, int samplesPerBlock)
    {
        dsp.prepare (sampleRate);
    }

    void setParameters (float decay, float brightness, int excitationType,
                        float excitationLength, float pickPosition, int pickModel,
                        float velBrightnessAmt, float velDecayAmt, float outputLevel)
    {
        dsp.setParameters (decay, brightness, excitationType, excitationLength,
                           pickPosition, pickModel, velBrightnessAmt, velDecayAmt,
                           outputLevel);
    }

    bool canPlaySound (juce::SynthesiserSound*) override { return true; }

    void startNote (int midiNoteNumber, float velocity,
                    juce::SynthesiserSound*, int) override
    {
        auto freq = juce::MidiMessage::getMidiNoteInHertz (midiNoteNumber);
        dsp.noteOn (freq, velocity);
    }

    void stopNote (float, bool allowTailOff) override
    {
        dsp.noteOff (allowTailOff);
        if (! allowTailOff)
            clearCurrentNote();
    }

    void renderNextBlock (juce::AudioBuffer<float>& outputBuffer, int startSample, int numSamples) override
    {
        for (int smp = 0; smp < numSamples; ++smp)
        {
            float out = dsp.processSample();
            outputBuffer.addSample (0, startSample + smp, out);
            outputBuffer.addSample (1, startSample + smp, out);

            if (dsp.isSilent())
            {
                clearCurrentNote();
                break;
            }
        }
    }

    void pitchWheelMoved (int) override {}
    void controllerMoved (int, int) override {}

    KarplusStrongDsp dsp;
};