use crate::{
    error::QuickError,
    value::{self, Exception, JSValueRef},
};
use anyhow::Result;
use quickjs_sys as sys;

pub struct Function {
    value: JSValueRef,
}

impl Function {
    pub fn new(value: JSValueRef) -> Result<Self> {
        Ok(Function { value })
    }

    pub fn call(
        &self,
        this: Option<JSValueRef>,
        args: Vec<JSValueRef>,
    ) -> Result<JSValueRef, QuickError> {
        let this_raw = match this {
            Some(ref v) => v.val,
            None => value::make_undefined(),
        };
        let args_raw: Vec<_> = args.iter().map(|arg| arg.val).collect();

        let value = unsafe {
            sys::JS_Call(
                self.value.ctx,
                self.value.val,
                this_raw,
                args_raw.len() as _,
                args_raw.as_ptr() as _,
            )
        };

        let value = JSValueRef::from_js_value(self.value.ctx, value);
        if value.is_exception() {
            let value = unsafe { sys::JS_GetException(self.value.ctx) };
            let value = JSValueRef::from_js_value(self.value.ctx, value);

            Err(QuickError::CallError(Exception(value).to_string()))
        } else {
            Ok(value)
        }
    }
}
