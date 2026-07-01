use quick_rs::{
    Context, Error, Function, GetSet, JsClass, Method, MethodFn, Runtime, Value, ValueRef,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

fn context() -> (Runtime, Context) {
    let runtime = Runtime::new();
    let context = runtime.context();
    (runtime, context)
}

struct Point {
    x: f64,
    y: f64,
}

fn norm(p: &Point, ctx: &Context, _this: ValueRef, _args: &[ValueRef]) -> Result<Value, Error> {
    Ok(ctx.make_float((p.x * p.x + p.y * p.y).sqrt()))
}

fn get_x(p: &Point, ctx: &Context) -> Result<Value, Error> {
    Ok(ctx.make_float(p.x))
}

fn set_x(p: &mut Point, _ctx: &Context, v: ValueRef) -> Result<(), Error> {
    p.x = v.to_number()?;
    Ok(())
}

fn get_y(p: &Point, ctx: &Context) -> Result<Value, Error> {
    Ok(ctx.make_float(p.y))
}

fn set_y(p: &mut Point, _ctx: &Context, v: ValueRef) -> Result<(), Error> {
    p.y = v.to_number()?;
    Ok(())
}

impl JsClass for Point {
    const NAME: &'static str = "Point";
    const CTOR_ARGC: i32 = 2;
    const METHODS: &'static [Method<Self>] = &[Method {
        name: "norm",
        argc: 0,
        func: MethodFn::Ref(norm),
    }];
    const GETSETS: &'static [GetSet<Self>] = &[
        GetSet {
            name: "x",
            get: get_x,
            set: Some(set_x),
        },
        GetSet {
            name: "y",
            get: get_y,
            set: Some(set_y),
        },
    ];

    fn constructor(_ctx: &Context, args: &[ValueRef]) -> Result<Self, Error> {
        let x = args.first().map(|v| v.to_number::<f64>()).transpose()?;
        let y = args.get(1).map(|v| v.to_number::<f64>()).transpose()?;
        Ok(Point {
            x: x.unwrap_or(0.0),
            y: y.unwrap_or(0.0),
        })
    }
}

fn point_context() -> (Runtime, Context) {
    let (rt, ctx) = context();
    let ctor = ctx.make_class::<Point>().unwrap();
    ctx.global().set_property("Point", ctor).unwrap();
    (rt, ctx)
}

#[test]
fn method_dispatch() {
    let (_rt, ctx) = point_context();
    let value = ctx.eval_global("new Point(3, 4).norm()", "t").unwrap();
    assert_eq!(value.to_f64().unwrap(), 5.0);
}

#[test]
fn getter_and_setter() {
    let (_rt, ctx) = point_context();
    assert_eq!(
        ctx.eval_global("new Point(3, 4).x", "t")
            .unwrap()
            .to_f64()
            .unwrap(),
        3.0
    );
    let value = ctx
        .eval_global(
            "(() => { const p = new Point(3, 4); p.x = 6; return p.norm(); })()",
            "t",
        )
        .unwrap();
    assert_eq!(value.to_f64().unwrap(), 52f64.sqrt());
}

#[test]
fn instance_of_and_constructor() {
    let (_rt, ctx) = point_context();
    assert!(ctx
        .eval_global("new Point(1, 1) instanceof Point", "t")
        .unwrap()
        .to_bool()
        .unwrap());
}

#[test]
fn wrong_receiver_throws() {
    let (_rt, ctx) = point_context();
    assert!(ctx
        .eval_global("Point.prototype.norm.call({})", "t")
        .is_err());
}

static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

struct Counted;

