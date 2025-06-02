use crate::{
    context::Context,
    value::{self, JSValueRef},
};
use quickjs_sys as sys;
use std::{
    collections::{HashMap, HashSet},
    ffi::{c_char, c_int, CString},
};
use tracing::error;

pub(crate) mod console;
pub(crate) mod timer;

pub trait UserLoader {
    fn load(
        &self,
        ctx: *mut sys::JSContext,
        module_name: *const c_char,
    ) -> Option<*mut sys::JSModuleDef>;
}

pub trait ModuleDef {
    fn export(ctx: Context) -> HashMap<impl AsRef<str>, JSValueRef>;
    fn define() -> HashSet<impl AsRef<str>>;
}

/// # Safety
pub unsafe fn evaluate<D>(
    ctx: *mut sys::JSContext,
    name: impl AsRef<str>,
) -> Option<*mut sys::JSModuleDef>
where
    D: ModuleDef,
{
    extern "C" fn inner<D>(ctx: *mut sys::JSContext, ptr: *mut sys::JSModuleDef) -> c_int
    where
        D: ModuleDef,
    {
        let ctx = Context(ctx);

        for (name, value) in D::export(ctx.clone()) {
            let name = match CString::new(name.as_ref()) {
                Ok(v) => v,
                Err(e) => {
                    error!("{e}");
                    return -1;
                }
            };

            unsafe {
                sys::JS_SetModuleExport(
                    ctx.ptr(),
                    ptr,
                    name.as_ptr(),
                    value::dup_value(ctx.ptr(), value.val()),
                );
            }
        }

        std::mem::forget(ctx);
        0
    }

    let name = match CString::new(name.as_ref()) {
        Ok(v) => v,
        Err(e) => {
            error!("{e}");
            return None;
        }
    };
    let module = unsafe { sys::JS_NewCModule(ctx, name.as_ptr(), Some(inner::<D>)) };

    for name in D::define() {
        let name = match CString::new(name.as_ref()) {
            Ok(v) => v,
            Err(e) => {
                error!("{e}");
                return None;
            }
        };

        unsafe {
            sys::JS_AddModuleExport(ctx, module, name.as_ptr());
        }
    }

    Some(module)
}
