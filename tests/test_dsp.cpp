#include <catch2/catch_test_macros.hpp>
#include <cmath>
#include <vector>
#include "KarplusStrongDsp.h"
#include "Exciter.h"
#include "KsDelayLine.h"
#include "CircularBuffer.h"
#include "OnePoleLowpass.h"
#include "DcBlocker.h"
#include "SilenceDetector.h"
#include "KsCalibration.h"
#include "KsParams.h"

static KsParams makeParams (float decayTime, float brightness, int excitationType,
                            float excitationLength, float pickPosition, int pickModel,
                            float velBrightness, float velDecay, float outputLevel,
                            float keyTrack)
{
    KsParams p;
    p.decayTime        = decayTime;
    p.brightness       = brightness;
    p.excitationType   = excitationType;
    p.excitationLength = excitationLength;
    p.pickPosition     = pickPosition;
    p.pickModel        = pickModel;
    p.velBrightness    = velBrightness;
    p.velDecay         = velDecay;
    p.outputLevel      = outputLevel;
    p.keyTrack         = keyTrack;
    return p;
}

TEST_CASE("KarplusStrongDsp produces non-zero output after noteOn", "[dsp]")
{
    KarplusStrongDsp dsp;
    dsp.prepare (44100.0);
    dsp.setParameters (makeParams (0.3f, 0.5f, 0, 100.0f, 0.2f, 0, 0.0f, 0.0f, 0.8f, 0.0f));

    dsp.noteOn (440.0f, 0.8f);

    bool anyNonZero = false;
    for (int i = 0; i < 1000; ++i)
    {
        float out = dsp.processSample();
        if (std::abs (out) > 1e-6f)
        {
            anyNonZero = true;
            break;
        }
    }
    REQUIRE (anyNonZero);
}

TEST_CASE("Output decays over time", "[dsp]")
{
    KarplusStrongDsp dsp;
    dsp.prepare (44100.0);
    dsp.setParameters (makeParams (0.3f, 0.5f, 0, 100.0f, 0.2f, 0, 0.0f, 0.0f, 0.8f, 0.0f));

    dsp.noteOn (440.0f, 1.0f);

    float maxEarly = 0.0f;
    for (int i = 0; i < 1000; ++i)
        maxEarly = std::max (maxEarly, std::abs (dsp.processSample()));

    for (int i = 0; i < 44100 * 3; ++i)
        dsp.processSample();

    float maxLate = 0.0f;
    for (int i = 0; i < 1000; ++i)
        maxLate = std::max (maxLate, std::abs (dsp.processSample()));

    REQUIRE (maxLate < maxEarly);
}

TEST_CASE("Higher notes have shorter delay", "[dsp]")
{
    KarplusStrongDsp dsp;
    dsp.prepare (44100.0);
    dsp.setParameters (makeParams (0.3f, 1.0f, 0, 50.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f, 0.0f));

    dsp.noteOn (220.0f, 1.0f);
    float maxLow = 0.0f;
    for (int i = 0; i < 500; ++i)
        maxLow = std::max (maxLow, std::abs (dsp.processSample()));

    dsp.reset();
    dsp.noteOn (880.0f, 1.0f);
    float maxHigh = 0.0f;
    for (int i = 0; i < 500; ++i)
        maxHigh = std::max (maxHigh, std::abs (dsp.processSample()));

    REQUIRE (maxHigh > 0.0f);
    REQUIRE (maxLow > 0.0f);
}

TEST_CASE("Different excitation types produce different output", "[dsp]")
{
    KarplusStrongDsp dsp;
    dsp.prepare (44100.0);

    dsp.setParameters (makeParams (0.3f, 0.5f, 0, 200.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f, 0.0f));
    dsp.noteOn (440.0f, 1.0f);
    std::vector<float> noiseOut;
    for (int i = 0; i < 500; ++i)
        noiseOut.push_back (dsp.processSample());

    dsp.setParameters (makeParams (0.3f, 0.5f, 1, 200.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f, 0.0f));
    dsp.noteOn (440.0f, 1.0f);
    std::vector<float> sineOut;
    for (int i = 0; i < 500; ++i)
        sineOut.push_back (dsp.processSample());

    bool anyDifferent = false;
    for (int i = 0; i < 500; ++i)
    {
        if (std::abs (noiseOut[i] - sineOut[i]) > 1e-4f)
        {
            anyDifferent = true;
            break;
        }
    }
    REQUIRE (anyDifferent);
}

