use flume::{Receiver, Sender};
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicI32, Ordering as AtomicOrdering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    task,
    time::{sleep_until, Instant},
};
use tracing::error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(pub i32);

enum Command {
    InsertTimer {
        id: TimerId,
        delay: Duration,
        cb: Pin<Box<dyn Future<Output = ()>>>,
    },
    RemoveTimer {
        id: TimerId,
    },
}

struct Timer {
    id: TimerId,
    expire: Instant,
}

impl Eq for Timer {}

impl PartialEq for Timer {
    fn eq(&self, other: &Self) -> bool {
        self.expire == other.expire
    }
}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> Ordering {
        other.expire.cmp(&self.expire)
    }
}

impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct TimerCallback {
    cb: Pin<Box<dyn Future<Output = ()>>>,
    expire: Instant,
}

pub struct TimerHeap {
    command: Sender<Command>,
    id_counter: Arc<AtomicI32>,
}

impl TimerHeap {
    pub fn new() -> Self {
        let (tx, rx) = flume::unbounded::<Command>();
        let id_counter = Arc::new(AtomicI32::new(0));

        task::spawn_local(Self::run(rx));

        Self {
            command: tx,
            id_counter,
        }
    }

    pub fn insert_timer<F>(&self, delay: Duration, cb: F) -> TimerId
    where
        F: Future<Output = ()> + 'static,
    {
        let id = TimerId(self.id_counter.fetch_add(1, AtomicOrdering::Relaxed));

        let command = Command::InsertTimer {
            id,
            delay,
            cb: Box::pin(cb),
        };

        let _ = self.command.send(command);
        id
    }

    pub fn remove_timer(&self, id: TimerId) {
        let command = Command::RemoveTimer { id };
        let _ = self.command.send(command);
    }

    async fn run(rx: Receiver<Command>) {
        let mut heap: BinaryHeap<Timer> = BinaryHeap::new();
        let mut cbs: HashMap<TimerId, TimerCallback> = HashMap::new();

        let mut next_wake: Option<Instant> = None;

        loop {
            let now = Instant::now();
            while let Some(timer) = heap.peek() {
                if timer.expire <= now {
                    let timer = heap.pop().unwrap();
                    if let Some(timer_cb) = cbs.remove(&timer.id) {
                        if timer_cb.expire == timer.expire {
                            task::spawn_local(timer_cb.cb);
                        }
                    }
                } else {
                    break;
                }
            }

            let timeout = heap.peek().map(|timer| timer.expire);

            if timeout != next_wake {
                next_wake = timeout;
            }

            let command = match timeout {
                Some(wake_time) if wake_time > Instant::now() => {
                    tokio::select! {
                        command = rx.recv_async() => command,
                        _ = sleep_until(wake_time) => continue,
                    }
                }
                _ => rx.recv_async().await,
            };

            match command {
                Ok(Command::InsertTimer { id, delay, cb }) => {
                    let expire = Instant::now() + delay;

                    let timer = Timer { id, expire };
                    let timer_cb = TimerCallback { cb, expire };

                    heap.push(timer);
                    cbs.insert(id, timer_cb);

                    if Some(expire) < next_wake || next_wake.is_none() {
                        next_wake = Some(expire);
                    }
                }
                Ok(Command::RemoveTimer { id }) => {
                    cbs.remove(&id);
                }
                Err(e) => {
                    error!("TimerHeap({e})");
                    break;
                }
            }
        }
    }
}
