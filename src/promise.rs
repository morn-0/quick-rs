use crate::{
    error::QuickError,
    function::Function,
    runtime::WAKER,
    value::{self, JSValueRef},
};
use parking_lot::Mutex;
use quickjs_sys as sys;
use std::{
    ffi::c_int,
    future::Future,
    mem::ManuallyDrop,
    ops::Deref,
    pin::Pin,
    rc::Rc,
    slice,
    sync::Weak,
    task::{Context, Poll, Waker},
};
use tracing::error;

#[derive(Default, Clone)]
struct PromiseState {
    data: Option<Result<JSValueRef, JSValueRef>>,
    waker: Option<Waker>,
}

pub struct Promise {
    value: JSValueRef,
    state: Rc<Mutex<PromiseState>>,
}

impl Promise {
    pub fn new(value: JSValueRef) -> Self {
        let state = Rc::new(Mutex::new(PromiseState::default()));
        Promise { value, state }
    }
}

impl Future for Promise {
    type Output = Result<JSValueRef, JSValueRef>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.lock();

        if let Some(data) = state.data.take() {
            return Poll::Ready(data);
        }

        if state.waker.replace(cx.waker().clone()).is_none() {
            unsafe {
                let state = Box::into_raw(Box::new(Rc::downgrade(&self.state)));
                let value = self.value.clone();

                let then = {
                    let ctx = value.ctx().clone();

                    let then = sys::JS_GetProperty(ctx.ptr(), value.val(), sys::JS_ATOM_then);
                    Function::new(JSValueRef::from_value(ctx.clone(), then))
                };

                let resolve = {
                    let ctx = value.ctx().clone();

                    #[rustfmt::skip]
                    let resolve = sys::JS_NewCFunctionData(ctx.ptr(), Some(resolve), 1, 0, 1, state as _);
                    JSValueRef::from_value(ctx, resolve)
                };

                let reject = {
                    let ctx = value.ctx().clone();

                    #[rustfmt::skip]
                    let reject = sys::JS_NewCFunctionData(ctx.ptr(), Some(reject), 1, 0, 1, state as _);
                    JSValueRef::from_value(ctx, reject)
                };

                if let Err(QuickError::Call(e)) = then.call(Some(value), vec![resolve, reject]) {
                    error!("{e}");
                    return Poll::Ready(Ok(self.value.ctx().make_undefined()));
                }
            }

            let waker = WAKER.with(|v| v.0.clone());
            if let Err(e) = waker.send(None) {
                error!("{e}");
            }
        }

        Poll::Pending
    }
}

unsafe extern "C" fn resolve(
    ctx: *mut sys::JSContext,
    _this: sys::JSValue,
    argc: c_int,
    argv: *mut sys::JSValue,
    _magic: c_int,
    data: *mut sys::JSValue,
) -> sys::JSValue {
    let ctx = ManuallyDrop::new(crate::context::Context(ctx));

    let state = Box::from_raw(data as *mut Weak<Mutex<PromiseState>>);
    let state = ManuallyDrop::new(state);

    let Some(state) = state.upgrade() else {
        return ctx.make_undefined().val();
    };
    let mut state = state.lock();

    let args = slice::from_raw_parts(argv, argc as usize);
    let value = if !args.is_empty() {
        value::dup_value(ctx.ptr(), args[0])
    } else {
        ctx.make_undefined().val()
    };

    {
        let ctx = ctx.deref().clone();
        state.data = Some(Ok(JSValueRef::from_value(ctx, value)));
    }

    if let Some(waker) = state.waker.take() {
        waker.wake();
    }

    ctx.make_undefined().val()
}

unsafe extern "C" fn reject(
    ctx: *mut sys::JSContext,
    _this: sys::JSValue,
    argc: c_int,
    argv: *mut sys::JSValue,
    _magic: c_int,
    data: *mut sys::JSValue,
) -> sys::JSValue {
    let ctx = ManuallyDrop::new(crate::context::Context(ctx));

    let state = Box::from_raw(data as *mut Weak<Mutex<PromiseState>>);
    let state = ManuallyDrop::new(state);

    let Some(state) = state.upgrade() else {
        return ctx.make_undefined().val();
    };

    let mut state = state.lock();

    let args = slice::from_raw_parts(argv, argc as usize);
    let value = if !args.is_empty() {
        value::dup_value(ctx.ptr(), args[0])
    } else {
        ctx.make_undefined().val()
    };

    {
        let ctx = ctx.deref().clone();
        state.data = Some(Err(JSValueRef::from_value(ctx, value)));
    }

    if let Some(waker) = state.waker.take() {
        waker.wake();
    }

    ctx.make_undefined().val()
}
