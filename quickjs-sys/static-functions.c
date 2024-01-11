#include "quickjs.h"

JSValueConst JS_GetModuleExport_real(JSContext *ctx, JSModuleDef *m, const char *export_name)
{
    return JS_GetModuleExport(ctx, m, export_name);
}

int JS_VALUE_GET_INT_real(JSValue val)
{
    return JS_VALUE_GET_INT(val);
}

double JS_VALUE_GET_FLOAT64_real(JSValue val)
{
    return JS_VALUE_GET_FLOAT64(val);
}

JSValue JS_MKVAL_real(int tag, int val)
{
    return JS_MKVAL(tag, val);
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

JSValue JS_NewFloat64_real(JSContext *ctx, double d)
{
    return JS_NewFloat64(ctx, d);
}
