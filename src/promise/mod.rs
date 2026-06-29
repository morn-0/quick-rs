use crate::{error::Error, function::Function, value::Value};
use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
    task::{Context, Poll, Waker},
};

pub mod timer;

#[derive(Default)]
struct State {
    value: Option<Result<Value, Value>>,
    waker: Option<Waker>,
}

pub struct Promise {
    value: Value,
    state: Rc<RefCell<State>>,
    registered: bool,
}

impl Promise {
    pub fn new(value: Value) -> Self {
        Promise {
            value,
            state: Rc::new(RefCell::new(State::default())),
            registered: false,
        }
    }

    fn register(&self) -> Result<(), Error> {
        let ctx = self.value.context();
        let then = Function::new(self.value.get_property("then")?);

        let on_ok = {
            let weak = Rc::downgrade(&self.state);

            ctx.make_closure(1, move |ctx, _this, args| {
                let value = args
                    .first()
                    .map(|v| v.to_owned())
                    .unwrap_or_else(|| ctx.make_undefined());
                settle(&weak, Ok(value));
                Ok(ctx.make_undefined())
            })
        };

        let on_err = {
            let weak = Rc::downgrade(&self.state);

            ctx.make_closure(1, move |ctx, _this, args| {
                let value = args
                    .first()
                    .map(|v| v.to_owned())
                    .unwrap_or_else(|| ctx.make_undefined());
                settle(&weak, Err(value));
                Ok(ctx.make_undefined())
            })
        };

        then.call(
            Some(self.value.borrow()),
            &[on_ok.borrow(), on_err.borrow()],
        )?;
        Ok(())
    }
}

fn settle(weak: &Weak<RefCell<State>>, value: Result<Value, Value>) {
    if let Some(state) = weak.upgrade() {
        let mut state = state.borrow_mut();
        state.value = Some(value);

        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
}

impl Future for Promise {
    type Output = Result<Value, Value>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        {
            let mut state = this.state.borrow_mut();
            if let Some(result) = state.value.take() {
                return Poll::Ready(result);
            }
            state.waker = Some(cx.waker().clone());
        }

        if !this.registered {
            this.registered = true;
            if let Err(e) = this.register() {
                let ctx = this.value.context();

                let reason = ctx
                    .make_string(e.to_string())
                    .unwrap_or_else(|_| ctx.make_undefined());
                return Poll::Ready(Err(reason));
            }
        }

        Poll::Pending
    }
}
