use std::{mem::ManuallyDrop, time::Duration};

pub use quickjs_sys as sys;
use runtime::COMPIO;

pub mod context;
pub mod error;
pub mod function;
pub mod module;
pub mod promise;
pub mod runtime;
pub mod value;

#[test]
fn main() {
    use crate::{
        context::Context, function::Function, module::Module, promise::Promise, runtime::Runtime,
    };
    use std::rc::Rc;

    let runtime = Runtime::default();
    let context = Rc::new(Context::from(&runtime));
    context.make_function(None, "print", 1, |ctx, argv| {
        let v = argv.get(0).unwrap().to_string().unwrap();
        println!("{v}");
        ctx.make_undefined()
    });
    context.make_function(None, "setTimeout", 2, |ctx, argv| {
        COMPIO.with(|v| {
            v.spawn(async move {
                let millis = argv.get(1).unwrap().to_i32().unwrap();
                println!("{millis}");
                compio::time::sleep(std::time::Duration::from_millis(millis as u64)).await;

                let function =
                    Function::new(ManuallyDrop::into_inner(argv.get(0).unwrap().clone())).unwrap();
                function
                    .call(
                        None,
                        argv[2..]
                            .iter()
                            .map(|v| ManuallyDrop::into_inner(v.clone()))
                            .collect(),
                    )
                    .unwrap();
            });
        });

        std::thread::sleep(Duration::from_secs(1));
        ctx.make_undefined()
    });

    runtime.event_loop(
        |ctx| {
            let script = r#"
            async function async1() {
                return 1;
            }

            async function async2() {
                return await async1();
            }

            async function async3() {
                return await async2();
            }

            async function async4() {
                return await async3();
            }

            async function main() {
                async1();
                async2();
                async3();
                async4();

                setTimeout(() => {
                    print("setTimeout");
                }, 1000);
                return await async4();
            }

            main()
            "#;
            let value = ctx.eval_global(script, "main").unwrap();
            let promise = promise::Promise::new(value);

            Box::pin(async move {
                let now = std::time::Instant::now();
                compio::runtime::spawn(async move {
                    println!("cnm, 5ms");
                });

                let value = promise.await.unwrap().to_i32().unwrap();
                println!("{value}");
                compio::time::sleep(std::time::Duration::from_secs(5)).await;
                println!("{value}, {}ms", now.elapsed().as_millis());
            })
        },
        context.clone(),
    );

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

    for _ in 0..10 {
        let now = std::time::Instant::now();
        let value = function
            .call(
                None,
                vec![
                    val.clone(),
                    nb.clone(),
                    context.make_string("test").unwrap(),
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
