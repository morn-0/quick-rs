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
    use {context::Context, function::Function, module::Module, runtime::Runtime};

    const SCRIPT: &str = r#"
function main(config) {
  if (Array.isArray(config.rules)) {
    config.rules = [...config.rules, "add"];
  }
  // console.log(config);
  config.proxies = ["111"];
  return config;
}
"#;

    let runtime = Runtime::new();
    let context = Context::from(&runtime);

    let script = format!(
        "{};export function _main(config) {{return JSON.stringify(main(JSON.parse(config)));}}",
        SCRIPT
    );
    let value = context.eval_module(&script, "_main").unwrap();
    let module = Module::new(value).unwrap();

    let value = module.get("_main").unwrap();
    let function = Function::new(value).unwrap();

    let string = context.new_string(r#"{"rules":[]}"#).unwrap();
    let value = function.call(vec![string]).unwrap();

    println!("{}", value.to_string().unwrap());
}
