use crate::value::{self, Exception, JSValueRef};
use parking_lot::Mutex;
use quickjs_sys as sys;
use std::{
    ffi::c_int,
    future::Future,
    pin::Pin,
    ptr, slice,
    sync::Arc,
    task::{Context, Poll, Waker},
};

#[derive(Debug, Default, Clone)]
struct PromiseState {
    data: Option<*mut sys::JSValue>,
    waker: Option<Waker>,
}

pub struct Promise {
    ctx: Mutex<*mut sys::JSContext>,
    val: sys::JSValue,
    state: Arc<Mutex<PromiseState>>,
}

impl Promise {
    pub fn new(ctx: *mut sys::JSContext, val: sys::JSValue) -> Self {
        let state = PromiseState::default();

        Promise {
            ctx: Mutex::new(ctx),
            val,
            state: Arc::new(Mutex::new(state)),
        }
    }
}

impl Future for Promise {
    type Output = JSValueRef;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.lock();
        let ctx = self.ctx.lock();

        if let Some(data) = state.data.take() {
            let data = unsafe { Box::from_raw(data) };
            return Poll::Ready(JSValueRef::from_js_value(*ctx, *data));
        }

        if state.waker.replace(cx.waker().clone()).is_none() {
            unsafe {
                #[rustfmt::skip]
                let then = sys::JS_GetPropertyInternal(*ctx, self.val, sys::JS_ATOM_then, self.val, 0);

                let data = Box::into_raw(Box::new(self.state.clone()));
                let reject = sys::JS_NewCFunctionData(*ctx, Some(reject), 1, 0, 1, data as _);

                let data = Box::into_raw(Box::new(self.state.clone()));
                let resolve = sys::JS_NewCFunctionData(*ctx, Some(resolve), 1, 0, 1, data as _);

                let mut args = [resolve, reject];

                let res = sys::JS_Call(*ctx, then, self.val, 2, ptr::addr_of_mut!(args) as _);
                let val = JSValueRef::from_js_value(*ctx, res);

                if val.is_exception() {
                    let msg = Exception(val).to_string();
                    println!("exception {msg}");
                }
            }
        }

        Poll::Pending
    }
}

unsafe extern "C" fn reject(
    ctx: *mut sys::JSContext,
    this_val: sys::JSValue,
    argc: c_int,
    argv: *mut sys::JSValue,
    magic: c_int,
    func_data: *mut sys::JSValue,
) -> sys::JSValue {
    println!("reject");

    let state = Box::from_raw(func_data as *mut Arc<Mutex<PromiseState>>);
    if let Some(waker) = state.lock().waker.take() {
        waker.wake();
    }

    value::make_undefined()
}

unsafe extern "C" fn resolve(
    ctx: *mut sys::JSContext,
    _: sys::JSValue,
    argc: c_int,
    argv: *mut sys::JSValue,
    magic: c_int,
    func_data: *mut sys::JSValue,
) -> sys::JSValue {
    println!("resolve");

    let state = Box::from_raw(func_data as *mut Arc<Mutex<PromiseState>>);
    let mut state = state.lock();

    let args = slice::from_raw_parts(argv, argc as usize);
    let val = if args.len() > 0 {
        Box::into_raw(Box::new(args[0]))
    } else {
        Box::into_raw(Box::new(value::make_undefined()))
    };
    state.data = Some(val);

    if let Some(waker) = state.waker.take() {
        waker.wake();
    }

    value::make_undefined()
}
