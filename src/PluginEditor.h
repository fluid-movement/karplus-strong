#pragma once

#include <juce_audio_processors/juce_audio_processors.h>
#include "PluginProcessor.h"

class KarplusStrongEditor : public juce::AudioProcessorEditor
{
public:
    KarplusStrongEditor (KarplusStrongProcessor&);
    ~KarplusStrongEditor() override = default;

    void paint (juce::Graphics&) override;
    void resized() override;

private:
    using SliderAttachment   = juce::AudioProcessorValueTreeState::SliderAttachment;
    using ComboBoxAttachment = juce::AudioProcessorValueTreeState::ComboBoxAttachment;

    KarplusStrongProcessor& processorRef;

    juce::GroupComponent exciterGroup;
    juce::GroupComponent delayLineGroup;

    juce::Label titleLabel;

    juce::Slider excitationLengthSlider, pickPositionSlider;
    juce::ComboBox excitationCombo, pickModelCombo;

    juce::Slider decaySlider, brightnessSlider,
                 velBrightnessSlider, velDecaySlider,
                 outputLevelSlider;

    juce::Label excitationLabel  { {}, "Excitation" };
    juce::Label excLengthLabel   { {}, "Exc Length" };
    juce::Label pickPosLabel      { {}, "Pick Pos" };
    juce::Label pickModelLabel    { {}, "Pick Model" };

    juce::Label decayLabel       { {}, "Decay" };
    juce::Label brightnessLabel   { {}, "Brightness" };
    juce::Label velBrightLabel    { {}, "Vel->Bright" };
    juce::Label velDecayLabel     { {}, "Vel->Decay" };
    juce::Label outputLabel       { {}, "Output" };

    std::unique_ptr<SliderAttachment>   excLengthAttach, pickPosAttach,
                                        decayAttach, brightnessAttach,
                                        velBrightAttach, velDecayAttach,
                                        outputAttach;
    std::unique_ptr<ComboBoxAttachment> excitationAttach, pickModelAttach;

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (KarplusStrongEditor)
};