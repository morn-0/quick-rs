use crate::value::JSValueRef;
use quickjs_sys as sys;
use std::{ffi::c_char, slice};

pub fn to_string(value: JSValueRef) -> String {
    let buf = unsafe {
        let mut len = 0;
        let data = sys::JS_ToCStringLen2(value.ctx, &mut len, value.val, 0) as *const _;
        slice::from_raw_parts(data, len)
    };
    String::from_utf8_lossy(buf).to_string()
}

pub fn to_property(value: JSValueRef, prop: *const c_char) -> JSValueRef {
    let js_value = unsafe { sys::JS_GetPropertyStr(value.ctx, value.val, prop) };
    JSValueRef::from_js_value(value.ctx, js_value)
}