TEST_CASE("Pick model routing changes output", "[dsp]")
{
    KarplusStrongDsp dsp;
    dsp.prepare (44100.0);

    dsp.setParameters (makeParams (0.3f, 0.5f, 0, 200.0f, 0.3f, 0, 0.0f, 0.0f, 1.0f, 0.0f));
    dsp.noteOn (440.0f, 1.0f);
    std::vector<float> offOut;
    for (int i = 0; i < 500; ++i)
        offOut.push_back (dsp.processSample());

    dsp.setParameters (makeParams (0.3f, 0.5f, 0, 200.0f, 0.3f, 1, 0.0f, 0.0f, 1.0f, 0.0f));
    dsp.noteOn (440.0f, 1.0f);
    std::vector<float> combOut;
    for (int i = 0; i < 500; ++i)
        combOut.push_back (dsp.processSample());

    bool anyDifferent = false;
    for (int i = 0; i < 500; ++i)
    {
        if (std::abs (offOut[i] - combOut[i]) > 1e-4f)
        {
            anyDifferent = true;
            break;
        }
    }
    REQUIRE (anyDifferent);
}

TEST_CASE("Velocity affects output level", "[dsp]")
{
    KarplusStrongDsp dsp;
    dsp.prepare (44100.0);
    dsp.setParameters (makeParams (0.3f, 0.5f, 0, 200.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f, 0.0f));

    dsp.noteOn (440.0f, 1.0f);
    float maxHighVel = 0.0f;
    for (int i = 0; i < 500; ++i)
        maxHighVel = std::max (maxHighVel, std::abs (dsp.processSample()));

    dsp.noteOn (440.0f, 0.1f);
    float maxLowVel = 0.0f;
    for (int i = 0; i < 500; ++i)
        maxLowVel = std::max (maxLowVel, std::abs (dsp.processSample()));

    REQUIRE (maxHighVel > maxLowVel);
}

TEST_CASE("isSilent returns true after output decays naturally", "[dsp]")
{
    KarplusStrongDsp dsp;
    dsp.prepare (44100.0);
    dsp.setParameters (makeParams (0.3f, 0.0f, 0, 50.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f, 0.0f));

    dsp.noteOn (440.0f, 1.0f);
    REQUIRE (! dsp.isSilent());

    for (int i = 0; i < 44100 * 7; ++i)
        dsp.processSample();

    REQUIRE (dsp.isSilent());
}

TEST_CASE("noteOff is a no-op — string rings out naturally", "[dsp]")
{
    KarplusStrongDsp dsp;
    dsp.prepare (44100.0);
    dsp.setParameters (makeParams (0.3f, 1.0f, 0, 100.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f, 0.0f));

    dsp.noteOn (440.0f, 1.0f);
    for (int i = 0; i < 500; ++i)
        dsp.processSample();

    REQUIRE ( ! dsp.isSilent());

    dsp.noteOff (true);

    float maxAfterNoteOff = 0.0f;
    for (int i = 0; i < 44100; ++i)
        maxAfterNoteOff = std::max (maxAfterNoteOff, std::abs (dsp.processSample()));

    REQUIRE (maxAfterNoteOff > 0.01f);
}

TEST_CASE("Reset clears delay buffer", "[dsp]")
{
    KarplusStrongDsp dsp;
    dsp.prepare (44100.0);
    dsp.setParameters (makeParams (0.3f, 1.0f, 0, 500.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f, 0.0f));

    dsp.noteOn (220.0f, 1.0f);
    for (int i = 0; i < 1000; ++i)
        dsp.processSample();

    float before = std::abs (dsp.processSample());
    REQUIRE (before > 0.0f);

    dsp.reset();
    float after = std::abs (dsp.processSample());
    REQUIRE (after < 1e-6f);
}

