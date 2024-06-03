#include "quickjs.h"

int JS_VALUE_GET_TAG_real(JSValue v)
{
    return JS_VALUE_GET_TAG(v);
}

int JS_VALUE_GET_INT_real(JSValue val)
{
    return JS_VALUE_GET_INT(val);
}

double JS_VALUE_GET_FLOAT64_real(JSValue val)
{
    return JS_VALUE_GET_FLOAT64(val);
}

void *JS_VALUE_GET_PTR_real(JSValue v)
{
    return JS_VALUE_GET_PTR(v);
}

JSValue JS_MKVAL_real(int tag, int val)
{
    return JS_MKVAL(tag, val);
}

JSValue JS_MKPTR_real(int tag, void *ptr)
{
    return JS_MKPTR(tag, ptr);
}

JSValue JS_DupValue_real(JSContext *ctx, JSValueConst v)
{
    return JS_DupValue(ctx, v);
}

void JS_FreeValue_real(JSContext *ctx, JSValue v)
{
    JS_FreeValue(ctx, v);
}

JSValue JS_NewFloat64_real(JSContext *ctx, double d)
{
    return JS_NewFloat64(ctx, d);
}

JS_BOOL JS_IsArrayBuffer_real(JSValueConst val)
{
    return JS_IsArrayBuffer(val);
}

JSValueConst JS_GetModuleExport_real(JSContext *ctx, JSModuleDef *m, const char *export_name)
{
    return JS_GetModuleExport(ctx, m, export_name);
}
