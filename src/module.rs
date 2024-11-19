use crate::{
    error::QuickError,
    value::{self, Exception, JSValueRef},
};
use quickjs_sys as sys;

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
        let module_ptr = self.value.ptr() as *mut _;

        let namespace = unsafe { sys::JS_GetModuleNamespace(self.value.ctx().ptr(), module_ptr) };
        let namespace = JSValueRef::from_value(self.value.ctx().clone(), namespace);

        namespace.get_property(name)
    }
}
