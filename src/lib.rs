pub use quickjs_sys as sys;

pub mod context;
pub mod error;
pub mod function;
pub mod module;
pub mod promise;
pub mod runtime;
pub mod value;

#[test]
fn test() {
    use crate::{function::Function, module::Module};
    use std::rc::Rc;

    let runtime = runtime::Runtime::default();

    let context = context::Context::from(&runtime);
    runtime.event_loop(
        |ctx| {
            Box::pin(async move {
                let script = r#"
async function main() {
    return await test1();
}

async function test1() {
    return await test2();
}

async function test2() {
    return await test3();
}

async function test3() {
    return await test4();
}

async function test4() {
    return "test4";
}

main();
"#;

                let val = ctx.eval_global(script, "main").unwrap();

                let mut ts = vec![];
                for _ in 0..10 {
                    let val = val.clone();
                    let t = compio::runtime::spawn(async move {
                        let val = promise::Promise::new(val).await.unwrap();
                        println!("abc: {:?}", val.to_string().unwrap());
                    });
                    ts.push(t);
                }

                compio::time::sleep(std::time::Duration::from_secs(1)).await;
            })
        },
        Rc::new(context),
    );
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
    return {
        "data": uint8,
        "array": [1, "2"]
    };
}
"#;
    let value = context.eval_module(script, "_main").unwrap();
    let module = Module::new(value).unwrap();

    let value = module.get("main").unwrap();
    let function = Function::new(value).unwrap();

    for _ in 0..10 {
        let value = function.call(None, vec![val.clone()]).unwrap();
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
