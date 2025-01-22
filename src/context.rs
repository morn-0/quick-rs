use crate::{
    error::QuickError,
    runtime::Runtime,
    value::{self, Exception, JSValueRef},
};
use quickjs_sys as sys;
use std::{
    ffi::{c_double, c_int, c_void, CString},
    mem,
    ptr::{self, slice_from_raw_parts_mut},
    slice,
};

extern "C" {
    fn JS_MKVAL_real(tag: i32, val: i32) -> sys::JSValue;
    fn JS_MKPTR_real(tag: i32, ptr: *mut c_void) -> sys::JSValue;
    fn JS_NewFloat64_real(ctx: *mut sys::JSContext, val: c_double) -> sys::JSValue;
}

pub struct Context(pub(crate) *mut sys::JSContext);

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

impl Clone for Context {
    fn clone(&self) -> Self {
        Context(unsafe { sys::JS_DupContext(self.0) })
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            sys::JS_FreeContext(self.0);
        }
    }
}

impl Context {
    pub fn global(&self) -> JSValueRef {
        JSValueRef::from_value(self.clone(), unsafe { sys::JS_GetGlobalObject(self.0) })
    }

    pub fn eval_module(
        &self,
        source: impl AsRef<str>,
        name: impl AsRef<str>,
    ) -> Result<JSValueRef, QuickError> {
        const FLAGS: i32 = (sys::JS_EVAL_TYPE_MODULE | sys::JS_EVAL_FLAG_COMPILE_ONLY) as i32;
        self.eval(source, name, FLAGS)
    }

    pub fn eval_global(
        &self,
        source: impl AsRef<str>,
        name: impl AsRef<str>,
    ) -> Result<JSValueRef, QuickError> {
        self.eval(source, name, sys::JS_EVAL_TYPE_GLOBAL as i32)
    }

    pub fn eval(
        &self,
        source: impl AsRef<str>,
        name: impl AsRef<str>,
        flags: i32,
    ) -> Result<JSValueRef, QuickError> {
        let (c_source, c_name) = match (CString::new(source.as_ref()), CString::new(name.as_ref()))
        {
            (Ok(a), Ok(b)) => (a, b),
            _ => return Err(QuickError::CString(source.as_ref().to_string())),
        };

        unsafe {
            let value = sys::JS_Eval(
                self.0,
                c_source.as_ptr(),
                source.as_ref().len() - 1,
                c_name.as_ptr(),
                flags,
            );
            let value = JSValueRef::from_value(self.clone(), value);

            if value.tag() == sys::JS_TAG_EXCEPTION {
                let value = sys::JS_GetException(self.0);
                let value = JSValueRef::from_value(self.clone(), value);

                Err(QuickError::Eval(Exception(value).to_string()))
            } else {
                Ok(value)
            }
        }
    }

    pub fn ptr(&self) -> *mut sys::JSContext {
        self.0
    }

    pub fn make_bool(&self, value: bool) -> JSValueRef {
        let value = unsafe { JS_MKVAL_real(sys::JS_TAG_BOOL, if value { 1 } else { 0 }) };
        JSValueRef::from_value(self.clone(), value)
    }

    pub fn make_buffer(&self, value: impl AsRef<[u8]>) -> Result<JSValueRef, QuickError> {
        unsafe extern "C" fn free(_: *mut sys::JSRuntime, opaque: *mut c_void, ptr: *mut c_void) {
            if !opaque.is_null() {
                let len = ptr::read::<usize>(opaque as *const usize);
                let ptr = slice_from_raw_parts_mut(ptr as *mut u8, len);
                drop(Box::from_raw(ptr));
            }
        }

        let value = value.as_ref();

        let mut len = value.len();
        let opaque = if len == 0 {
            ptr::null_mut()
        } else {
            ptr::addr_of_mut!(len)
        } as *mut c_void;

        let value = Box::into_raw(value.to_owned().into_boxed_slice()) as *mut u8;
        let value = unsafe { sys::JS_NewArrayBuffer(self.0, value, len, Some(free), opaque, 0) };
        Ok(JSValueRef::from_value(self.clone(), value))
    }

    pub fn make_float(&self, value: f64) -> JSValueRef {
        let value = unsafe { JS_NewFloat64_real(self.0, value) };
        JSValueRef::from_value(self.clone(), value)
    }

    pub fn make_function(
        &self,
        argc: i32,
        call: fn(Context, JSValueRef, Option<Vec<JSValueRef>>) -> JSValueRef,
    ) -> JSValueRef {
        unsafe extern "C" fn inner(
            ctx: *mut sys::JSContext,
            this: sys::JSValue,
            argc: c_int,
            argv: *mut sys::JSValue,
            _: c_int,
            func: *mut sys::JSValue,
        ) -> sys::JSValue {
            let ctx = Context(ctx);

            let closure = JSValueRef::from_value(ctx.clone(), *func);
            #[rustfmt::skip]
            let closure: fn(Context, JSValueRef, Option<Vec<JSValueRef>>) -> JSValueRef = mem::transmute(closure.ptr());

            let this = {
                let ctx = ctx.clone();
                let val = value::dup_value(ctx.ptr(), this);

                JSValueRef::from_value(ctx, val)
            };

            let args = if argc == 0 {
                None
            } else {
                let args = unsafe { slice::from_raw_parts_mut(argv, argc as usize) };
                let args = args
                    .iter()
                    .map(|v| {
                        let ctx = ctx.clone();
                        let val = value::dup_value(ctx.ptr(), *v);

                        JSValueRef::from_value(ctx.clone(), val)
                    })
                    .collect();

                Some(args)
            };

            let value = closure(ctx.clone(), this, args);
            let value = value::dup_value(ctx.ptr(), value.val());

            std::mem::forget(ctx);
            value
        }

        let mut data = unsafe { JS_MKPTR_real(sys::JS_TAG_NULL, call as *mut c_void) };
        let data = ptr::addr_of_mut!(data);

        let func = unsafe { sys::JS_NewCFunctionData(self.0, Some(inner), argc, 0, 1, data) };
        JSValueRef::from_value(self.clone(), func)
    }

    pub fn make_int(&self, value: i32) -> JSValueRef {
        let value = unsafe { JS_MKVAL_real(sys::JS_TAG_INT, value) };
        JSValueRef::from_value(self.clone(), value)
    }

    pub fn make_null(&self) -> JSValueRef {
        let value = unsafe { JS_MKVAL_real(sys::JS_TAG_NULL, 0) };
        JSValueRef::from_value(self.clone(), value)
    }

    pub fn make_object(&self) -> JSValueRef {
        let value = unsafe { sys::JS_NewObject(self.0) };
        JSValueRef::from_value(self.clone(), value)
    }

    pub fn make_string(&self, value: impl AsRef<str>) -> Result<JSValueRef, QuickError> {
        let value = match CString::new(value.as_ref()) {
            Ok(v) => v,
            Err(e) => {
                return Err(QuickError::CString(e.to_string()));
            }
        };
        let value = unsafe { sys::JS_NewStringLen(self.0, value.as_ptr(), value.as_bytes().len()) };

        Ok(JSValueRef::from_value(self.clone(), value))
    }

    pub fn make_undefined(&self) -> JSValueRef {
        let value = unsafe { JS_MKVAL_real(sys::JS_TAG_UNDEFINED, 0) };
        JSValueRef::from_value(self.clone(), value)
    }
}
