#include "quickjs.h"

JSValueConst JS_GetModuleExport_real(JSContext *ctx, JSModuleDef *m, const char *export_name)
{
    return JS_GetModuleExport(ctx, m, export_name);
}

int JS_ValueGetTag_real(JSValue v)
{
    return JS_VALUE_GET_TAG(v);
}

void *JS_ValueGetPtr_real(JSValue v)
{
    return JS_VALUE_GET_PTR(v);
}

void JS_FreeValue_real(JSContext *ctx, JSValue v)
{
    JS_FreeValue(ctx, v);
}

JSValue JS_DupValue_real(JSContext *ctx, JSValue v)
{
    return JS_DupValue(ctx, v);
}
