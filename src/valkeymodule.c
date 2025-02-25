#include "valkeymodule.h"

// ValkeyModule_Init is defined as a static function and so won't be exported as
// a symbol. Export a version under a slightly different name so that we can
// get access to it from Rust.

int Export_ValkeyModule_Init(ValkeyModuleCtx *ctx, const char *name, int ver, int apiver) {
    return ValkeyModule_Init(ctx, name, ver, apiver);
}

void Export_ValkeyModule_InitAPI(ValkeyModuleCtx *ctx) {
    ValkeyModule_InitAPI(ctx);
}
