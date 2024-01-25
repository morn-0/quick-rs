pub use quickjs_sys as sys;

pub mod context;
pub mod error;
pub mod function;
pub mod module;
pub mod promise;
pub mod runtime;
pub mod util;
pub mod value;

#[test]
fn test() {
    {
        let runtime = runtime::Runtime::new(None);
        let context = context::Context::from(&runtime);
        let context = std::rc::Rc::new(context);

        for _ in 0..100 {
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

                        let value = ctx.eval_global(script, "main").unwrap();
                        let value = promise::Promise::new(value).await.unwrap();
                        println!("{}", value.to_string().unwrap());
                    })
                },
                context.clone(),
            );
        }
    }
}
