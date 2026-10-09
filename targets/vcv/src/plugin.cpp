/*
 * plugin.cpp — Rack plugin registration. Nothing else belongs here.
 */

#include "plugin.hpp"

Plugin* pluginInstance;

void init(Plugin* p) {
    pluginInstance = p;
    p->addModel(modelKarplusStrong);
}
