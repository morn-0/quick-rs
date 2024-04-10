pub use quickjs_sys as sys;

pub mod context;
pub mod error;
pub mod function;
pub mod module;
pub mod runtime;
pub mod value;

#[test]
fn test() {
    use crate::{function::Function, module::Module};

    let runtime = runtime::Runtime::default();
    let context = context::Context::from(&runtime);

    let script = r#"
function main() {
    let buffer = new ArrayBuffer(10);
    let array = new Uint8Array(buffer);
    for (var i = 0; i < array.length; i++) {
        array[i] = i * 10;
    }
    return array;
}

main();
"#;
    let val = context.eval_global(script, "main").unwrap();
    let mut buffer = val.property("buffer").unwrap();
    let buffer = buffer.to_buffer_mut::<u8>().unwrap();
    println!("{:?}", buffer);
    buffer[0] = 42;

    let script = r#"
export function main(uint8) {
    uint8[1] = 43;
    return uint8;
}
"#;
    let value = context.eval_module(script, "_main").unwrap();
    let module = Module::new(value).unwrap();

    let value = module.get("main").unwrap();
    let function = Function::new(value).unwrap();

    let value = function.call(None, vec![val]).unwrap();
    println!(
        "{:?}",
        value.property("buffer").unwrap().to_buffer::<u8>().unwrap()
    );
}
