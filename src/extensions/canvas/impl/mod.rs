use tiny_skia::Pixmap;

pub(crate) mod text;

pub(crate) trait Draw {
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
        let pixmap = Pixmap::new(width, height)?;

        Some(Canvas {
            pixmap,
            width,
            height,
        })
    }
}
