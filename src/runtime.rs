use crate::{context::Context, extensions::EXTENSION_MAP, util};
use log::error;
use quickjs_sys as sys;
use std::{
    ffi::{c_char, c_void, CStr},
    fs,
    mem::ManuallyDrop,
    path::Path,
    ptr::null_mut,
};

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

    if let Some(extension) = EXTENSION_MAP.get(&module_name) {
        return extension.load(ctx);
    }

    let src = if util::is_url(&module_name) {
        if let Ok(request) = reqwest::blocking::get(&module_name) {
            request.text().ok()
        } else {
            None
        }
    } else if Path::new(&module_name).exists() {
        fs::read_to_string(&module_name).ok()
    } else {
        None
    };

    if let Some(src) = src {
        let ctx = ManuallyDrop::new(Context(ctx));

        return match ctx.eval_module(src.as_str(), module_name.as_str()) {
            Ok(value) => value.ptr() as *mut sys::JSModuleDef,
            Err(e) => {
                error!("{e}");
                null_mut()
            }
        };
    }

    null_mut()
}

pub struct Runtime(pub *mut sys::JSRuntime);

impl Runtime {
    pub fn new() -> Self {
        let rt = unsafe {
            let rt = sys::JS_NewRuntime();

            sys::js_std_init_handlers(rt);
            #[cfg(target_pointer_width = "32")]
            let heap_size = 1024 * 1024 * 16;
            #[cfg(target_pointer_width = "64")]
            let heap_size = 1024 * 1024 * 32;
            sys::JS_SetMemoryLimit(rt, heap_size);
            #[cfg(target_pointer_width = "32")]
            let stack_size = 1024 * 1024;
            #[cfg(target_pointer_width = "64")]
            let stack_size = 1024 * 1024 * 2;
            sys::JS_SetMaxStackSize(rt, stack_size);
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

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            sys::JS_FreeRuntime(self.0);
        }
    }
}
