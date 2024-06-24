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
    let nb = context.make_buffer(vec![1, 2, 3]).unwrap();

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

    context.make_function(None, "fibonacci", 2, |ctx, args| {
        fn fibonacci(n: u32) -> u64 {
            if n == 0 {
                return 0;
            } else if n == 1 {
                return 1;
            } else {
                return fibonacci(n - 1) + fibonacci(n - 2);
            }
        }

        let v = fibonacci(args[0].to_i32().unwrap() as u32) as i32;
        let string = args[1].to_string().unwrap();
        drop(string);

        println!("{v}");
        ctx.make_int(v)
    });

    let script = r#"
export function main(uint8, buffer, text) {
    uint8[1] = 43;

    return {
        "data": uint8,
        "array": [fibonacci(30, text), 1, "2", text],
        "buffer": buffer
    };
}
"#;
    let value = context.eval_module(script, "_main").unwrap();
    let module = Module::new(value).unwrap();

    let value = module.get("main").unwrap();
    let function = Function::new(value).unwrap();

    loop {
        let now = std::time::Instant::now();
        let value = function
            .call(
                None,
                vec![
                    val.clone(),
                    nb.clone(),
                    context
                        .make_string(include_str!(
                            "/Users/morning/Downloads/quick-rs/randomfile.txt"
                        ))
                        .unwrap(),
                ],
            )
            .unwrap();
        println!(
            "{}ms, {}",
            now.elapsed().as_millis(),
            value.to_json().unwrap().len()
        );
    }
}
