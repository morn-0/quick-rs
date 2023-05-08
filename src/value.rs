use crate::{error::QuickError, util};
use once_cell::sync::Lazy;
use quickjs_sys as sys;
use std::ffi::c_void;

pub static UNDEFINED: Lazy<sys::JSValue> =
    Lazy::new(|| unsafe { JS_MKVAL_real(sys::JS_TAG_UNDEFINED, 0) });

extern "C" {
    pub(crate) fn JS_MKVAL_real(tag: i32, val: i32) -> sys::JSValue;
    fn JS_ValueGetTag_real(v: sys::JSValue) -> i32;
    fn JS_ValueGetPtr_real(v: sys::JSValue) -> *mut c_void;
    pub(crate) fn JS_DupValue_real(ctx: *mut sys::JSContext, v: sys::JSValue) -> sys::JSValue;
    pub(crate) fn JS_FreeValue_real(ctx: *mut sys::JSContext, v: sys::JSValue);
}

pub struct JSValueRef {
    pub ctx: *mut sys::JSContext,
    pub val: sys::JSValue,
    tag: i32,
    ptr: *mut c_void,
}

impl JSValueRef {
    pub fn from_js_value(ctx: *mut sys::JSContext, val: sys::JSValue) -> Self {
        let tag = unsafe { JS_ValueGetTag_real(val) };
        let ptr = unsafe { JS_ValueGetPtr_real(val) };
        JSValueRef { ctx, tag, ptr, val }
    }

    #[inline(always)]
    pub fn is_exception(&self) -> bool {
        self.tag == sys::JS_TAG_EXCEPTION
    }

    pub fn to_string(&self) -> Result<String, QuickError> {
        if self.tag == sys::JS_TAG_STRING {
            Ok(util::to_string(self.clone()))
        } else {
            Err(QuickError::UnsupportedTypeError(self.tag))
        }
    }

    #[inline(always)]
    pub fn tag(&self) -> i32 {
        self.tag
    }

    #[inline(always)]
    pub fn ptr(&self) -> *mut c_void {
        self.ptr
    }
}

impl Clone for JSValueRef {
    fn clone(&self) -> Self {
        let v = unsafe { JS_DupValue_real(self.ctx, self.val) };
        Self::from_js_value(self.ctx, v)
    }
}

impl Drop for JSValueRef {
    fn drop(&mut self) {
        unsafe {
            JS_FreeValue_real(self.ctx, self.val);
        }
    }
}

pub struct Exception(pub JSValueRef);

impl ToString for Exception {
    fn to_string(&self) -> String {
        let name = util::to_string(util::to_property(
            self.0.clone(),
            "name\0".as_ptr() as *const _,
        ));
        let message = util::to_string(util::to_property(
            self.0.clone(),
            "message\0".as_ptr() as *const _,
        ));
        let stack = util::to_string(util::to_property(
            self.0.clone(),
            "stack\0".as_ptr() as *const _,
        ));
        format!("{name} {message} {stack}")
    }
}
