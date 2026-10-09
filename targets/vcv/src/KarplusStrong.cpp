/*
 * KarplusStrong.cpp — the shim.
 *
 * Rack needs a subclass of rack::engine::Module with virtual methods, which
 * Rust cannot provide. So this file exists. It reads controls, fills a
 * KarplusParams per channel, calls the core, and writes the output. It
 * contains no DSP and makes no decisions about sound; if it needs real
 * changes, the core's boundary is wrong (docs/architecture.md).
 *
 * Polyphony is the cable's: one voice per channel of the wider of V/OCT and
 * GATE, up to sixteen. A mono cable on any other input applies to every
 * voice, which is Rack's convention.
 *
 * Rack calls process() once per sample; the core wants blocks, because a
 * gate edge must be heard at the sample it happens on and the core works out
 * its coefficients once per block. So inputs are collected for BLOCK samples,
 * the block is rendered, and played out while the next is collected: BLOCK
 * samples of latency, a third of a millisecond at 48 kHz.
 */

#include "plugin.hpp"

#include "controls.hpp"

extern "C" {
#include "karplus.h"
}

/// Samples per core call. Small, because it is also the latency.
static const int BLOCK = 16;

struct KarplusStrong : Module {
    enum ParamId {
        DECAY_PARAM,
        BRIGHT_PARAM,
        PICK_PARAM,
        DRIVE_PARAM,
        DECAY_ATTEN_PARAM,
        BRIGHT_ATTEN_PARAM,
        PICK_ATTEN_PARAM,
        DRIVE_ATTEN_PARAM,
        PITCH_PARAM,
        TONE_PARAM,
        LENGTH_PARAM,
        DAMP_PARAM,
        HUMANIZE_PARAM,
        KEY_TRACK_PARAM,
        HARMONIC_PARAM,
        EXCITER_PARAM,
        VEL_BRIGHT_PARAM,
        VEL_DECAY_PARAM,
        VEL_LENGTH_PARAM,
        PLUCK_PARAM,
        PARAMS_LEN
    };
    enum InputId {
        DECAY_INPUT,
        BRIGHT_INPUT,
        PICK_INPUT,
        DRIVE_INPUT,
        VOCT_INPUT,
        GATE_INPUT,
        VELOCITY_INPUT,
        AUDIO_INPUT,
        INPUTS_LEN
    };
    enum OutputId { AUDIO_OUTPUT, OUTPUTS_LEN };
    enum LightId { LIGHTS_LEN };

    KarplusVoice* voices[karplus::MAX_CHANNELS] = {};
    int channels = 1;
    int cursor = 0;
    float gate[karplus::MAX_CHANNELS][BLOCK] = {};
    float audio[karplus::MAX_CHANNELS][BLOCK] = {};
    float out[karplus::MAX_CHANNELS][BLOCK] = {};

