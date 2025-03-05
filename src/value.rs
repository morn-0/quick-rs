use crate::{context::Context, error::QuickError};
use log::error;
use quickjs_sys as sys;
use std::{
    f64,
    ffi::{c_void, CString},
    fmt::Display,
    mem::{self, MaybeUninit},
    slice,
};

extern "C" {
    fn JS_VALUE_GET_TAG_real(v: sys::JSValue) -> i32;
    fn JS_VALUE_GET_INT_real(val: sys::JSValue) -> i32;
    fn JS_VALUE_GET_FLOAT64_real(val: sys::JSValue) -> f64;
    fn JS_VALUE_GET_PTR_real(v: sys::JSValue) -> *mut c_void;
    fn JS_DupValue_real(ctx: *mut sys::JSContext, v: sys::JSValue) -> sys::JSValue;
    fn JS_FreeValue_real(ctx: *mut sys::JSContext, v: sys::JSValue);
}

pub(crate) fn dup_value(ctx: *mut sys::JSContext, v: sys::JSValue) -> sys::JSValue {
    unsafe { JS_DupValue_real(ctx, v) }
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
    ctx: Context,
    val: sys::JSValue,
    tag: i32,
    ptr: *mut c_void,
}

impl Clone for JSValueRef {
    fn clone(&self) -> Self {
        let v = unsafe { JS_DupValue_real(self.ctx.ptr(), self.val) };
        Self::from_value(self.ctx.clone(), v)
    }
}

impl Drop for JSValueRef {
    fn drop(&mut self) {
        unsafe {
            JS_FreeValue_real(self.ctx.ptr(), self.val);
        }
    }
}

impl JSValueRef {
    pub fn from_value(ctx: Context, val: sys::JSValue) -> Self {
        let tag = unsafe { JS_VALUE_GET_TAG_real(val) };
        let ptr = unsafe { JS_VALUE_GET_PTR_real(val) };
        JSValueRef { ctx, tag, ptr, val }
    }

    #[inline(always)]
    pub fn ctx(&self) -> &Context {
        &self.ctx
    }

    #[inline(always)]
    pub fn val(&self) -> sys::JSValue {
        self.val
    }

    #[inline(always)]
    pub fn tag(&self) -> i32 {
        self.tag
    }

    #[inline(always)]
    pub fn ptr(&self) -> *mut c_void {
        self.ptr
    }

    pub fn get_property(&self, prop: impl AsRef<str>) -> Result<JSValueRef, QuickError> {
        let atom = unsafe {
            let ptr = prop.as_ref().as_ptr();
            let len = prop.as_ref().len();

            sys::JS_NewAtomLen(self.ctx.ptr(), ptr as *const _, len)
        };

        let value = unsafe { sys::JS_GetProperty(self.ctx.ptr(), self.val, atom) };
        Ok(JSValueRef::from_value(self.ctx.clone(), value))
    }

    pub fn set_property(&self, prop: impl AsRef<str>, value: JSValueRef) -> Result<(), QuickError> {
        let atom = unsafe {
            let ptr = prop.as_ref().as_ptr();
            let len = prop.as_ref().len();

            sys::JS_NewAtomLen(self.ctx.ptr(), ptr as *const _, len)
        };

        unsafe {
            sys::JS_SetProperty(
                self.ctx.ptr(),
                self.val,
                atom,
                dup_value(value.ctx().ptr(), value.val()),
            );
        }
        Ok(())
    }

    pub fn to_array(&self) -> Result<Vec<JSValueRef>, QuickError> {
        let length = self.get_property("length")?;
        let length = length.to_i32()?;

        let mut array = Vec::with_capacity(length as usize);

        for i in 0..length {
            unsafe {
                let value = sys::JS_GetPropertyUint32(self.ctx.ptr(), self.val, i as u32);
                array.push(JSValueRef::from_value(self.ctx.clone(), value));
            }
        }

        Ok(array)
    }

