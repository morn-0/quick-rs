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
                let width = 3840;
                let height = 2160;

                let content = "A time will come for me to ride the wind and cleace the waves, i'll set my cloud white sail and cross the sea which waves.";
                let font = "/home/arch/quick-rs/learning_curve_regular_ot_tt.ttf";
                let size = 96;

                var canvas = _canvas.invoke(JSON.stringify({
                    "call": 0,
                    "style": {
                        "width": width,
                        "height": height
                    }
                }));

                for (let i = 0; i < 1000; i++) {
                    let text_width = _canvas.invoke(JSON.stringify({
                        "call": 3,
                        "paint": {
                            "content": content
                        },
                        "style": {
                            "font": font,
                            "size": size
                        }
                    }));
                    _canvas.invoke(JSON.stringify({
                        "call": 2,
                        "target": canvas,
                        "paint": {
                            "content": content
                        },
                        "style": {
                            "font": font,
                            "size": size
                        },
                        "point": [(width - text_width) / 2, (height - size) / 2]
                    }));
                }

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