    KarplusStrong() {
        config(PARAMS_LEN, INPUTS_LEN, OUTPUTS_LEN, LIGHTS_LEN);

        // Defaults come from the core, so the panel and the scenarios agree
        // on what an untouched module sounds like.
        KarplusParams d;
        karplus_params_default(&d);

        configParam(DECAY_PARAM, 0.f, 1.f, d.decay, "Decay");
        configParam(BRIGHT_PARAM, 0.f, 1.f, d.brightness, "Brightness");
        configParam(PICK_PARAM, 0.f, 1.f, d.pick, "Pick position");
        configParam(DRIVE_PARAM, 0.f, 1.f, d.drive, "Drive");
        configParam(DECAY_ATTEN_PARAM, -1.f, 1.f, 0.f, "Decay CV");
        configParam(BRIGHT_ATTEN_PARAM, -1.f, 1.f, 0.f, "Brightness CV");
        configParam(PICK_ATTEN_PARAM, -1.f, 1.f, 0.f, "Pick position CV");
        configParam(DRIVE_ATTEN_PARAM, -1.f, 1.f, 0.f, "Drive CV");

        configParam(PITCH_PARAM, -4.f, 4.f, 0.f, "Pitch", " V");
        configParam(TONE_PARAM, 0.f, 1.f, d.tone, "Exciter tone");
        configParam(LENGTH_PARAM, 0.f, 1.f, d.length, "Burst length");
        configParam(DAMP_PARAM, 0.f, 1.f, d.damp, "Damping on release");

        configParam(HUMANIZE_PARAM, 0.f, 1.f, d.humanize, "Humanize");
        configParam(KEY_TRACK_PARAM, 0.f, 1.f, d.key_track, "Decay key tracking");
        configParam(HARMONIC_PARAM, 1.f, 8.f, (float)d.harmonic, "Sine harmonic");
        getParamQuantity(HARMONIC_PARAM)->snapEnabled = true;
        configSwitch(EXCITER_PARAM, 0.f, 2.f, (float)d.exciter, "Exciter",
                     {"Noise", "Dust", "Sine"});

        configParam(VEL_BRIGHT_PARAM, 0.f, 1.f, d.velocity_brightness,
                    "Velocity to brightness");
        configParam(VEL_DECAY_PARAM, 0.f, 1.f, d.velocity_decay, "Velocity to decay");
        configParam(VEL_LENGTH_PARAM, 0.f, 1.f, d.velocity_length, "Velocity to length");
        configButton(PLUCK_PARAM, "Pluck");

        configInput(DECAY_INPUT, "Decay CV");
        configInput(BRIGHT_INPUT, "Brightness CV");
        configInput(PICK_INPUT, "Pick position CV");
        configInput(DRIVE_INPUT, "Drive CV");
        configInput(VOCT_INPUT, "1V/octave pitch");
        configInput(GATE_INPUT, "Gate");
        configInput(VELOCITY_INPUT, "Velocity (0-10 V)");
        configInput(AUDIO_INPUT, "Exciter audio");
        configOutput(AUDIO_OUTPUT, "Audio");

        buildVoices(APP->engine->getSampleRate());
    }

    ~KarplusStrong() {
        for (int c = 0; c < karplus::MAX_CHANNELS; c++) {
            karplus_voice_free(voices[c]);
        }
    }

    /// One voice per possible channel, each with its own noise.
    void buildVoices(float rate) {
        for (int c = 0; c < karplus::MAX_CHANNELS; c++) {
            karplus_voice_free(voices[c]);
            voices[c] = karplus_voice_new(rate, (uint32_t)(c + 1));
        }
        cursor = 0;
    }

    void onSampleRateChange(const SampleRateChangeEvent& e) override {
        buildVoices(e.sampleRate);
    }

    void onReset(const ResetEvent& e) override {
        Module::onReset(e);
        for (int c = 0; c < karplus::MAX_CHANNELS; c++) {
            karplus_voice_reset(voices[c]);
        }
    }

