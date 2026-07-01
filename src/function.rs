use crate::{
    context::Context,
    error::Error,
    value::{self, Value, ValueRef},
};
use quickjs_sys as sys;
use std::{
    ffi::{c_int, c_void},
    mem,
    panic::{self, AssertUnwindSafe},
};

pub type Callback = fn(&Context, ValueRef, &[ValueRef]) -> Result<Value, Error>;

pub struct Function {
    value: Value,
}

impl Function {
    pub fn new(value: Value) -> Self {
        Function { value }
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn call(&self, this: Option<ValueRef>, args: &[ValueRef]) -> Result<Value, Error> {
        let ctx = self.value.context();

        let this = this
            .map(|t| t.raw())
            .unwrap_or_else(|| value::mkval(sys::JS_TAG_UNDEFINED, 0));
        let argv: Vec<sys::JSValue> = args.iter().map(|a| a.raw()).collect();

        let raw = unsafe {
            sys::JS_Call(
                ctx.as_raw(),
                self.value.raw(),
                this,
                argv.len() as c_int,
                argv.as_ptr().cast_mut(),
            )
        };
        ctx.check(Value::from_raw(ctx.clone(), raw))
    }
}

impl Context {
    pub fn make_function(&self, argc: i32, call: Callback) -> Value {
        let mut data = value::mkptr(sys::JS_TAG_NULL, call as *mut c_void);
        let raw = unsafe {
            sys::JS_NewCFunctionData(self.as_raw(), Some(trampoline), argc, 0, 1, &mut data)
        };
        Value::from_raw(self.clone(), raw)
    }
}

unsafe extern "C" fn trampoline(
    raw_ctx: *mut sys::JSContext,
    this: sys::JSValue,
    argc: c_int,
    argv: *mut sys::JSValue,
    _magic: c_int,
    data: *mut sys::JSValue,
) -> sys::JSValue {
    let ctx = match unsafe { Context::from_opaque(raw_ctx) } {
        Some(ctx) => ctx,
        None => return value::mkval(sys::JS_TAG_EXCEPTION, 0),
    };

    let call: Callback = unsafe { mem::transmute(value::ptr_of(*data)) };

    let this_ref = ValueRef {
        ctx: &ctx,
        raw: this,
    };
    let args: Vec<ValueRef> = (0..argc as isize)
        .map(|i| ValueRef {
            ctx: &ctx,
            raw: unsafe { *argv.offset(i) },
        })
        .collect();

    let outcome = panic::catch_unwind(AssertUnwindSafe(|| call(&ctx, this_ref, &args)));
    match outcome {
        Ok(Ok(value)) => value.into_raw(),
        Ok(Err(err)) => ctx.throw(&err.to_string()),
        Err(_) => ctx.throw("rust callback panicked"),
    }
}

#[cfg(feature = "async")]
mod closure {
    use super::*;
    use std::{ptr, sync::OnceLock};

    pub(crate) type ClosureFn = dyn Fn(&Context, ValueRef, &[ValueRef]) -> Result<Value, Error>;

    static CLASS_ID: OnceLock<sys::JSClassID> = OnceLock::new();

    pub(crate) fn register_class(rt: *mut sys::JSRuntime) {
        let mut fresh: sys::JSClassID = 0;
        unsafe { sys::JS_NewClassID(rt, &mut fresh) };
        let id = *CLASS_ID.get_or_init(|| fresh);
        debug_assert_eq!(
            fresh, id,
            "closure class id must be identical across runtimes"
        );

        let def = sys::JSClassDef {
            class_name: c"RustClosure".as_ptr(),
            finalizer: Some(finalizer),
            gc_mark: None,
            call: None,
            exotic: ptr::null_mut(),
        };

        unsafe { sys::JS_NewClass(rt, id, &def) };
    }

    unsafe extern "C" fn finalizer(_rt: *mut sys::JSRuntime, val: sys::JSValue) {
        let Some(&id) = CLASS_ID.get() else { return };

        let opaque = unsafe { sys::JS_GetOpaque(val, id) };
        if !opaque.is_null() {
            drop(unsafe { Box::from_raw(opaque as *mut Box<ClosureFn>) });
        }
    }

    impl Context {
        pub fn make_closure<F>(&self, argc: i32, call: F) -> Value
        where
            F: Fn(&Context, ValueRef, &[ValueRef]) -> Result<Value, Error> + 'static,
        {
            let boxed: Box<ClosureFn> = Box::new(call);
            let ptr = Box::into_raw(Box::new(boxed));

            let id = *CLASS_ID
                .get()
                .expect("closure class is registered in Runtime::with_options");

            let mut host = unsafe {
                let obj = sys::JS_NewObjectClass(self.as_raw(), id);
                sys::JS_SetOpaque(obj, ptr as *mut c_void);
                obj
            };
            let raw = unsafe {
                sys::JS_NewCFunctionData(self.as_raw(), Some(trampoline), argc, 0, 1, &mut host)
            };

            value::free(self.as_raw(), host);
            Value::from_raw(self.clone(), raw)
        }
    }

    unsafe extern "C" fn trampoline(
        raw_ctx: *mut sys::JSContext,
        this: sys::JSValue,
        argc: c_int,
        argv: *mut sys::JSValue,
        _magic: c_int,
        data: *mut sys::JSValue,
    ) -> sys::JSValue {
        let ctx = match unsafe { Context::from_opaque(raw_ctx) } {
            Some(ctx) => ctx,
            None => return value::mkval(sys::JS_TAG_EXCEPTION, 0),
        };
        let Some(&id) = CLASS_ID.get() else {
            return value::mkval(sys::JS_TAG_EXCEPTION, 0);
        };

        let opaque = unsafe { sys::JS_GetOpaque(*data, id) };
        if opaque.is_null() {
            return value::mkval(sys::JS_TAG_EXCEPTION, 0);
        }
        let call: &ClosureFn = unsafe { &*(opaque as *const Box<ClosureFn>) };

        let this = ValueRef {
            ctx: &ctx,
            raw: this,
        };
        let args: Vec<ValueRef> = (0..argc as isize)
            .map(|i| ValueRef {
                ctx: &ctx,
                raw: unsafe { *argv.offset(i) },
            })
            .collect();

        let outcome = panic::catch_unwind(AssertUnwindSafe(|| call(&ctx, this, &args)));
        match outcome {
            Ok(Ok(value)) => value.into_raw(),
            Ok(Err(err)) => ctx.throw(&err.to_string()),
            Err(_) => ctx.throw("rust closure panicked"),
        }
    }
}

#[cfg(feature = "async")]
pub(crate) use closure::register_class as register_closure_class;
