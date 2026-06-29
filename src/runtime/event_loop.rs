use crate::{
    context::Context,
    error::Error,
    function::Function,
    runtime::{timers::TimerQueue, Runtime},
    value::{Value, ValueRef},
};
use futures_util::{
    future::{FutureExt, LocalBoxFuture},
    stream::{FuturesUnordered, StreamExt},
};
use log::error;
use quickjs_sys as sys;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    ffi::c_void,
    future::{poll_fn, Future},
    pin::Pin,
    ptr,
    task::{Context as TaskContext, Poll, Waker},
};

struct Task {
    func: Function,
    this: Option<Value>,
    args: Vec<Value>,
}

impl Task {
    fn call(self) -> Result<Value, Error> {
        let Task { func, this, args } = self;
        let this = this.as_ref().map(|v| v.borrow());
        let args: Vec<ValueRef> = args.iter().map(|v| v.borrow()).collect();

        func.call(this, &args)
    }
}

pub(crate) struct AsyncState {
    next_id: Cell<u64>,
    tasks: RefCell<HashMap<u64, Task>>,
    ready: RefCell<VecDeque<u64>>,
    waker: RefCell<Option<Waker>>,
    timers: TimerQueue,
    spawned: RefCell<FuturesUnordered<LocalBoxFuture<'static, ()>>>,
    pending: RefCell<Vec<LocalBoxFuture<'static, ()>>>,
}

impl AsyncState {
    fn new() -> Self {
        AsyncState {
            next_id: Cell::new(0),
            tasks: RefCell::new(HashMap::new()),
            ready: RefCell::new(VecDeque::new()),
            waker: RefCell::new(None),
            timers: TimerQueue::new(),
            spawned: RefCell::new(FuturesUnordered::new()),
            pending: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn insert(&self, func: Function, this: Option<Value>, args: Vec<Value>) -> u64 {
        let id = self.next_id.get();
        self.next_id.set(id.wrapping_add(1));
        self.tasks
            .borrow_mut()
            .insert(id, Task { func, this, args });
        id
    }

    pub(crate) fn remove(&self, id: u64) {
        if self.tasks.borrow_mut().remove(&id).is_some() {
            self.timers.cancel();
        }
    }

    pub(crate) fn push_timer(&self, id: u64, delay: std::time::Duration) {
        self.timers.push(id, delay);
        self.waker();
    }

    pub(crate) fn ready(&self, id: u64) {
        self.ready.borrow_mut().push_back(id);
        self.waker();
    }

    pub(crate) fn spawn(&self, fut: LocalBoxFuture<'static, ()>) {
        self.pending.borrow_mut().push(fut);
        self.waker();
    }

    fn waker(&self) {
        if let Some(waker) = self.waker.borrow().as_ref() {
            waker.wake_by_ref();
        }
    }
}

struct OpaqueGuard<'a>(&'a Runtime);

impl Drop for OpaqueGuard<'_> {
    fn drop(&mut self) {
        unsafe { sys::JS_SetRuntimeOpaque(self.0.as_raw(), ptr::null_mut()) };
    }
}

impl Runtime {
    pub fn spawn_local(&self, fut: impl Future<Output = ()> + 'static) {
        if let Some(state) = self.async_state() {
            state.spawn(fut.boxed_local());
        }
    }

    pub async fn run_until<F: Future + 'static>(&self, fut: F) -> F::Output {
        let state = Box::new(AsyncState::new());
        let _guard = self.enter(&state);

        let mut fut = Box::pin(fut);
        poll_fn(|cx| self.drive(cx, &state, fut.as_mut())).await
    }

    pub async fn run(&self) {
        let state = Box::new(AsyncState::new());
        let _guard = self.enter(&state);

        poll_fn(|cx| self.drive_idle(cx, &state)).await
    }

    fn enter(&self, state: &AsyncState) -> OpaqueGuard<'_> {
        assert!(
            unsafe { sys::JS_GetRuntimeOpaque(self.as_raw()).is_null() },
            "nested run_until/run is not supported"
        );

        let ptr = (state as *const AsyncState).cast::<c_void>().cast_mut();
        unsafe { sys::JS_SetRuntimeOpaque(self.as_raw(), ptr) };
        OpaqueGuard(self)
    }

    fn drive<F: Future>(
        &self,
        cx: &mut TaskContext<'_>,
        state: &AsyncState,
        mut fut: Pin<&mut F>,
    ) -> Poll<F::Output> {
        *state.waker.borrow_mut() = Some(cx.waker().clone());

        loop {
            let mut progress = false;

            self.execute_job(&mut progress);
            if let Poll::Ready(out) = fut.as_mut().poll(cx) {
                return Poll::Ready(out);
            }

            self.execute_job(&mut progress);
            self.poll_tasks(cx, state, &mut progress);

            if !progress {
                return Poll::Pending;
            }
        }
    }

    fn drive_idle(&self, cx: &mut TaskContext<'_>, state: &AsyncState) -> Poll<()> {
        *state.waker.borrow_mut() = Some(cx.waker().clone());

        loop {
            let mut progress = false;

            self.execute_job(&mut progress);
            self.poll_tasks(cx, state, &mut progress);

            if !progress {
                break;
            }
        }

        let idle = !unsafe { sys::JS_IsJobPending(self.as_raw()) }
            && state.spawned.borrow().is_empty()
            && state.ready.borrow().is_empty()
            && state.timers.is_idle();
        if idle {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }

    fn poll_tasks(&self, cx: &mut TaskContext<'_>, state: &AsyncState, progress: &mut bool) {
        if state.timers.poll(cx, |id| {
            let live = state.tasks.borrow().contains_key(&id);
            if live {
                state.ready(id);
            }
            live
        }) {
            *progress = true;
        }

        {
            let mut pending = state.pending.borrow_mut();
            if !pending.is_empty() {
                let spawned = state.spawned.borrow_mut();

                for fut in pending.drain(..) {
                    spawned.push(fut);
                }
            }
        }

        loop {
            let polled = state.spawned.borrow_mut().poll_next_unpin(cx);
            match polled {
                Poll::Ready(Some(())) => *progress = true,
                _ => break,
            }
        }

        loop {
            let id = state.ready.borrow_mut().pop_front();
            let Some(id) = id else { break };

            let task = state.tasks.borrow_mut().remove(&id);
            if let Some(task) = task {
                *progress = true;
                if let Err(e) = task.call() {
                    log::error!("{e}");
                }

                self.execute_job(progress);
            }
        }
    }

    fn execute_job(&self, progress: &mut bool) {
        let rt = self.as_raw();

        while unsafe { sys::JS_IsJobPending(rt) } {
            *progress = true;
            let mut ptr: *mut sys::JSContext = ptr::null_mut();

            let ret = unsafe { sys::JS_ExecutePendingJob(rt, &mut ptr) };
            if ret < 0 && !ptr.is_null() {
                if let Some(ctx) = unsafe { Context::from_opaque(ptr) } {
                    error!("{}", ctx.catch());
                }
            }
        }
    }
}
