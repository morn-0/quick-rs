use crate::{
    error::QuickError,
    value::{Exception, JSValueRef},
};
use quickjs_sys as sys;
use std::ffi::{c_char, CString};

extern "C" {
    pub(crate) fn JS_GetModuleExport_real(
        ctx: *mut sys::JSContext,
        m: *mut sys::JSModuleDef,
        export_name: *const c_char,
    ) -> sys::JSValue;
}

pub struct Module {
    value: JSValueRef,
}

impl Module {
    pub fn new(value: JSValueRef) -> Result<Self, QuickError> {
        let _value = unsafe { sys::JS_EvalFunction(value.ctx, value.clone().val()) };
        let _value = JSValueRef::from_js_value(value.ctx, _value);

        if _value.tag() == sys::JS_TAG_EXCEPTION {
            let exception = unsafe { sys::JS_GetException(value.ctx) };
            let exception = JSValueRef::from_js_value(value.ctx, exception);

            Err(QuickError::EvalError(Exception(exception).to_string()))
        } else {
            Ok(Module { value })
        }
    }

    pub fn get(&self, name: &str) -> Result<JSValueRef, QuickError> {
        let c_name = match CString::new(name) {
            Ok(c_name) => c_name,
            Err(e) => return Err(QuickError::CStringError(e.to_string())),
        };

        let value = unsafe {
            JS_GetModuleExport_real(
                self.value.ctx,
                self.value.ptr() as *mut sys::JSModuleDef,
                c_name.as_ptr() as *const _,
            )
        };
        let value = JSValueRef::from_js_value(self.value.ctx, value);

        if value.tag() == sys::JS_TAG_EXCEPTION {
            let value = unsafe { sys::JS_GetException(self.value.ctx) };
            let value = JSValueRef::from_js_value(self.value.ctx, value);

            Err(QuickError::EvalError(Exception(value).to_string()))
        } else {
            Ok(value)
        }
    }
}
