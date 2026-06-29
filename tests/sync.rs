use quick_rs::{Context, Error, Function, Module, Runtime};

fn context() -> (Runtime, Context) {
    let runtime = Runtime::new();
    let context = runtime.context();
    (runtime, context)
}

#[test]
fn eval_arithmetic() {
    let (_rt, ctx) = context();
    let value = ctx.eval_global("1 + 2 * 3", "t").unwrap();
    assert_eq!(value.to_i32().unwrap(), 7);
}

#[test]
fn eval_types() {
    let (_rt, ctx) = context();
    assert!(ctx.eval_global("true", "t").unwrap().to_bool().unwrap());
    assert_eq!(ctx.eval_global("3.5", "t").unwrap().to_f64().unwrap(), 3.5);
    assert_eq!(
        ctx.eval_global("'hi'", "t").unwrap().to_string().unwrap(),
        "hi"
    );
}

#[test]
fn string_unicode_roundtrip() {
    let (_rt, ctx) = context();
    let text = "héllo 世界 🌍 \u{1F680}";
    let value = ctx.make_string(text).unwrap();
    assert_eq!(value.to_string().unwrap(), text);
}

#[test]
fn object_properties() {
    let (_rt, ctx) = context();
    let obj = ctx.make_object();
    obj.set_property("n", ctx.make_int(42)).unwrap();
    obj.set_property("s", ctx.make_string("hi").unwrap())
        .unwrap();
    assert_eq!(obj.get_property("n").unwrap().to_i32().unwrap(), 42);
    assert_eq!(obj.get_property("s").unwrap().to_string().unwrap(), "hi");
}

#[test]
fn json_stringify_nested() {
    let (_rt, ctx) = context();
    let value = ctx
        .eval_global("({ a: 1, b: [2, 3], c: 'x' })", "t")
        .unwrap();
    assert_eq!(value.to_json().unwrap(), r#"{"a":1,"b":[2,3],"c":"x"}"#);
}

#[test]
fn make_array_roundtrip() {
    let (_rt, ctx) = context();
    let array = ctx.make_array([
        ctx.make_int(1),
        ctx.make_string("two").unwrap(),
        ctx.make_bool(true),
    ]);

    assert_eq!(array.to_json().unwrap(), r#"[1,"two",true]"#);

    let items = array.to_array().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].to_i32().unwrap(), 1);
    assert_eq!(items[1].to_string().unwrap(), "two");
    assert!(items[2].to_bool().unwrap());

    // 是真正的 JS Array
    ctx.global().set_property("a", array).unwrap();
    assert!(ctx
        .eval_global("Array.isArray(a)", "t")
        .unwrap()
        .to_bool()
        .unwrap());
}

#[test]
fn make_array_empty() {
    let (_rt, ctx) = context();
    let array = ctx.make_array(Vec::new());
    assert_eq!(array.to_json().unwrap(), "[]");
    assert_eq!(array.to_array().unwrap().len(), 0);
}

#[test]
fn array_iteration() {
    let (_rt, ctx) = context();
    let array = ctx.eval_global("[10, 20, 30]", "t").unwrap();
    let sum: i32 = array
        .to_array()
        .unwrap()
        .iter()
        .map(|v| v.to_i32().unwrap())
        .sum();
    assert_eq!(sum, 60);
}

#[test]
fn rust_function_called_from_js() {
    let (_rt, ctx) = context();
    let add = ctx.make_function(2, |ctx, _this, args| {
        let a = args.first().map(|v| v.to_i32()).transpose()?.unwrap_or(0);
        let b = args.get(1).map(|v| v.to_i32()).transpose()?.unwrap_or(0);
        Ok(ctx.make_int(a + b))
    });
    ctx.global().set_property("add", add).unwrap();
    assert_eq!(
        ctx.eval_global("add(3, 4)", "t").unwrap().to_i32().unwrap(),
        7
    );
}

