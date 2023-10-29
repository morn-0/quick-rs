use crate::{
    context::Context,
    extensions::canvas::{
        r#impl::text::{Text, TextStyle},
        Canvas, Invoke, Paint,
    },
    value,
};
use log::error;
use quickjs_sys as sys;
use serde_json::Value;
use std::{error::Error, mem::ManuallyDrop};

pub unsafe fn invoke(
    ctx: *mut sys::JSContext,
    invoke: Invoke,
) -> Result<sys::JSValue, Box<dyn Error>> {
    match invoke.call {
        -1 => {
            drop(
                match invoke
                    .target
                    .as_str()
                    .and_then(|p| match p.parse::<i64>() {
                        Ok(p) => Some(p),
                        Err(e) => {
                            error!("{e}");
                            None
                        }
                    })
                    .map(|p| Box::from_raw(p as *mut Canvas))
                {
                    Some(c) => c,
                    None => return Ok(value::make_undefined()),
                },
            );
        }
        0 => {
            let width = invoke.style.get("width").and_then(Value::as_u64);
            let heigth = invoke.style.get("height").and_then(Value::as_u64);

            if let (Some(width), Some(height)) = (width, heigth) {
                let canvas = Box::new(Canvas::new(width as u32, height as u32));
                let ptr = (Box::into_raw(canvas) as i64).to_string();

                return Ok(ManuallyDrop::new(Context(ctx)).new_string(&ptr)?.val());
            }
        }
        1 => {
            let canvas = match invoke.target.as_str().and_then(canvas) {
                Some(c) => c,
                None => return Ok(value::make_undefined()),
            };

            if let Some(path) = invoke.paint.get("path").and_then(Value::as_str) {
                std::fs::write(path, canvas.to_png_bytes()?)?;
            }
        }
        2 => {
            let mut canvas = match invoke.target.as_str().and_then(canvas) {
                Some(c) => c,
                None => return Ok(value::make_undefined()),
            };

            let mut paint = serde_json::from_value::<Text>(invoke.paint)?;
            let style = serde_json::from_value::<TextStyle>(invoke.style)?;
            let point = serde_json::from_value::<(f32, f32)>(invoke.point)?;

            paint.draw(&mut canvas, style, point);
        }
        3 => {
            let paint = serde_json::from_value::<Text>(invoke.paint)?;
            let style = serde_json::from_value::<TextStyle>(invoke.style)?;

            let width = paint.measure(style);
            return Ok(value::make_int(width));
        }
        _ => {}
    }

    Ok(value::make_undefined())
}

fn canvas(ptr: &str) -> Option<ManuallyDrop<Box<Canvas>>> {
    let ptr = match ptr.parse::<i64>() {
        Ok(p) => p,
        Err(e) => {
            error!("{e}");
            return None;
        }
    };

    Some(ManuallyDrop::new(unsafe {
        Box::from_raw(ptr as *mut Canvas)
    }))
}
