use crate::{
    context::{Context, ThrowKind},
    function::Function,
    loader::ModuleDef,
    runtime::{TASK, WAKER},
    value::JSValueRef,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::Duration,
};
use tracing::error;

pub struct Channel;

impl ModuleDef for Channel {
    fn export(ctx: Context) -> HashMap<impl AsRef<str>, JSValueRef> {
        let mut map = HashMap::default();

        map.insert("send", ctx.make_function(2, send));
        map.insert("recv", ctx.make_function(1, recv));
        map.insert("drop", ctx.make_function(1, drop));

        map
    }

    fn define() -> HashSet<impl AsRef<str>> {
        HashSet::from(["send", "recv", "drop"])
    }
}

fn send(ctx: Context, _: JSValueRef, args: Option<Vec<JSValueRef>>) -> JSValueRef {
    ctx.make_undefined()
}

fn recv(ctx: Context, _: JSValueRef, args: Option<Vec<JSValueRef>>) -> JSValueRef {
    ctx.make_undefined()
}

fn drop(ctx: Context, _: JSValueRef, args: Option<Vec<JSValueRef>>) -> JSValueRef {
    ctx.make_undefined()
}
