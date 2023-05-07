use crate::{
    error::EvalError,
    value::{self, Exception, JSValueRef, UNDEFINED},
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

    pub fn call(&self, args: Vec<sys::JSValue>) -> Result<JSValueRef, EvalError> {
        let value = unsafe {
            let undefined = *UNDEFINED;

            let result = sys::JS_Call(
                self.value.ctx,
                self.value.val,
                undefined,
                args.len() as _,
                args.as_ptr() as _,
            );

            value::JS_FreeValue_real(self.value.ctx, undefined);
            for arg in args {
                value::JS_FreeValue_real(self.value.ctx, arg);
            }

            result
        };

        let value = JSValueRef::from_js_value(self.value.ctx, value);
        if value.tag() == sys::JS_TAG_EXCEPTION {
            let value = unsafe { sys::JS_GetException(self.value.ctx) };
            let value = JSValueRef::from_js_value(self.value.ctx, value);

            Err(EvalError::ExecuteError(Exception(value).to_string()))
        } else {
            Ok(value)
        }
    }
}
