use crate::{
    extensions::{AsExtension, Extension},
    value::{self, JSValueRef, JS_MKVAL_real},
};
use barcoders::{generators::image::Image as BarImage, sym::code128::Code128};
use fast_qr::{
    convert::{image::ImageBuilder, Builder, Shape},
    QRBuilder, ECL,
};
use image::{
    imageops::{grayscale, FilterType},
    DynamicImage, GenericImageView, ImageBuffer, Luma,
};
use image_hasher::{Hasher, HasherConfig};
use imageproc::contrast::{adaptive_threshold, otsu_level, threshold};
use log::error;
use once_cell::sync::Lazy;
use quickjs_sys as sys;
use reqwest::blocking::Client;
use std::{ffi::c_int, fs::File, io::Read, mem::ManuallyDrop, slice};

pub(crate) struct PrintExtension;

impl Extension for PrintExtension {
    fn name(&self) -> String {
        "print".into()
    }

    fn load(&self, ctx: *mut sys::JSContext) -> *mut sys::JSModuleDef {
        unsafe extern "C" fn func(
            ctx: *mut sys::JSContext,
            module: *mut sys::JSModuleDef,
        ) -> c_int {
            let function = sys::JS_NewCFunction2(
                ctx,
                Some(image),
                "image\0".as_ptr() as *const _,
                3,
                sys::JSCFunctionEnum_JS_CFUNC_generic,
                sys::JSCFunctionEnum_JS_CFUNC_generic_magic as i32,
            );
            sys::JS_SetModuleExport(ctx, module, "image\0".as_ptr() as *const _, function);
            let function = sys::JS_NewCFunction2(
                ctx,
                Some(qrcode),
                "qrcode\0".as_ptr() as *const _,
                3,
                sys::JSCFunctionEnum_JS_CFUNC_generic,
                sys::JSCFunctionEnum_JS_CFUNC_generic_magic as i32,
            );
            sys::JS_SetModuleExport(ctx, module, "qrcode\0".as_ptr() as *const _, function);
            let function = sys::JS_NewCFunction2(
                ctx,
                Some(barcode),
                "barcode\0".as_ptr() as *const _,
                3,
                sys::JSCFunctionEnum_JS_CFUNC_generic,
                sys::JSCFunctionEnum_JS_CFUNC_generic_magic as i32,
            );
            sys::JS_SetModuleExport(ctx, module, "barcode\0".as_ptr() as *const _, function);

            0
        }

        unsafe {
            let module = sys::JS_NewCModule(ctx, "print\0".as_ptr() as *const _, Some(func));

            sys::JS_AddModuleExport(ctx, module, "image\0".as_ptr() as *const _);
            sys::JS_AddModuleExport(ctx, module, "qrcode\0".as_ptr() as *const _);
            sys::JS_AddModuleExport(ctx, module, "barcode\0".as_ptr() as *const _);

            module
        }
    }
}

impl AsExtension for PrintExtension {
    fn r#as(self) -> Box<dyn Extension> {
        Box::new(self) as Box<dyn Extension>
    }
}

/// addr
/// mode: 1 - esc, 2 - zpl
/// width
unsafe extern "C" fn image(
    ctx: *mut sys::JSContext,
    _: sys::JSValue,
    argc: c_int,
    argv: *mut sys::JSValue,
) -> sys::JSValue {
    static HTTP: Lazy<Client> = Lazy::new(|| Client::new());

    let args = slice::from_raw_parts(argv, argc as usize);

    let addr = match ManuallyDrop::new(JSValueRef::from_js_value(ctx, args[0])).to_string() {
        Ok(addr) => addr,
        Err(e) => {
            error!("{e}");
            return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
        }
    };
    let mode = match ManuallyDrop::new(JSValueRef::from_js_value(ctx, args[1])).to_i32() {
        Ok(mode) => mode,
        Err(e) => {
            error!("{e}");
            return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
        }
    };
    let width = if argc >= 3 {
        match ManuallyDrop::new(JSValueRef::from_js_value(ctx, args[2])).to_i32() {
            Ok(width) => Some(width),
            Err(e) => {
                error!("{e}");
                return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
            }
        }
    } else {
        None
    };

    let mut image_buf = vec![];
    if File::open(&addr)
        .and_then(|mut file| file.read_to_end(&mut image_buf))
        .is_err()
    {
        if let Ok(bytes) = HTTP.get(&addr).send().and_then(|response| response.bytes()) {
            image_buf = bytes.to_vec();
        }
    }

    if let Ok(image) = image::load_from_memory(&image_buf) {
        let data = print_image(image, width, true, mode);

        let value = sys::JS_NewArray(ctx);
        for (i, v) in data.iter().enumerate() {
            sys::JS_SetPropertyUint32(
                ctx,
                value,
                i as u32,
                JS_MKVAL_real(sys::JS_TAG_INT, *v as i32),
            );
        }

        return value;
    }

    return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
}