    pub fn to_bool(&self) -> Result<bool, QuickError> {
        if self.tag == sys::JS_TAG_BOOL {
            Ok(unsafe { JS_VALUE_GET_INT_real(self.val) } != 0)
        } else {
            Err(QuickError::Type(self.tag))
        }
    }

    pub fn to_buffer<T: Number>(&self) -> Result<&[T], QuickError> {
        if unsafe { sys::JS_IsArrayBuffer(self.val) == 1 } {
            let mut size = MaybeUninit::<usize>::uninit();

            #[rustfmt::skip]
            let ptr = unsafe { sys::JS_GetArrayBuffer(self.ctx.ptr(), size.as_mut_ptr(), self.val) };
            let len: usize = unsafe { size.assume_init() };

            let len = len / mem::size_of::<T>();
            Ok(unsafe { slice::from_raw_parts(ptr.cast(), len) })
        } else {
            Err(QuickError::Type(self.tag))
        }
    }

    pub fn to_buffer_mut<T: Number>(&mut self) -> Result<&mut [T], QuickError> {
        if unsafe { sys::JS_IsArrayBuffer(self.val) == 1 } {
            let mut size = MaybeUninit::<usize>::uninit();

            #[rustfmt::skip]
            let ptr = unsafe { sys::JS_GetArrayBuffer(self.ctx.ptr(), size.as_mut_ptr(), self.val) };
            let len: usize = unsafe { size.assume_init() };

            let len = len / mem::size_of::<T>();
            Ok(unsafe { slice::from_raw_parts_mut(ptr.cast(), len) })
        } else {
            Err(QuickError::Type(self.tag))
        }
    }

    pub fn to_f64(&self) -> Result<f64, QuickError> {
        if self.tag == sys::JS_TAG_FLOAT64 {
            Ok(unsafe { JS_VALUE_GET_FLOAT64_real(self.val) })
        } else {
            Err(QuickError::Type(self.tag))
        }
    }

    pub fn to_i32(&self) -> Result<i32, QuickError> {
        if self.tag == sys::JS_TAG_INT {
            Ok(unsafe { JS_VALUE_GET_INT_real(self.val) })
        } else {
            Err(QuickError::Type(self.tag))
        }
    }

    pub fn to_json(&self) -> Result<String, QuickError> {
        let undefined = self.ctx.make_undefined().val();

        #[rustfmt::skip]
        let value = unsafe { sys::JS_JSONStringify(self.ctx.ptr(), self.val, undefined, undefined) };
        JSValueRef::from_value(self.ctx.clone(), value).to_string()
    }

    pub fn to_string(&self) -> Result<String, QuickError> {
        if self.tag == sys::JS_TAG_STRING {
            let (data, len) = unsafe {
                let mut len = 0;
                let data = sys::JS_ToCStringLen2(self.ctx.ptr(), &mut len, self.val, 0) as *const _;
                (data, len)
            };
            let buf = unsafe { slice::from_raw_parts(data, len) };

            let string = String::from_utf8_lossy(buf).to_string();
            unsafe {
                sys::JS_FreeCString(self.ctx.ptr(), data as *const _);
            }

            Ok(string)
        } else {
            Err(QuickError::Type(self.tag))
        }
    }
}

pub struct Exception(pub JSValueRef);

impl Display for Exception {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self.0.get_property("name").and_then(|v| v.to_string()) {
            Ok(v) => v,
            Err(e) => {
                error!("{e}");
                String::from("none")
            }
        };

        let message = match self.0.get_property("message").and_then(|v| v.to_string()) {
            Ok(v) => v,
            Err(e) => {
                error!("{e}");
                String::from("none")
            }
        };

        let stack = match self.0.get_property("stack").and_then(|v| v.to_string()) {
            Ok(v) => v,
            Err(e) => {
                error!("{e}");
                String::from("none")
            }
        };

        write!(f, "name: {name}, message: {message}, stack: {stack}")
    }
}
