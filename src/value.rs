use crate::{error::QuickError, util};
use anyhow::Result;
use quickjs_sys as sys;
use std::{
    ffi::{c_double, c_void, CString},
    mem::ManuallyDrop,
    ptr,
};

extern "C" {
    pub(crate) fn JS_NewFloat64_real(ctx: *mut sys::JSContext, val: c_double) -> sys::JSValue;
    pub(crate) fn JS_MKVAL_real(tag: i32, val: i32) -> sys::JSValue;
    fn JS_VALUE_GET_INT_real(val: sys::JSValue) -> i32;
    fn JS_VALUE_GET_FLOAT64_real(val: sys::JSValue) -> f64;
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

    pub fn to_i32(&self) -> Result<i32, QuickError> {
        if self.tag == sys::JS_TAG_INT {
            Ok(unsafe { JS_VALUE_GET_INT_real(self.val) })
        } else {
            Err(QuickError::UnsupportedTypeError(self.tag))
        }
    }

    pub fn to_f64(&self) -> Result<f64, QuickError> {
        if self.tag == sys::JS_TAG_FLOAT64 {
            Ok(unsafe { JS_VALUE_GET_FLOAT64_real(self.val) })
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

    #[inline(always)]
    pub fn val(self) -> sys::JSValue {
        ManuallyDrop::new(self).val
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
        let name = util::to_string(unsafe {
            util::to_property(self.0.clone(), "name\0".as_ptr() as *const _)
        });
        let message = util::to_string(unsafe {
            util::to_property(self.0.clone(), "message\0".as_ptr() as *const _)
        });
        let stack = util::to_string(unsafe {
            util::to_property(self.0.clone(), "stack\0".as_ptr() as *const _)
        });

        format!("{name} {message} {stack}")
    }
}

pub fn make_undefined() -> sys::JSValue {
    unsafe { JS_MKVAL_real(sys::JS_TAG_UNDEFINED, 0) }
}

pub fn make_null() -> sys::JSValue {
    unsafe { JS_MKVAL_real(sys::JS_TAG_NULL, 0) }
}

pub fn make_int(value: i32) -> sys::JSValue {
    unsafe { JS_MKVAL_real(sys::JS_TAG_INT, value) }
}

pub fn make_float(value: f64) -> sys::JSValue {
    unsafe { JS_NewFloat64_real(ptr::null_mut(), value) }
}

pub fn make_string(ctx: *mut sys::JSContext, value: &str) -> Result<sys::JSValue> {
    let c_value = match CString::new(value) {
        Ok(c_value) => c_value,
        Err(e) => return Err(anyhow::anyhow!(e)),
    };

    Ok(unsafe { sys::JS_NewString(ctx, c_value.as_ptr()) })
}