TEST_CASE("Dialed decay time roughly matches time to silence", "[dsp]")
{
    KarplusStrongDsp dsp;
    dsp.prepare (44100.0);
    dsp.setParameters (makeParams (1.0f, 0.5f, 0, 100.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f, 1.0f));

    dsp.noteOn (440.0f, 1.0f);
    int n = 0;
    while (!dsp.isSilent() && n < 44100 * 10)
    {
        dsp.processSample();
        ++n;
    }

    REQUIRE (n > 44100);
    REQUIRE (n < 44100 * 3);
}

TEST_CASE("Exciter produces non-zero output during burst", "[exciter]")
{
    Exciter exc;
    exc.prepare (44100.0);
    exc.setParameters (makeParams (0.3f, 0.5f, 0, 100.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f, 0.0f));

    exc.noteOn (100.0f);
    bool anyNonZero = false;
    for (int i = 0; i < 100; ++i)
    {
        if (std::abs (exc.processSample()) > 1e-6f)
        {
            anyNonZero = true;
            break;
        }
    }
    REQUIRE (anyNonZero);
}

TEST_CASE("Exciter becomes inactive after burst length samples", "[exciter]")
{
    Exciter exc;
    exc.prepare (44100.0);
    exc.setParameters (makeParams (0.3f, 0.5f, 0, 50.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f, 0.0f));

    exc.noteOn (100.0f);
    REQUIRE (exc.isActive());

    for (int i = 0; i < 49; ++i)
        exc.processSample();

    REQUIRE (exc.isActive());

    exc.processSample();
    REQUIRE (! exc.isActive());
}

TEST_CASE("Exciter getExcitationLength returns configured value", "[exciter]")
{
    Exciter exc;
    exc.prepare (44100.0);
    exc.setParameters (makeParams (0.3f, 0.5f, 0, 250.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f, 0.0f));

    REQUIRE (std::abs (exc.getExcitationLength() - 250.0f) < 1e-3f);
}

TEST_CASE("Exciter processSample returns zero after burst ends", "[exciter]")
{
    Exciter exc;
    exc.prepare (44100.0);
    exc.setParameters (makeParams (0.3f, 0.5f, 0, 20.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f, 0.0f));

    exc.noteOn (200.0f);
    for (int i = 0; i < 21; ++i)
        exc.processSample();

    for (int i = 0; i < 10; ++i)
        REQUIRE (std::abs (exc.processSample()) < 1e-6f);
}

TEST_CASE("KsDelayLine produces non-zero output after noteOn", "[delayline]")
{
    KsDelayLine dl;
    dl.prepare (44100.0);
    dl.setParameters (makeParams (0.3f, 0.5f, 0, 100.0f, 0.0f, 0, 1.0f, 0.0f, 1.0f, 0.0f));

    dl.noteOn (440.0f, 1.0f, 100.0f);
    bool anyNonZero = false;
    for (int i = 0; i < 2000; ++i)
    {
        float exc = (i < 100) ? 0.5f : 0.0f;
        float out = dl.processSample (exc);
        if (std::abs (out) > 1e-6f)
        {
            anyNonZero = true;
            break;
        }
    }
    REQUIRE (anyNonZero);
}

TEST_CASE("KsDelayLine output decays over time", "[delayline]")
{
    KsDelayLine dl;
    dl.prepare (44100.0);
    dl.setParameters (makeParams (0.3f, 0.0f, 0, 50.0f, 0.0f, 0, 1.0f, 0.0f, 1.0f, 0.0f));

    dl.noteOn (440.0f, 1.0f, 50.0f);
    for (int i = 0; i < 50; ++i)
        dl.processSample (0.8f);

    float maxEarly = 0.0f;
    for (int i = 0; i < 1000; ++i)
        maxEarly = std::max (maxEarly, std::abs (dl.processSample (0.0f)));

    for (int i = 0; i < 44100 * 2; ++i)
        dl.processSample (0.0f);

    float maxLate = 0.0f;
    for (int i = 0; i < 1000; ++i)
        maxLate = std::max (maxLate, std::abs (dl.processSample (0.0f)));

    REQUIRE (maxLate < maxEarly);
}