/// data
/// width
/// mode: 1 - esc, 2 - zpl
unsafe extern "C" fn qrcode(
    ctx: *mut sys::JSContext,
    _: sys::JSValue,
    argc: c_int,
    argv: *mut sys::JSValue,
) -> sys::JSValue {
    let args = slice::from_raw_parts(argv, argc as usize);

    let data = match ManuallyDrop::new(JSValueRef::from_js_value(ctx, args[0])).to_string() {
        Ok(data) => data,
        Err(e) => {
            error!("{e}");
            return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
        }
    };
    let width = match ManuallyDrop::new(JSValueRef::from_js_value(ctx, args[1])).to_i32() {
        Ok(width) => width,
        Err(e) => {
            error!("{e}");
            return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
        }
    };
    let mode = match ManuallyDrop::new(JSValueRef::from_js_value(ctx, args[2])).to_i32() {
        Ok(mode) => mode,
        Err(e) => {
            error!("{e}");
            return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
        }
    };

    if let Ok(qrcode) = QRBuilder::new(data)
        .mask(fast_qr::Mask::HorizontalLines)
        .ecl(ECL::M)
        .build()
    {
        let pixmap = ImageBuilder::default()
            .shape(Shape::Square)
            .background_color([255, 255, 255, 0])
            .fit_width(width as u32)
            .to_pixmap(&qrcode);

        if let Ok(image_buf) = pixmap.encode_png() {
            if let Ok(image) = image::load_from_memory(&image_buf) {
                let data = print_image(image, None, false, mode);

                let value = sys::JS_NewArray(ctx);
                for (i, v) in data.iter().enumerate() {
                    sys::JS_SetPropertyUint32(
                        ctx,
                        value,
                        i as u32,
                        JS_MKVAL_real(sys::JS_TAG_INT, *v as i32),
                    );
                }

                return value;
            }
        }
    }

    return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
}

/// data
/// heigth
/// mode: 1 - esc, 2 - zpl
unsafe extern "C" fn barcode(
    ctx: *mut sys::JSContext,
    _: sys::JSValue,
    argc: c_int,
    argv: *mut sys::JSValue,
) -> sys::JSValue {
    let args = slice::from_raw_parts(argv, argc as usize);

    let data = match ManuallyDrop::new(JSValueRef::from_js_value(ctx, args[0])).to_string() {
        Ok(data) => data,
        Err(e) => {
            error!("{e}");
            return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
        }
    };
    let height = match ManuallyDrop::new(JSValueRef::from_js_value(ctx, args[1])).to_i32() {
        Ok(height) => height,
        Err(e) => {
            error!("{e}");
            return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
        }
    };
    let mode = match ManuallyDrop::new(JSValueRef::from_js_value(ctx, args[2])).to_i32() {
        Ok(mode) => mode,
        Err(e) => {
            error!("{e}");
            return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
        }
    };

    if let Ok(barcode) = Code128::new(format!("Ɓ{data}")) {
        let encoded = barcode.encode();
        let height = height as u32;

        if let Ok(image_buf) = BarImage::png(height).generate(encoded) {
            if let Ok(image) = image::load_from_memory(&image_buf) {
                let data = print_image(image, None, false, mode);

                let value = sys::JS_NewArray(ctx);
                for (i, v) in data.iter().enumerate() {
                    sys::JS_SetPropertyUint32(
                        ctx,
                        value,
                        i as u32,
                        JS_MKVAL_real(sys::JS_TAG_INT, *v as i32),
                    );
                }

                return value;
            }
        }
    }

    return value::JS_MKVAL_real(sys::JS_TAG_NULL, 0);
}

