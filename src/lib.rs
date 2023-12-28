pub use quickjs_sys as qjs;

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
async function main() {
    return "test";
}

main();
"#;
    let value = context.eval_global(script, "main");

    println!("{:?}", value.map(|v| v.tag()).unwrap());
}
