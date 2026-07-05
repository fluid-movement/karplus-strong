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

    enum class Group { exciter, delayLine };

    struct Control
    {
        Group group = Group::exciter;
        juce::Label label;
        std::unique_ptr<juce::Slider> slider;
        std::unique_ptr<juce::ComboBox> combo;
        std::unique_ptr<SliderAttachment> sliderAttachment;
        std::unique_ptr<ComboBoxAttachment> comboAttachment;

        juce::Component& component()
        {
            return slider != nullptr ? static_cast<juce::Component&> (*slider)
                                     : static_cast<juce::Component&> (*combo);
        }
    };

    void addControl (Group group, const juce::String& paramId, const juce::String& labelText);
    void layoutGroup (Group group, juce::Rectangle<int> inner, int columnWidth);

    KarplusStrongProcessor& processorRef;

    juce::GroupComponent exciterGroup, delayLineGroup;
    juce::Label titleLabel;

    std::vector<std::unique_ptr<Control>> controls;

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (KarplusStrongEditor)
};
