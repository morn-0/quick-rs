use futures_timer::Delay;
use std::{
    cell::{Cell, RefCell},
    cmp::Reverse,
    collections::BinaryHeap,
    future::Future,
    pin::Pin,
    task::{Context as TaskContext, Poll},
    time::{Duration, Instant},
};

pub(crate) struct TimerQueue {
    heap: RefCell<BinaryHeap<Reverse<(Instant, u64)>>>,
    delay: RefCell<Option<Pin<Box<Delay>>>>,
    armed: Cell<Option<Instant>>,
    live: Cell<usize>,
}

impl TimerQueue {
    pub(crate) fn new() -> Self {
        TimerQueue {
            heap: RefCell::new(BinaryHeap::new()),
            delay: RefCell::new(None),
            armed: Cell::new(None),
            live: Cell::new(0),
        }
    }

    pub(crate) fn push(&self, id: u64, delay: Duration) {
        let deadline = Instant::now() + delay;
        self.heap.borrow_mut().push(Reverse((deadline, id)));

        self.live.set(self.live.get() + 1);
    }

    pub(crate) fn cancel(&self) {
        self.live.set(self.live.get().saturating_sub(1));
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.live.get() == 0
    }

    pub(crate) fn poll(&self, cx: &mut TaskContext<'_>, mut sink: impl FnMut(u64) -> bool) -> bool {
        let now = Instant::now();
        let mut progress = false;

        loop {
            let expired = {
                let mut heap = self.heap.borrow_mut();
                match heap.peek() {
                    Some(Reverse((deadline, _))) if *deadline <= now => {
                        heap.pop().map(|Reverse((_, id))| id)
                    }
                    _ => None,
                }
            };
            let Some(id) = expired else { break };

            progress = true;
            if sink(id) {
                self.live.set(self.live.get().saturating_sub(1));
            }
        }

        let next = self
            .heap
            .borrow()
            .peek()
            .map(|Reverse((deadline, _))| *deadline);

        match next {
            Some(deadline) => {
                if self.armed.get() != Some(deadline) {
                    self.armed.set(Some(deadline));

                    let remaining = deadline.saturating_duration_since(now);
                    *self.delay.borrow_mut() = Some(Box::pin(Delay::new(remaining)));
                }
                if let Some(delay) = self.delay.borrow_mut().as_mut() {
                    if delay.as_mut().poll(cx) == Poll::Ready(()) {
                        progress = true;
                    }
                }
            }
            None => {
                self.armed.set(None);
                *self.delay.borrow_mut() = None;
            }
        }

        progress
    }
}
