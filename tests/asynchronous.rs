#![cfg(feature = "async")]

use quick_rs::{Context, Function, Module, Promise, Runtime};
use std::future::Future;

/// 建一个 Runtime + Context，把 `body` 产出的 future 跑到完成（事件循环由 tokio 单线程执行器驱动）。
async fn run<F, Fut, T>(body: F) -> T
where
    F: FnOnce(Context) -> Fut,
    Fut: Future<Output = T> + 'static,
{
    let runtime = Runtime::new();
    let context = runtime.context();
    runtime.run_until(body(context)).await
}

#[tokio::test(flavor = "current_thread")]
async fn promise_resolve() {
    let result = run(|ctx| async move {
        let promise = ctx.eval_global("Promise.resolve(42)", "t").unwrap();
        Promise::new(promise).await
    })
    .await;
    assert_eq!(result.unwrap().to_i32().unwrap(), 42);
}

#[tokio::test(flavor = "current_thread")]
async fn promise_reject() {
    let result = run(|ctx| async move {
        let promise = ctx
            .eval_global("Promise.reject(new Error('nope'))", "t")
            .unwrap();
        Promise::new(promise).await
    })
    .await;
    let reason = result.unwrap_err();
    assert_eq!(
        reason.get_property("message").unwrap().to_string().unwrap(),
        "nope"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn js_async_await() {
    let out = run(|ctx| async move {
        let src = "(async () => { const a = await Promise.resolve(20); return a + 22; })()";
        let promise = ctx.eval_global(src, "t").unwrap();
        Promise::new(promise).await
    })
    .await
    .unwrap();
    assert_eq!(out.to_i32().unwrap(), 42);
}

#[tokio::test(flavor = "current_thread")]
async fn set_timeout_runs() {
    let out = run(|ctx| async move {
        let src = r#"
            import { setTimeout } from "timer";
            export const done = new Promise((r) => setTimeout(() => r("fired"), 10));
        "#;
        let module = Module::new(ctx.eval_module(src, "m").unwrap()).unwrap();
        Promise::new(module.get("done").unwrap()).await
    })
    .await
    .unwrap();
    assert_eq!(out.to_string().unwrap(), "fired");
}

#[tokio::test(flavor = "current_thread")]
async fn set_timeout_orders_by_delay() {
    let out = run(|ctx| async move {
        let src = r#"
            import { setTimeout } from "timer";
            let order = [];
            setTimeout(() => order.push("c"), 30);   // 注册最早、触发最晚
            setTimeout(() => order.push("a"), 10);
            setTimeout(() => order.push("b"), 20);
            export const done = new Promise((r) => setTimeout(() => r(order.join("")), 50));
        "#;
        let module = Module::new(ctx.eval_module(src, "m").unwrap()).unwrap();
        Promise::new(module.get("done").unwrap()).await
    })
    .await
    .unwrap();
    assert_eq!(out.to_string().unwrap(), "abc");
}

#[tokio::test(flavor = "current_thread")]
async fn clear_timeout_cancels() {
    let out = run(|ctx| async move {
        let src = r#"
            import { setTimeout, clearTimeout } from "timer";
            let hit = false;
            const id = setTimeout(() => { hit = true; }, 10);
            clearTimeout(id);
            export const done = new Promise((r) => setTimeout(() => r(hit), 30));
        "#;
        let module = Module::new(ctx.eval_module(src, "m").unwrap()).unwrap();
        Promise::new(module.get("done").unwrap()).await
    })
    .await
    .unwrap();
    assert!(!out.to_bool().unwrap());
}

#[tokio::test(flavor = "current_thread")]
async fn microtask_before_macrotask() {
    let out = run(|ctx| async move {
        let src = r#"
            import { setTimeout } from "timer";
            let order = [];
            setTimeout(() => order.push("macro"), 0);
            Promise.resolve().then(() => order.push("micro"));
            order.push("sync");
            export const done = new Promise((r) => setTimeout(() => r(order.join(",")), 20));
        "#;
        let module = Module::new(ctx.eval_module(src, "m").unwrap()).unwrap();
        Promise::new(module.get("done").unwrap()).await
    })
    .await
    .unwrap();
    assert_eq!(out.to_string().unwrap(), "sync,micro,macro");
}

#[tokio::test(flavor = "current_thread")]
async fn promise_all_preserves_order() {
    let out = run(|ctx| async move {
        let src = r#"
            import { setTimeout } from "timer";
            const d = (ms, v) => new Promise((r) => setTimeout(() => r(v), ms));
            export const done = Promise.all([d(20, 1), d(10, 2), d(15, 3)]).then((a) => a.join(""));
        "#;
        let module = Module::new(ctx.eval_module(src, "m").unwrap()).unwrap();
        Promise::new(module.get("done").unwrap()).await
    })
    .await
    .unwrap();
    assert_eq!(out.to_string().unwrap(), "123");
}

#[tokio::test(flavor = "current_thread")]
async fn rust_join_two_js_promises() {
    let (a, b) = run(|ctx| async move {
        let src = r#"
            import { setTimeout } from "timer";
            globalThis.delay = (ms, v) => new Promise((r) => setTimeout(() => r(v), ms));
        "#;
        Module::new(ctx.eval_module(src, "setup").unwrap()).unwrap();
        let delay = Function::new(ctx.global().get_property("delay").unwrap());
        let make = |ms: i32, v: i32| {
            let ms = ctx.make_int(ms);
            let v = ctx.make_int(v);
            Promise::new(delay.call(None, &[ms.borrow(), v.borrow()]).unwrap())
        };
        tokio::join!(make(30, 1), make(15, 2))
    })
    .await;
    assert_eq!(a.unwrap().to_i32().unwrap(), 1);
    assert_eq!(b.unwrap().to_i32().unwrap(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn rust_resolves_js_promise() {
    let value = run(|ctx| async move {
        let (promise, resolve, _reject) = ctx.make_promise().unwrap();
        let resolve = Function::new(resolve);
        let factory = ctx.clone();
        // 一个 Rust 任务把这个 JS promise 兑现，由事件循环驱动。
        ctx.runtime().spawn_local(async move {
            let value = factory.make_int(42);
            let _ = resolve.call(None, &[value.borrow()]);
        });
        Promise::new(promise).await
    })
    .await;
    assert_eq!(value.unwrap().to_i32().unwrap(), 42);
}

#[test]
fn closure_captures_state() {
    use std::cell::Cell;
    use std::rc::Rc;

    let runtime = Runtime::new();
    let ctx = runtime.context();

    let counter = Rc::new(Cell::new(0));
    let inner = counter.clone();
    let inc = ctx.make_closure(0, move |ctx, _this, _args| {
        inner.set(inner.get() + 1);
        Ok(ctx.make_int(inner.get()))
    });
    ctx.global().set_property("inc", inc).unwrap();

    ctx.eval_global("inc(); inc(); inc()", "t").unwrap();
    assert_eq!(counter.get(), 3);
}
