use crate::value::JSValueRef;
use quickjs_sys as sys;
use std::{ffi::c_char, slice};

pub fn to_string(value: JSValueRef) -> String {
    let (data, len) = unsafe {
        let mut len = 0;
        let data = sys::JS_ToCStringLen2(value.ctx, &mut len, value.val, 0) as *const _;
        (data, len)
    };
    let buf = unsafe { slice::from_raw_parts(data, len) };

    let string = String::from_utf8_lossy(buf).to_string();
    unsafe {
        sys::JS_FreeCString(value.ctx, data as *const _);
    }
    string
}

pub fn to_property(value: JSValueRef, prop: *const c_char) -> JSValueRef {
    let js_value = unsafe { sys::JS_GetPropertyStr(value.ctx, value.val, prop) };
    JSValueRef::from_js_value(value.ctx, js_value)
}

pub fn is_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}
