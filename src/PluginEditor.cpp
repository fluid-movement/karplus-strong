#include "PluginEditor.h"
#include "Presets.h"

KarplusStrongEditor::KarplusStrongEditor (KarplusStrongProcessor& p)
    : juce::AudioProcessorEditor (&p), processorRef (p)
{
    titleLabel.setText ("Karplus-Strong", juce::dontSendNotification);
    titleLabel.setJustificationType (juce::Justification::centred);
    titleLabel.setFont (juce::Font (juce::FontOptions (18.0f, juce::Font::bold)));
    titleLabel.setColour (juce::Label::textColourId, juce::Colour (0xffeeeeee));
    addAndMakeVisible (titleLabel);

    presetLabel.setText ("Preset", juce::dontSendNotification);
    presetLabel.setJustificationType (juce::Justification::centredRight);
    presetLabel.setFont (juce::Font (juce::FontOptions (12.0f, juce::Font::bold)));
    presetLabel.setColour (juce::Label::textColourId, juce::Colour (0xffcccccc));
    addAndMakeVisible (presetLabel);

    presetCombo.addItem ("Select Preset...", 1);
    {
        int itemId = 2;
        for (auto& preset : getFactoryPresets())
            presetCombo.addItem (preset.name, itemId++);
    }
    presetCombo.setSelectedId (1, juce::dontSendNotification);
    presetCombo.onChange = [this]
    {
        auto& presets = getFactoryPresets();
        int index = presetCombo.getSelectedId() - 2;
        if (index >= 0 && index < static_cast<int> (presets.size()))
            applyPreset (processorRef.apvts, presets[static_cast<size_t> (index)]);
    };
    addAndMakeVisible (presetCombo);

    auto setupGroup = [this] (juce::GroupComponent& group, const juce::String& text)
    {
        group.setText (text);
        group.setColour (juce::GroupComponent::outlineColourId, juce::Colour (0xff444466));
        group.setColour (juce::GroupComponent::textColourId, juce::Colour (0xffaaaaee));
        addAndMakeVisible (group);
    };

    setupGroup (exciterGroup, "Exciter");
    setupGroup (delayLineGroup, "Delay Line");

    addControl (Group::exciter, "excitation", "Excitation");
    addControl (Group::exciter, "excitation_length", "Exc Length");
    addControl (Group::exciter, "pick_position", "Pick Pos");
    addControl (Group::exciter, "pick_model", "Pick Model");
    addControl (Group::exciter, "sine_harmonic", "Sine Harm");
    addControl (Group::exciter, "exciter_tone", "Exc Tone");
    addControl (Group::exciter, "vel_excitation_length", "Vel->Length");
    addControl (Group::exciter, "velvet_density", "Velvet Dens");

    addControl (Group::delayLine, "decay_time", "Decay Time");
    addControl (Group::delayLine, "brightness", "Brightness");
    addControl (Group::delayLine, "vel_brightness", "Vel->Bright");
    addControl (Group::delayLine, "vel_decay", "Vel->Decay");
    addControl (Group::delayLine, "output_level", "Output");
    addControl (Group::delayLine, "voices", "Voices");
    addControl (Group::delayLine, "key_track", "Key Track");
    addControl (Group::delayLine, "drive", "Drive");
    addControl (Group::delayLine, "stiffness", "Stiffness");
    addControl (Group::delayLine, "damp_mode", "Damp Mode");
    addControl (Group::delayLine, "release_time", "Release");
    addControl (Group::delayLine, "humanize", "Humanize");
    addControl (Group::delayLine, "stereo_spread", "Stereo");
    addControl (Group::delayLine, "sympathy", "Sympathy");

    setSize (800, 460);
}

void KarplusStrongEditor::addControl (Group group, const juce::String& paramId, const juce::String& labelText)
{
    auto control = std::make_unique<Control>();
    control->group = group;

    control->label.setText (labelText, juce::dontSendNotification);
    control->label.setJustificationType (juce::Justification::centred);
    control->label.setFont (juce::Font (juce::FontOptions (12.0f, juce::Font::bold)));
    control->label.setColour (juce::Label::textColourId, juce::Colour (0xffcccccc));
    addAndMakeVisible (control->label);

    if (auto* choiceParam = dynamic_cast<juce::AudioParameterChoice*> (processorRef.apvts.getParameter (paramId)))
    {
        control->combo = std::make_unique<juce::ComboBox>();
        control->combo->addItemList (choiceParam->choices, 1);
        addAndMakeVisible (*control->combo);
        control->comboAttachment = std::make_unique<ComboBoxAttachment> (processorRef.apvts, paramId, *control->combo);
    }
    else
    {
        control->slider = std::make_unique<juce::Slider>();
        control->slider->setSliderStyle (juce::Slider::RotaryHorizontalVerticalDrag);
        control->slider->setTextBoxStyle (juce::Slider::TextBoxBelow, false, 70, 16);
        addAndMakeVisible (*control->slider);
        control->sliderAttachment = std::make_unique<SliderAttachment> (processorRef.apvts, paramId, *control->slider);
    }

    controls.push_back (std::move (control));
}

void KarplusStrongEditor::paint (juce::Graphics& g)
{
    g.fillAll (juce::Colour (0xff1a1a2e));
}

void KarplusStrongEditor::resized()
{
    auto bounds = getLocalBounds();
    auto topStrip = bounds.removeFromTop (30);

    auto presetArea = topStrip.removeFromRight (220).reduced (4);
    presetLabel.setBounds (presetArea.removeFromLeft (50));
    presetCombo.setBounds (presetArea);

    titleLabel.setBounds (topStrip.reduced (0, 4));

    auto area = bounds.reduced (10);
    int gap = 8;
    int exciterWidth = 320;

    auto exciterBounds = area.withWidth (exciterWidth);
    auto delayBounds   = area.withTrimmedLeft (exciterWidth + gap);

    exciterGroup.setBounds (exciterBounds);
    delayLineGroup.setBounds (delayBounds);

    layoutGroup (Group::exciter, exciterBounds.reduced (12, 20), 70);
    layoutGroup (Group::delayLine, delayBounds.reduced (12, 20), 58);
}

void KarplusStrongEditor::layoutGroup (Group group, juce::Rectangle<int> inner, int columnWidth)
{
    constexpr int lblH = 16;
    constexpr int ctrlH = 84;
    constexpr int ctrlGap = 4;
    constexpr int rowGap = 4;
    constexpr int rowHeight = lblH + 2 + ctrlH;

    int x = inner.getX();
    int y = inner.getY();

    for (auto& control : controls)
    {
        if (control->group != group)
            continue;

        if (x + columnWidth > inner.getRight() && x != inner.getX())
        {
            x = inner.getX();
            y += rowHeight + rowGap;
        }

        control->label.setBounds (x, y, columnWidth, lblH);
        control->component().setBounds (x, y + lblH + 2, columnWidth, ctrlH);
        x += columnWidth + ctrlGap;
    }
}
