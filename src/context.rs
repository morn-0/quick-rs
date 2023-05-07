use crate::{
    error::EvalError,
    runtime::Runtime,
    value::{Exception, JSValueRef},
};
use log::error;
use quickjs_sys as sys;
use std::ffi::CString;

const SYS_MOD: &str = r#"import * as std from 'std';import * as os from 'os';globalThis.std = std;globalThis.os = os;"#;

pub struct Context(pub *mut sys::JSContext);

impl Context {
    pub fn new(rt: &Runtime) -> Self {
        let ctx = unsafe {
            let ctx = sys::JS_NewContext(rt.0);
            sys::js_std_init_handlers(rt.0);
            sys::JS_EnableBignumExt(ctx, 1);
            sys::js_init_module_std(ctx, "std\0".as_ptr() as *const _);
            sys::js_init_module_os(ctx, "os\0".as_ptr() as *const _);
            ctx
        };
        let ctx = Context(ctx);

        const FLAGS: i32 = sys::JS_EVAL_TYPE_MODULE as i32;
        if let Err(e) = ctx.eval(SYS_MOD, "SYS_MOD", FLAGS) {
            error!("{e}");
        }

        ctx
    }
}

impl Context {
    pub fn eval_module(&self, src: &str, name: &str) -> Result<JSValueRef, EvalError> {
        const FLAGS: i32 = (sys::JS_EVAL_TYPE_MODULE | sys::JS_EVAL_FLAG_COMPILE_ONLY) as i32;
        self.eval(src, name, FLAGS)
    }

    pub fn eval_global(&self, src: &str, name: &str) -> Result<JSValueRef, EvalError> {
        self.eval(src, name, sys::JS_EVAL_TYPE_GLOBAL as i32)
    }

    pub fn eval(&self, src: &str, name: &str, flags: i32) -> Result<JSValueRef, EvalError> {
        let (c_src, c_name) = match (CString::new(src), CString::new(name)) {
            (Ok(c_src), Ok(c_name)) => (c_src, c_name),
            _ => return Err(EvalError::CStringError(src.to_string())),
        };

        unsafe {
            let value = sys::JS_Eval(self.0, c_src.as_ptr(), src.len(), c_name.as_ptr(), flags);
            let value = JSValueRef::from_js_value(self.0, value);

            if value.tag() == sys::JS_TAG_EXCEPTION {
                let value = sys::JS_GetException(self.0);
                let value = JSValueRef::from_js_value(self.0, value);

                Err(EvalError::ExecuteError(Exception(value).to_string()))
            } else {
                Ok(value)
            }
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            let rt = sys::JS_GetRuntime(self.0);
            sys::js_std_free_handlers(rt);
            sys::JS_FreeContext(self.0);
        }
    }
}
