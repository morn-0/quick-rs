#[cfg(feature = "async")]
use crate::runtime::event_loop::AsyncState;
use crate::{context::Context, loader::UserLoader, value};
use log::error;
use quickjs_sys as sys;
use std::{
    ffi::{c_char, c_void, CStr},
    path::Path,
    rc::Rc,
    {fs, ptr},
};

#[cfg(feature = "async")]
pub(crate) mod event_loop;
#[cfg(feature = "async")]
pub(crate) mod timers;

#[cfg(feature = "mimalloc")]
#[no_mangle]
extern "C" fn rust_calloc(_: *mut c_void, count: usize, size: usize) -> *mut c_void {
    unsafe { libmimalloc_sys::mi_calloc(count, size) }
}

#[cfg(feature = "mimalloc")]
#[no_mangle]
extern "C" fn rust_malloc(_: *mut c_void, size: usize) -> *mut c_void {
    unsafe { libmimalloc_sys::mi_malloc(size) }
}

#[cfg(feature = "mimalloc")]
#[no_mangle]
extern "C" fn rust_free(_: *mut c_void, ptr: *mut c_void) {
    unsafe {
        libmimalloc_sys::mi_free(ptr);
    }
}

#[cfg(feature = "mimalloc")]
#[no_mangle]
extern "C" fn rust_realloc(_: *mut c_void, ptr: *mut c_void, size: usize) -> *mut c_void {
    unsafe { libmimalloc_sys::mi_realloc(ptr, size) }
}

#[cfg(feature = "mimalloc")]
#[no_mangle]
extern "C" fn rust_usable_size(ptr: *const c_void) -> usize {
    unsafe { libmimalloc_sys::mi_usable_size(ptr) }
}

#[cfg(feature = "mimalloc")]
static MF: sys::JSMallocFunctions = sys::JSMallocFunctions {
    js_calloc: Some(rust_calloc),
    js_malloc: Some(rust_malloc),
    js_free: Some(rust_free),
    js_realloc: Some(rust_realloc),
    js_malloc_usable_size: Some(rust_usable_size),
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
    opaque: *mut c_void,
) -> *mut sys::JSModuleDef {
    if !opaque.is_null() {
        let loader = unsafe { &*(opaque as *const Box<dyn UserLoader>) };
        if let Some(module) = loader.load(ctx, module_name) {
            return module;
        }
    }

    let name = unsafe { CStr::from_ptr(module_name) }
        .to_string_lossy()
        .into_owned();

    #[cfg(feature = "async")]
    if name == "timer" {
        return unsafe {
            use crate::{loader, promise::timer::Timer};
            loader::evaluate::<Timer>(ctx, "timer")
        }
        .unwrap_or(ptr::null_mut());
    }

    let source = if Path::new(&name).exists() {
        fs::read_to_string(&name).ok()
    } else {
        None
    };

    if let Some(source) = source {
        if let Some(ctx) = unsafe { Context::from_opaque(ctx) } {
            return match ctx.eval_module(&source, &name) {
                Ok(module) => value::ptr_of(module.raw()).cast::<sys::JSModuleDef>(),
                Err(e) => {
                    error!("{e}");
                    ptr::null_mut()
                }
            };
        }
    }

    ptr::null_mut()
}

pub(crate) struct RuntimeInner {
    raw: *mut sys::JSRuntime,
    loader: *mut Box<dyn UserLoader>,
}

impl Drop for RuntimeInner {
    fn drop(&mut self) {
        unsafe {
            sys::JS_FreeRuntime(self.raw);
            if !self.loader.is_null() {
                drop(Box::from_raw(self.loader));
            }
        }
    }
}

#[derive(Clone)]
pub struct Runtime(Rc<RuntimeInner>);

impl Runtime {
    pub fn new() -> Self {
        Self::with_options(0, 0, None)
    }

    pub fn with_options(heap: usize, stack: usize, loader: Option<Box<dyn UserLoader>>) -> Self {
        let raw = unsafe {
            #[cfg(feature = "mimalloc")]
            let raw = sys::JS_NewRuntime2(&MF as *const _, ptr::null_mut());
            #[cfg(not(feature = "mimalloc"))]
            let raw = sys::JS_NewRuntime();

            if heap != 0 {
                sys::JS_SetMemoryLimit(raw, heap);
            }
            if stack != 0 {
                sys::JS_SetMaxStackSize(raw, stack);
            }

            #[cfg(feature = "async")]
            crate::function::register_closure_class(raw);

            raw
        };

        let loader = match loader {
            Some(loader) => Box::into_raw(Box::new(loader)),
            None => ptr::null_mut(),
        };
        unsafe {
            sys::JS_SetModuleLoaderFunc(
                raw,
                Some(module_normalize),
                Some(module_loader),
                loader.cast::<c_void>(),
            );
        }

        Runtime(Rc::new(RuntimeInner { raw, loader }))
    }

    pub fn context(&self) -> Context {
        Context::new(self.clone())
    }

    pub(crate) fn as_raw(&self) -> *mut sys::JSRuntime {
        self.0.raw
    }

    #[cfg(feature = "async")]
    pub(crate) fn async_state(&self) -> Option<&AsyncState> {
        unsafe { (sys::JS_GetRuntimeOpaque(self.as_raw()) as *const AsyncState).as_ref() }
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
