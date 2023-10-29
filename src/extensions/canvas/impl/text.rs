use crate::extensions::canvas::r#impl::{Canvas, Paint};
use compact_str::{format_compact, CompactString, ToCompactString};
use fontdue::{
    layout::{CoordinateSystem, Layout, LayoutSettings},
    Font, FontSettings,
};
use once_cell::sync::Lazy;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::{fs, sync::RwLock};

#[rustfmt::skip]
static FONT_CACHE: Lazy<RwLock<FxHashMap<CompactString, Font>>> = Lazy::new(|| RwLock::new(FxHashMap::default()));

#[derive(Deserialize, Serialize)]
pub(crate) struct Text {
    content: CompactString,
    #[serde(skip)]
    glyph_cache: FxHashMap<CompactString, Vec<u8>>,
}

impl Text {
    pub fn new(content: impl AsRef<str> + ToCompactString) -> Self {
        Self {
            content: content.to_compact_string(),
            glyph_cache: FxHashMap::default(),
        }
    }
}

impl Paint for Text {
    type Target = Canvas;
    type Style = TextStyle;
    type Point = (f32, f32);

    fn draw(&mut self, target: &mut Self::Target, style: Self::Style, point: Self::Point) {
        init_font(&style.font);

        let lock = match FONT_CACHE.read() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{e}");
                return;
            }
        };
        let font = match lock.get(&style.font) {
            Some(f) => f,
            None => return,
        };

        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: point.0,
            y: point.1,
            ..Default::default()
        });
        layout.append(
            &[font],
            &fontdue::layout::TextStyle::new(&self.content, style.size, 0),
        );

        let width = target.width as usize;
        let data = target.pixmap.data_mut();
        let data_ptr = data.as_mut_ptr();

        for glyph in layout.glyphs() {
            let hash = format_compact!("{}{}{}", style.font, glyph.parent, style.size);
            let char = self.glyph_cache.entry(hash).or_insert_with(|| {
                let (_, char) = font.rasterize(glyph.parent, style.size);
                char
            });

            let glyph_x = glyph.x as usize;
            let glyph_y = glyph.y as usize;

            let base_index = (glyph_y * width + glyph_x) * 4;

            for gx in 0..glyph.width {
                for gy in 0..glyph.height {
                    let index = gy * glyph.width + gx;
                    let coverage = unsafe { 255 - char.get_unchecked(index) };

                    let index = base_index + (gy * width + gx) * 4;
                    if index < data.len() {
                        unsafe {
                            let ptr = data_ptr.add(index);
                            *ptr = coverage;
                            *ptr.add(1) = coverage;
                            *ptr.add(2) = coverage;
                            *ptr.add(3) = 255;
                        }
                    }
                }
            }
        }
    }
}

impl Text {
    pub(crate) fn measure(&self, style: TextStyle) -> i32 {
        init_font(&style.font);

        let lock = match FONT_CACHE.read() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{e}");
                return -1;
            }
        };
        let font = match lock.get(&style.font) {
            Some(f) => f,
            None => return -1,
        };

        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            ..Default::default()
        });
        layout.append(
            &[font],
            &fontdue::layout::TextStyle::new(&self.content, style.size, 0),
        );

        let (mut x1, mut x2) = (0, 0);
        for pos in layout.glyphs() {
            x1 = x1.min(pos.x as i32);
            x2 = x2.max(pos.x as i32 + pos.width as i32);
        }

        x2 - x1
    }
}

#[inline(always)]
fn init_font(font: &str) {
    if !FONT_CACHE
        .read()
        .map(|c| c.contains_key(font))
        .unwrap_or(false)
    {
        let mut lock = match FONT_CACHE.write() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("{e}");
                return;
            }
        };

        let bytes = match fs::read(font) {
            Ok(b) => Some(b),
            Err(e) => {
                eprintln!("{e}");
                None
            }
        };

        if let Some(bytes) = bytes {
            let new_font = Font::from_bytes(bytes.as_slice(), FontSettings::default());

            match new_font {
                Ok(f) => {
                    lock.insert(font.to_compact_string(), f);
                }
                Err(e) => {
                    eprintln!("{e}");
                }
            };
        }
    };
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
