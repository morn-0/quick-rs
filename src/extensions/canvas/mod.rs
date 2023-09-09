use crate::{
    context::Context,
    extensions::{bind_function, AsExtension, Extension},
    value::{self, JSValueRef},
};
use log::error;
use plotters::prelude::BitMapBackend;
use quickjs_sys as sys;
use std::{ffi::c_int, mem::ManuallyDrop, slice};

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
            bind_function(ctx, module, "new", Some(new), 2);
            bind_function(ctx, module, "drop", Some(drop), 1);

            0
        }

        unsafe {
            let module = sys::JS_NewCModule(ctx, "_canvas\0".as_ptr() as *const _, Some(func));

            sys::JS_AddModuleExport(ctx, module, "new\0".as_ptr() as *const _);
            sys::JS_AddModuleExport(ctx, module, "invoke\0".as_ptr() as *const _);
            sys::JS_AddModuleExport(ctx, module, "drop\0".as_ptr() as *const _);

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

unsafe extern "C" fn new(
    ctx: *mut sys::JSContext,
    _: sys::JSValue,
    argc: c_int,
    argv: *mut sys::JSValue,
) -> sys::JSValue {
    if argc != 2 {
        return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
    }
    let args = slice::from_raw_parts(argv, argc as usize);

    let width = match ManuallyDrop::new(JSValueRef::from_js_value(ctx, args[0])).to_i32() {
        Ok(width) => width,
        Err(e) => {
            error!("{e}");
            return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
        }
    };
    let height = match ManuallyDrop::new(JSValueRef::from_js_value(ctx, args[1])).to_i32() {
        Ok(height) => height,
        Err(e) => {
            error!("{e}");
            return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
        }
    };

    let mut buffer = vec![0; 3 * (width * height) as usize];
    let bit_map = BitMapBackend::with_buffer(&mut buffer, (width as u32, height as u32));

    let ptr = (Box::into_raw(Box::new(bit_map)) as u64).to_string();
    if let Ok(ptr) = ManuallyDrop::new(Context(ctx)).new_string(&ptr) {
        return ptr.val;
    }

    value::JS_MKVAL_real(sys::JS_TAG_NULL, 0)
}

unsafe extern "C" fn drop(
    ctx: *mut sys::JSContext,
    _: sys::JSValue,
    argc: c_int,
    argv: *mut sys::JSValue,
) -> sys::JSValue {
    if argc != 1 {
        return value::JS_MKVAL_real(sys::JS_TAG_UNDEFINED, 0);
    }
    let args = slice::from_raw_parts(argv, argc as usize);

    let ptr = match ManuallyDrop::new(JSValueRef::from_js_value(ctx, args[0])).to_string() {
        Ok(ptr) => ptr,
        Err(e) => {
            error!("{e}");
            return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
        }
    };
    if let Ok(ptr) = ptr.parse::<u64>() {
        let bit_map = Box::from_raw(ptr as *mut BitMapBackend);
        core::mem::drop(bit_map);
    }

    value::JS_MKVAL_real(sys::JS_TAG_UNDEFINED, 0)
}
