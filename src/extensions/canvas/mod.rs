use crate::{
    extensions::{bind_function, canvas::r#impl::Canvas, AsExtension, Extension},
    value::{self, JSValueRef},
};
use log::error;
use quickjs_sys as sys;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{ffi::c_int, mem::ManuallyDrop, slice};

use self::r#impl::Paint;

pub(crate) mod r#impl;
pub(crate) mod invoke;

pub(crate) struct CanvasExtension;

impl Extension for CanvasExtension {
    fn name(&self) -> String {
        "_canvas".into()
    }

    fn load(&self, ctx: *mut sys::JSContext) -> *mut sys::JSModuleDef {
        unsafe extern "C" fn func(
            ctx: *mut sys::JSContext,
            module: *mut sys::JSModuleDef,
        ) -> c_int {
            bind_function(ctx, module, "invoke", Some(invoke), 1);

            0
        }

        unsafe {
            let module = sys::JS_NewCModule(ctx, "_canvas\0".as_ptr() as *const _, Some(func));

            sys::JS_AddModuleExport(ctx, module, "invoke\0".as_ptr() as *const _);

            module
        }
    }

    fn is_global(&self) -> bool {
        true
    }
}

impl AsExtension for CanvasExtension {
    fn r#as(self) -> Box<dyn Extension> {
        Box::new(self) as Box<dyn Extension>
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Invoke {
    call: i32,
    #[serde(default)]
    target: Value,
    #[serde(default)]
    paint: Value,
    #[serde(default)]
    style: Value,
    #[serde(default)]
    point: Value,
}

unsafe extern "C" fn invoke(
    ctx: *mut sys::JSContext,
    _: sys::JSValue,
    argc: c_int,
    argv: *mut sys::JSValue,
) -> sys::JSValue {
    if argc != 1 {
        return value::make_undefined();
    }
    let args = slice::from_raw_parts(argv, argc as usize);

    let obj = match ManuallyDrop::new(JSValueRef::from_js_value(ctx, args[0])).to_string() {
        Ok(width) => width,
        Err(e) => {
            error!("{e}");
            return value::make_undefined();
        }
    };

    let invoke = match serde_json::from_str::<Invoke>(&obj) {
        Ok(v) => v,
        Err(e) => {
            error!("{e}");
            return value::make_undefined();
        }
    };

    match invoke::invoke(ctx, invoke) {
        Ok(value) => value,
        Err(e) => {
            error!("{e}");
            value::make_undefined()
        }
    }
}