    /// Every control for one channel.
    KarplusParams readControls(int c) {
        KarplusParams p;
        karplus_params_default(&p);

        p.pitch_volts = karplus::pitch_volts(params[PITCH_PARAM].getValue(),
                                             inputs[VOCT_INPUT].getPolyVoltage(c));
        p.decay = karplus::unipolar(params[DECAY_PARAM].getValue(),
                                    params[DECAY_ATTEN_PARAM].getValue(),
                                    inputs[DECAY_INPUT].getPolyVoltage(c));
        p.brightness = karplus::unipolar(params[BRIGHT_PARAM].getValue(),
                                         params[BRIGHT_ATTEN_PARAM].getValue(),
                                         inputs[BRIGHT_INPUT].getPolyVoltage(c));
        p.pick = karplus::unipolar(params[PICK_PARAM].getValue(),
                                   params[PICK_ATTEN_PARAM].getValue(),
                                   inputs[PICK_INPUT].getPolyVoltage(c));
        p.drive = karplus::unipolar(params[DRIVE_PARAM].getValue(),
                                    params[DRIVE_ATTEN_PARAM].getValue(),
                                    inputs[DRIVE_INPUT].getPolyVoltage(c));

        p.tone = params[TONE_PARAM].getValue();
        p.length = params[LENGTH_PARAM].getValue();
        p.damp = params[DAMP_PARAM].getValue();
        p.humanize = params[HUMANIZE_PARAM].getValue();
        p.key_track = params[KEY_TRACK_PARAM].getValue();
        p.harmonic = (uint32_t)karplus::position(params[HARMONIC_PARAM].getValue(), 8);
        p.exciter = (KarplusExciter)karplus::position(params[EXCITER_PARAM].getValue(), 2);
        p.velocity_brightness = params[VEL_BRIGHT_PARAM].getValue();
        p.velocity_decay = params[VEL_DECAY_PARAM].getValue();
        p.velocity_length = params[VEL_LENGTH_PARAM].getValue();

        // Rack knows whether a jack is patched, which is the same thing the
        // hardware would sense with a switched jack. The core decides what
        // an unpatched velocity means.
        p.velocity_patched = inputs[VELOCITY_INPUT].isConnected() ? 1 : 0;
        p.velocity_volts = inputs[VELOCITY_INPUT].getPolyVoltage(c);
        p.manual_gate = params[PLUCK_PARAM].getValue() > 0.5f ? 1 : 0;
        return p;
    }

    void renderBlock() {
        const int wanted = karplus::voice_count(inputs[VOCT_INPUT].getChannels(),
                                                inputs[GATE_INPUT].getChannels());
        // A voice coming back into use starts silent, rather than resuming
        // whatever it was ringing when it was last heard.
        for (int c = channels; c < wanted; c++) {
            karplus_voice_reset(voices[c]);
            for (int i = 0; i < BLOCK; i++) {
                gate[c][i] = 0.f;
                audio[c][i] = 0.f;
                out[c][i] = 0.f;
            }
        }
        channels = wanted;

        const bool audioPatched = inputs[AUDIO_INPUT].isConnected();
        for (int c = 0; c < channels; c++) {
            KarplusParams p = readControls(c);
            karplus_voice_process(voices[c], &p, gate[c], audioPatched ? audio[c] : NULL,
                                  out[c], BLOCK);
        }
    }

    void process(const ProcessArgs& args) override {
        for (int c = 0; c < channels; c++) {
            gate[c][cursor] = inputs[GATE_INPUT].getPolyVoltage(c);
            audio[c][cursor] =
                karplus::audio_from_volts(inputs[AUDIO_INPUT].getPolyVoltage(c));
            outputs[AUDIO_OUTPUT].setVoltage(karplus::audio_to_volts(out[c][cursor]), c);
        }
        outputs[AUDIO_OUTPUT].setChannels(channels);

        if (++cursor >= BLOCK) {
            cursor = 0;
            renderBlock();
        }
    }
};

