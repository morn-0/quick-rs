use crate::{
    context::Context,
    error::QuickError,
    value::{Exception, JSValueRef},
};
use anyhow::Result;
use quickjs_sys as sys;
use std::mem::ManuallyDrop;

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
        let this_raw = match &this {
            Some(v) => v.val,
            None => {
                let ctx = ManuallyDrop::new(Context(self.value.ctx));
                ctx.make_undefined().val()
            }
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
