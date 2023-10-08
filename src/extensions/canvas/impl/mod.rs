use std::error::Error;
use tiny_skia::{Color, Pixmap, Rect, Transform};

pub(crate) mod barcode;
pub(crate) mod qrcode;
pub(crate) mod text;

#[derive(Clone, Copy)]
pub(crate) struct Point {
    x: i32,
    y: i32,
}

impl Into<Point> for (i32, i32) {
    fn into(self) -> Point {
        Point {
            x: self.0,
            y: self.1,
        }
    }
}

pub(crate) trait Paint {
    type Target;
    type Style;

    fn draw(&mut self, target: &mut Self::Target, style: Self::Style, point: Point);
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

#[test]
fn test() {
    use crate::extensions::canvas::r#impl::{
        barcode::{BarStyle, Barcode},
        qrcode::{QrStyle, Qrcode},
        text::{Text, TextStyle},
    };
    use std::{fs, time::Instant};

    let now = Instant::now();
    let mut canvas = Canvas::new(1920, 1080).unwrap();

    let text_style = TextStyle::new("LXGWWenKai-Regular.ttf", 42.0);
    let qr_style = QrStyle::new(0, 300);
    let bar_style = BarStyle::new(120, 4);

    Text::new("遥想公瑾当年，小乔初嫁了，雄姿英发。").draw(
        &mut canvas,
        text_style.clone(),
        (0, 0).into(),
    );
    Text::new("羽扇纶巾，谈笑间、樯橹灰飞烟灭。").draw(
        &mut canvas,
        text_style.clone(),
        (0, 50).into(),
    );
    Text::new("故国神游，多情应笑我，早生华发。").draw(
        &mut canvas,
        text_style.clone(),
        (0, 100).into(),
    );
    Text::new("人间如梦，一尊还酹江月。").draw(&mut canvas, text_style.clone(), (0, 150).into());
    Qrcode::new("人间如梦，一尊还酹江月。").draw(&mut canvas, qr_style, (0, 250).into());
    Barcode::new("1234567890").draw(&mut canvas, bar_style, (0, 600).into());

    fs::write("test.png", canvas.to_png_bytes().unwrap()).unwrap();
    println!("usage {}", now.elapsed().as_millis());
}
