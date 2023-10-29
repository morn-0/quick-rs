use std::error::Error;
use tiny_skia::{Color, Pixmap, Rect, Transform};

pub(crate) mod barcode;
pub(crate) mod qrcode;
pub(crate) mod text;

pub(crate) trait Paint {
    type Target;
    type Style;
    type Point;

    fn draw(&mut self, target: &mut Self::Target, style: Self::Style, point: Self::Point);
}

pub(crate) struct Canvas {
    pixmap: Pixmap,
    width: u32,
    height: u32,
}

impl Canvas {
    pub(crate) fn new(width: u32, height: u32) -> Option<Self> {
        let mut pixmap = Pixmap::new(width, height)?;

        let mut paint = tiny_skia::Paint::default();
        paint.set_color(Color::WHITE);
        let rect = Rect::from_xywh(0.0, 0.0, width as f32, height as f32)?;
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);

        Some(Canvas {
            pixmap,
            width,
            height,
        })
    }

    pub(crate) fn to_png_bytes(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        Ok(self.pixmap.encode_png()?)
    }
}
