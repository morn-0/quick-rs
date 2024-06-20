use crate::{
    error::QuickError,
    runtime::Runtime,
    value::{Exception, JSValueRef},
};
use quickjs_sys as sys;
use std::{
    ffi::{c_double, c_void, CString},
    ptr::{self, slice_from_raw_parts_mut},
};

extern "C" {
    fn JS_MKVAL_real(tag: i32, val: i32) -> sys::JSValue;
    fn JS_MKPTR_real(tag: i32, ptr: *mut c_void) -> sys::JSValue;
    fn JS_NewFloat64_real(ctx: *mut sys::JSContext, val: c_double) -> sys::JSValue;
}

pub struct Context(pub *mut sys::JSContext);

impl From<&Runtime> for Context {
    fn from(value: &Runtime) -> Self {
        let ctx = unsafe {
            let ctx = sys::JS_NewContext(value.0);

            sys::JS_AddIntrinsicRegExpCompiler(ctx);

            ctx
        };

        Context(ctx)
    }
}

impl Context {
    pub fn eval_module(&self, src: &str, name: &str) -> Result<JSValueRef, QuickError> {
        const FLAGS: i32 = (sys::JS_EVAL_TYPE_MODULE | sys::JS_EVAL_FLAG_COMPILE_ONLY) as i32;
        self.eval(src, name, FLAGS)
    }

    pub fn eval_global(&self, src: &str, name: &str) -> Result<JSValueRef, QuickError> {
        self.eval(src, name, sys::JS_EVAL_TYPE_GLOBAL as i32)
    }

    pub fn eval(&self, src: &str, name: &str, flags: i32) -> Result<JSValueRef, QuickError> {
        let (c_src, c_name) = match (CString::new(src), CString::new(name)) {
            (Ok(c_src), Ok(c_name)) => (c_src, c_name),
            _ => return Err(QuickError::CStringError(src.to_string())),
        };

        unsafe {
            let value = sys::JS_Eval(self.0, c_src.as_ptr(), src.len(), c_name.as_ptr(), flags);
            let value = JSValueRef::from_value(self.0, value);

            if value.tag() == sys::JS_TAG_EXCEPTION {
                let value = sys::JS_GetException(self.0);
                let value = JSValueRef::from_value(self.0, value);

                Err(QuickError::EvalError(Exception(value).to_string()))
            } else {
                Ok(value)
            }
        }
    }

    pub fn make_undefined(&self) -> JSValueRef {
        let value = unsafe { JS_MKVAL_real(sys::JS_TAG_UNDEFINED, 0) };
        JSValueRef::from_value(self.0, value)
    }

    pub fn make_bool(&self, flag: bool) -> JSValueRef {
        let value = unsafe { JS_MKVAL_real(sys::JS_TAG_BOOL, if flag { 1 } else { 0 }) };
        JSValueRef::from_value(self.0, value)
    }

    pub fn make_null(&self) -> JSValueRef {
        let value = unsafe { JS_MKVAL_real(sys::JS_TAG_NULL, 0) };
        JSValueRef::from_value(self.0, value)
    }

    /// # Safety
    pub unsafe fn make_ptr(&self, ptr: *mut c_void) -> JSValueRef {
        let value = unsafe { JS_MKPTR_real(sys::JS_TAG_NULL, ptr) };
        JSValueRef::from_value(self.0, value)
    }

    pub fn make_int(&self, value: i32) -> JSValueRef {
        let value = unsafe { JS_MKVAL_real(sys::JS_TAG_INT, value) };
        JSValueRef::from_value(self.0, value)
    }

    pub fn make_float(&self, value: f64) -> JSValueRef {
        let value = unsafe { JS_NewFloat64_real(self.0, value) };
        JSValueRef::from_value(self.0, value)
    }

    pub fn make_string(&self, value: impl AsRef<str>) -> Result<JSValueRef, QuickError> {
        let value = match CString::new(value.as_ref()) {
            Ok(v) => v,
            Err(e) => {
                return Err(QuickError::CStringError(e.to_string()));
            }
        };
        let value = unsafe { sys::JS_NewStringLen(self.0, value.as_ptr(), value.as_bytes().len()) };

        Ok(JSValueRef::from_value(self.0, value))
    }

    pub fn make_buffer(&self, value: Vec<u8>) -> Result<JSValueRef, QuickError> {
        unsafe extern "C" fn free(_: *mut sys::JSRuntime, opaque: *mut c_void, ptr: *mut c_void) {
            if !opaque.is_null() {
                let len = ptr::read::<usize>(opaque as *const usize);
                let ptr = slice_from_raw_parts_mut(ptr as *mut u8, len);
                drop(Box::from_raw(ptr));
            }
        }

        let mut len = value.len();
        let value = Box::into_raw(value.into_boxed_slice()) as *mut u8;

        let value = unsafe {
            sys::JS_NewArrayBuffer(
                self.0,
                value,
                len,
                Some(free),
                if len == 0 {
                    ptr::null_mut()
                } else {
                    ptr::addr_of_mut!(len)
                } as *mut c_void,
                0,
            )
        };
        Ok(JSValueRef::from_value(self.0, value))
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            sys::JS_FreeContext(self.0);
        }
    }
}
