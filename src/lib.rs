use std::sync::Arc;

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
    for _ in 0..1000 {
        let i = Box::new(1);
        let _value = context
            .eval_global(
                r#"var data = JSON.stringify(print.barcode("1234567890", 100, 1));data;"#,
                "test",
            )
            .unwrap();
        // println!("{}", value.tag());
        // println!("{:?}", value.to_string());
    }
    println!("{}", now.elapsed().as_millis());
}
