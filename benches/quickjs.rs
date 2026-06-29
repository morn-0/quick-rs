//! quick-rs 各热点路径的性能基准（criterion）。
//!
//! 跑法：`cargo bench`（或 `cargo bench -- <名字过滤>`）。
//! 注意：默认开启 mimalloc + async feature，与生产配置一致。

use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pprof::criterion::{Output, PProfProfiler};
use quick_rs::{Context, Function, Runtime};

fn setup() -> (Runtime, Context) {
    let runtime = Runtime::new();
    let context = runtime.context();
    (runtime, context)
}

/// 值构造：mkval/字符串/对象。
fn values(c: &mut Criterion) {
    let (_rt, ctx) = setup();
    let mut group = c.benchmark_group("values");

    group.bench_function("make_int", |b| {
        b.iter(|| black_box(ctx.make_int(black_box(42))))
    });
    group.bench_function("make_float", |b| {
        b.iter(|| black_box(ctx.make_float(black_box(3.5))))
    });
    group.bench_function("make_string", |b| {
        b.iter(|| black_box(ctx.make_string(black_box("hello world")).unwrap()))
    });
    group.bench_function("make_object", |b| b.iter(|| black_box(ctx.make_object())));

    group.finish();
}

/// 属性读写（atom 分配/释放 + 值传递）。
fn properties(c: &mut Criterion) {
    let (_rt, ctx) = setup();
    let obj = ctx.make_object();
    obj.set_property("n", ctx.make_int(1)).unwrap();

    let mut group = c.benchmark_group("properties");
    group.bench_function("set_property", |b| {
        b.iter(|| {
            obj.set_property(black_box("n"), ctx.make_int(black_box(7)))
                .unwrap()
        })
    });
    group.bench_function("get_property", |b| {
        b.iter(|| black_box(obj.get_property(black_box("n")).unwrap()))
    });
    group.finish();
}

/// 跨边界调用：Rust→JS、JS→Rust。
fn calls(c: &mut Criterion) {
    let (_rt, ctx) = setup();

    // 纯 JS 函数：测 Rust→JS 一次往返 + 值编组。
    let js_fn = Function::new(ctx.eval_global("(x => x + 1)", "b").unwrap());
    let arg = ctx.make_int(41);

    // JS 函数内部调用一个 Rust 闭包：测往返 + trampoline。
    let add = ctx.make_function(2, |c, _t, a| {
        let x = a.first().map(|v| v.to_i32()).transpose()?.unwrap_or(0);
        let y = a.get(1).map(|v| v.to_i32()).transpose()?.unwrap_or(0);
        Ok(c.make_int(x + y))
    });
    ctx.global().set_property("add", add).unwrap();
    let caller = Function::new(ctx.eval_global("(() => add(1, 2))", "b").unwrap());

    let mut group = c.benchmark_group("calls");
    group.bench_function("rust_calls_js", |b| {
        b.iter(|| black_box(js_fn.call(None, black_box(&[arg.borrow()])).unwrap()))
    });
    group.bench_function("js_calls_rust", |b| {
        b.iter(|| black_box(caller.call(None, &[]).unwrap()))
    });
    group.finish();
}

/// 求值：编译+执行小表达式；纯 JS 计算（递归 fib）。
fn eval(c: &mut Criterion) {
    let (_rt, ctx) = setup();
    let fib = Function::new(
        ctx.eval_global(
            "(function f(n){ return n < 2 ? n : f(n-1) + f(n-2); })",
            "b",
        )
        .unwrap(),
    );
    let n = ctx.make_int(25);

    let mut group = c.benchmark_group("eval");
    group.bench_function("eval_expr", |b| {
        b.iter(|| black_box(ctx.eval_global(black_box("1 + 2 * 3"), "b").unwrap()))
    });
    group.bench_function("js_fib_25", |b| {
        b.iter(|| black_box(fib.call(None, &[n.borrow()]).unwrap()))
    });
    group.finish();
}

/// 值 → Rust 的转换：JSON 序列化、数组读出。
fn convert(c: &mut Criterion) {
    let (_rt, ctx) = setup();
    let obj = ctx
        .eval_global("({ a: 1, b: [2, 3, 4], c: 'hello' })", "b")
        .unwrap();
    let arr = ctx.eval_global("[1,2,3,4,5,6,7,8,9,10]", "b").unwrap();

    let mut group = c.benchmark_group("convert");
    group.bench_function("to_json", |b| b.iter(|| black_box(obj.to_json().unwrap())));
    group.bench_function("to_array_10", |b| {
        b.iter(|| black_box(arr.to_array().unwrap()))
    });
    group.finish();
}

/// 数组 / buffer 构造。
fn build(c: &mut Criterion) {
    let (_rt, ctx) = setup();
    let bytes = vec![0u8; 1024];

    let mut group = c.benchmark_group("build");
    group.bench_function("make_array_10", |b| {
        b.iter(|| {
            let values: Vec<_> = (0..10).map(|i| ctx.make_int(i)).collect();
            black_box(ctx.make_array(values))
        })
    });
    group.bench_function("make_buffer_1k", |b| {
        b.iter(|| black_box(ctx.make_buffer(black_box(&bytes)).unwrap()))
    });
    group.finish();
}

/// Runtime + Context 创建开销。
fn lifecycle(c: &mut Criterion) {
    c.bench_function("runtime_create", |b| {
        b.iter(|| {
            let rt = Runtime::new();
            black_box(rt.context())
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(50)
        .with_profiler(PProfProfiler::new(1000, Output::Flamegraph(None)));
    targets = values, properties, calls, eval, convert, build, lifecycle
}
criterion_main!(benches);
