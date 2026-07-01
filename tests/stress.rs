//! 压力 / 泄漏回归测试。
//!
//! 两类思路：
//! - **create+drop**：每轮新建 Runtime、跑遍各路径、再 drop。若某路径漏了 Context/Runtime 的 Rc，
//!   该 Runtime 永不释放（每个 ~MB），RSS 会暴涨。这能抓住 `into_raw` 那类「漏 Rc 钉住 Runtime」的 bug。
//! - **same-runtime**：同一个 Runtime 上反复跑只产临时值的操作，抓 atom / Value / JS 堆的累积。
//!
//! 阈值取得很宽（真泄漏是数百 MB~GB，正常 < 10MB），对分配器抖动与并行噪音稳健。

use quick_rs::{
    Context, Error, Function, JsClass, Method, MethodFn, Module, Runtime, Value, ValueRef,
};
use std::time::Duration;

fn rss_kb() -> i64 {
    let s = std::fs::read_to_string("/proc/self/statm").unwrap();
    s.split_whitespace().nth(1).unwrap().parse::<i64>().unwrap() * 4
}

fn assert_no_leak(name: &str, before: i64, after: i64, max_kb: i64) {
    let delta = after - before;
    eprintln!("[{name}] RSS {before}->{after}KB delta={delta}KB (limit {max_kb}KB)");
    assert!(
        delta < max_kb,
        "{name}: RSS 增长 {delta}KB（上限 {max_kb}KB）—— 疑似泄漏"
    );
}

/// 跑遍同步各路径，产生的值都应在本函数结束时释放干净。
fn exercise_sync(ctx: &Context) {
    let _ = ctx.eval_global("1 + 2 * 3", "t").unwrap();

    let obj = ctx.make_object();
    obj.set_property("n", ctx.make_int(1)).unwrap();
    let _ = obj.get_property("n").unwrap();

    let add = ctx.make_function(2, |c, _t, a| {
        let x = a.first().map(|v| v.to_i32()).transpose()?.unwrap_or(0);
        Ok(c.make_int(x + 1))
    });
    let _ = Function::new(add)
        .call(None, &[ctx.make_int(41).borrow()])
        .unwrap();

    let array = ctx.make_array([ctx.make_int(1), ctx.make_string("x").unwrap()]);
    let _ = array.to_json().unwrap();
    let _ = array.to_array().unwrap();

    let _ = ctx.eval_global("null.x", "t").unwrap_err();

    let _ = ctx
        .make_string("héllo 世界 🌍")
        .unwrap()
        .to_string()
        .unwrap();

    let buffer = ctx.make_buffer(vec![1u8, 2, 3, 4]).unwrap();
    let _ = buffer.to_buffer::<u8>().unwrap();
}

struct Tally {
    n: i64,
}

fn tally_add(
    t: &mut Tally,
    ctx: &Context,
    _this: ValueRef,
    _args: &[ValueRef],
) -> Result<Value, Error> {
    t.n += 1;
    Ok(ctx.make_int(t.n as i32))
}

impl JsClass for Tally {
    const NAME: &'static str = "Tally";
    const METHODS: &'static [Method<Self>] = &[Method {
        name: "add",
        argc: 0,
        func: MethodFn::Mut(tally_add),
    }];

    fn constructor(_ctx: &Context, _args: &[ValueRef]) -> Result<Self, Error> {
        Ok(Tally { n: 0 })
    }
}

/// 造大量实例、调方法：实例应在 GC 时经 finalizer 释放（`Box<RefCell<T>>` 不累积）。
fn exercise_class(ctx: &Context) {
    let _ = ctx
        .eval_global(
            "for (let i = 0; i < 100; i++) { const t = new Tally(); t.add(); t.add(); }",
            "t",
        )
        .unwrap();
}

/// 反复武装超时中止死循环 + 正常 eval：验证中断路径（异常清理、令牌）无累积。
fn exercise_interrupt(rt: &Runtime, ctx: &Context) {
    {
        let _guard = rt.interrupt(Some(Duration::from_millis(2)));
        let _ = ctx.eval_global("while (true) {}", "t");
    }
    let _ = ctx.eval_global("1 + 1", "t").unwrap();
}