TEST_CASE("KsDelayLine isSilent after natural decay", "[delayline]")
{
    KsDelayLine dl;
    dl.prepare (44100.0);
    dl.setParameters (makeParams (0.3f, 0.0f, 0, 50.0f, 0.0f, 0, 1.0f, 0.0f, 1.0f, 0.0f));

    dl.noteOn (440.0f, 1.0f, 50.0f);
    for (int i = 0; i < 50; ++i)
        dl.processSample (0.5f);
    for (int i = 0; i < 44100 * 7; ++i)
        dl.processSample (0.0f);

    REQUIRE (dl.isSilent());
}

TEST_CASE("KsDelayLine reset clears buffer", "[delayline]")
{
    KsDelayLine dl;
    dl.prepare (44100.0);
    dl.setParameters (makeParams (0.3f, 1.0f, 0, 100.0f, 0.0f, 0, 1.0f, 0.0f, 1.0f, 0.0f));

    dl.noteOn (220.0f, 1.0f, 100.0f);
    for (int i = 0; i < 100; ++i)
        dl.processSample (0.5f);
    for (int i = 0; i < 1000; ++i)
        dl.processSample (0.0f);

    float before = std::abs (dl.processSample (0.0f));
    REQUIRE (before > 0.0f);

    dl.reset();
    float after = std::abs (dl.processSample (0.0f));
    REQUIRE (after < 1e-6f);
}

TEST_CASE("Key tracking flattens decay across octaves", "[dsp]")
{
    KarplusStrongDsp dspLow, dspHigh;
    dspLow.prepare (44100.0);
    dspHigh.prepare (44100.0);

    dspLow.setParameters (makeParams (0.3f, 0.5f, 0, 100.0f, 0.2f, 0, 0.0f, 0.0f, 1.0f, 1.0f));
    dspHigh.setParameters(makeParams (0.3f, 0.5f, 0, 100.0f, 0.2f, 0, 0.0f, 0.0f, 1.0f, 1.0f));

    dspLow.noteOn (220.0f, 1.0f);
    dspHigh.noteOn(440.0f, 1.0f);

    auto runToSilent = [](KarplusStrongDsp& d, int maxSamples)
    {
        int n = 0;
        while (!d.isSilent() && n < maxSamples)
        {
            d.processSample();
            ++n;
        }
        return n;
    };

    int samplesLow = runToSilent(dspLow, 44100 * 10);
    int samplesHigh = runToSilent(dspHigh, 44100 * 10);

    REQUIRE (samplesLow > 0);
    REQUIRE (samplesHigh > 0);
    REQUIRE (std::abs(samplesLow - samplesHigh) < 2000);
}

TEST_CASE("Key track amount ramps compensation linearly", "[dsp]")
{
    KarplusStrongDsp dsp;
    dsp.prepare (44100.0);

    auto computePeakSamples = [&](float keyTrack) {
        dsp.setParameters (makeParams (0.3f, 0.5f, 0, 100.0f, 0.2f, 0, 0.0f, 0.0f, 1.0f, keyTrack));
        dsp.noteOn (220.0f, 1.0f);
        int n = 0;
        while (!dsp.isSilent() && n < 44100 * 10) { dsp.processSample(); ++n; }
        return n;
    };

    int s0 = computePeakSamples(0.0f);
    int s5 = computePeakSamples(0.5f);
    int s1 = computePeakSamples(1.0f);

    REQUIRE (s0 > s1);
    REQUIRE (s5 >= s1);
    REQUIRE (s5 <= s0);
}

TEST_CASE("KsDelayLine getSampleRate returns prepared value", "[delayline]")
{
    KsDelayLine dl;
    dl.prepare (48000.0);
    REQUIRE (dl.getSampleRate() == 48000.0);
}

TEST_CASE("CircularBuffer returns written sample after the given delay", "[primitives]")
{
    CircularBuffer buf;
    buf.prepare (64);

    buf.write (1.0f);
    for (int i = 0; i < 9; ++i)
        buf.write (0.0f);

    REQUIRE (buf.readDelayed (10.0f) == 1.0f);
    REQUIRE (buf.readDelayed (5.0f) == 0.0f);
}

