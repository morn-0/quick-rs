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
    module: *const c_char,
    opaque: *mut c_void,
) -> *mut sys::JSModuleDef {
    if !opaque.is_null() {
        let loader = unsafe { Box::from_raw(opaque as *mut &mut dyn UserLoader) };
        let loader = ManuallyDrop::new(loader);

        if let Some(module) = loader.load(ctx, module) {
            return module;
        }
    }

    let module = unsafe { CStr::from_ptr(module) }
        .to_string_lossy()
        .to_string();

    let source = if Path::new(&module).exists() {
        fs::read_to_string(&module).ok()
    } else {
        None
    };

    if let Some(source) = source {
        let ctx = ManuallyDrop::new(Context(ctx));

        return match ctx.eval_module(source.as_str(), module.as_str()) {
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
    pub fn new(heap: usize, stack: usize, loader: Option<Box<&mut dyn UserLoader>>) -> Self {
        let rt = unsafe {
            let rt = sys::JS_NewRuntime();

            if heap != 0 {
                sys::JS_SetMemoryLimit(rt, heap);
            }

            if stack != 0 {
                sys::JS_SetMaxStackSize(rt, stack);
            }

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
        Self::new(0, 0, None)
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            sys::JS_FreeRuntime(self.0);
        }
    }
}
