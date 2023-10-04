use crate::extensions::canvas::r#impl::{Canvas, Paint};
use compact_str::{CompactString, ToCompactString};
use fontdue::{
    layout::{CoordinateSystem, GlyphPosition, Layout, LayoutSettings},
    Font, FontSettings,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, sync::RwLock};
use tiny_skia::{PixmapPaint, PixmapRef, Transform};

#[rustfmt::skip]
static FONT_CACHE: Lazy<RwLock<HashMap<CompactString, Font>>> = Lazy::new(|| RwLock::new(HashMap::new()));

#[derive(Deserialize, Serialize)]
pub(crate) struct Text {
    content: CompactString,
}

impl Text {
    pub fn new(content: impl AsRef<str> + ToCompactString) -> Self {
        Self {
            content: content.to_compact_string(),
        }
    }
}

impl Paint for Text {
    type Target = Canvas;

    type Style = TextStyle;

    type Point = (i32, i32);

    fn draw(&mut self, target: &mut Self::Target, style: Self::Style, point: Self::Point) {
        fn compute_dim(layout: &Layout) -> (usize, usize) {
            let (mut x1, mut y1, mut x2, mut y2): (i32, i32, i32, i32) = (0, 0, 0, 0);

            for pos in layout.glyphs() {
                x1 = x1.min(pos.x as i32);
                y1 = y1.min(pos.y as i32);
                x2 = x2.max(pos.x as i32 + pos.width as i32);
                y2 = y2.max(pos.y as i32 + pos.height as i32);
            }

            (1 + (x2 - x1) as usize, (y2 - y1) as usize)
        }

        if !FONT_CACHE
            .read()
            .map(|c| c.contains_key(&style.font))
            .unwrap_or(false)
        {
            let mut write = match FONT_CACHE.write() {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("{e}");
                    return;
                }
            };

            let bytes = match fs::read(style.font.as_str()) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("{e}");
                    return;
                }
            };

            let font = match Font::from_bytes(bytes.as_slice(), FontSettings::default()) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("{e}");
                    return;
                }
            };

            write.insert(style.font.clone(), font);
        };

        let read = match FONT_CACHE.read() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{e}");
                return;
            }
        };

        let font = match read.get(&style.font) {
            Some(f) => f,
            None => return,
        };

        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            ..Default::default()
        });
        layout.append(
            &[font],
            &fontdue::layout::TextStyle::new(&self.content, style.size, 0),
        );

        let dim = compute_dim(&layout);

        let mut glyphs: Vec<Vec<u8>> = Vec::with_capacity(self.content.len());
        self.content.chars().for_each(|c| {
            let (_, bitmap) = font.rasterize(c, style.size);
            glyphs.push(bitmap);
        });

        let mut bitmap: Vec<u8> = vec![0; dim.0 * dim.1];
        for (pos, char) in std::iter::zip(layout.glyphs(), &glyphs) {
            let GlyphPosition {
                x,
                y,
                width,
                height,
                ..
            } = pos;

            let x = *x as usize;
            let y = *y as usize;

            let width = x + width;
            let height = y + height;

            let mut i = 0;
            for y in y..height {
                for x in x..width {
                    let index = y * dim.0 + x;

                    if index < bitmap.len() {
                        bitmap[index] = char[i];
                    }

                    i += 1;
                }
            }
        }

        let mut rgba_bitmap: Vec<u8> = Vec::with_capacity(bitmap.len() * 4);
        for i in &bitmap {
            rgba_bitmap.extend([0, 0, 0, *i].iter());
        }

        if let Some(text) = PixmapRef::from_bytes(&rgba_bitmap, dim.0 as u32, dim.1 as u32) {
            target.pixmap.draw_pixmap(
                point.0,
                point.1,
                text,
                &PixmapPaint::default(),
                Transform::default(),
                None,
            );
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub(crate) struct TextStyle {
    font: CompactString,
    size: f32,
}

impl TextStyle {
    pub fn new(font: impl AsRef<str> + ToCompactString, size: f32) -> Self {
        Self {
            font: font.to_compact_string(),
            size,
        }
    }
}
