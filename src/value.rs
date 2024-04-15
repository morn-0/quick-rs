use crate::error::QuickError;
use anyhow::Result;
use log::error;
use quickjs_sys as sys;
use std::{
    f64,
    ffi::{c_double, c_void, CString},
    mem::{self, ManuallyDrop, MaybeUninit},
    slice,
};

extern "C" {
    fn JS_IsArrayBuffer_real(val: sys::JSValue) -> i32;
    fn JS_VALUE_GET_TAG_real(v: sys::JSValue) -> i32;
    fn JS_VALUE_GET_INT_real(val: sys::JSValue) -> i32;
    fn JS_VALUE_GET_FLOAT64_real(val: sys::JSValue) -> f64;
    fn JS_VALUE_GET_PTR_real(v: sys::JSValue) -> *mut c_void;
    pub(crate) fn JS_MKVAL_real(tag: i32, val: i32) -> sys::JSValue;
    fn JS_DupValue_real(ctx: *mut sys::JSContext, v: sys::JSValue) -> sys::JSValue;
    fn JS_FreeValue_real(ctx: *mut sys::JSContext, v: sys::JSValue);
    pub(crate) fn JS_NewFloat64_real(ctx: *mut sys::JSContext, val: c_double) -> sys::JSValue;
}

pub trait Number {}

impl Number for i8 {}
impl Number for u8 {}
impl Number for i16 {}
impl Number for u16 {}
impl Number for i32 {}
impl Number for u32 {}
impl Number for f32 {}
impl Number for f64 {}

pub struct JSValueRef {
    pub(crate) ctx: *mut sys::JSContext,
    pub(crate) val: sys::JSValue,
    tag: i32,
    ptr: *mut c_void,
}

impl JSValueRef {
    pub fn from_js_value(ctx: *mut sys::JSContext, val: sys::JSValue) -> Self {
        let tag = unsafe { JS_VALUE_GET_TAG_real(val) };
        let ptr = unsafe { JS_VALUE_GET_PTR_real(val) };
        JSValueRef { ctx, tag, ptr, val }
    }

    pub fn property(&self, prop: &str) -> Result<JSValueRef, QuickError> {
        let prop = match CString::new(prop) {
            Ok(v) => v,
            Err(e) => {
                return Err(QuickError::CStringError(e.to_string()));
            }
        };
        let value = unsafe { sys::JS_GetPropertyStr(self.ctx, self.val, prop.as_ptr()) };
        Ok(JSValueRef::from_js_value(self.ctx, value))
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

    pub fn to_string(&self) -> Result<String, QuickError> {
        if self.tag == sys::JS_TAG_STRING {
            let (data, len) = unsafe {
                let mut len = 0;
                let data = sys::JS_ToCStringLen2(self.ctx, &mut len, self.val, 0) as *const _;
                (data, len)
            };
            let buf = unsafe { slice::from_raw_parts(data, len) };

            let string = String::from_utf8_lossy(buf).to_string();
            unsafe {
                sys::JS_FreeCString(self.ctx, data as *const _);
            }

            Ok(string)
        } else {
            Err(QuickError::UnsupportedTypeError(self.tag))
        }
    }

    pub fn to_array(&self) -> Result<Vec<JSValueRef>, QuickError> {
        let length = self.property("length")?;
        let length = length.to_i32()?;

        let mut array = Vec::with_capacity(length as usize);

        for i in 0..length {
            unsafe {
                let value = sys::JS_GetPropertyUint32(self.ctx, self.val, i as u32);
                array.push(JSValueRef::from_js_value(self.ctx, value));
            }
        }

        Ok(array)
    }

    pub fn to_buffer<T: Number>(&self) -> Result<&[T], QuickError> {
        if unsafe { JS_IsArrayBuffer_real(self.val) == 1 } {
            let mut size = MaybeUninit::<usize>::uninit();

            let ptr = unsafe { sys::JS_GetArrayBuffer(self.ctx, size.as_mut_ptr(), self.val) };
            let len: usize = unsafe { size.assume_init() };

            let len = len / mem::size_of::<T>();
            Ok(unsafe { slice::from_raw_parts(ptr.cast(), len) })
        } else {
            Err(QuickError::UnsupportedTypeError(self.tag))
        }
    }

    pub fn to_buffer_mut<T: Number>(&mut self) -> Result<&mut [T], QuickError> {
        if unsafe { JS_IsArrayBuffer_real(self.val) == 1 } {
            let mut size = MaybeUninit::<usize>::uninit();

            let ptr = unsafe { sys::JS_GetArrayBuffer(self.ctx, size.as_mut_ptr(), self.val) };
            let len: usize = unsafe { size.assume_init() };

            let len = len / mem::size_of::<T>();
            Ok(unsafe { slice::from_raw_parts_mut(ptr.cast(), len) })
        } else {
            Err(QuickError::UnsupportedTypeError(self.tag))
        }
    }

    #[inline(always)]
    pub fn is_exception(&self) -> bool {
        self.tag == sys::JS_TAG_EXCEPTION
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
        let name = match self.0.property("name").and_then(|v| v.to_string()) {
            Ok(v) => v,
            Err(e) => {
                error!("{e}");
                String::from("none")
            }
        };

        let message = match self.0.property("message").and_then(|v| v.to_string()) {
            Ok(v) => v,
            Err(e) => {
                error!("{e}");
                String::from("none")
            }
        };

        let stack = match self.0.property("stack").and_then(|v| v.to_string()) {
            Ok(v) => v,
            Err(e) => {
                error!("{e}");
                String::from("none")
            }
        };

        format!("{name} {message} {stack}")
    }
}