impl Drop for Counted {
    fn drop(&mut self) {
        DROP_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

impl JsClass for Counted {
    const NAME: &'static str = "Counted";

    fn constructor(_ctx: &Context, _args: &[ValueRef]) -> Result<Self, Error> {
        Ok(Counted)
    }
}

#[test]
fn finalizer_releases_every_instance() {
    DROP_COUNT.store(0, Ordering::SeqCst);
    {
        let (_rt, ctx) = context();
        let ctor = ctx.make_class::<Counted>().unwrap();
        ctx.global().set_property("Counted", ctor).unwrap();
        ctx.eval_global("for (let i = 0; i < 1000; i++) new Counted();", "t")
            .unwrap();
    }
    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1000);
}

// ---- Counter：可变状态、setter 校验、重入安全 ----

struct Counter {
    n: i32,
}

fn inc(
    c: &mut Counter,
    _ctx: &Context,
    this: ValueRef,
    _args: &[ValueRef],
) -> Result<Value, Error> {
    c.n += 1;
    // 返回 this 支持链式调用。
    Ok(this.to_owned())
}

fn run(c: &mut Counter, ctx: &Context, _this: ValueRef, args: &[ValueRef]) -> Result<Value, Error> {
    // 触碰 self 确保 &mut 借用真实持有；其间回调 JS 函数，若它重入本实例的方法则借用冲突。
    c.n += 1;
    let callback = Function::new(
        args.first()
            .ok_or(Error::Argument("run expects a callback"))?
            .to_owned(),
    );
    callback.call(None, &[])?;
    Ok(ctx.make_int(c.n))
}

fn observe(c: &Counter, ctx: &Context, _this: ValueRef, args: &[ValueRef]) -> Result<Value, Error> {
    // 只读借用期间回调 JS：允许其它只读访问（读 value），可变访问（inc）会被挡下转异常。
    let callback = Function::new(
        args.first()
            .ok_or(Error::Argument("observe expects a callback"))?
            .to_owned(),
    );
    callback.call(None, &[])?;
    Ok(ctx.make_int(c.n))
}

fn counter_get(c: &Counter, ctx: &Context) -> Result<Value, Error> {
    Ok(ctx.make_int(c.n))
}

fn counter_set(c: &mut Counter, _ctx: &Context, v: ValueRef) -> Result<(), Error> {
    let n = v.to_i32()?;
    if n < 0 {
        return Err(Error::Argument("value must be non-negative"));
    }
    c.n = n;
    Ok(())
}

impl JsClass for Counter {
    const NAME: &'static str = "Counter";
    const METHODS: &'static [Method<Self>] = &[
        Method {
            name: "inc",
            argc: 0,
            func: MethodFn::Mut(inc),
        },
        Method {
            name: "run",
            argc: 1,
            func: MethodFn::Mut(run),
        },
        Method {
            name: "observe",
            argc: 1,
            func: MethodFn::Ref(observe),
        },
    ];
    const GETSETS: &'static [GetSet<Self>] = &[GetSet {
        name: "value",
        get: counter_get,
        set: Some(counter_set),
    }];

    fn constructor(_ctx: &Context, _args: &[ValueRef]) -> Result<Self, Error> {
        Ok(Counter { n: 0 })
    }
}

fn counter_context() -> (Runtime, Context) {
    let (rt, ctx) = context();
    let ctor = ctx.make_class::<Counter>().unwrap();
    ctx.global().set_property("Counter", ctor).unwrap();
    (rt, ctx)
}

#[test]
fn method_mutation_persists() {
    let (_rt, ctx) = counter_context();
    let v = ctx
        .eval_global(
            "(() => { const c = new Counter(); c.inc(); c.inc(); c.inc(); return c.value; })()",
            "t",
        )
        .unwrap();
    assert_eq!(v.to_i32().unwrap(), 3);
}

#[test]
fn setter_validates_and_rejects() {
    let (_rt, ctx) = counter_context();
    let ok = ctx
        .eval_global(
            "(() => { const c = new Counter(); c.value = 5; return c.value; })()",
            "t",
        )
        .unwrap();
    assert_eq!(ok.to_i32().unwrap(), 5);
    // 非法赋值抛出且不改状态。
    let kept = ctx
        .eval_global(
            "(() => { const c = new Counter(); c.value = 7; try { c.value = -1; } catch (e) {} return c.value; })()",
            "t",
        )
        .unwrap();
    assert_eq!(kept.to_i32().unwrap(), 7);
}

