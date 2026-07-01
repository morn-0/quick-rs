use quick_rs::{Context, Error, Runtime};
use std::{thread, time::Duration};

fn context() -> (Runtime, Context) {
    let runtime = Runtime::new();
    let context = runtime.context();
    (runtime, context)
}

#[test]
fn timeout_aborts_infinite_loop() {
    let (rt, ctx) = context();
    {
        let _guard = rt.interrupt(Some(Duration::from_millis(20)));
        let result = ctx.eval_global("while (true) {}", "t");
        assert!(matches!(result, Err(Error::Interrupted)), "got {result:?}");
    }
    // 守卫离开作用域 → 解除武装 → 正常 eval 恢复。
    assert_eq!(ctx.eval_global("1 + 1", "t").unwrap().to_i32().unwrap(), 2);
}

#[test]
fn no_interrupt_runs_normally() {
    let (_rt, ctx) = context();
    let value = ctx
        .eval_global("let s = 0; for (let i = 0; i < 1000; i++) s += i; s", "t")
        .unwrap();
    assert_eq!(value.to_i32().unwrap(), 499500);
}

#[test]
fn cross_thread_cancel() {
    let (rt, ctx) = context();
    // 守卫（带长超时兜底）移入取消线程；粘性 cancel 与守卫 Drop 不竞态。
    let guard = rt.interrupt(Some(Duration::from_secs(10)));
    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        guard.cancel();
    });

    let result = ctx.eval_global("while (true) {}", "t");
    handle.join().unwrap();
    assert!(matches!(result, Err(Error::Interrupted)), "got {result:?}");

    // cancel 粘性：重新武装即复位，之后正常。
    drop(rt.interrupt(None));
    assert_eq!(ctx.eval_global("2 + 3", "t").unwrap().to_i32().unwrap(), 5);
}

#[test]
fn genuine_error_stays_exception() {
    let (_rt, ctx) = context();
    // 未武装中断时，真·JS 错误仍是 Exception，不被误判为 Interrupted。
    let result = ctx.eval_global("throw new Error('boom')", "t");
    assert!(matches!(result, Err(Error::Exception(_))), "got {result:?}");
}

#[test]
fn zero_timeout_aborts_loop() {
    let (rt, ctx) = context();
    let _guard = rt.interrupt(Some(Duration::from_millis(0)));
    let result = ctx.eval_global("while (true) {}", "t");
    assert!(matches!(result, Err(Error::Interrupted)), "got {result:?}");
}

#[test]
fn finite_loop_completes_within_timeout() {
    let (rt, ctx) = context();
    let _guard = rt.interrupt(Some(Duration::from_secs(10)));
    let value = ctx
        .eval_global("let s = 0; for (let i = 0; i < 100000; i++) s += 1; s", "t")
        .unwrap();
    assert_eq!(value.to_i32().unwrap(), 100000);
}

#[test]
fn none_timeout_does_not_abort() {
    let (rt, ctx) = context();
    let _guard = rt.interrupt(None);
    let value = ctx
        .eval_global("let s = 0; for (let i = 0; i < 1000; i++) s += i; s", "t")
        .unwrap();
    assert_eq!(value.to_i32().unwrap(), 499500);
}

#[test]
fn cancel_is_sticky_until_renew() {
    let (rt, ctx) = context();
    // 有限但带回边的循环：cancel 时中断点触发中止，未 cancel 时跑完返回 1。
    let loops = "for (let i = 0; i < 1000000; i++) {} 1";

    let guard = rt.interrupt(None);
    guard.cancel();
    assert!(
        matches!(ctx.eval_global(loops, "t"), Err(Error::Interrupted)),
        "cancel 后应粘性中止"
    );
    // guard Drop 只清 deadline，不复位 cancelled → 仍中止。
    drop(guard);
    assert!(
        matches!(ctx.eval_global(loops, "t"), Err(Error::Interrupted)),
        "guard drop 不应复位 cancelled"
    );
    // 只有 renew（再次 interrupt）复位 cancelled。
    drop(rt.interrupt(None));
    assert_eq!(ctx.eval_global(loops, "t").unwrap().to_i32().unwrap(), 1);
}

#[test]
fn interrupt_inside_callback() {
    let (rt, ctx) = context();
    let _guard = rt.interrupt(Some(Duration::from_millis(20)));
    // 中断在嵌套的 JS 调用帧（forEach 回调）里触发。
    let result = ctx.eval_global("[1].forEach(() => { while (true) {} })", "t");
    assert!(matches!(result, Err(Error::Interrupted)), "got {result:?}");
}

#[test]
fn interrupt_inside_getter() {
    let (rt, ctx) = context();
    let _guard = rt.interrupt(Some(Duration::from_millis(20)));
    // 中断在属性 getter 的循环里触发。
    let result = ctx.eval_global(
        "const o = {}; Object.defineProperty(o, 'x', { get() { while (true) {} } }); o.x",
        "t",
    );
    assert!(matches!(result, Err(Error::Interrupted)), "got {result:?}");
}

#[test]
fn sequential_timeout_scopes() {
    let (rt, ctx) = context();
    // 同一 Runtime 上反复武装/解除，各轮独立、互不残留。
    for _ in 0..5 {
        {
            let _guard = rt.interrupt(Some(Duration::from_millis(20)));
            assert!(matches!(
                ctx.eval_global("while (true) {}", "t"),
                Err(Error::Interrupted)
            ));
        }
        assert_eq!(ctx.eval_global("1 + 1", "t").unwrap().to_i32().unwrap(), 2);
    }
}

#[test]
fn two_runtimes_isolated() {
    // A：无限循环 + 20ms 超时 → 中止；B：有限循环 + 长超时 → 正常完成，不受 A 影响。
    let a = thread::spawn(|| {
        let (rt, ctx) = context();
        let _guard = rt.interrupt(Some(Duration::from_millis(20)));
        matches!(
            ctx.eval_global("while (true) {}", "t"),
            Err(Error::Interrupted)
        )
    });
    let b = thread::spawn(|| {
        let (rt, ctx) = context();
        let _guard = rt.interrupt(Some(Duration::from_secs(10)));
        ctx.eval_global("let s = 0; for (let i = 0; i < 100000; i++) s += 1; s", "t")
            .map(|v| v.to_i32().unwrap())
    });
    assert!(a.join().unwrap(), "A 应被超时中止");
    assert_eq!(b.join().unwrap().unwrap(), 100000, "B 应正常完成");
}

#[test]
fn many_threads_timeout_abort() {
    // 16 线程各自 Runtime + 超时 + 死循环，全部应被各自的中断处理器中止。
    let handles: Vec<_> = (0..16)
        .map(|_| {
            thread::spawn(|| {
                let (rt, ctx) = context();
                let _guard = rt.interrupt(Some(Duration::from_millis(20)));
                matches!(
                    ctx.eval_global("while (true) {}", "t"),
                    Err(Error::Interrupted)
                )
            })
        })
        .collect();
    assert!(handles.into_iter().all(|h| h.join().unwrap()));
}
