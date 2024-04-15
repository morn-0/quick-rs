use crate::context::Context;
use log::error;
use quickjs_sys as sys;
use std::{
    ffi::{c_char, c_void, CStr},
    fs,
    mem::ManuallyDrop,
    path::Path,
    ptr::null_mut,
};

pub trait UserLoader {
    fn load(
        &self,
        ctx: *mut sys::JSContext,
        module_name: *const c_char,
    ) -> Option<*mut sys::JSModuleDef>;
}

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
    opaque: *mut c_void,
) -> *mut sys::JSModuleDef {
    fn is_url(url: &str) -> bool {
        url.starts_with("https://") || url.starts_with("http://")
    }

    if !opaque.is_null() {
        let loader = unsafe { Box::from_raw(opaque as *mut &mut dyn UserLoader) };
        let loader = ManuallyDrop::new(loader);

        if let Some(module) = loader.load(ctx, module_name) {
            return module;
        }
    }

    let module_name = unsafe { CStr::from_ptr(module_name) }
        .to_string_lossy()
        .to_string();

    let src = if is_url(&module_name) {
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

pub struct Runtime(pub(crate) *mut sys::JSRuntime);

impl Runtime {
    pub fn new(loader: Option<Box<&mut dyn UserLoader>>) -> Self {
        let rt = unsafe {
            let rt = sys::JS_NewRuntime();

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

            let opaque = match loader {
                Some(loader) => Box::into_raw(loader) as _,
                None => null_mut(),
            };
            sys::JS_SetModuleLoaderFunc(rt, Some(module_normalize), Some(module_loader), opaque);

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
        Self::new(None)
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            sys::JS_FreeRuntime(self.0);
        }
    }
}
