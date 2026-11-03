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
 *
 * The right half of the panel is the bridge (K19-K22). The strings' sum goes
 * through one shared body and the sympathetic strings (karplus_bridge_*) to
 * the BRIDGE output. The loop jacks bring each string's delay line out to
 * SEND and back from RETURN; the return is read one block late and the core
 * is told so (karplus LOOP_LATENCY, 34 samples = two blocks and two cables).
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
        STIFF_PARAM,
        TENSION_PARAM,
        BUZZ_PARAM,
        TRIM_PARAM,
        BODY_SHAPE_PARAM,
        BODY_SIZE_PARAM,
        BODY_MATERIAL_PARAM,
        BODY_MIX_PARAM,
        BODY_POSITION_PARAM,
        BODY_SPREAD_PARAM,
        BODY_TRACK_PARAM,
        SCENE_PARAM,
        ROOT_PARAM,
        LEVEL_PARAM,
        RING_PARAM,
        FOLLOW_PARAM,
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
        RETURN_INPUT,
        SHAPE_INPUT,
        SCENE_INPUT,
        BODY_INPUT,
        INPUTS_LEN
    };
    enum OutputId { AUDIO_OUTPUT, SEND_OUTPUT, BRIDGE_LEFT_OUTPUT, BRIDGE_RIGHT_OUTPUT, OUTPUTS_LEN };
    enum LightId { LIGHTS_LEN };

    KarplusVoice* voices[karplus::MAX_CHANNELS] = {};
    int channels = 1;
    int cursor = 0;
    float gate[karplus::MAX_CHANNELS][BLOCK] = {};
    float audio[karplus::MAX_CHANNELS][BLOCK] = {};
    float out[karplus::MAX_CHANNELS][BLOCK] = {};
    float send[karplus::MAX_CHANNELS][BLOCK] = {};
    /// The return, as collected during the previous block (what the core is
    /// given) and during this one (what it will be given next).
    float returned[karplus::MAX_CHANNELS][BLOCK] = {};
    float returning[karplus::MAX_CHANNELS][BLOCK] = {};
    KarplusBridge* bridge = NULL;
    float bridgeLeft[BLOCK] = {};
    float bridgeRight[BLOCK] = {};
    float bodyIn[BLOCK] = {};

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

        // The bridge: loop stages, body, sympathetic strings.
        KarplusBridgeParams b;
        karplus_bridge_params_default(&b);
        configParam(STIFF_PARAM, 0.f, 1.f, d.stiffness, "Stiffness");
        configParam(TENSION_PARAM, 0.f, 1.f, d.tension, "Tension modulation");
        configParam(BUZZ_PARAM, 0.f, 1.f, d.buzz, "Buzz");
        configParam(TRIM_PARAM, 0.f, 1.f, 0.5f, "Outboard loop tuning trim");
        configParam(BODY_SHAPE_PARAM, 0.f, 1.f, b.body_shape,
                    "Body shape (string, bar, drum, plate)");
        configParam(BODY_SIZE_PARAM, 0.f, 1.f, b.body_size, "Body size");
        configParam(BODY_MATERIAL_PARAM, 0.f, 1.f, b.body_material,
                    "Body material (wood to glass)");
        configParam(BODY_MIX_PARAM, 0.f, 1.f, b.body_mix, "Body mix");
        configParam(BODY_POSITION_PARAM, 0.f, 1.f, b.body_position,
                    "Body pickup position (all modes to hollow)");
        configParam(BODY_SPREAD_PARAM, 0.f, 1.f, b.body_spread, "Stereo spread");
        configSwitch(BODY_TRACK_PARAM, 0.f, 1.f, 0.f, "Body", {"Fixed in Hz", "Follows the note"});
        configParam(SCENE_PARAM, 0.f, 1.f, b.scene,
                    "Sympathetic tuning (octaves, major, minor, raga, tanpura, cluster)");
        configParam(ROOT_PARAM, 0.f, 1.f, b.root, "Sympathetic root");
        configParam(LEVEL_PARAM, 0.f, 1.f, b.level, "Sympathetic level");
        configParam(RING_PARAM, 0.f, 1.f, b.decay, "Sympathetic ring time");
        configSwitch(FOLLOW_PARAM, 0.f, 1.f, 1.f, "Sympathetic root", {"Fixed", "Follows the note"});
        configInput(RETURN_INPUT, "Loop return");
        configInput(SHAPE_INPUT, "Body shape CV");
        configInput(SCENE_INPUT, "Sympathetic tuning CV");
        configOutput(SEND_OUTPUT, "Loop send");
        configInput(BODY_INPUT, "Body input (audio into the body and sympathetic strings)");
        configOutput(BRIDGE_LEFT_OUTPUT, "Bridge left (body and sympathetic strings)");
        configOutput(BRIDGE_RIGHT_OUTPUT, "Bridge right");

        buildVoices(APP->engine->getSampleRate());
    }

    ~KarplusStrong() {
        karplus_bridge_free(bridge);
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
        karplus_bridge_free(bridge);
        bridge = karplus_bridge_new(rate);
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
        karplus_bridge_reset(bridge);
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

        p.stiffness = params[STIFF_PARAM].getValue();
        p.tension = params[TENSION_PARAM].getValue();
        p.buzz = params[BUZZ_PARAM].getValue();
        p.loop_return = inputs[RETURN_INPUT].isConnected() ? 1 : 0;
        p.loop_trim_volts = karplus::trim_volts(params[TRIM_PARAM].getValue());
        return p;
    }

    /// The bridge's controls. The lead voice's pitch is channel 1's.
    KarplusBridgeParams readBridgeControls() {
        KarplusBridgeParams b;
        karplus_bridge_params_default(&b);
        b.root_volts = karplus::pitch_volts(params[PITCH_PARAM].getValue(),
                                            inputs[VOCT_INPUT].getVoltage(0));
        b.body_shape = karplus::unipolar(params[BODY_SHAPE_PARAM].getValue(), 1.f,
                                         inputs[SHAPE_INPUT].getVoltage(0));
        b.body_size = params[BODY_SIZE_PARAM].getValue();
        b.body_material = params[BODY_MATERIAL_PARAM].getValue();
        b.body_mix = params[BODY_MIX_PARAM].getValue();
        b.body_position = params[BODY_POSITION_PARAM].getValue();
        b.body_spread = params[BODY_SPREAD_PARAM].getValue();
        b.body_track = params[BODY_TRACK_PARAM].getValue() > 0.5f ? 1 : 0;
        b.scene = karplus::unipolar(params[SCENE_PARAM].getValue(), 1.f,
                                    inputs[SCENE_INPUT].getVoltage(0));
        b.root = params[ROOT_PARAM].getValue();
        b.level = params[LEVEL_PARAM].getValue();
        b.decay = params[RING_PARAM].getValue();
        b.follow = params[FOLLOW_PARAM].getValue() > 0.5f ? 1 : 0;
        return b;
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
                send[c][i] = 0.f;
                returned[c][i] = 0.f;
                returning[c][i] = 0.f;
            }
        }
        channels = wanted;

        const bool audioPatched = inputs[AUDIO_INPUT].isConnected();
        for (int c = 0; c < channels; c++) {
            KarplusParams p = readControls(c);
            karplus_voice_process_loop(voices[c], &p, gate[c], audioPatched ? audio[c] : NULL,
                                       p.loop_return ? returned[c] : NULL, send[c], out[c],
                                       BLOCK);
            // What came back during this block is for the next.
            for (int i = 0; i < BLOCK; i++) {
                returned[c][i] = returning[c][i];
            }
        }

        KarplusBridgeParams b = readBridgeControls();
        karplus_bridge_process(bridge, &b, &out[0][0], (size_t)channels, (size_t)BLOCK,
                               inputs[BODY_INPUT].isConnected() ? bodyIn : NULL, bridgeLeft,
                               bridgeRight, (size_t)BLOCK);
    }

    void process(const ProcessArgs& args) override {
        for (int c = 0; c < channels; c++) {
            gate[c][cursor] = inputs[GATE_INPUT].getPolyVoltage(c);
            audio[c][cursor] =
                karplus::audio_from_volts(inputs[AUDIO_INPUT].getPolyVoltage(c));
            returning[c][cursor] =
                karplus::audio_from_volts(inputs[RETURN_INPUT].getPolyVoltage(c));
            outputs[AUDIO_OUTPUT].setVoltage(karplus::audio_to_volts(out[c][cursor]), c);
            outputs[SEND_OUTPUT].setVoltage(karplus::audio_to_volts(send[c][cursor]), c);
        }
        outputs[AUDIO_OUTPUT].setChannels(channels);
        outputs[SEND_OUTPUT].setChannels(channels);
        bodyIn[cursor] = karplus::audio_from_volts(inputs[BODY_INPUT].getVoltage());
        outputs[BRIDGE_LEFT_OUTPUT].setVoltage(karplus::audio_to_volts(bridgeLeft[cursor]));
        outputs[BRIDGE_RIGHT_OUTPUT].setVoltage(karplus::audio_to_volts(bridgeRight[cursor]));

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

        // The right half: BRIDGE_X, RCOLUMNS, and the rows in panel.py.
        const float bx = 81.28f;
        const float rcol[4] = {bx + 12.89f, bx + 31.39f, bx + 49.89f, bx + 68.39f};

        // LOOP_Y: stiffness, tension, buzz, and the outboard loop's trim.
        const int loop[4] = {KarplusStrong::STIFF_PARAM, KarplusStrong::TENSION_PARAM,
                             KarplusStrong::BUZZ_PARAM, KarplusStrong::TRIM_PARAM};
        for (int i = 0; i < 4; i++) {
            addParam(createParamCentered<RoundBlackKnob>(mm2px(Vec(rcol[i], 25.0f)), module,
                                                         loop[i]));
        }

        // BODY_Y: the body, then its switch.
        const int body[4] = {KarplusStrong::BODY_SHAPE_PARAM, KarplusStrong::BODY_SIZE_PARAM,
                             KarplusStrong::BODY_MATERIAL_PARAM, KarplusStrong::BODY_MIX_PARAM};
        for (int i = 0; i < 4; i++) {
            addParam(createParamCentered<RoundBlackKnob>(mm2px(Vec(rcol[i], 51.0f)), module,
                                                         body[i]));
        }
        addParam(createParamCentered<CKSS>(mm2px(Vec(rcol[0], 63.5f)), module,
                                           KarplusStrong::BODY_TRACK_PARAM));
        addParam(createParamCentered<Trimpot>(mm2px(Vec(rcol[1], 63.5f)), module,
                                              KarplusStrong::BODY_POSITION_PARAM));
        addParam(createParamCentered<Trimpot>(mm2px(Vec(rcol[2], 63.5f)), module,
                                              KarplusStrong::BODY_SPREAD_PARAM));
        addInput(createInputCentered<PJ301MPort>(mm2px(Vec(rcol[3], 63.5f)), module,
                                                 KarplusStrong::SHAPE_INPUT));

        // STRINGS_Y: the sympathetic strings, then their switch.
        const int strings[4] = {KarplusStrong::SCENE_PARAM, KarplusStrong::ROOT_PARAM,
                                KarplusStrong::LEVEL_PARAM, KarplusStrong::RING_PARAM};
        for (int i = 0; i < 4; i++) {
            addParam(createParamCentered<RoundBlackKnob>(mm2px(Vec(rcol[i], 88.0f)), module,
                                                         strings[i]));
        }
        addParam(createParamCentered<CKSS>(mm2px(Vec(rcol[0], 99.5f)), module,
                                           KarplusStrong::FOLLOW_PARAM));
        addInput(createInputCentered<PJ301MPort>(mm2px(Vec(rcol[3], 99.5f)), module,
                                                 KarplusStrong::SCENE_INPUT));

        // JACK_Y, right half.
        addOutput(createOutputCentered<PJ301MPort>(mm2px(Vec(bx + 9.5f, 116.5f)), module,
                                                   KarplusStrong::SEND_OUTPUT));
        addInput(createInputCentered<PJ301MPort>(mm2px(Vec(bx + 24.0f, 116.5f)), module,
                                                 KarplusStrong::RETURN_INPUT));
        addInput(createInputCentered<PJ301MPort>(mm2px(Vec(bx + 38.5f, 116.5f)), module,
                                                 KarplusStrong::BODY_INPUT));
        addOutput(createOutputCentered<PJ301MPort>(mm2px(Vec(bx + 56.0f, 116.5f)), module,
                                                   KarplusStrong::BRIDGE_LEFT_OUTPUT));
        addOutput(createOutputCentered<PJ301MPort>(mm2px(Vec(bx + 70.28f, 116.5f)), module,
                                                   KarplusStrong::BRIDGE_RIGHT_OUTPUT));
    }
};

// No context menu: every control is on the panel (docs/decisions.md, K5).

Model* modelKarplusStrong = createModel<KarplusStrong, KarplusStrongWidget>("KarplusStrong");
