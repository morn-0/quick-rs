use crate::extensions::canvas::r#impl::{Canvas, Paint};
use ab_glyph::{point as ab_point, Font, FontArc, Glyph, GlyphId, PxScale, ScaleFont};
use compact_str::{CompactString, ToCompactString};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::fs;
use swash::{
    shape::ShapeContext,
    text::{Language, Script},
    FontRef,
};

#[rustfmt::skip]
static FONT_BUF: Lazy<RwLock<FxHashMap<CompactString, &'static [u8]>>> = Lazy::new(|| RwLock::new(FxHashMap::default()));

#[rustfmt::skip]
static FONT_REF: Lazy<RwLock<FxHashMap<CompactString, FontRef>>> = Lazy::new(|| RwLock::new(FxHashMap::default()));

#[rustfmt::skip]
static FONT_ARC: Lazy<RwLock<FxHashMap<CompactString, FontArc>>> = Lazy::new(|| RwLock::new(FxHashMap::default()));

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
    type Point = (f32, f32);

    fn draw(&mut self, target: &mut Self::Target, mut style: Self::Style, point: Self::Point) {
        init_font(&style.font);

        let font_ref = FONT_REF.read();
        let font_ref = match font_ref.get(&style.font) {
            Some(f) => f,
            None => {
                return;
            }
        };

        let mut shape_context = ShapeContext::new();
        let mut shaper = shape_context
            .builder(*font_ref)
            .size(style.size)
            .script(Script::Han)
            .language(Language::parse("zh"))
            .build();

        let font_arc = FONT_ARC.read();
        let font_arc = match font_arc.get(&style.font) {
            Some(f) => f,
            None => {
                return;
            }
        };

        if let Some(unit) = font_arc.units_per_em() {
            style.size = style.size * font_arc.height_unscaled() / unit;
        }

        let scaled_font = Font::as_scaled(&font_arc, style.size);

        let (mut base_x, mut base_y) = (point.0, point.1);
        base_y += scaled_font.ascent();

        let scale = PxScale::from(style.size);
        let data = target.pixmap.data_mut();
        let data_ptr = data.as_mut_ptr();

        shaper.add_str(&self.content);
        shaper.shape_with(|c| {
            for glyph in c.glyphs {
                let outlined = scaled_font.outline_glyph(Glyph {
                    id: GlyphId(glyph.id),
                    scale,
                    position: ab_point(base_x, base_y),
                });

                if let Some(outlined) = outlined {
                    let bounds = outlined.px_bounds();

                    outlined.draw(|x, y, c| {
                        let x = (x as f32 + bounds.min.x) as u32;
                        let y = (y as f32 + bounds.min.y) as u32;
                        let c = (255.0 * (1.0 - c)) as u8;

                        let index = ((x + y * target.width) * 4) as usize;
                        if index + 2 < data.len() {
                            unsafe {
                                let ptr = data_ptr.add(index);

                                *ptr = c;
                                *ptr.add(1) = c;
                                *ptr.add(2) = c;
                            }
                        }
                    });
                }

                base_x += glyph.advance;
            }
        });
    }
}

impl Text {
    pub(crate) fn measure(&self, style: TextStyle) -> i32 {
        init_font(&style.font);

        let mut width = 0;

        let font_ref = FONT_REF.read();
        let font_ref = match font_ref.get(&style.font) {
            Some(f) => f,
            None => {
                return width;
            }
        };

        let mut shape_context = ShapeContext::new();
        let mut shaper = shape_context
            .builder(*font_ref)
            .size(style.size)
            .script(Script::Han)
            .language(Language::parse("zh"))
            .build();

        shaper.add_str(&self.content);
        shaper.shape_with(|c| width += c.glyphs.iter().map(|g| g.advance).sum::<f32>() as i32);

        width
    }
}

#[inline(always)]
fn init_font(font: &str) {
    if !FONT_BUF.read().contains_key(font) {
        let mut font_buf = FONT_BUF.write();
        let mut font_ref = FONT_REF.write();
        let mut font_arc = FONT_ARC.write();

        match fs::read(font) {
            Ok(b) => {
                let b = Box::leak(b.into_boxed_slice());
                let r#ref = FontRef::from_index(b, 0);
                let arc = FontArc::try_from_slice(b);

                match (arc, r#ref) {
                    (Ok(a), Some(f)) => {
                        font_ref.insert(font.to_compact_string(), f);
                        font_arc.insert(font.to_compact_string(), a);
                        font_buf.insert(font.to_compact_string(), b);
                    }
                    (Err(e), _) => {
                        eprintln!("{e}");
                    }
                    _ => {}
                };
            }
            Err(e) => {
                eprintln!("{e}");
            }
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
