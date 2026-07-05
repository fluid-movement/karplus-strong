#pragma once

#include <algorithm>
#include <juce_audio_basics/juce_audio_basics.h>
#include "KarplusStrongDsp.h"
#include "KsParams.h"

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

    void setParameters (const KsParams& params)
    {
        dsp.setParameters (params);
    }

    void setEnabled (bool shouldBeEnabled) { enabled = shouldBeEnabled; }

    void setPan (float newPan) { pan = std::clamp (newPan, -1.0f, 1.0f); }

    bool canPlaySound (juce::SynthesiserSound*) override { return enabled; }

    void startNote (int midiNoteNumber, float velocity,
                    juce::SynthesiserSound*, int) override
    {
        auto freq = juce::MidiMessage::getMidiNoteInHertz (midiNoteNumber);
        dsp.noteOn (static_cast<float> (freq), velocity);
    }

    void stopNote (float, bool allowTailOff) override
    {
        dsp.noteOff (allowTailOff);

        if (! allowTailOff)
        {
            dsp.reset();
            clearCurrentNote();
        }
    }

    void renderNextBlock (juce::AudioBuffer<float>& outputBuffer, int startSample, int numSamples) override
    {
        float leftGain  = 1.0f - std::max (0.0f, pan);
        float rightGain = 1.0f + std::min (0.0f, pan);

        for (int smp = 0; smp < numSamples; ++smp)
        {
            float out = dsp.processSample();
            outputBuffer.addSample (0, startSample + smp, out * leftGain);
            outputBuffer.addSample (1, startSample + smp, out * rightGain);

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

private:
    bool enabled = true;
    float pan = 0.0f;
};
