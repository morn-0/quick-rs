pub use quickjs_sys as sys;

pub mod context;
pub mod error;
pub mod function;
pub mod loader;
pub mod module;
pub mod promise;
pub mod runtime;
pub mod value;

// #[test]
fn main() {
    use crate::{
        context::Context, function::Function, module::Module, promise::Promise, runtime::Runtime,
    };

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
    let mut buffer = val.get_property("buffer").unwrap();
    let buffer = buffer.to_buffer_mut::<u8>().unwrap();
    println!("{:?}", buffer);
    buffer[0] = 42;

    let obj = context.make_object();
    obj.set_property("num", context.make_int(30)).unwrap();
    obj.set_property("text", context.make_string("test").unwrap())
        .unwrap();

    let func = context.make_function(2, |ctx, this, args| {
        fn fibonacci(n: u32) -> u64 {
            if n == 0 {
                0
            } else if n == 1 {
                return 1;
            } else {
                return fibonacci(n - 1) + fibonacci(n - 2);
            }
        }

        println!("{}", this.tag());
        println!(
            "a: {}",
            this.get_property("textb").unwrap().to_string().unwrap()
        );

        let args = args.unwrap();
        let v = fibonacci(args[0].to_i32().unwrap() as u32) as i32;
        let string = args[1].to_string().unwrap();

        println!("{v}, {string}");
        ctx.make_int(v)
    });
    obj.set_property("fibonacci", func).unwrap();

    context.global().set_property("obj", obj).unwrap();

    let script = r#"
export function main(uint8, buffer, text) {
    uint8[1] = 43;

    obj.textb = "platform";
    return {
        "data": uint8,
        "array": [obj.fibonacci(obj.num, obj.text), 1, "2", text],
        "buffer": buffer
    };
}
"#;
    let value = context.eval_module(script, "_main").unwrap();
    let module = Module::new(value).unwrap();

    let value = module.get("main").unwrap();
    let function = Function::new(value);

    for _ in 0..100 {
        let now = std::time::Instant::now();
        let value = function
            .call(
                None,
                vec![
                    val.clone(),
                    nb.clone(),
                    context.make_string("testa").unwrap(),
                    context.make_bool(true),
                    context.make_int(32),
                    context.make_object(),
                    context.make_float(0.2),
                    context.make_null(),
                    context.make_undefined(),
                ],
            )
            .unwrap();
        println!(
            "{}ms, {}",
            now.elapsed().as_millis(),
            value.to_json().unwrap().len()
        );
    }

    let value = runtime.event_loop(
        |ctx| {
            let function = ctx.make_function(1, |ctx, _, args| {
                let string = args.unwrap().first().unwrap().to_string().unwrap();
                println!("{string}");
                ctx.make_undefined()
            });
            ctx.global().set_property("println", function).unwrap();

            let value = ctx
                .eval_global(
                    r#"
            println("hi")

            setTimeout(() => {
                println("1000")
            }, 1000)

            let id = setTimeout(() => {
                println("3000")
            }, 3000)

            setTimeout(async () => {
                let i = await recv("loop")
                println("recv, " + i)
                println("clearTimeout, " + id)
                clearTimeout(id)
                await send("test channel", "setTimeout test channel")
                println("5.send")
            }, 2999)
            send("test channel", "test channel")

            setTimeout(async () => {
                let i = 0;
                while (i <= 100) {
                    await send("loop", "send, " + i)
                    await sleep(1)
                    i += 1
                }
            }, 10)
            setTimeout(async () => {
                while (true) {
                    let i = await recv("loop")
                    println("recv, " + i)

                    if (i.indexOf("100") != -1) {
                        break
                    }
                }
            }, 3000)

            function sleep(ms) {
                return new Promise(resolve => setTimeout(resolve, ms))
            }

            async function main() {
                println("recv, " + await recv("test channel"))
                println("recv, " + await recv("test channel"))

                setTimeout(async () => {
                    println("before recv")
                    println("after recv")
                }, 3000)

                for (let i = 0; i < 5; i++) {
                    await sleep(1000)
                    println("sleep, " + i)
                }

                for (let i = 0; i < 10000; i++) {
                    setTimeout(() => {
                        println("setTimeout, " + i)
                    }, i)
                }

                return "main done";
            }

            main()
            "#,
                    "event_loop",
                )
                .unwrap();

            tokio::task::spawn_local(async move {
                let send = Function::new(ctx.global().get_property("send").unwrap());
                let v = send
                    .call(
                        None,
                        vec![ctx.make_string("loop").unwrap(), ctx.make_int(123456789)],
                    )
                    .unwrap();
                Promise::new(v).await;
                println!("loop ======================");
            });

            Box::pin(Promise::new(value))
        },
        context,
    );

    match value {
        Ok(v) => {
            println!("{:?}", v.to_string().unwrap());
        }
        Err(e) => {
            println!("{e}");
        }
    }
}
