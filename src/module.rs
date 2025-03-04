use crate::{
    error::QuickError,
    value::{self, Exception, JSValueRef},
};
use quickjs_sys as sys;
use std::ffi::{c_char, CString};

extern "C" {
    fn JS_GetModuleExport_real(
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
        let function = value::dup_value(value.ctx().ptr(), value.val());

        let ret = unsafe { sys::JS_EvalFunction(value.ctx().ptr(), function) };
        let ret = JSValueRef::from_value(value.ctx().clone(), ret);

        if ret.tag() == sys::JS_TAG_EXCEPTION {
            let exception = unsafe { sys::JS_GetException(value.ctx().ptr()) };
            let exception = JSValueRef::from_value(value.ctx().clone(), exception);

            Err(QuickError::Eval(Exception(exception).to_string()))
        } else {
            Ok(Module { value })
        }
    }

    pub fn get(&self, name: impl AsRef<str>) -> Result<JSValueRef, QuickError> {
        let c_name = match CString::new(name.as_ref()) {
            Ok(c_name) => c_name,
            Err(e) => return Err(QuickError::CString(e.to_string())),
        };

        let value = unsafe {
            JS_GetModuleExport_real(
                self.value.ctx().ptr(),
                self.value.ptr() as *mut sys::JSModuleDef,
                c_name.as_ptr() as *const _,
            )
        };
        let value = JSValueRef::from_value(self.value.ctx().clone(), value);

        if value.tag() == sys::JS_TAG_EXCEPTION {
            let value = unsafe { sys::JS_GetException(self.value.ctx().ptr()) };
            let value = JSValueRef::from_value(self.value.ctx().clone(), value);

            Err(QuickError::Eval(Exception(value).to_string()))
        } else {
            Ok(value)
        }
    }
}
