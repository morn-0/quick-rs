use crate::{
    error::QuickError,
    runtime::Runtime,
    value::{Exception, JSValueRef},
};
use quickjs_sys as sys;
use std::ffi::CString;

#[cfg(target_env = "gnu")]
const FLAGS: i32 = sys::JS_EVAL_TYPE_MODULE as i32;
#[cfg(target_env = "gnu")]
const SYS_MOD: &str = r#"import * as std from 'std';import * as os from 'os';globalThis.std = std;globalThis.os = os;"#;

pub struct Context(pub *mut sys::JSContext);

impl From<&Runtime> for Context {
    fn from(value: &Runtime) -> Self {
        let ctx = unsafe {
            let ctx = sys::JS_NewContext(value.0);

            sys::JS_AddIntrinsicBigFloat(ctx);
            sys::JS_AddIntrinsicBigDecimal(ctx);
            sys::JS_AddIntrinsicOperators(ctx);
            sys::JS_EnableBignumExt(ctx, 1);

            #[cfg(target_env = "gnu")]
            sys::js_init_module_std(ctx, "std\0".as_ptr() as *const _);
            #[cfg(target_env = "gnu")]
            sys::js_init_module_os(ctx, "os\0".as_ptr() as *const _);

            ctx
        };
        let ctx = Context(ctx);

        #[cfg(target_env = "gnu")]
        if let Err(e) = ctx.eval(SYS_MOD, "SYS_MOD", FLAGS) {
            log::error!("{e}");
        }

        ctx
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
            let value = JSValueRef::from_js_value(self.0, value);

            if value.tag() == sys::JS_TAG_EXCEPTION {
                let value = sys::JS_GetException(self.0);
                let value = JSValueRef::from_js_value(self.0, value);

                Err(QuickError::EvalError(Exception(value).to_string()))
            } else {
                Ok(value)
            }
        }
    }

    pub fn new_string(&self, value: &str) -> Result<JSValueRef, QuickError> {
        let c_value = match CString::new(value) {
            Ok(c_value) => c_value,
            Err(e) => return Err(QuickError::CStringError(e.to_string())),
        };

        let value = unsafe { sys::JS_NewString(self.0, c_value.as_ptr()) };
        Ok(JSValueRef::from_js_value(self.0, value))
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            #[cfg(target_env = "gnu")]
            let rt = sys::JS_GetRuntime(self.0);
            #[cfg(target_env = "gnu")]
            sys::js_std_free_handlers(rt);
            sys::JS_FreeContext(self.0);
        }
    }
}
