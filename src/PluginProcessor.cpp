#include "PluginProcessor.h"
#include "PluginEditor.h"
#include "dsp/KsCalibration.h"
#include <algorithm>

KarplusStrongProcessor::KarplusStrongProcessor()
    : juce::AudioProcessor (BusesProperties().withOutput ("Output", juce::AudioChannelSet::stereo(), true)),
      apvts (*this, nullptr, "PARAMETERS",
          {
              std::make_unique<juce::AudioParameterChoice>(
                  juce::ParameterID { "excitation", 1 }, "Excitation Type",
                  juce::StringArray { "Noise", "Sine", "Dust", "Chirp", "Velvet" }, 0),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "excitation_length", 1 }, "Excitation Length",
                  juce::NormalisableRange<float> (1.0f, 1000.0f), 100.0f),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "pick_position", 1 }, "Pick Position",
                  juce::NormalisableRange<float> (0.0f, 0.5f), 0.2f),
              std::make_unique<juce::AudioParameterChoice>(
                  juce::ParameterID { "pick_model", 1 }, "Pick Model",
                  juce::StringArray { "Off", "Comb", "Two-delay" }, 0),
              std::make_unique<juce::AudioParameterInt>(
                  juce::ParameterID { "sine_harmonic", 1 }, "Sine Harmonic", 1, 8, 1),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "exciter_tone", 1 }, "Exciter Tone",
                  juce::NormalisableRange<float> (-1.0f, 1.0f), 0.0f),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "vel_excitation_length", 1 }, "Velocity->Length",
                  juce::NormalisableRange<float> (0.0f, 1.0f), 0.0f),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "velvet_density", 1 }, "Velvet Density",
                  [] { auto r = juce::NormalisableRange<float> (50.0f, 8000.0f); r.setSkewForCentre (1500.0f); return r; }(),
                  2000.0f),
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
                  juce::ParameterID { "drive", 1 }, "Drive",
                  juce::NormalisableRange<float> (0.0f, 1.0f), 0.0f),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "stiffness", 1 }, "Stiffness",
                  juce::NormalisableRange<float> (0.0f, 1.0f), 0.0f),
              std::make_unique<juce::AudioParameterChoice>(
                  juce::ParameterID { "damp_mode", 1 }, "Damp Mode",
                  juce::StringArray { "Ring", "Damp" }, 0),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "release_time", 1 }, "Release Time",
                  juce::NormalisableRange<float> (0.01f, 2.0f), 0.3f,
                  juce::AudioParameterFloatAttributes()
                      .withStringFromValueFunction ([](float value, int) { return juce::String (value, 2) + " s"; })
                      .withValueFromStringFunction ([](const juce::String& text) { return text.trimCharactersAtEnd (" s").getFloatValue(); })),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "humanize", 1 }, "Humanize",
                  juce::NormalisableRange<float> (0.0f, 1.0f), 0.0f),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "output_level", 1 }, "Output Level",
                  juce::NormalisableRange<float> (0.0f, 1.0f), 0.8f),
              std::make_unique<juce::AudioParameterInt>(
                  juce::ParameterID { "voices", 1 }, "Voices", 1, 16, 8),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "stereo_spread", 1 }, "Stereo Spread",
                  juce::NormalisableRange<float> (0.0f, 1.0f), 0.0f),
              std::make_unique<juce::AudioParameterFloat>(
                  juce::ParameterID { "sympathy", 1 }, "Sympathy",
                  juce::NormalisableRange<float> (0.0f, 1.0f), 0.0f),
          })
{
    excitationParam          = apvts.getRawParameterValue ("excitation");
    excitationLengthParam    = apvts.getRawParameterValue ("excitation_length");
    pickPositionParam        = apvts.getRawParameterValue ("pick_position");
    pickModelParam           = apvts.getRawParameterValue ("pick_model");
    sineHarmonicParam        = apvts.getRawParameterValue ("sine_harmonic");
    exciterToneParam         = apvts.getRawParameterValue ("exciter_tone");
    velExcitationLengthParam = apvts.getRawParameterValue ("vel_excitation_length");
    velvetDensityParam       = apvts.getRawParameterValue ("velvet_density");
    decayTimeParam           = apvts.getRawParameterValue ("decay_time");
    keyTrackParam            = apvts.getRawParameterValue ("key_track");
    brightnessParam          = apvts.getRawParameterValue ("brightness");
    velBrightnessParam       = apvts.getRawParameterValue ("vel_brightness");
    velDecayParam            = apvts.getRawParameterValue ("vel_decay");
    driveParam               = apvts.getRawParameterValue ("drive");
    stiffnessParam           = apvts.getRawParameterValue ("stiffness");
    dampModeParam            = apvts.getRawParameterValue ("damp_mode");
    releaseTimeParam         = apvts.getRawParameterValue ("release_time");
    humanizeParam            = apvts.getRawParameterValue ("humanize");
    outputLevelParam         = apvts.getRawParameterValue ("output_level");
    voicesParam              = apvts.getRawParameterValue ("voices");
    stereoSpreadParam        = apvts.getRawParameterValue ("stereo_spread");
    sympathyParam            = apvts.getRawParameterValue ("sympathy");

    synth.addSound (new KarplusStrongSound());
    for (int i = 0; i < maxVoices; ++i)
        synth.addVoice (new KarplusStrongVoice());

    updateNumVoices (static_cast<int> (voicesParam->load()));
    updateVoicePans();
}

