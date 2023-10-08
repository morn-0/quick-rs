use crate::extensions::canvas::r#impl::{Canvas, Paint, Point};
use barcoders::{
    generators::image::{Color, Image, Rotation},
    sym::code128::Code128,
};
use compact_str::{CompactString, ToCompactString};
use image::ImageFormat;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use tiny_skia::{PixmapPaint, PixmapRef, Transform};

#[derive(Deserialize, Serialize)]
pub(crate) struct Barcode {
    content: CompactString,
}

impl Barcode {
    pub fn new(content: impl AsRef<str> + ToCompactString) -> Self {
        Self {
            content: content.to_compact_string(),
        }
    }
}

impl Paint for Barcode {
    type Target = Canvas;

    type Style = BarStyle;

    fn draw(&mut self, target: &mut Self::Target, style: Self::Style, point: Point) {
        if let Ok(barcode) = Code128::new(format!("Ɓ{}", self.content)) {
            let encoded = barcode.encode();

            let image = Image::JPEG {
                height: style.height,
                xdim: style.xdim,
                rotation: Rotation::Zero,
                foreground: Color::black(),
                background: Color::white(),
            };

            if let Ok(data) = image
                .generate(encoded)
                .map(Cursor::new)
                .map(|c| image::load(c, ImageFormat::Jpeg))
            {
                if let Ok(image) = data.map(|i| i.to_rgba8()) {
                    let (data, width, height) = (image.as_raw(), image.width(), image.height());

                    let pixmap = PixmapRef::from_bytes(data, width, height);
                    if let Some(pixmap) = pixmap {
                        target.pixmap.draw_pixmap(
                            point.x,
                            point.y,
                            pixmap,
                            &PixmapPaint::default(),
                            Transform::default(),
                            None,
                        );
                    }
                }
            }
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub(crate) struct BarStyle {
    height: u32,
    xdim: u32,
}

impl BarStyle {
    pub fn new(height: u32, xdim: u32) -> Self {
        Self { height, xdim }
    }
}