TEST_CASE("CircularBuffer clamps delay to valid range", "[primitives]")
{
    CircularBuffer buf;
    buf.prepare (16);

    buf.write (0.5f);
    REQUIRE (buf.readDelayed (0.0f) == 0.5f);
    REQUIRE (buf.readDelayed (1000.0f) == 0.0f);
}

TEST_CASE("CircularBuffer clear zeroes contents", "[primitives]")
{
    CircularBuffer buf;
    buf.prepare (16);

    for (int i = 0; i < 16; ++i)
        buf.write (1.0f);

    buf.clear();
    for (int i = 1; i < 16; ++i)
        REQUIRE (buf.readDelayed (static_cast<float> (i)) == 0.0f);
}

TEST_CASE("OnePoleLowpass converges to a constant input", "[primitives]")
{
    OnePoleLowpass lp;
    lp.setCutoff (1000.0f, 44100.0f);

    float out = 0.0f;
    for (int i = 0; i < 1000; ++i)
        out = lp.process (1.0f);

    REQUIRE (std::abs (out - 1.0f) < 1e-3f);

    lp.reset();
    REQUIRE (std::abs (lp.process (0.0f)) < 1e-6f);
}

TEST_CASE("DcBlocker removes a constant offset", "[primitives]")
{
    DcBlocker dc;
    dc.prepare (44100.0);

    float out = 0.0f;
    for (int i = 0; i < 44100; ++i)
        out = dc.process (1.0f);

    REQUIRE (std::abs (out) < 1e-3f);
}

TEST_CASE("SilenceDetector respects the minimum-samples guard", "[primitives]")
{
    SilenceDetector det;
    det.noteOn (100);

    for (int i = 0; i < 50; ++i)
        det.track (0.0f);

    REQUIRE (! det.isSilent());

    for (int i = 0; i < 50; ++i)
        det.track (0.0f);

    REQUIRE (det.isSilent());
}

TEST_CASE("SilenceDetector goes silent only after the peak decays", "[primitives]")
{
    SilenceDetector det;
    det.noteOn (10);

    det.track (1.0f);
    for (int i = 0; i < 100; ++i)
        det.track (0.0f);

    REQUIRE (! det.isSilent());

    for (int i = 0; i < 6000; ++i)
        det.track (0.0f);

    REQUIRE (det.isSilent());
}

TEST_CASE("computeCutoffHz maps brightness to the cutoff range", "[calibration]")
{
    REQUIRE (ks::computeCutoffHz (0.0f) == ks::minCutoffHz);
    REQUIRE (ks::computeCutoffHz (1.0f) == ks::minCutoffHz + ks::cutoffRangeHz);
    REQUIRE (ks::computeCutoffHz (-1.0f) == ks::minCutoffHz);
    REQUIRE (ks::computeCutoffHz (2.0f) == ks::minCutoffHz + ks::cutoffRangeHz);
}

TEST_CASE("computeFeedbackGain hits the -60dB target when losses are negligible", "[calibration]")
{
    float sr = 44100.0f;
    float delay = sr / 440.0f;
    float gain = ks::computeFeedbackGain (1.0f, 440.0f, delay, 1.0f, 1.0e6f, sr);
    float expected = std::exp (-ks::ln1000 * delay / sr);
    REQUIRE (std::abs (gain - expected) < 1e-4f);
}

TEST_CASE("computeFeedbackGain compensates the lowpass loss at the fundamental", "[calibration]")
{
    float sr = 44100.0f;
    float delay = sr / 440.0f;
    float gain = ks::computeFeedbackGain (0.03f, 440.0f, delay, 1.0f, 440.0f, sr);
    float base = std::exp (-ks::ln1000 * delay / (0.03f * sr));
    REQUIRE (std::abs (gain - base * std::sqrt (2.0f)) < 1e-4f);
}

TEST_CASE("computeFeedbackGain clamps at the maximum stable gain", "[calibration]")
{
    float sr = 44100.0f;
    float delay = sr / 880.0f;
    float gain = ks::computeFeedbackGain (100.0f, 880.0f, delay, 0.0f, ks::minCutoffHz, sr);
    REQUIRE (gain == ks::maxFeedbackGain);
}
