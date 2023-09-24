use crate::extensions::canvas::r#impl::{Canvas, Draw};
use fontdue::{Font, FontSettings};
use once_cell::sync::Lazy;
use std::{collections::HashMap, fs, marker::PhantomData, sync::RwLock};

static FONT_CACHE: Lazy<RwLock<HashMap<String, Font>>> = Lazy::new(|| RwLock::new(HashMap::new()));

pub(crate) struct Text<'a> {
    content: String,
    phantom: &'a PhantomData<()>,
}

impl<'a> Draw for Text<'a> {
    type Target = Canvas;

    type Style = TextStyle<'a>;

    type Point = (i32, i32);

    fn draw(&mut self, target: &mut Self::Target, style: Self::Style, point: Self::Point) {}
}

pub(crate) struct TextStyle<'a> {
    font: &'a Font,
    size: f32,
}

impl<'a> TextStyle<'a> {
    pub(crate) fn with_path(path: &str, size: f32) -> Self {
        let font = FONT_CACHE.read().ok().and_then(|r| r.get(path));

        let font = match font {
            Some(font) => font,
            None => {
                let data = fs::read(path).unwrap();
                let font = Font::from_bytes(data, FontSettings::default()).unwrap();
                FONT_CACHE
                    .write()
                    .ok()
                    .and_then(|w| w.insert(path.to_string(), font));

                FONT_CACHE.read().ok().and_then(|r| r.get(path)).unwrap()
            }
        };

        Self { font, size }
    }
}