/// mode: 1 - esc, 2 - zpl
fn print_image(
    mut image: DynamicImage,
    width: Option<i32>,
    binarization: bool,
    mode: i32,
) -> Vec<u8> {
    if let Some(pixel) = image.as_mut_rgba8() {
        pixel.enumerate_pixels_mut().for_each(|(_, _, rgba)| {
            const RATE: f32 = 1.003921569;
            let (r, g, b, a) = (
                rgba.0[0] as i32,
                rgba.0[1] as i32,
                rgba.0[2] as i32,
                rgba.0[3] as i32,
            );
            let r = (((255 - a) * 255 + a * r) >> 8) as f32 * RATE + 0.5;
            let g = (((255 - a) * 255 + a * g) >> 8) as f32 * RATE + 0.5;
            let b = (((255 - a) * 255 + a * b) >> 8) as f32 * RATE + 0.5;
            rgba.0 = [r as u8, g as u8, b as u8, 255];
        });
    }

    let mut image = if binarization {
        DynamicImage::from(auto_threshold(grayscale(&image)))
    } else {
        image
    };

    if let Some(width) = width {
        let ratio = image.width() as i32 / width;
        image = image.resize(
            width as u32,
            (image.height() as i32 / ratio) as u32,
            FilterType::Lanczos3,
        );
    }

    let (width, height) = (image.width(), image.height());

    let len = (width + 7) / 8;
    let mut buf: Vec<u8> = vec![0; (len * height) as usize];

    if mode == 1 {
        for y in 0..height {
            for x in 0..len {
                for b in 0..8 {
                    let i = x * 8 + b;
                    if i < width && !is_blank_pixel(&image, i, y) {
                        buf[(y * len + x) as usize] += 0x80 >> (b & 0x7);
                    }
                }
            }
        }
    } else if mode == 2 {
        for y in 0..height {
            for x in 0..len {
                for b in 0..8 {
                    let i = x * 8 + b;
                    if i < width && is_blank_pixel(&image, i, y) {
                        buf[(y * len + x) as usize] += 0x80 >> (b & 0x7);
                    }
                }
            }
        }
    }

    if mode == 1 {
        let mut data = Vec::with_capacity(8 + buf.len());
        data.extend_from_slice(b"\x1d\x76\x30\x03");
        data.extend_from_slice(&(len as u16).to_le_bytes());
        data.extend_from_slice(&(height as u16).to_le_bytes());
        data.extend_from_slice(&buf);
        data
    } else if mode == 2 {
        let command = format!("BITMAP 0,0,{},{},0,", (width + 7) / 8, height);
        let command = command.as_bytes();

        let mut data = Vec::with_capacity(command.len() + buf.len());
        data.extend_from_slice(command);
        data.extend_from_slice(&buf);
        data
    } else {
        vec![]
    }
}

fn auto_threshold(image: ImageBuffer<Luma<u8>, Vec<u8>>) -> ImageBuffer<Luma<u8>, Vec<u8>> {
    static HASHER: Lazy<Hasher> = Lazy::new(|| HasherConfig::new().to_hasher());

    let hash = HASHER.hash_image(&image);

    let otsu = threshold(&image, otsu_level(&image));
    let otsu_dist = hash.dist(&HASHER.hash_image(&otsu));

    let adaptive_13 = adaptive_threshold(&image, 13);
    let adaptive_13_dist = hash.dist(&HASHER.hash_image(&adaptive_13));

    let adaptive_45 = adaptive_threshold(&image, 45);
    let adaptive_45_dist = hash.dist(&HASHER.hash_image(&adaptive_45));

    if otsu_dist < adaptive_13_dist {
        if otsu_dist < adaptive_45_dist {
            otsu
        } else {
            adaptive_45
        }
    } else if adaptive_13_dist < adaptive_45_dist {
        adaptive_13
    } else {
        adaptive_45
    }
}

fn is_blank_pixel(image: &DynamicImage, x: u32, y: u32) -> bool {
    let pixel = image.get_pixel(x, y);
    pixel[3] == 0 || (pixel[0] & pixel[1] & pixel[2]) == 0xFF
}