#[test]
fn reentrant_same_instance_throws() {
    let (_rt, ctx) = counter_context();
    // run 借用 &mut self 期间回调重入同一实例 → 借用冲突 → 抛异常（而非 UB/panic）。
    let result = ctx.eval_global(
        "(() => { const c = new Counter(); c.run(() => c.inc()); })()",
        "t",
    );
    assert!(
        result.is_err(),
        "reentrant access should throw, got {result:?}"
    );
}

#[test]
fn reentrant_other_instance_ok() {
    let (_rt, ctx) = counter_context();
    // 回调里动的是另一个实例 → 无冲突。
    let v = ctx
        .eval_global(
            "(() => { const a = new Counter(); const b = new Counter(); a.run(() => b.inc()); return b.value; })()",
            "t",
        )
        .unwrap();
    assert_eq!(v.to_i32().unwrap(), 1);
}

#[test]
fn method_this_enables_chaining() {
    let (_rt, ctx) = counter_context();
    // inc 返回 this → 链式调用 c.inc().inc().inc()。
    let v = ctx
        .eval_global(
            "(() => { const c = new Counter(); return c.inc().inc().inc().value; })()",
            "t",
        )
        .unwrap();
    assert_eq!(v.to_i32().unwrap(), 3);
}

#[test]
fn readonly_method_allows_reentrant_read() {
    let (_rt, ctx) = counter_context();
    // observe 持共享借用期间读同实例 value（只读 + 只读）→ 允许。
    let v = ctx
        .eval_global(
            "(() => { const c = new Counter(); c.inc(); c.inc(); let seen = -1; c.observe(() => { seen = c.value; }); return seen; })()",
            "t",
        )
        .unwrap();
    assert_eq!(v.to_i32().unwrap(), 2);
}

#[test]
fn readonly_method_blocks_reentrant_mutation() {
    let (_rt, ctx) = counter_context();
    // observe 持共享借用期间 inc（只读 + 可变）→ 借用冲突 → 抛异常。
    let result = ctx.eval_global(
        "(() => { const c = new Counter(); c.observe(() => c.inc()); })()",
        "t",
    );
    assert!(
        result.is_err(),
        "shared borrow should block mutation, got {result:?}"
    );
}

#[test]
fn constructor_error_propagates() {
    let (_rt, ctx) = point_context();
    // Symbol 无法 ToNumber → 构造函数返回 Err → new 抛出，可被 JS 捕获。
    let caught = ctx
        .eval_global(
            "(() => { try { new Point(Symbol('s'), 0); return 'no-throw'; } catch (e) { return 'caught'; } })()",
            "t",
        )
        .unwrap();
    assert_eq!(caught.to_string().unwrap(), "caught");
}

#[test]
fn subclass_inherits_methods() {
    let (_rt, ctx) = point_context();
    // 经 new_target.prototype 造对象 → extends 正常：既是 Point 也是 P3，继承 norm。
    let v = ctx
        .eval_global(
            "class P3 extends Point {}; (() => { const p = new P3(3, 4); return [p instanceof Point, p instanceof P3, p.norm()]; })()",
            "t",
        )
        .unwrap();
    let arr = v.to_array().unwrap();
    assert!(arr[0].to_bool().unwrap(), "instanceof Point");
    assert!(arr[1].to_bool().unwrap(), "instanceof P3");
    assert_eq!(arr[2].to_f64().unwrap(), 5.0);
}

#[test]
fn classes_across_threads() {
    // 每个线程独立 Runtime 注册并使用 Point：验证 class id 逐 Runtime 分配、并发无冲突。
    let handles: Vec<_> = (0..8)
        .map(|_| {
            thread::spawn(|| {
                let (_rt, ctx) = point_context();
                let value = ctx.eval_global("new Point(3, 4).norm()", "t").unwrap();
                assert_eq!(value.to_f64().unwrap(), 5.0);
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
}
