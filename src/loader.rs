use crate::{context::Context, value::Value};
use log::error;
use quickjs_sys as sys;
use std::{
    collections::{HashMap, HashSet},
    ffi::{c_char, c_int, CString},
};

pub trait UserLoader {
    fn load(
        &self,
        ctx: *mut sys::JSContext,
        module_name: *const c_char,
    ) -> Option<*mut sys::JSModuleDef>;
}

pub trait ModuleDef {
    fn export(ctx: &Context) -> HashMap<impl AsRef<str>, Value>;
    fn define() -> HashSet<impl AsRef<str>>;
}

/// 注册一个由 Rust 提供导出的 C 模块。
///
/// # Safety
/// `ctx` 必须是有效且存活的 `JSContext`，且本函数在模块加载回调内被调用。
pub unsafe fn evaluate<D>(
    ctx: *mut sys::JSContext,
    name: impl AsRef<str>,
) -> Option<*mut sys::JSModuleDef>
where
    D: ModuleDef,
{
    extern "C" fn init<D>(raw_ctx: *mut sys::JSContext, module: *mut sys::JSModuleDef) -> c_int
    where
        D: ModuleDef,
    {
        let ctx = match unsafe { Context::from_opaque(raw_ctx) } {
            Some(ctx) => ctx,
            None => return -1,
        };

        for (name, value) in D::export(&ctx) {
            let name = match CString::new(name.as_ref()) {
                Ok(name) => name,
                Err(e) => {
                    error!("{e}");
                    return -1;
                }
            };

            unsafe {
                sys::JS_SetModuleExport(ctx.as_raw(), module, name.as_ptr(), value.into_raw())
            };
        }

        0
    }

    let c_name = match CString::new(name.as_ref()) {
        Ok(name) => name,
        Err(e) => {
            error!("{e}");
            return None;
        }
    };
    let module = unsafe { sys::JS_NewCModule(ctx, c_name.as_ptr(), Some(init::<D>)) };

    for name in D::define() {
        let name = match CString::new(name.as_ref()) {
            Ok(name) => name,
            Err(e) => {
                error!("{e}");
                return None;
            }
        };
        unsafe { sys::JS_AddModuleExport(ctx, module, name.as_ptr()) };
    }

    Some(module)
}
