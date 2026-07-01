#[cfg(feature = "async")]
use crate::runtime::event_loop::AsyncState;
use quickjs_sys as sys;
#[cfg(feature = "async")]
use std::cell::Cell;
use std::{
    any::TypeId,
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    os::raw::c_int,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub(crate) struct RuntimeState {
    claxxs: RefCell<HashMap<TypeId, sys::JSClassID>>,
    interrupt: Arc<InterruptState>,
    #[cfg(feature = "async")]
    async_state: Cell<*const AsyncState>,
}

impl RuntimeState {
    pub(crate) fn new() -> Self {
        RuntimeState {
            claxxs: RefCell::new(HashMap::new()),
            interrupt: Arc::new(InterruptState::new()),
            #[cfg(feature = "async")]
            async_state: Cell::new(std::ptr::null()),
        }
    }

    pub(crate) fn claxxs(&self) -> &RefCell<HashMap<TypeId, sys::JSClassID>> {
        &self.claxxs
    }

    pub(crate) fn interrupt(&self, timeout: Option<Duration>) -> Interrupt {
        self.interrupt.renew(timeout);
        Interrupt(self.interrupt.clone())
    }

    pub(crate) fn interrupt_triggered(&self) -> bool {
        self.interrupt.triggered()
    }

    #[cfg(feature = "async")]
    pub(crate) fn enter_async(&self, state: &AsyncState) {
        assert!(
            self.async_state.get().is_null(),
            "nested run_until/run is not supported"
        );
        self.async_state.set(state as *const AsyncState);
    }

    #[cfg(feature = "async")]
    pub(crate) fn exit_async(&self) {
        self.async_state.set(std::ptr::null());
    }

    #[cfg(feature = "async")]
    pub(crate) fn async_state(&self) -> Option<&AsyncState> {
        unsafe { self.async_state.get().as_ref() }
    }
}

pub(crate) unsafe extern "C" fn interrupt_handler(
    _rt: *mut sys::JSRuntime,
    opaque: *mut c_void,
) -> c_int {
    let state = unsafe { &*(opaque as *const RuntimeState) };
    state.interrupt.triggered() as c_int
}

struct InterruptState {
    cancelled: AtomicBool,
    origin: Instant,
    deadline: AtomicU64,
}

impl InterruptState {
    fn new() -> Self {
        InterruptState {
            cancelled: AtomicBool::new(false),
            deadline: AtomicU64::new(0),
            origin: Instant::now(),
        }
    }

    fn renew(&self, timeout: Option<Duration>) {
        self.cancelled.store(false, Ordering::Relaxed);
        let deadline = match timeout {
            Some(d) => (self.origin.elapsed().saturating_add(d).as_nanos() as u64).max(1),
            None => 0,
        };
        self.deadline.store(deadline, Ordering::Relaxed);
    }

    fn clear(&self) {
        self.deadline.store(0, Ordering::Relaxed);
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    fn triggered(&self) -> bool {
        if self.cancelled.load(Ordering::Relaxed) {
            return true;
        }
        let deadline = self.deadline.load(Ordering::Relaxed);
        deadline != 0 && self.origin.elapsed().as_nanos() as u64 >= deadline
    }
}

pub struct Interrupt(Arc<InterruptState>);

impl Interrupt {
    pub fn cancel(&self) {
        self.0.cancel();
    }
}

impl Drop for Interrupt {
    fn drop(&mut self) {
        self.0.clear();
    }
}
