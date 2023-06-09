use canvas::CanvasExtension;
use once_cell::sync::Lazy;
use print::PrintExtension;
use quickjs_sys as sys;
use std::collections::HashMap;

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
}

trait AsExtension {
    fn r#as(self) -> Box<dyn Extension>;
}
