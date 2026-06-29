use crate::{
    context::Context,
    error::Error,
    function::Function,
    loader::ModuleDef,
    value::{Value, ValueRef},
};
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

pub struct Timer;

impl ModuleDef for Timer {
    fn export(ctx: &Context) -> HashMap<impl AsRef<str>, Value> {
        let mut exports = HashMap::new();
        exports.insert("setTimeout", ctx.make_function(2, set_timeout));
        exports.insert("clearTimeout", ctx.make_function(1, clear_timeout));
        exports
    }

    fn define() -> HashSet<impl AsRef<str>> {
        HashSet::from(["setTimeout", "clearTimeout"])
    }
}

fn set_timeout(ctx: &Context, _this: ValueRef, args: &[ValueRef]) -> Result<Value, Error> {
    let func = Function::new(
        args.first()
            .ok_or(Error::Argument("setTimeout requires a callback"))?
            .to_owned(),
    );

    let delay = args.get(1).map(|v| v.to_i32()).transpose()?.unwrap_or(0);
    let delay = Duration::from_millis(delay.max(0) as u64);

    let extra: Vec<Value> = args
        .get(2..)
        .unwrap_or(&[])
        .iter()
        .map(|v| v.to_owned())
        .collect();

    let state = ctx
        .runtime()
        .async_state()
        .ok_or(Error::Argument("setTimeout requires a running event loop"))?;
    let id = state.insert(func, None, extra);
    state.push_timer(id, delay);

    Ok(ctx.make_int(id as i32))
}

fn clear_timeout(ctx: &Context, _this: ValueRef, args: &[ValueRef]) -> Result<Value, Error> {
    if let Some(id) = args.first() {
        let id = id.to_i32()? as u64;
        if let Some(state) = ctx.runtime().async_state() {
            state.remove(id);
        }
    }

    Ok(ctx.make_undefined())
}
