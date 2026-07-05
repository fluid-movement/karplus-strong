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
                  juce::ParameterID { "decay", 1 }, "Decay",
                  juce::NormalisableRange<float> (0.50f, 0.999f), 0.95f),
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
    synth.addSound (new KarplusStrongSound());
    updateNumVoices (8);
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

    int numVoices = static_cast<int> (apvts.getRawParameterValue ("voices")->load());
    if (numVoices != currentNumVoices)
        updateNumVoices (numVoices);

    updateVoiceParameters();
    synth.renderNextBlock (buffer, midi, 0, buffer.getNumSamples());
}

void KarplusStrongProcessor::updateVoiceParameters()
{
    float decay            = apvts.getRawParameterValue ("decay")->load();
    float brightness       = apvts.getRawParameterValue ("brightness")->load();
    int   excitationType  = static_cast<int> (apvts.getRawParameterValue ("excitation")->load());
    float excitationLength= apvts.getRawParameterValue ("excitation_length")->load();
    float pickPosition    = apvts.getRawParameterValue ("pick_position")->load();
    int   pickModel       = static_cast<int> (apvts.getRawParameterValue ("pick_model")->load());
    float velBright       = apvts.getRawParameterValue ("vel_brightness")->load();
    float velDecay        = apvts.getRawParameterValue ("vel_decay")->load();
    float outputLevel     = apvts.getRawParameterValue ("output_level")->load();

    for (int i = 0; i < synth.getNumVoices(); ++i)
        if (auto* voice = dynamic_cast<KarplusStrongVoice*> (synth.getVoice (i)))
            voice->setParameters (decay, brightness, excitationType, excitationLength,
                                  pickPosition, pickModel, velBright, velDecay, outputLevel);
}

void KarplusStrongProcessor::updateNumVoices (int newNumVoices)
{
    if (newNumVoices == currentNumVoices)
        return;

    synth.clearVoices();
    for (int i = 0; i < newNumVoices; ++i)
        synth.addVoice (new KarplusStrongVoice());

    currentNumVoices = newNumVoices;

    if (getSampleRate() > 0)
    {
        for (int i = 0; i < synth.getNumVoices(); ++i)
            if (auto* voice = dynamic_cast<KarplusStrongVoice*> (synth.getVoice (i)))
                voice->prepare (getSampleRate(), 512);
    }

    updateVoiceParameters();
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