#[test]
fn rust_function_this_binding() {
    let (_rt, ctx) = context();
    let obj = ctx.make_object();
    obj.set_property("v", ctx.make_int(7)).unwrap();
    let get = ctx.make_function(0, |_ctx, this, _args| this.get_property("v"));
    obj.set_property("get", get).unwrap();
    ctx.global().set_property("obj", obj).unwrap();
    assert_eq!(
        ctx.eval_global("obj.get()", "t").unwrap().to_i32().unwrap(),
        7
    );
}

#[test]
fn rust_function_error_throws_in_js() {
    let (_rt, ctx) = context();
    let boom = ctx.make_function(0, |_ctx, _this, _args| Err(Error::Argument("boom")));
    ctx.global().set_property("boom", boom).unwrap();
    let caught = ctx
        .eval_global("try { boom() } catch (e) { e.message }", "t")
        .unwrap();
    assert_eq!(caught.to_string().unwrap(), "boom");
}

#[test]
fn array_buffer_roundtrip() {
    let (_rt, ctx) = context();
    let buffer = ctx.make_buffer(vec![1u8, 2, 3, 4]).unwrap();
    ctx.global().set_property("buf", buffer).unwrap();
    ctx.eval_global("new Uint8Array(buf)[0] = 99", "t").unwrap();

    let buffer = ctx.global().get_property("buf").unwrap();
    assert_eq!(buffer.to_buffer::<u8>().unwrap(), &[99, 2, 3, 4]);
}

#[test]
fn module_export_and_call() {
    let (_rt, ctx) = context();
    let module = Module::new(
        ctx.eval_module("export function twice(x) { return x * 2; }", "m")
            .unwrap(),
    )
    .unwrap();
    let twice = Function::new(module.get("twice").unwrap());
    let arg = ctx.make_int(21);
    assert_eq!(
        twice.call(None, &[arg.borrow()]).unwrap().to_i32().unwrap(),
        42
    );
}

#[test]
fn exception_has_name_and_message() {
    let (_rt, ctx) = context();
    match ctx.eval_global("null.x", "t").unwrap_err() {
        Error::Exception(e) => {
            assert_eq!(e.name, "TypeError");
            assert!(!e.message.is_empty());
        }
        other => panic!("expected exception, got {other:?}"),
    }
}

#[test]
fn type_mismatch_is_error() {
    let (_rt, ctx) = context();
    let value = ctx.make_string("not a number").unwrap();
    assert!(matches!(value.to_i32(), Err(Error::Type { .. })));
}

#[test]
fn to_number_coerces_and_narrows() {
    let (_rt, ctx) = context();
    let num = |src: &str| ctx.eval_global(src, "t").unwrap();

    // ToNumber 强转：字符串/bool/null 都转
    assert_eq!(num("'42'").to_number::<i32>().unwrap(), 42);
    assert_eq!(num("true").to_number::<f64>().unwrap(), 1.0);
    assert_eq!(num("null").to_number::<i32>().unwrap(), 0);

    // INT tag 与 FLOAT64 tag 都能读（补 to_i32/to_f64 单一 tag 的缺口）
    assert_eq!(ctx.make_int(7).to_number::<f64>().unwrap(), 7.0);
    assert_eq!(ctx.make_float(3.9).to_number::<i32>().unwrap(), 3);

    // 整型按 Rust 饱和收窄，不是 JS 回绕（JS (2**32+1)|0 == 1）
    assert_eq!(num("2**32 + 1").to_number::<i32>().unwrap(), i32::MAX);

    // 无法 ToNumber 的值抛异常（Symbol）
    assert!(matches!(
        num("Symbol()").to_number::<f64>(),
        Err(Error::Exception(_))
    ));
}

#[test]
fn value_clone_is_independent() {
    let (_rt, ctx) = context();
    let a = ctx.make_string("shared").unwrap();
    let b = a.clone();
    drop(a);
    assert_eq!(b.to_string().unwrap(), "shared");
}

#[test]
fn string_with_interior_nul() {
    let (_rt, ctx) = context();
    let value = ctx.make_string("a\0b").unwrap();
    assert_eq!(value.to_string().unwrap(), "a\0b");
}