/// Every coordinate below is in millimetres and comes from `res/panel.py`,
/// which draws the panel. Change a position there, rerun it, copy it here.
struct KarplusStrongWidget : ModuleWidget {
    KarplusStrongWidget(KarplusStrong* module) {
        setModule(module);
        setPanel(createPanel(asset::plugin(pluginInstance, "res/KarplusStrong.svg")));

        addChild(createWidget<ScrewSilver>(Vec(RACK_GRID_WIDTH, 0)));
        addChild(createWidget<ScrewSilver>(Vec(box.size.x - 2 * RACK_GRID_WIDTH, 0)));
        addChild(createWidget<ScrewSilver>(
            Vec(RACK_GRID_WIDTH, RACK_GRID_HEIGHT - RACK_GRID_WIDTH)));
        addChild(createWidget<ScrewSilver>(
            Vec(box.size.x - 2 * RACK_GRID_WIDTH, RACK_GRID_HEIGHT - RACK_GRID_WIDTH)));

        // COLUMNS in panel.py.
        const float col[4] = {12.89f, 31.39f, 49.89f, 68.39f};

        // MAIN_Y, ATTEN_Y, CV_Y: the four controls that take CV.
        const int mains[4] = {KarplusStrong::DECAY_PARAM, KarplusStrong::BRIGHT_PARAM,
                              KarplusStrong::PICK_PARAM, KarplusStrong::DRIVE_PARAM};
        const int attens[4] = {
            KarplusStrong::DECAY_ATTEN_PARAM, KarplusStrong::BRIGHT_ATTEN_PARAM,
            KarplusStrong::PICK_ATTEN_PARAM, KarplusStrong::DRIVE_ATTEN_PARAM};
        const int cvs[4] = {KarplusStrong::DECAY_INPUT, KarplusStrong::BRIGHT_INPUT,
                            KarplusStrong::PICK_INPUT, KarplusStrong::DRIVE_INPUT};
        for (int i = 0; i < 4; i++) {
            addParam(createParamCentered<RoundLargeBlackKnob>(mm2px(Vec(col[i], 25.0f)),
                                                              module, mains[i]));
            addParam(createParamCentered<Trimpot>(mm2px(Vec(col[i], 37.0f)), module,
                                                  attens[i]));
            addInput(createInputCentered<PJ301MPort>(mm2px(Vec(col[i], 45.5f)), module,
                                                     cvs[i]));
        }

        // PLUCK_Y: how the string is plucked.
        const int pluck[4] = {KarplusStrong::PITCH_PARAM, KarplusStrong::TONE_PARAM,
                              KarplusStrong::LENGTH_PARAM, KarplusStrong::DAMP_PARAM};
        for (int i = 0; i < 4; i++) {
            addParam(createParamCentered<RoundBlackKnob>(mm2px(Vec(col[i], 65.0f)), module,
                                                         pluck[i]));
        }

        // FEEL_Y: three trimpots and the exciter switch.
        const int feel[3] = {KarplusStrong::HUMANIZE_PARAM, KarplusStrong::KEY_TRACK_PARAM,
                             KarplusStrong::HARMONIC_PARAM};
        for (int i = 0; i < 3; i++) {
            addParam(createParamCentered<Trimpot>(mm2px(Vec(col[i], 82.0f)), module,
                                                  feel[i]));
        }
        addParam(createParamCentered<CKSSThree>(mm2px(Vec(col[3] - 4.0f, 82.0f)), module,
                                                KarplusStrong::EXCITER_PARAM));

        // VELOCITY_Y: how much velocity does, and the PLUCK button.
        const int vel[3] = {KarplusStrong::VEL_BRIGHT_PARAM, KarplusStrong::VEL_DECAY_PARAM,
                            KarplusStrong::VEL_LENGTH_PARAM};
        for (int i = 0; i < 3; i++) {
            addParam(createParamCentered<Trimpot>(mm2px(Vec(col[i], 98.5f)), module,
                                                  vel[i]));
        }
        addParam(createParamCentered<VCVButton>(mm2px(Vec(col[3], 98.5f)), module,
                                                KarplusStrong::PLUCK_PARAM));

        // JACK_Y.
        addInput(createInputCentered<PJ301MPort>(mm2px(Vec(9.5f, 116.5f)), module,
                                                 KarplusStrong::VOCT_INPUT));
        addInput(createInputCentered<PJ301MPort>(mm2px(Vec(24.0f, 116.5f)), module,
                                                 KarplusStrong::GATE_INPUT));
        addInput(createInputCentered<PJ301MPort>(mm2px(Vec(38.5f, 116.5f)), module,
                                                 KarplusStrong::VELOCITY_INPUT));
        addInput(createInputCentered<PJ301MPort>(mm2px(Vec(53.0f, 116.5f)), module,
                                                 KarplusStrong::AUDIO_INPUT));
        addOutput(createOutputCentered<PJ301MPort>(mm2px(Vec(70.28f, 116.5f)), module,
                                                   KarplusStrong::AUDIO_OUTPUT));
    }
};

// No context menu: every control is on the panel (docs/decisions.md, K5).

Model* modelKarplusStrong = createModel<KarplusStrong, KarplusStrongWidget>("KarplusStrong");
