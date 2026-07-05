#include "PluginProcessor.h"
#include "PluginEditor.h"

KarplusStrongProcessor::KarplusStrongProcessor()
    : juce::AudioProcessor (BusesProperties().withOutput ("Output", juce::AudioChannelSet::stereo(), true)),
      apvts (*this, nullptr, "PARAMETERS",
          {
              std::make_unique<juce::AudioParameterChoice>(
                  juce::ParameterID { "excitation", 1 }, "Excitation Type",
                  juce::StringArray { "Noise", "Sine", "Dust" }, 0),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "excitation_length", 1 }, "Excitation Length",
                  juce::NormalisableRange<float> (1.0f, 1000.0f), 100.0f),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "pick_position", 1 }, "Pick Position",
                  juce::NormalisableRange<float> (0.0f, 0.5f), 0.2f),
              std::make_unique<juce::AudioParameterChoice>(
                  juce::ParameterID { "pick_model", 1 }, "Pick Model",
                  juce::StringArray { "Off", "Comb", "Two-delay" }, 0),
               std::make_unique<juce::AudioParameterFloat>(
                   juce::ParameterID { "decay_time", 1 }, "Decay Time",
                   [] { auto r = juce::NormalisableRange<float> (0.5f, 40.0f); r.setSkewForCentre (8.0f); return r; }(),
                   4.0f,
                   juce::AudioParameterFloatAttributes()
                       .withStringFromValueFunction ([](float value, int) { return juce::String (value, 2) + " s"; })
                       .withValueFromStringFunction ([](const juce::String& text) { return text.trimCharactersAtEnd (" s").getFloatValue(); })),
               std::make_unique<juce::AudioParameterFloat>(
                   juce::ParameterID { "key_track", 1 }, "Key Track",
                   juce::NormalisableRange<float> (0.0f, 1.0f), 0.0f),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "brightness", 1 }, "Brightness",
                  juce::NormalisableRange<float> (0.0f, 1.0f), 0.5f),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "vel_brightness", 1 }, "Velocity->Brightness",
                  juce::NormalisableRange<float> (0.0f, 1.0f), 0.0f),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "vel_decay", 1 }, "Velocity->Decay",
                  juce::NormalisableRange<float> (0.0f, 1.0f), 0.0f),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "output_level", 1 }, "Output Level",
                  juce::NormalisableRange<float> (0.0f, 1.0f), 0.8f),
              std::make_unique<juce::AudioParameterInt>(
                  juce::ParameterID { "voices", 1 }, "Voices", 1, 16, 8),
          })
{
    excitationParam       = apvts.getRawParameterValue ("excitation");
    excitationLengthParam = apvts.getRawParameterValue ("excitation_length");
    pickPositionParam     = apvts.getRawParameterValue ("pick_position");
    pickModelParam        = apvts.getRawParameterValue ("pick_model");
    decayTimeParam        = apvts.getRawParameterValue ("decay_time");
    keyTrackParam         = apvts.getRawParameterValue ("key_track");
    brightnessParam       = apvts.getRawParameterValue ("brightness");
    velBrightnessParam    = apvts.getRawParameterValue ("vel_brightness");
    velDecayParam         = apvts.getRawParameterValue ("vel_decay");
    outputLevelParam      = apvts.getRawParameterValue ("output_level");
    voicesParam           = apvts.getRawParameterValue ("voices");

    synth.addSound (new KarplusStrongSound());
    for (int i = 0; i < maxVoices; ++i)
        synth.addVoice (new KarplusStrongVoice());

    updateNumVoices (static_cast<int> (voicesParam->load()));
}

void KarplusStrongProcessor::prepareToPlay (double sampleRate, int samplesPerBlock)
{
    for (int i = 0; i < synth.getNumVoices(); ++i)
        if (auto* voice = dynamic_cast<KarplusStrongVoice*> (synth.getVoice (i)))
            voice->prepare (sampleRate, samplesPerBlock);

    synth.setCurrentPlaybackSampleRate (sampleRate);
    updateVoiceParameters();
}

void KarplusStrongProcessor::processBlock (juce::AudioBuffer<float>& buffer, juce::MidiBuffer& midi)
{
    buffer.clear();

    updateNumVoices (static_cast<int> (voicesParam->load()));
    updateVoiceParameters();
    synth.renderNextBlock (buffer, midi, 0, buffer.getNumSamples());
}

void KarplusStrongProcessor::updateVoiceParameters()
{
    KsParams params;
    params.excitationType   = static_cast<int> (excitationParam->load());
    params.excitationLength = excitationLengthParam->load();
    params.pickPosition     = pickPositionParam->load();
    params.pickModel        = static_cast<int> (pickModelParam->load());
    params.decayTime        = decayTimeParam->load();
    params.keyTrack         = keyTrackParam->load();
    params.brightness       = brightnessParam->load();
    params.velBrightness    = velBrightnessParam->load();
    params.velDecay         = velDecayParam->load();
    params.outputLevel      = outputLevelParam->load();

    for (int i = 0; i < synth.getNumVoices(); ++i)
        if (auto* voice = dynamic_cast<KarplusStrongVoice*> (synth.getVoice (i)))
            voice->setParameters (params);
}

void KarplusStrongProcessor::updateNumVoices (int newNumVoices)
{
    if (newNumVoices == currentNumVoices)
        return;

    for (int i = 0; i < synth.getNumVoices(); ++i)
        if (auto* voice = dynamic_cast<KarplusStrongVoice*> (synth.getVoice (i)))
            voice->setEnabled (i < newNumVoices);

    currentNumVoices = newNumVoices;
}

juce::AudioProcessorEditor* KarplusStrongProcessor::createEditor()
{
    return new KarplusStrongEditor (*this);
}

void KarplusStrongProcessor::getStateInformation (juce::MemoryBlock& destData)
{
    auto state = apvts.copyState();
    std::unique_ptr<juce::XmlElement> xml (state.createXml());
    copyXmlToBinary (*xml, destData);
}

void KarplusStrongProcessor::setStateInformation (const void* data, int sizeInBytes)
{
    std::unique_ptr<juce::XmlElement> xml (getXmlFromBinary (data, sizeInBytes));
    if (xml != nullptr && xml->hasTagName (apvts.state.getType()))
        apvts.replaceState (juce::ValueTree::fromXml (*xml));
}

juce::AudioProcessor* JUCE_CALLTYPE createPluginFilter()
{
    return new KarplusStrongProcessor();
}