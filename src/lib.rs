pub use quickjs_sys as sys;

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
    let context = Context::from(&runtime);

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

                return await async4();
            }

            main()
            "#;
            let value = ctx.eval_global(script, "main").unwrap();
            let promise = promise::Promise::new(value);

            Box::pin(async move {
                let value = promise.await.unwrap().to_i32().unwrap();
                println!("{value}");
            })
        },
        Rc::new(context),
    );
}
