use crate::{
    context::{Context, ThrowKind},
    function::Function,
    loader::ModuleDef,
    runtime::{ARGS, TASK, TASK_ID, WAKER},
    value::JSValueRef,
};
use heap::{TimerHeap, TimerId};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::atomic::Ordering,
    time::Duration,
};
use tracing::error;

mod heap;

thread_local! {
    static HEAP: TimerHeap = TimerHeap::new();
}

pub struct Timer;

impl ModuleDef for Timer {
    fn export(ctx: Context) -> HashMap<impl AsRef<str>, JSValueRef> {
        let mut map = HashMap::default();

        map.insert("setTimeout", ctx.make_function(2, set_timeout));
        map.insert("clearTimeout", ctx.make_function(1, clear_timeout));

        map
    }

    fn define() -> HashSet<impl AsRef<str>> {
        HashSet::from(["setTimeout", "clearTimeout"])
    }
}

fn clear_timeout(ctx: Context, _: JSValueRef, args: Option<Vec<JSValueRef>>) -> JSValueRef {
    let Some(mut args) = args else {
        return ctx.throw(ThrowKind::PlainError(String::from("Parameter is empty")));
    };

    if let Some(task_id) = args.pop() {
        let task_id = match task_id.to_i32() {
            Ok(v) => v as u64,
            Err(_) => {
                return ctx.make_exception();
            }
        };

        HEAP.with(|v| v.remove_timer(TimerId(task_id)));

        TASK.with(|v| v.borrow_mut().remove(&task_id));
        ARGS.with(|v| v.borrow_mut().remove(&task_id));
    }

    ctx.make_undefined()
}

fn set_timeout(ctx: Context, _: JSValueRef, args: Option<Vec<JSValueRef>>) -> JSValueRef {
    let Some(args) = args else {
        return ctx.throw(ThrowKind::PlainError(String::from("Parameter is empty")));
    };
    let mut args = VecDeque::from(args);

    let Some(function) = args.pop_front() else {
        return ctx.throw(ThrowKind::TypeError(String::from("Not a function")));
    };
    let function = Function::new(function);

    let mut delay = 0;
    if let Some(time) = args.pop_front() {
        delay = match time.to_i32() {
            Ok(v) => v,
            Err(_) => {
                return ctx.make_exception();
            }
        };
    }
    let delay = Duration::from_millis(delay as u64);

    let task_id = TASK_ID.with(|v| v.fetch_add(1, Ordering::Relaxed));
    let args = Vec::from(args);

    TASK.with(|v| v.borrow_mut().insert(task_id, (function, None)));
    ARGS.with(|v| v.borrow_mut().insert(task_id, args));

    HEAP.with(|v| {
        let cb = async move {
            let waker = WAKER.with(|v| v.0.clone());

            if let Err(e) = waker.send_async(Some(task_id)).await {
                error!("wake timer({task_id}), {e}");
            }
        };
        v.insert_timer(delay, cb);
    });

    ctx.make_int(task_id as i32)
}
