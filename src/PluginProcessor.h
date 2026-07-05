#pragma once

#include <juce_audio_processors/juce_audio_processors.h>
#include <juce_audio_basics/juce_audio_basics.h>
#include "dsp/KarplusStrongVoice.h"

class KarplusStrongProcessor : public juce::AudioProcessor
{
public:
    static constexpr int maxVoices = 16;

    KarplusStrongProcessor();
    ~KarplusStrongProcessor() override = default;

    void prepareToPlay (double sampleRate, int samplesPerBlock) override;
    void releaseResources() override {}
    void processBlock (juce::AudioBuffer<float>&, juce::MidiBuffer&) override;

    juce::AudioProcessorEditor* createEditor() override;
    bool hasEditor() const override { return true; }

    const juce::String getName() const override { return "Karplus-Strong"; }
    bool acceptsMidi() const override { return true; }
    bool producesMidi() const override { return false; }
    bool isMidiEffect() const override { return false; }
    double getTailLengthSeconds() const override { return 0.0; }

    int getNumPrograms() override { return 1; }
    int getCurrentProgram() override { return 0; }
    void setCurrentProgram (int) override {}
    const juce::String getProgramName (int) override { return {}; }
    void changeProgramName (int, const juce::String&) override {}

    void getStateInformation (juce::MemoryBlock& destData) override;
    void setStateInformation (const void* data, int sizeInBytes) override;

    bool isBusesLayoutSupported (const BusesLayout& layouts) const override
    {
        return layouts.getMainOutputChannelSet() == juce::AudioChannelSet::stereo();
    }

    juce::AudioProcessorValueTreeState apvts;

private:
    juce::Synthesiser synth;
    int currentNumVoices = 0;

    std::atomic<float>* excitationParam         = nullptr;
    std::atomic<float>* excitationLengthParam   = nullptr;
    std::atomic<float>* pickPositionParam       = nullptr;
    std::atomic<float>* pickModelParam          = nullptr;
    std::atomic<float>* sineHarmonicParam       = nullptr;
    std::atomic<float>* exciterToneParam        = nullptr;
    std::atomic<float>* velExcitationLengthParam = nullptr;
    std::atomic<float>* decayTimeParam          = nullptr;
    std::atomic<float>* keyTrackParam           = nullptr;
    std::atomic<float>* brightnessParam         = nullptr;
    std::atomic<float>* velBrightnessParam      = nullptr;
    std::atomic<float>* velDecayParam           = nullptr;
    std::atomic<float>* driveParam              = nullptr;
    std::atomic<float>* dampModeParam           = nullptr;
    std::atomic<float>* releaseTimeParam        = nullptr;
    std::atomic<float>* humanizeParam           = nullptr;
    std::atomic<float>* outputLevelParam        = nullptr;
    std::atomic<float>* voicesParam             = nullptr;
    std::atomic<float>* stereoSpreadParam       = nullptr;

    void updateVoiceParameters();
    void updateNumVoices (int newNumVoices);
    void updateVoicePans();

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (KarplusStrongProcessor)
};