use crate::{
    context::{Context, ThrowKind},
    function::Function,
    loader::ModuleDef,
    runtime::{ARGS, TASK, TASK_ID, WAKER},
    value::JSValueRef,
};
use flume::{Receiver, Sender};
use quickjs_sys as sys;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    mem,
    sync::atomic::Ordering,
};
use tokio::task;
use tracing::error;

type Item = (Sender<JSValueRef>, Receiver<JSValueRef>);
thread_local! {
    static CHANNEL: RefCell<HashMap<String, Item>> = RefCell::new(HashMap::new());
}

pub struct Channel;

impl ModuleDef for Channel {
    fn export(ctx: Context) -> HashMap<impl AsRef<str>, JSValueRef> {
        let mut map = HashMap::default();

        map.insert("send", ctx.make_function(2, send));
        map.insert("recv", ctx.make_function(1, recv));

        map
    }

    fn define() -> HashSet<impl AsRef<str>> {
        HashSet::from(["send", "recv"])
    }
}

fn send(ctx: Context, _: JSValueRef, args: Option<Vec<JSValueRef>>) -> JSValueRef {
    let Some(args) = args else {
        return ctx.throw(ThrowKind::PlainError(String::from("Parameter is empty")));
    };
    let mut args = VecDeque::from(args);

    let Some(topic) = args.pop_front() else {
        return ctx.throw(ThrowKind::PlainError(String::from("Parameter is empty")));
    };
    let topic = match topic.to_string() {
        Ok(v) => v,
        Err(_) => {
            return ctx.make_exception();
        }
    };

    let Some(value) = args.pop_front() else {
        return ctx.throw(ThrowKind::PlainError(String::from("Parameter is empty")));
    };

    let sender = CHANNEL.with(|v| {
        v.borrow_mut()
            .entry(topic.clone())
            .or_insert(flume::bounded(64))
            .0
            .clone()
    });

    let (promise, resolve, reject) = make_promise(ctx);

    let resolve_id = TASK_ID.with(|v| v.fetch_add(1, Ordering::Relaxed));
    TASK.with(|v| v.borrow_mut().insert(resolve_id, Function::new(resolve)));

    let reject_id = TASK_ID.with(|v| v.fetch_add(1, Ordering::Relaxed));
    TASK.with(|v| v.borrow_mut().insert(reject_id, Function::new(reject)));

    task::spawn_local(async move {
        let task_id = match sender.send_async(value).await {
            Ok(_) => {
                TASK.with(|v| v.borrow_mut().remove(&reject_id));
                resolve_id
            }
            Err(_) => {
                TASK.with(|v| v.borrow_mut().remove(&resolve_id));
                reject_id
            }
        };

        let waker = WAKER.with(|v| v.0.clone());
        if let Err(e) = waker.send_async(Some(task_id)).await {
            error!("wake channel({task_id}), {e}");
        }
    });

    promise
}

fn recv(ctx: Context, _: JSValueRef, args: Option<Vec<JSValueRef>>) -> JSValueRef {
    let Some(args) = args else {
        return ctx.throw(ThrowKind::PlainError(String::from("Parameter is empty")));
    };
    let mut args = VecDeque::from(args);

    let Some(topic) = args.pop_front() else {
        return ctx.throw(ThrowKind::PlainError(String::from("Parameter is empty")));
    };
    let topic = match topic.to_string() {
        Ok(v) => v,
        Err(_) => {
            return ctx.make_exception();
        }
    };

    let receiver = CHANNEL.with(|v| {
        v.borrow_mut()
            .entry(topic)
            .or_insert(flume::bounded(64))
            .1
            .clone()
    });

    let (promise, resolve, reject) = make_promise(ctx);

    let resolve_id = TASK_ID.with(|v| v.fetch_add(1, Ordering::Relaxed));
    TASK.with(|v| v.borrow_mut().insert(resolve_id, Function::new(resolve)));

    let reject_id = TASK_ID.with(|v| v.fetch_add(1, Ordering::Relaxed));
    TASK.with(|v| v.borrow_mut().insert(reject_id, Function::new(reject)));

    task::spawn_local(async move {
        let task_id = match receiver.recv_async().await {
            Ok(value) => {
                TASK.with(|v| v.borrow_mut().remove(&reject_id));

                ARGS.with(|v| v.borrow_mut().insert(resolve_id, vec![value]));
                resolve_id
            }
            Err(_) => {
                TASK.with(|v| v.borrow_mut().remove(&resolve_id));

                reject_id
            }
        };

        let waker = WAKER.with(|v| v.0.clone());
        if let Err(e) = waker.send_async(Some(task_id)).await {
            error!("wake channel({task_id}), {e}");
        }
    });

    promise
}

fn make_promise(ctx: Context) -> (JSValueRef, JSValueRef, JSValueRef) {
    let mut funcs: [sys::JSValue; 2] = unsafe { mem::zeroed() };

    let promise = unsafe {
        let ptr = funcs.as_mut_ptr();
        sys::JS_NewPromiseCapability(ctx.0, ptr)
    };
    let promise = JSValueRef::from_value(ctx.clone(), promise);

    let resolve = JSValueRef::from_value(ctx.clone(), funcs[0]);
    let reject = JSValueRef::from_value(ctx.clone(), funcs[1]);

    (promise, resolve, reject)
}
