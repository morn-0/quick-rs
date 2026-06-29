use crate::{
    error::Error,
    value::{self, Value},
};
use quickjs_sys as sys;

pub struct Module {
    value: Value,
}

impl Module {
    pub fn new(value: Value) -> Result<Self, Error> {
        let ctx = value.context().clone();
        let raw = value::dup(ctx.as_raw(), value.raw());
        let raw = unsafe { sys::JS_EvalFunction(ctx.as_raw(), raw) };

        ctx.check(Value::from_raw(ctx.clone(), raw))?;
        Ok(Module { value })
    }

    pub fn namespace(&self) -> Result<Value, Error> {
        let ctx = self.value.context();
        let ptr = value::ptr_of(self.value.raw()).cast::<sys::JSModuleDef>();

        let raw = unsafe { sys::JS_GetModuleNamespace(ctx.as_raw(), ptr) };
        ctx.check(Value::from_raw(ctx.clone(), raw))
    }

    pub fn get(&self, name: impl AsRef<str>) -> Result<Value, Error> {
        self.namespace()?.get_property(name)
    }
}
