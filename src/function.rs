use crate::{
    error::QuickError,
    value::{Exception, JSValueRef},
};
use quickjs_sys as sys;

pub struct Function {
    value: JSValueRef,
}

impl Function {
    pub fn new(value: JSValueRef) -> Self {
        Function { value }
    }

    pub fn call<'a>(
        &'a self,
        this: Option<&'a JSValueRef>,
        args: Vec<JSValueRef>,
    ) -> Result<JSValueRef, QuickError> {
        let this_raw = match this {
            Some(v) => v.val(),
            None => self.value.ctx().make_undefined().val(),
        };
        let args_raw: Vec<_> = args.iter().map(|arg| arg.val()).collect();

        let value = unsafe {
            sys::JS_Call(
                self.value.ctx().ptr(),
                self.value.val(),
                this_raw,
                args_raw.len() as _,
                args_raw.as_ptr() as _,
            )
        };

        let value = JSValueRef::from_value(self.value.ctx().clone(), value);
        if value.tag() == sys::JS_TAG_EXCEPTION {
            let value = unsafe { sys::JS_GetException(self.value.ctx().ptr()) };
            let value = JSValueRef::from_value(self.value.ctx().clone(), value);

            Err(QuickError::Call(Exception(value)))
        } else {
            Ok(value)
        }
    }
}
