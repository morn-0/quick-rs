use crate::{
    function::Function,
    runtime::TASK_CHANNEL,
    value::{self, Exception, JSValueRef},
};
use anyhow::Result;
use log::error;
use parking_lot::Mutex;
use quickjs_sys as sys;
use std::{
    ffi::c_int,
    future::Future,
    mem::ManuallyDrop,
    pin::Pin,
    rc::Rc,
    slice,
    sync::Weak,
    task::{Context, Poll, Waker},
};

#[derive(Default, Clone)]
struct PromiseState {
    data: Option<JSValueRef>,
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
    type Output = Result<JSValueRef>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.lock();

        if let Some(data) = state.data.take() {
            return Poll::Ready(Ok(data));
        }

        if state.waker.replace(cx.waker().clone()).is_none() {
            unsafe {
                let state = Box::into_raw(Box::new(Rc::downgrade(&self.state)));
                let value = self.value.clone();

                let ctx = value.ctx;
                let this = value.val();

                #[rustfmt::skip]
                let then = sys::JS_GetPropertyInternal(ctx, this, sys::JS_ATOM_then, this, 0);
                let then = match Function::new(JSValueRef::from_js_value(ctx, then)) {
                    Ok(v) => v,
                    Err(e) => {
                        return Poll::Ready(Err(e));
                    }
                };

                let resolve = sys::JS_NewCFunctionData(ctx, Some(resolve), 1, 0, 1, state as _);
                let resolve = JSValueRef::from_js_value(ctx, resolve);

                let reject = sys::JS_NewCFunctionData(ctx, Some(reject), 1, 0, 1, state as _);
                let reject = JSValueRef::from_js_value(ctx, reject);

                let this = JSValueRef::from_js_value(ctx, this);
                let value = match then.call(Some(this), vec![resolve, reject]) {
                    Ok(v) => v,
                    Err(e) => {
                        return Poll::Ready(Err(anyhow::anyhow!(e.to_string())));
                    }
                };

                if value.is_exception() {
                    let msg = Exception(value).to_string();
                    return Poll::Ready(Err(anyhow::anyhow!(msg)));
                }
            }

            TASK_CHANNEL.with(|v| {
                if let Err(e) = v.0.send(()) {
                    error!("{e}");
                }
            });
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
    let state = Box::from_raw(data as *mut Weak<Mutex<PromiseState>>);
    let state = ManuallyDrop::new(state);

    let Some(state) = state.upgrade() else {
        return value::make_undefined();
    };
    let mut state = state.lock();

    let args = slice::from_raw_parts(argv, argc as usize);
    let value = if !args.is_empty() {
        value::JS_DupValue_real(ctx, args[0])
    } else {
        value::make_undefined()
    };

    state.data = Some(JSValueRef::from_js_value(ctx, value));

    if let Some(waker) = state.waker.take() {
        waker.wake();
    }

    value::make_undefined()
}

unsafe extern "C" fn reject(
    _ctx: *mut sys::JSContext,
    _this: sys::JSValue,
    _argc: c_int,
    _argv: *mut sys::JSValue,
    _magic: c_int,
    data: *mut sys::JSValue,
) -> sys::JSValue {
    let state = Box::from_raw(data as *mut Weak<Mutex<PromiseState>>);
    let state = ManuallyDrop::new(state);

    let Some(state) = state.upgrade() else {
        return value::make_undefined();
    };

    if let Some(waker) = state.lock().waker.take() {
        waker.wake();
    }

    value::make_undefined()
}