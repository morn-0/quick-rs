use crate::context::Context;
use log::error;
use quickjs_sys as sys;
use std::{
    ffi::{c_char, c_void, CStr},
    fs,
    mem::ManuallyDrop,
    ptr::null_mut,
};

pub struct Runtime(pub *mut sys::JSRuntime);

impl Runtime {
    pub fn new() -> Self {
        let rt = unsafe {
            let rt = sys::JS_NewRuntime();

            #[cfg(target_pointer_width = "32")]
            sys::JS_SetMemoryLimit(rt, 1024 * 1024 * 16);
            #[cfg(target_pointer_width = "64")]
            sys::JS_SetMemoryLimit(rt, 1024 * 1024 * 32);

            #[cfg(target_pointer_width = "32")]
            sys::JS_SetMaxStackSize(rt, 1024 * 1024);
            #[cfg(target_pointer_width = "64")]
            sys::JS_SetMaxStackSize(rt, 1024 * 1024 * 2);

            extern "C" fn module_normalize(
                ctx: *mut sys::JSContext,
                _module_base_name: *const c_char,
                module_name: *const c_char,
                _opaque: *mut c_void,
            ) -> *mut c_char {
                unsafe { sys::js_strdup(ctx, module_name) }
            }
            extern "C" fn module_loader(
                ctx: *mut sys::JSContext,
                module_name: *const c_char,
                _opaque: *mut c_void,
            ) -> *mut sys::JSModuleDef {
                let module_name = unsafe { CStr::from_ptr(module_name) }
                    .to_string_lossy()
                    .to_string();

                if let Ok(source) = fs::read_to_string(&module_name) {
                    const FLAGS: i32 = (sys::JS_EVAL_TYPE_MODULE
                        | sys::JS_EVAL_FLAG_STRICT
                        | sys::JS_EVAL_FLAG_COMPILE_ONLY)
                        as i32;
                    let ctx = ManuallyDrop::new(Context(ctx));

                    return match ctx.eval(source.as_str(), module_name.as_str(), FLAGS) {
                        Ok(value) => value.ptr() as *mut sys::JSModuleDef,
                        Err(e) => {
                            error!("{e}");
                            null_mut()
                        }
                    };
                }

                null_mut()
            }
            sys::JS_SetModuleLoaderFunc(
                rt,
                Some(module_normalize),
                Some(module_loader),
                null_mut(),
            );

            rt
        };

        Self(rt)
    }

    pub fn gc(&self) {
        unsafe {
            sys::JS_RunGC(self.0);
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            sys::JS_FreeRuntime(self.0);
        }
    }
}
