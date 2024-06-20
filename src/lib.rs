pub use quickjs_sys as sys;

pub mod context;
pub mod error;
pub mod function;
pub mod module;
pub mod runtime;
pub mod value;

fn main() {
    use crate::{context::Context, function::Function, module::Module, runtime::Runtime};

    let runtime = Runtime::default();
    let context = Context::from(&runtime);
    let nb = context.make_buffer(vec![]).unwrap();

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
export function main(uint8, buffer, text) {
    uint8[1] = 43;
    return {
        "data": uint8,
        "array": [1, "2", text]
    };
}
"#;
    let value = context.eval_module(script, "_main").unwrap();
    let module = Module::new(value).unwrap();

    let value = module.get("main").unwrap();
    let function = Function::new(value).unwrap();

    loop {
        let value = function
            .call(
                None,
                vec![
                    val.clone(),
                    nb.clone(),
                    context.make_string("举头望明月，低头思故乡。").unwrap(),
                ],
            )
            .unwrap();
        println!("{}", value.to_json().unwrap());
        println!(
            "{:?}",
            value
                .property("data")
                .unwrap()
                .property("buffer")
                .unwrap()
                .to_buffer::<u8>()
                .unwrap()
        );

        let array = value.property("array").unwrap().to_array().unwrap();
        println!("{}", array.first().unwrap().to_i32().unwrap());
        println!("{}", array.get(1).unwrap().to_string().unwrap());
    }
}
