#pragma once

#include <juce_audio_processors/juce_audio_processors.h>
#include <utility>
#include <vector>

struct Preset
{
    juce::String name;
    std::vector<std::pair<juce::String, float>> values;
};

inline const std::vector<Preset>& getFactoryPresets()
{
    static const std::vector<Preset> presets =
    {
        { "Init", {
            { "excitation", 0.0f }, { "excitation_length", 100.0f }, { "pick_position", 0.2f },
            { "pick_model", 0.0f }, { "sine_harmonic", 1.0f }, { "exciter_tone", 0.0f },
            { "vel_excitation_length", 0.0f }, { "velvet_density", 2000.0f }, { "decay_time", 4.0f },
            { "key_track", 0.0f }, { "brightness", 0.5f }, { "vel_brightness", 0.0f },
            { "vel_decay", 0.0f }, { "drive", 0.0f }, { "stiffness", 0.0f }, { "damp_mode", 0.0f },
            { "release_time", 0.3f }, { "humanize", 0.0f }, { "output_level", 0.8f },
            { "voices", 8.0f }, { "stereo_spread", 0.0f }, { "sympathy", 0.0f }
        } },
        { "Nylon Guitar", {
            { "excitation", 0.0f }, { "excitation_length", 150.0f }, { "pick_position", 0.15f },
            { "pick_model", 1.0f }, { "sine_harmonic", 1.0f }, { "exciter_tone", -0.3f },
            { "vel_excitation_length", 0.3f }, { "velvet_density", 2000.0f }, { "decay_time", 6.0f },
            { "key_track", 0.5f }, { "brightness", 0.4f }, { "vel_brightness", 0.2f },
            { "vel_decay", 0.1f }, { "drive", 0.0f }, { "stiffness", 0.0f }, { "damp_mode", 0.0f },
            { "release_time", 0.3f }, { "humanize", 0.15f }, { "output_level", 0.8f },
            { "voices", 8.0f }, { "stereo_spread", 0.3f }, { "sympathy", 0.1f }
        } },
        { "Steel Guitar", {
            { "excitation", 0.0f }, { "excitation_length", 60.0f }, { "pick_position", 0.08f },
            { "pick_model", 1.0f }, { "sine_harmonic", 1.0f }, { "exciter_tone", 0.3f },
            { "vel_excitation_length", 0.4f }, { "velvet_density", 2000.0f }, { "decay_time", 8.0f },
            { "key_track", 0.6f }, { "brightness", 0.7f }, { "vel_brightness", 0.3f },
            { "vel_decay", 0.1f }, { "drive", 0.1f }, { "stiffness", 0.1f }, { "damp_mode", 0.0f },
            { "release_time", 0.3f }, { "humanize", 0.1f }, { "output_level", 0.8f },
            { "voices", 8.0f }, { "stereo_spread", 0.3f }, { "sympathy", 0.1f }
        } },
        { "Glass Bell", {
            { "excitation", 1.0f }, { "excitation_length", 200.0f }, { "pick_position", 0.0f },
            { "pick_model", 0.0f }, { "sine_harmonic", 3.0f }, { "exciter_tone", 0.0f },
            { "vel_excitation_length", 0.0f }, { "velvet_density", 2000.0f }, { "decay_time", 15.0f },
            { "key_track", 1.0f }, { "brightness", 0.9f }, { "vel_brightness", 0.0f },
            { "vel_decay", 0.0f }, { "drive", 0.0f }, { "stiffness", 0.8f }, { "damp_mode", 0.0f },
            { "release_time", 0.3f }, { "humanize", 0.0f }, { "output_level", 0.7f },
            { "voices", 8.0f }, { "stereo_spread", 0.5f }, { "sympathy", 0.0f }
        } },
        { "Sitar", {
            { "excitation", 0.0f }, { "excitation_length", 120.0f }, { "pick_position", 0.25f },
            { "pick_model", 2.0f }, { "sine_harmonic", 1.0f }, { "exciter_tone", 0.0f },
            { "vel_excitation_length", 0.2f }, { "velvet_density", 2000.0f }, { "decay_time", 10.0f },
            { "key_track", 0.3f }, { "brightness", 0.6f }, { "vel_brightness", 0.2f },
            { "vel_decay", 0.0f }, { "drive", 0.4f }, { "stiffness", 0.2f }, { "damp_mode", 0.0f },
            { "release_time", 0.3f }, { "humanize", 0.3f }, { "output_level", 0.75f },
            { "voices", 8.0f }, { "stereo_spread", 0.4f }, { "sympathy", 0.3f }
        } },
        { "Sub Pluck Bass", {
            { "excitation", 2.0f }, { "excitation_length", 40.0f }, { "pick_position", 0.3f },
            { "pick_model", 0.0f }, { "sine_harmonic", 1.0f }, { "exciter_tone", -0.5f },
            { "vel_excitation_length", 0.0f }, { "velvet_density", 2000.0f }, { "decay_time", 1.2f },
            { "key_track", 0.0f }, { "brightness", 0.2f }, { "vel_brightness", 0.0f },
            { "vel_decay", 0.2f }, { "drive", 0.2f }, { "stiffness", 0.0f }, { "damp_mode", 1.0f },
            { "release_time", 0.4f }, { "humanize", 0.05f }, { "output_level", 0.9f },
            { "voices", 6.0f }, { "stereo_spread", 0.0f }, { "sympathy", 0.0f }
        } },
        { "Ambient Drone", {
            { "excitation", 4.0f }, { "excitation_length", 400.0f }, { "pick_position", 0.1f },
            { "pick_model", 0.0f }, { "sine_harmonic", 1.0f }, { "exciter_tone", -0.2f },
            { "vel_excitation_length", 0.0f }, { "velvet_density", 800.0f }, { "decay_time", 35.0f },
            { "key_track", 0.2f }, { "brightness", 0.35f }, { "vel_brightness", 0.0f },
            { "vel_decay", 0.0f }, { "drive", 0.15f }, { "stiffness", 0.3f }, { "damp_mode", 0.0f },
            { "release_time", 0.3f }, { "humanize", 0.4f }, { "output_level", 0.65f },
            { "voices", 16.0f }, { "stereo_spread", 0.8f }, { "sympathy", 0.8f }
        } },
    };
    return presets;
}

inline void applyPreset (juce::AudioProcessorValueTreeState& apvts, const Preset& preset)
{
    for (auto& [id, value] : preset.values)
        if (auto* param = apvts.getParameter (id))
            param->setValueNotifyingHost (param->convertTo0to1 (value));
}
