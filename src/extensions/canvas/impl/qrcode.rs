use crate::extensions::canvas::r#impl::{Canvas, Paint};
use compact_str::{CompactString, ToCompactString};
use fast_qr::{
    convert::{image::ImageBuilder, Builder, Shape},
    Mask, QRBuilder, ECL,
};
use serde::{Deserialize, Serialize};
use tiny_skia::{PixmapPaint, PixmapRef, Transform};

#[derive(Deserialize, Serialize)]
pub(crate) struct Qrcode {
    content: CompactString,
}

impl Qrcode {
    pub fn new(content: impl AsRef<str> + ToCompactString) -> Self {
        Self {
            content: content.to_compact_string(),
        }
    }
}

impl Paint for Qrcode {
    type Target = Canvas;

    type Style = QrStyle;

    type Point = (i32, i32);

    fn draw(&mut self, target: &mut Self::Target, style: Self::Style, point: Self::Point) {
        if let Ok(qrcode) = QRBuilder::new(self.content.as_str())
            .mask(Mask::HorizontalLines)
            .ecl(ECL::M)
            .build()
        {
            let pixmap = ImageBuilder::default()
                .shape(Shape::Square)
                .margin(style.margin as usize)
                .background_color([255, 255, 255, 0])
                .fit_width(style.width)
                .to_pixmap(&qrcode);

            let pixmap = PixmapRef::from_bytes(pixmap.data(), pixmap.width(), pixmap.height());
            if let Some(pixmap) = pixmap {
                target.pixmap.draw_pixmap(
                    point.0,
                    point.1,
                    pixmap,
                    &PixmapPaint::default(),
                    Transform::default(),
                    None,
                );
            }
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub(crate) struct QrStyle {
    margin: u32,
    width: u32,
}

impl QrStyle {
    pub fn new(margin: u32, width: u32) -> Self {
        Self { margin, width }
    }
}
