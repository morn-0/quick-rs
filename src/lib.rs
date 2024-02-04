pub use quickjs_sys as sys;

pub mod context;
pub mod error;
pub mod function;
pub mod module;
pub mod runtime;
pub mod util;
pub mod value;

#[test]
fn test() {
    let runtime = runtime::Runtime::default();
    let context = context::Context::from(&runtime);

    let script = r#"
function main() {
    let buffer = new ArrayBuffer(10);
    let array = new Uint8Array(buffer);
    for (var i = 0; i < array.length; i++) {
        array[i] = i * 10;
    }
    return array.buffer;
}

main();
"#;
    let mut value = context.eval_global(script, "main").unwrap();
    let buffer = value.buffer_mut::<u8>().unwrap();
    println!("{:?}", buffer);
}