void KarplusStrongProcessor::prepareToPlay (double sampleRate, int samplesPerBlock)
{
    maxBlockSize = samplesPerBlock;
    voiceMixSum.assign (static_cast<size_t> (samplesPerBlock), 0.0f);

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
    updateVoicePans();

    const int numSamples = std::min (buffer.getNumSamples(), maxBlockSize);
    clearVoiceOutputScratch (numSamples);

    synth.renderNextBlock (buffer, midi, 0, buffer.getNumSamples());

    updateSympatheticBleed (numSamples);
}

void KarplusStrongProcessor::updateVoiceParameters()
{
    KsParams params;
    params.excitationType      = static_cast<int> (excitationParam->load());
    params.excitationLength    = excitationLengthParam->load();
    params.pickPosition        = pickPositionParam->load();
    params.pickModel           = static_cast<int> (pickModelParam->load());
    params.sineHarmonic        = static_cast<int> (sineHarmonicParam->load());
    params.exciterTone         = exciterToneParam->load();
    params.velExcitationLength = velExcitationLengthParam->load();
    params.velvetDensity       = velvetDensityParam->load();
    params.decayTime           = decayTimeParam->load();
    params.keyTrack            = keyTrackParam->load();
    params.brightness          = brightnessParam->load();
    params.velBrightness       = velBrightnessParam->load();
    params.velDecay            = velDecayParam->load();
    params.drive               = driveParam->load();
    params.stiffness           = stiffnessParam->load();
    params.dampMode            = static_cast<int> (dampModeParam->load());
    params.releaseTime         = releaseTimeParam->load();
    params.humanize            = humanizeParam->load();
    params.outputLevel         = outputLevelParam->load();

    for (int i = 0; i < synth.getNumVoices(); ++i)
        if (auto* voice = dynamic_cast<KarplusStrongVoice*> (synth.getVoice (i)))
            voice->setParameters (params);
}

void KarplusStrongProcessor::clearVoiceOutputScratch (int numSamples)
{
    for (int i = 0; i < synth.getNumVoices(); ++i)
        if (auto* voice = dynamic_cast<KarplusStrongVoice*> (synth.getVoice (i)))
            voice->clearOutputScratch (numSamples);
}

void KarplusStrongProcessor::updateSympatheticBleed (int numSamples)
{
    std::fill (voiceMixSum.begin(), voiceMixSum.begin() + numSamples, 0.0f);

    for (int i = 0; i < synth.getNumVoices(); ++i)
        if (auto* voice = dynamic_cast<KarplusStrongVoice*> (synth.getVoice (i)))
        {
            const auto& scratch = voice->getOutputScratch();
            for (int s = 0; s < numSamples; ++s)
                voiceMixSum[static_cast<size_t> (s)] += scratch[static_cast<size_t> (s)];
        }

    float sympathy = sympathyParam->load() * ks::sympathyMaxGain;

    for (int i = 0; i < synth.getNumVoices(); ++i)
        if (auto* voice = dynamic_cast<KarplusStrongVoice*> (synth.getVoice (i)))
        {
            const auto& scratch = voice->getOutputScratch();
            auto& external = voice->getSympatheticInputBuffer();
            for (int s = 0; s < numSamples; ++s)
                external[static_cast<size_t> (s)] =
                    sympathy * (voiceMixSum[static_cast<size_t> (s)] - scratch[static_cast<size_t> (s)]);
        }
}

void KarplusStrongProcessor::updateVoicePans()
{
    float spread = stereoSpreadParam->load();
    int numVoices = synth.getNumVoices();

    for (int i = 0; i < numVoices; ++i)
        if (auto* voice = dynamic_cast<KarplusStrongVoice*> (synth.getVoice (i)))
        {
            float pan = numVoices > 1
                ? spread * (2.0f * static_cast<float> (i) / static_cast<float> (numVoices - 1) - 1.0f)
                : 0.0f;
            voice->setPan (pan);
        }
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