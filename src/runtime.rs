use crate::{context::Context, util};
use compio::runtime as compio;
use log::error;
use quickjs_sys as sys;
use std::{
    cell::RefCell,
    ffi::{c_char, c_void, CStr},
    fs,
    future::Future,
    mem::ManuallyDrop,
    path::Path,
    pin::Pin,
    ptr::{self, null_mut},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

thread_local! {
    static CURRENT_RUNTIME: RefCell<Option<Runtime>> = RefCell::new(None);
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
    _opaque: *mut c_void,
) -> *mut sys::JSModuleDef {
    let module_name = unsafe { CStr::from_ptr(module_name) }
        .to_string_lossy()
        .to_string();

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
        let rt = Self(rt);

        rt
    }

    pub fn event_loop<C, R>(&self, consumer: C) -> R
    where
        C: FnOnce(Rc<Context>) -> Pin<Box<dyn Future<Output = R>>> + Send + 'static,
        R: Default + Send + 'static,
    {
        compio::block_on(async {
            let mut ctx = Context::from(self);

            let pctx = ptr::addr_of_mut!(ctx.0);
            let ctx = Rc::new(ctx);

            let result = compio::spawn({
                let ctx = ctx.clone();

                async move { consumer(ctx).await }
            });

            compio::time::sleep(Duration::from_millis(1)).await;
            while unsafe { sys::JS_IsJobPending(self.0) } > 0 {
                unsafe {
                    sys::JS_ExecutePendingJob(self.0, pctx);
                }
            }

            result.await
        })
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
