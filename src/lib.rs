pub mod context;
pub mod error;
pub(crate) mod extensions;
pub mod function;
pub mod module;
pub mod runtime;
pub mod util;
pub mod value;

#[test]
fn test() {
    let runtime = runtime::Runtime::new();
    let context = context::Context::from(&runtime);

    let now = std::time::Instant::now();

    for _ in 0..1 {
        let value = context
            .eval_global(
                r#"
                var canvas = _canvas.invoke(JSON.stringify({
                    "call": 0,
                    "style": {
                        "width": 1920,
                        "height": 1080
                    }
                }));

                _canvas.invoke(JSON.stringify({
                    "call": 2,
                    "target": canvas,
                    "paint": {
                        "content": "一点浩然气，千里快哉风。"
                    },
                    "style": {
                        "font": "/home/arch/quick-rs/LXGWWenKai-Regular.ttf",
                        "size": 32
                    },
                    "point": [15, 15]
                }));

                _canvas.invoke(JSON.stringify({
                    "call": 1,
                    "target": canvas,
                    "paint": {
                        "path": "test.png"
                    }
                }));

                _canvas.invoke(JSON.stringify({
                    "call": -1,
                    "target": canvas
                }));

                canvas;
                "#,
                "test",
            )
            .unwrap();
        println!("{:?}", value.to_string());
    }

    println!("{}", now.elapsed().as_millis());
}
