use canvas::CanvasExtension;
use once_cell::sync::Lazy;
use print::PrintExtension;
use quickjs_sys as sys;
use std::{collections::HashMap, ffi::CString, mem::ManuallyDrop};

mod canvas;
mod print;

pub(crate) static EXTENSION_MAP: Lazy<HashMap<String, Box<dyn Extension>>> = Lazy::new(|| {
    let mut map = HashMap::new();

    let print_extension = PrintExtension;
    map.insert(print_extension.name(), print_extension.r#as());
    let canvas_extension = CanvasExtension;
    map.insert(canvas_extension.name(), canvas_extension.r#as());

    map
});

pub(crate) trait Extension: Send + Sync {
    fn name(&self) -> String;
    fn load(&self, ctx: *mut sys::JSContext) -> *mut sys::JSModuleDef;
    fn is_global(&self) -> bool;
}

trait AsExtension {
    fn r#as(self) -> Box<dyn Extension>;
}

pub(crate) unsafe fn bind_function(
    ctx: *mut sys::JSContext,
    module: *mut sys::JSModuleDef,
    func_name: &str,
    func: sys::JSCFunction,
    argc: i32,
) {
    let func_name = CString::new(func_name).expect("CString::new failed");
    let func_name = ManuallyDrop::new(func_name);
    let func_name = func_name.as_ptr() as *const _;

    let func = sys::JS_NewCFunction2(
        ctx,
        func,
        func_name,
        argc,
        sys::JSCFunctionEnum_JS_CFUNC_generic,
        sys::JSCFunctionEnum_JS_CFUNC_generic_magic as i32,
    );
    sys::JS_SetModuleExport(ctx, module, func_name, func);
}
