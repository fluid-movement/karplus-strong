#include "PluginEditor.h"

KarplusStrongEditor::KarplusStrongEditor (KarplusStrongProcessor& p)
    : juce::AudioProcessorEditor (&p), processorRef (p)
{
    auto addSlider = [this] (juce::Slider& s)
    {
        s.setSliderStyle (juce::Slider::RotaryHorizontalVerticalDrag);
        s.setTextBoxStyle (juce::Slider::TextBoxBelow, false, 70, 16);
        addAndMakeVisible (s);
    };

    addSlider (decaySlider);
    addSlider (brightnessSlider);
    addSlider (excitationLengthSlider);
    addSlider (pickPositionSlider);
    addSlider (velBrightnessSlider);
    addSlider (velDecaySlider);
    addSlider (outputLevelSlider);

    auto addLabeled = [this] (auto& label, const juce::String& text)
    {
        label.setText (text, juce::dontSendNotification);
        label.setJustificationType (juce::Justification::centred);
        label.setFont (juce::Font (12.0f, juce::Font::bold));
        addAndMakeVisible (label);
    };

    addLabeled (titleLabel, "Karplus-Strong");
    titleLabel.setFont (juce::Font (18.0f, juce::Font::bold));
    addLabeled (decayLabel, "Decay");
    addLabeled (brightnessLabel, "Brightness");
    addLabeled (excitationLabel, "Excitation");
    addLabeled (excLengthLabel, "Exc Length");
    addLabeled (pickPosLabel, "Pick Pos");
    addLabeled (pickModelLabel, "Pick Model");
    addLabeled (velBrightLabel, "Vel->Bright");
    addLabeled (velDecayLabel, "Vel->Decay");
    addLabeled (outputLabel, "Output");

    excitationCombo.addItem ("Noise", 1);
    excitationCombo.addItem ("Sine", 2);
    excitationCombo.addItem ("Dust", 3);
    addAndMakeVisible (excitationCombo);

    pickModelCombo.addItem ("Off", 1);
    pickModelCombo.addItem ("Comb", 2);
    pickModelCombo.addItem ("Two-delay", 3);
    addAndMakeVisible (pickModelCombo);

    decayAttach       = std::make_unique<SliderAttachment> (processorRef.apvts, "decay", decaySlider);
    brightnessAttach  = std::make_unique<SliderAttachment> (processorRef.apvts, "brightness", brightnessSlider);
    excLengthAttach   = std::make_unique<SliderAttachment> (processorRef.apvts, "excitation_length", excitationLengthSlider);
    pickPosAttach     = std::make_unique<SliderAttachment> (processorRef.apvts, "pick_position", pickPositionSlider);
    velBrightAttach   = std::make_unique<SliderAttachment> (processorRef.apvts, "vel_brightness", velBrightnessSlider);
    velDecayAttach    = std::make_unique<SliderAttachment> (processorRef.apvts, "vel_decay", velDecaySlider);
    outputAttach      = std::make_unique<SliderAttachment> (processorRef.apvts, "output_level", outputLevelSlider);
    excitationAttach  = std::make_unique<ComboBoxAttachment> (processorRef.apvts, "excitation", excitationCombo);
    pickModelAttach   = std::make_unique<ComboBoxAttachment> (processorRef.apvts, "pick_model", pickModelCombo);

    setSize (480, 280);
}

void KarplusStrongEditor::paint (juce::Graphics& g)
{
    g.fillAll (juce::Colour (0xff1a1a2e));
}

void KarplusStrongEditor::resized()
{
    auto bounds = getLocalBounds();
    titleLabel.setBounds (bounds.removeFromTop (30));

    constexpr int cols     = 5;
    constexpr int rows     = 2;
    constexpr int colWidth = 90;
    constexpr int rowHeight = 120;
    constexpr int margin    = 10;

    auto area = bounds.reduced (margin);
    int x = area.getX();
    int y = area.getY();

    struct Control { juce::Component* label; juce::Component* control; };

    Control controls[] = {
        { &decayLabel,        &decaySlider },
        { &brightnessLabel,    &brightnessSlider },
        { &excitationLabel,    &excitationCombo },
        { &excLengthLabel,     &excitationLengthSlider },
        { &pickPosLabel,       &pickPositionSlider },
        { &pickModelLabel,     &pickModelCombo },
        { &velBrightLabel,     &velBrightnessSlider },
        { &velDecayLabel,      &velDecaySlider },
        { &outputLabel,        &outputLevelSlider },
    };

    for (int i = 0; i < 9; ++i)
    {
        int col = i % cols;
        int row = i / cols;
        int cx  = x + col * (colWidth + margin);
        int cy  = y + row * (rowHeight + margin);

        controls[i].label->setBounds (cx, cy, colWidth, 18);
        controls[i].control->setBounds (cx, cy + 20, colWidth, 90);
    }
}