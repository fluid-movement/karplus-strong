cmake_minimum_required(VERSION 3.22)

project(KarplusStrong VERSION 0.1.0)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

add_subdirectory(JUCE)

juce_add_plugin(KarplusStrong
    COMPANY_NAME "karplus-strong"
    PLUGIN_MANUFACTURER_CODE "KPs0"
    PLUGIN_CODE "Ks01"
    PRODUCT_NAME "Karplus-Strong"
    PLUGIN_NAME "Karplus-Strong"
    IS_SYNTH TRUE
    NEEDS_MIDI_INPUT TRUE
    NEEDS_MIDI_OUTPUT FALSE
    FORMATS AU VST3 Standalone
    VST3_CATEGORIES "Instrument Synth"
    AU_MAIN_TYPE "kAudioUnitType_MusicDevice"
    BUNDLE_ID "com.karplusstrong.synth"
    VERSION 0.1.0
    COPY_PLUGIN_AFTER_BUILD TRUE
)

target_sources(KarplusStrong
    PRIVATE
        src/PluginProcessor.cpp
        src/PluginEditor.cpp
)

target_compile_definitions(KarplusStrong
    PUBLIC
        JUCE_WEB_BROWSER=0
        JUCE_USE_CURL=0
        JUCE_VST3_CAN_REPLACE_VST2=0
)

target_link_libraries(KarplusStrong
    PRIVATE
        juce::juce_audio_utils
        juce::juce_dsp
    PUBLIC
        juce::juce_recommended_config_flags
        juce::juce_recommended_lto_flags
        juce::juce_recommended_warning_flags
)

# ----------------------------------------------------------------------
# Tests
# ----------------------------------------------------------------------
include(FetchContent)
FetchContent_Declare(
    Catch2
    GIT_REPOSITORY https://github.com/catchorg/Catch2.git
    GIT_TAG v3.5.0
)
FetchContent_MakeAvailable(Catch2)

add_executable(KarplusStrongTests
    tests/test_dsp.cpp
)

target_include_directories(KarplusStrongTests PRIVATE
    src/dsp
)

target_link_libraries(KarplusStrongTests PRIVATE
    Catch2::Catch2WithMain
)

include(CTest)
add_test(NAME KarplusStrongTests COMMAND KarplusStrongTests)

# ----------------------------------------------------------------------
# Integration test (exercises full AudioProcessor with MIDI)
# ----------------------------------------------------------------------
add_executable(IntegrationTest
    tests/test_integration.cpp
)

target_sources(IntegrationTest PRIVATE
    src/PluginProcessor.cpp
    src/PluginEditor.cpp
)

target_include_directories(IntegrationTest PRIVATE
    src
    src/dsp
)

target_link_libraries(IntegrationTest PRIVATE
    juce::juce_audio_utils
    juce::juce_dsp
)

add_test(NAME IntegrationTest COMMAND IntegrationTest)