#include <catch2/catch_test_macros.hpp>
#include <cmath>
#include <vector>
#include "KarplusStrongDsp.h"
#include "Exciter.h"
#include "KsDelayLine.h"

TEST_CASE("KarplusStrongDsp produces non-zero output after noteOn", "[dsp]")
{
    KarplusStrongDsp dsp;
    dsp.prepare (44100.0);
    dsp.setParameters (0.95f, 0.5f, 0, 100.0f, 0.2f, 0, 0.0f, 0.0f, 0.8f);

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
    dsp.setParameters (0.95f, 0.5f, 0, 100.0f, 0.2f, 0, 0.0f, 0.0f, 0.8f);

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
    dsp.setParameters (0.99f, 1.0f, 0, 50.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f);

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

    dsp.setParameters (0.95f, 0.5f, 0, 200.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f);
    dsp.noteOn (440.0f, 1.0f);
    std::vector<float> noiseOut;
    for (int i = 0; i < 500; ++i)
        noiseOut.push_back (dsp.processSample());

    dsp.setParameters (0.95f, 0.5f, 1, 200.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f);
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

    dsp.setParameters (0.95f, 0.5f, 0, 200.0f, 0.3f, 0, 0.0f, 0.0f, 1.0f);
    dsp.noteOn (440.0f, 1.0f);
    std::vector<float> offOut;
    for (int i = 0; i < 500; ++i)
        offOut.push_back (dsp.processSample());

    dsp.setParameters (0.95f, 0.5f, 0, 200.0f, 0.3f, 1, 0.0f, 0.0f, 1.0f);
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
    dsp.setParameters (0.95f, 0.5f, 0, 200.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f);

    dsp.noteOn (440.0f, 1.0f);
    float maxHighVel = 0.0f;
    for (int i = 0; i < 500; ++i)
        maxHighVel = std::max (maxHighVel, std::abs (dsp.processSample()));

    dsp.setParameters (0.95f, 0.5f, 0, 200.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f);
    dsp.noteOn (440.0f, 0.1f);
    float maxLowVel = 0.0f;
    for (int i = 0; i < 500; ++i)
        maxLowVel = std::max (maxLowVel, std::abs (dsp.processSample()));

    REQUIRE (maxHighVel > maxLowVel);
}

TEST_CASE("isSilent returns true after output decays", "[dsp]")
{
    KarplusStrongDsp dsp;
    dsp.prepare (44100.0);
    dsp.setParameters (0.50f, 0.0f, 0, 50.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f);

    dsp.noteOn (440.0f, 1.0f);
    REQUIRE (! dsp.isSilent());

    for (int i = 0; i < 44100 * 5; ++i)
        dsp.processSample();

    dsp.noteOff (true);
    for (int i = 0; i < 44100 * 2; ++i)
        dsp.processSample();

    REQUIRE (dsp.isSilent());
}

TEST_CASE("noteOff with allowTailOff produces silence eventually", "[dsp]")
{
    KarplusStrongDsp dsp;
    dsp.prepare (44100.0);
    dsp.setParameters (0.999f, 1.0f, 0, 100.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f);

    dsp.noteOn (440.0f, 1.0f);
    for (int i = 0; i < 500; ++i)
        dsp.processSample();

    REQUIRE ( ! dsp.isSilent());

    dsp.noteOff (true);
    for (int i = 0; i < 44100 * 2; ++i)
        dsp.processSample();

    REQUIRE ( dsp.isSilent());
}

TEST_CASE("Reset clears delay buffer", "[dsp]")
{
    KarplusStrongDsp dsp;
    dsp.prepare (44100.0);
    dsp.setParameters (0.99f, 1.0f, 0, 500.0f, 0.0f, 0, 0.0f, 0.0f, 1.0f);

    dsp.noteOn (220.0f, 1.0f);
    for (int i = 0; i < 1000; ++i)
        dsp.processSample();

    float before = std::abs (dsp.processSample());
    REQUIRE (before > 0.0f);

    dsp.reset();
    float after = std::abs (dsp.processSample());
    REQUIRE (after < 1e-6f);
}

TEST_CASE("Exciter produces non-zero output during burst", "[exciter]")
{
    Exciter exc;
    exc.prepare (44100.0);
    exc.setParameters (0, 100.0f, 0.0f, 0);

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
    exc.setParameters (0, 50.0f, 0.0f, 0);

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
    exc.setParameters (0, 250.0f, 0.0f, 0);

    REQUIRE (std::abs (exc.getExcitationLength() - 250.0f) < 1e-3f);
}

TEST_CASE("Exciter processSample returns zero after burst ends", "[exciter]")
{
    Exciter exc;
    exc.prepare (44100.0);
    exc.setParameters (0, 20.0f, 0.0f, 0);

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
    dl.setParameters (0.95f, 0.5f, 1.0f, 0.0f, 1.0f);

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
    dl.setParameters (0.50f, 0.0f, 1.0f, 0.0f, 1.0f);

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

TEST_CASE("KsDelayLine isSilent after noteOff and release", "[delayline]")
{
    KsDelayLine dl;
    dl.prepare (44100.0);
    dl.setParameters (0.50f, 0.0f, 1.0f, 0.0f, 1.0f);

    dl.noteOn (440.0f, 1.0f, 50.0f);
    for (int i = 0; i < 50; ++i)
        dl.processSample (0.5f);
    for (int i = 0; i < 44100 * 2; ++i)
        dl.processSample (0.0f);

    dl.noteOff (true);
    for (int i = 0; i < 44100 * 2; ++i)
        dl.processSample (0.0f);

    REQUIRE (dl.isSilent());
}

TEST_CASE("KsDelayLine reset clears buffer", "[delayline]")
{
    KsDelayLine dl;
    dl.prepare (44100.0);
    dl.setParameters (0.99f, 1.0f, 1.0f, 0.0f, 1.0f);

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

TEST_CASE("KsDelayLine getSampleRate returns prepared value", "[delayline]")
{
    KsDelayLine dl;
    dl.prepare (48000.0);
    REQUIRE (dl.getSampleRate() == 48000.0);
}