#[test]
fn sync_create_drop_no_leak() {
    for _ in 0..50 {
        let rt = Runtime::new();
        exercise_sync(&rt.context());
    }
    let before = rss_kb();
    for _ in 0..2000 {
        let rt = Runtime::new();
        let ctx = rt.context();
        // 模块只在 create+drop 里跑（同一 runtime 上模块会合理累积）。
        let module = Module::new(ctx.eval_module("export const x = 7;", "m").unwrap()).unwrap();
        assert_eq!(module.get("x").unwrap().to_i32().unwrap(), 7);
        exercise_sync(&ctx);
    }
    let after = rss_kb();
    assert_no_leak("sync_create_drop", before, after, 100_000);
}

#[test]
fn sync_same_runtime_no_leak() {
    let rt = Runtime::new();
    let ctx = rt.context();
    for _ in 0..200 {
        exercise_sync(&ctx);
    }
    let before = rss_kb();
    for _ in 0..20_000 {
        exercise_sync(&ctx);
    }
    let after = rss_kb();
    assert_no_leak("sync_same_runtime", before, after, 50_000);
}

#[test]
fn class_create_drop_no_leak() {
    for _ in 0..30 {
        let rt = Runtime::new();
        let ctx = rt.context();
        ctx.global()
            .set_property("Tally", ctx.make_class::<Tally>().unwrap())
            .unwrap();
        exercise_class(&ctx);
    }
    let before = rss_kb();
    for _ in 0..1000 {
        let rt = Runtime::new();
        let ctx = rt.context();
        ctx.global()
            .set_property("Tally", ctx.make_class::<Tally>().unwrap())
            .unwrap();
        exercise_class(&ctx);
    }
    let after = rss_kb();
    assert_no_leak("class_create_drop", before, after, 100_000);
}

#[test]
fn class_same_runtime_no_leak() {
    let rt = Runtime::new();
    let ctx = rt.context();
    ctx.global()
        .set_property("Tally", ctx.make_class::<Tally>().unwrap())
        .unwrap();
    for _ in 0..100 {
        exercise_class(&ctx);
    }
    let before = rss_kb();
    // 50 万实例 create+finalize，RSS 应平。
    for _ in 0..5000 {
        exercise_class(&ctx);
    }
    let after = rss_kb();
    assert_no_leak("class_same_runtime", before, after, 50_000);
}

#[test]
fn interrupt_same_runtime_no_leak() {
    let rt = Runtime::new();
    let ctx = rt.context();
    for _ in 0..20 {
        exercise_interrupt(&rt, &ctx);
    }
    let before = rss_kb();
    for _ in 0..200 {
        exercise_interrupt(&rt, &ctx);
    }
    let after = rss_kb();
    assert_no_leak("interrupt_same_runtime", before, after, 50_000);
}

#[cfg(feature = "async")]
mod asynchronous {
    use super::*;
    use quick_rs::Promise;

    /// 跑遍异步各路径：模块 + 定时器 + Promise.await + make_closure（含终结器）。
    async fn exercise_async(rt: &Runtime, ctx: &Context) {
        let ctx = ctx.clone();
        let _ = rt
            .run_until(async move {
                let source = r#"
                    import { setTimeout } from "timer";
                    export const done = new Promise((r) => setTimeout(() => r(1), 1));
                "#;
                let module = Module::new(ctx.eval_module(source, "m").unwrap()).unwrap();
                let _ = Promise::new(module.get("done").unwrap()).await;

                let closure = ctx.make_closure(0, |c, _t, _a| Ok(c.make_int(9)));
                ctx.global().set_property("cl", closure).unwrap();
                let _ = ctx.eval_global("cl()", "t").unwrap();

                let _ = Promise::new(ctx.eval_global("Promise.resolve(2)", "p").unwrap()).await;
                Ok::<(), Error>(())
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn async_create_drop_no_leak() {
        for _ in 0..30 {
            let rt = Runtime::new();
            let ctx = rt.context();
            exercise_async(&rt, &ctx).await;
        }
        let before = rss_kb();
        for _ in 0..400 {
            let rt = Runtime::new();
            let ctx = rt.context();
            exercise_async(&rt, &ctx).await;
        }
        let after = rss_kb();
        assert_no_leak("async_create_drop", before, after, 100_000);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn async_same_runtime_no_leak() {
        let rt = Runtime::new();
        let ctx = rt.context();
        for _ in 0..30 {
            exercise_async(&rt, &ctx).await;
        }
        let before = rss_kb();
        for _ in 0..1000 {
            exercise_async(&rt, &ctx).await;
        }
        let after = rss_kb();
        assert_no_leak("async_same_runtime", before, after, 50_000);
    }
}
