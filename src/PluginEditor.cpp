#include "PluginEditor.h"

KarplusStrongEditor::KarplusStrongEditor (KarplusStrongProcessor& p)
    : juce::AudioProcessorEditor (&p), processorRef (p)
{
    auto addRotary = [this] (juce::Slider& s)
    {
        s.setSliderStyle (juce::Slider::RotaryHorizontalVerticalDrag);
        s.setTextBoxStyle (juce::Slider::TextBoxBelow, false, 70, 16);
        addAndMakeVisible (s);
    };

    addRotary (excitationLengthSlider);
    addRotary (pickPositionSlider);
    addRotary (decaySlider);
    addRotary (brightnessSlider);
    addRotary (velBrightnessSlider);
    addRotary (velDecaySlider);
    addRotary (outputLevelSlider);

    auto addLabel = [this] (juce::Label& l, const juce::String& text)
    {
        l.setText (text, juce::dontSendNotification);
        l.setJustificationType (juce::Justification::centred);
        l.setFont (juce::Font (12.0f, juce::Font::bold));
        l.setColour (juce::Label::textColourId, juce::Colour (0xffcccccc));
        addAndMakeVisible (l);
    };

    titleLabel.setText ("Karplus-Strong", juce::dontSendNotification);
    titleLabel.setJustificationType (juce::Justification::centred);
    titleLabel.setFont (juce::Font (18.0f, juce::Font::bold));
    titleLabel.setColour (juce::Label::textColourId, juce::Colour (0xffeeeeee));
    addAndMakeVisible (titleLabel);

    addLabel (excitationLabel,  "Excitation");
    addLabel (excLengthLabel,   "Exc Length");
    addLabel (pickPosLabel,      "Pick Pos");
    addLabel (pickModelLabel,    "Pick Model");
    addLabel (decayLabel,       "Decay");
    addLabel (brightnessLabel,   "Brightness");
    addLabel (velBrightLabel,    "Vel->Bright");
    addLabel (velDecayLabel,     "Vel->Decay");
    addLabel (outputLabel,       "Output");

    excitationCombo.addItem ("Noise", 1);
    excitationCombo.addItem ("Sine", 2);
    excitationCombo.addItem ("Dust", 3);
    addAndMakeVisible (excitationCombo);

    pickModelCombo.addItem ("Off", 1);
    pickModelCombo.addItem ("Comb", 2);
    pickModelCombo.addItem ("Two-delay", 3);
    addAndMakeVisible (pickModelCombo);

    exciterGroup.setText ("Exciter");
    exciterGroup.setColour (juce::GroupComponent::outlineColourId, juce::Colour (0xff444466));
    exciterGroup.setColour (juce::GroupComponent::textColourId, juce::Colour (0xffaaaaee));
    addAndMakeVisible (exciterGroup);

    delayLineGroup.setText ("Delay Line");
    delayLineGroup.setColour (juce::GroupComponent::outlineColourId, juce::Colour (0xff444466));
    delayLineGroup.setColour (juce::GroupComponent::textColourId, juce::Colour (0xffaaaaee));
    addAndMakeVisible (delayLineGroup);

    excitationAttach = std::make_unique<ComboBoxAttachment> (processorRef.apvts, "excitation", excitationCombo);
    excLengthAttach  = std::make_unique<SliderAttachment>   (processorRef.apvts, "excitation_length", excitationLengthSlider);
    pickPosAttach    = std::make_unique<SliderAttachment>   (processorRef.apvts, "pick_position", pickPositionSlider);
    pickModelAttach  = std::make_unique<ComboBoxAttachment> (processorRef.apvts, "pick_model", pickModelCombo);

    decayAttach      = std::make_unique<SliderAttachment> (processorRef.apvts, "decay", decaySlider);
    brightnessAttach = std::make_unique<SliderAttachment> (processorRef.apvts, "brightness", brightnessSlider);
    velBrightAttach  = std::make_unique<SliderAttachment> (processorRef.apvts, "vel_brightness", velBrightnessSlider);
    velDecayAttach   = std::make_unique<SliderAttachment> (processorRef.apvts, "vel_decay", velDecaySlider);
    outputAttach     = std::make_unique<SliderAttachment> (processorRef.apvts, "output_level", outputLevelSlider);

    setSize (560, 320);
}

void KarplusStrongEditor::paint (juce::Graphics& g)
{
    g.fillAll (juce::Colour (0xff1a1a2e));
}

void KarplusStrongEditor::resized()
{
    auto bounds = getLocalBounds();
    titleLabel.setBounds (bounds.removeFromTop (30).reduced (0, 4));

    auto area = bounds.reduced (10);
    int halfWidth = area.getWidth() / 2;
    int gap = 8;

    auto exciterBounds = area.withWidth (halfWidth - gap / 2);
    auto delayBounds  = area.withTrimmedLeft (halfWidth + gap / 2);

    exciterGroup.setBounds (exciterBounds);
    delayLineGroup.setBounds (delayBounds);

    auto exInner = exciterBounds.reduced (12, 20);
    auto dlInner = delayBounds.reduced (12, 20);

    constexpr int colW = 84;
    constexpr int lblH = 16;
    constexpr int ctrlH = 84;
    constexpr int ctrlGap = 6;

    struct Control { juce::Component* label; juce::Component* ctrl; };

    Control exciterControls[] = {
        { &excitationLabel, &excitationCombo },
        { &excLengthLabel,   &excitationLengthSlider },
        { &pickPosLabel,     &pickPositionSlider },
        { &pickModelLabel,   &pickModelCombo },
    };

    int x = exInner.getX();
    int y = exInner.getY();
    for (auto& c : exciterControls)
    {
        c.label->setBounds (x, y, colW, lblH);
        c.ctrl ->setBounds (x, y + lblH + 2, colW, ctrlH);
        x += colW + ctrlGap;
    }

    Control delayControls[] = {
        { &decayLabel,       &decaySlider },
        { &brightnessLabel,   &brightnessSlider },
        { &velBrightLabel,    &velBrightnessSlider },
        { &velDecayLabel,     &velDecaySlider },
        { &outputLabel,       &outputLevelSlider },
    };

    x = dlInner.getX();
    y = dlInner.getY();
    for (auto& c : delayControls)
    {
        c.label->setBounds (x, y, colW, lblH);
        c.ctrl ->setBounds (x, y + lblH + 2, colW, ctrlH);
        x += colW + ctrlGap;
    }
}