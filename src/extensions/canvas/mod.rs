use crate::{
    extensions::{
        bind_function,
        canvas::r#impl::text::{Text, TextStyle},
        AsExtension, Extension,
    },
    value::{self, JSValueRef},
};
use log::error;
use quickjs_sys as sys;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{ffi::c_int, mem::ManuallyDrop, slice};

pub(crate) mod r#impl;

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

#[derive(Deserialize, Serialize)]
struct Invoke {
    tag: i32,
    paint: Value,
    style: Value,
    point: Value,
}

unsafe extern "C" fn invoke(
    ctx: *mut sys::JSContext,
    _: sys::JSValue,
    argc: c_int,
    argv: *mut sys::JSValue,
) -> sys::JSValue {
    if argc != 1 {
        return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
    }
    let args = slice::from_raw_parts(argv, argc as usize);

    let obj = match ManuallyDrop::new(JSValueRef::from_js_value(ctx, args[0])).to_string() {
        Ok(width) => width,
        Err(e) => {
            error!("{e}");
            return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
        }
    };

    let invoke = match serde_json::from_str::<Invoke>(&obj) {
        Ok(v) => v,
        Err(e) => {
            error!("{e}");
            return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
        }
    };

    match invoke.tag {
        1 => {
            let paint = serde_json::from_value::<Text>(invoke.paint).unwrap();
            let style = serde_json::from_value::<TextStyle>(invoke.style).unwrap();
            let point = serde_json::from_value::<(i32, i32)>(invoke.point).unwrap();
        }
        _ => {}
    }

    value::JS_MKVAL_real(sys::JS_TAG_NULL, 0)
}
