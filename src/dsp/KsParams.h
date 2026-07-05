#pragma once

struct KsParams
{
    int   excitationType   = 0;
    float excitationLength = 100.0f;
    float pickPosition     = 0.2f;
    int   pickModel        = 0;
    float decayTime        = 4.0f;
    float keyTrack         = 0.0f;
    float brightness       = 0.5f;
    float velBrightness    = 0.0f;
    float velDecay         = 0.0f;
    float outputLevel      = 0.8f;
};
