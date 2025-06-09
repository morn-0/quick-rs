use flume::{unbounded, Receiver, Sender};
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering as AtomicOrdering},
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
#[repr(transparent)]
pub struct TimerId(pub u64);

enum Command {
    InsertTimer {
        id: TimerId,
        delay: Duration,
        cb: Pin<Box<dyn Future<Output = ()> + Send>>,
    },
    RemoveTimer {
        id: TimerId,
    },
}

#[derive(Debug, Eq)]
struct Timer {
    id: TimerId,
    expire: Instant,
}

impl PartialEq for Timer {
    fn eq(&self, other: &Self) -> bool {
        self.expire == other.expire && self.id == other.id
    }
}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .expire
            .cmp(&self.expire)
            .then_with(|| other.id.0.cmp(&self.id.0))
    }
}

impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct TimerCallback {
    cb: Pin<Box<dyn Future<Output = ()> + Send>>,
    expire: Instant,
}

const WHEEL_BITS: usize = 8;
const WHEEL_SIZE: usize = 1 << WHEEL_BITS;
const WHEEL_MASK: u64 = (WHEEL_SIZE - 1) as u64;
const TICK_MS: u64 = 1;
const HEAP_THRESHOLD: Duration = Duration::from_millis(TICK_MS * WHEEL_SIZE as u64);
const MAX_BATCH_SIZE: usize = 32;

pub struct TimerHeap {
    command: Sender<Command>,
    id_counter: Arc<AtomicU64>,
}

impl TimerHeap {
    pub fn new() -> Self {
        let (tx, rx) = unbounded::<Command>();
        let id_counter = Arc::new(AtomicU64::new(0));

        task::spawn_local(Self::run(rx));

        Self {
            command: tx,
            id_counter,
        }
    }

    pub fn insert_timer<F>(&self, delay: Duration, cb: F) -> TimerId
    where
        F: Future<Output = ()> + Send + 'static,
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
        let _ = self.command.send(Command::RemoveTimer { id });
    }

    async fn run(rx: Receiver<Command>) {
        let mut heap: BinaryHeap<Timer> = BinaryHeap::new();
        let mut wheel: Vec<Vec<Timer>> = (0..WHEEL_SIZE).map(|_| Vec::new()).collect();
        let mut cbs: HashMap<TimerId, TimerCallback> = HashMap::with_capacity(1024);

        // 维护时间轮计数器, 避免每次遍历
        let mut wheel_timer_count: usize = 0;

        // 重用缓冲区, 减少内存分配
        let mut expired_buffer: Vec<Timer> = Vec::with_capacity(64);
        let mut commands_buffer: Vec<Command> = Vec::with_capacity(MAX_BATCH_SIZE);

        let start_time = Instant::now();
        let mut current_tick = 0u64;
        let mut next_wake: Option<Instant> = None;
        let mut next_tick_wake: Option<Instant> = None;

        loop {
            let now = Instant::now();

            // 时间轮滴答计算, 添加饱和保护
            let now_duration = now.saturating_duration_since(start_time);
            let new_tick = now_duration.as_millis() as u64 / TICK_MS;

            // 批量处理多个 tick, 添加溢出保护
            if current_tick < new_tick {
                let tick_end = new_tick.min(current_tick.saturating_add(WHEEL_SIZE as u64));

                while current_tick < tick_end {
                    current_tick += 1;
                    let slot = (current_tick & WHEEL_MASK) as usize;

                    if !wheel[slot].is_empty() {
                        let mut slot_timers = std::mem::take(&mut wheel[slot]);
                        wheel_timer_count -= slot_timers.len();

                        slot_timers.retain(|timer| cbs.contains_key(&timer.id));
                        wheel_timer_count += slot_timers.len();

                        for timer in slot_timers {
                            heap.push(timer);
                        }
                    }
                }

                // 时间轮状态改变, 需要重新计算唤醒时间
                next_tick_wake = None;
            }

            // 批量处理到期的定时器, 重用缓冲区
            expired_buffer.clear();
            while let Some(timer) = heap.peek() {
                if timer.expire <= now {
                    expired_buffer.push(heap.pop().unwrap());
                } else {
                    break;
                }
            }

            // 执行到期的回调
            for timer in expired_buffer.iter() {
                if let Some(timer_cb) = cbs.remove(&timer.id) {
                    if timer_cb.expire == timer.expire {
                        timer_cb.cb.await;
                    }
                }
            }

            // 唤醒时间计算
            let heap_timeout = heap.peek().map(|t| t.expire);

            // 使用计数器检查时间轮
            let tick_timeout = if wheel_timer_count > 0 {
                if next_tick_wake.is_none() {
                    next_tick_wake =
                        Some(start_time + Duration::from_millis((current_tick + 1) * TICK_MS));
                }
                next_tick_wake
            } else {
                next_tick_wake = None;
                None
            };

            let timeout = match (heap_timeout, tick_timeout) {
                (Some(h), Some(t)) => Some(h.min(t)),
                (Some(h), None) => Some(h),
                (None, Some(t)) => Some(t),
                (None, None) => None,
            };

            if timeout != next_wake {
                next_wake = timeout;
            }

            // 批量处理命令
            commands_buffer.clear();

            // 首先获取一个命令 (可能需要等待)
            let first_command = match timeout {
                Some(wake_time) if wake_time > now => {
                    tokio::select! {
                        command = rx.recv_async() => command,
                        _ = sleep_until(wake_time) => continue,
                    }
                }
                _ => rx.recv_async().await,
            };

            match first_command {
                Ok(cmd) => {
                    commands_buffer.push(cmd);

                    // 尝试收集更多命令进行批处理
                    while commands_buffer.len() < MAX_BATCH_SIZE {
                        if let Ok(cmd) = rx.try_recv() {
                            commands_buffer.push(cmd);
                        } else {
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("TimerHeap channel closed: {e}");
                    break;
                }
            }

            // 批量处理所有命令
            for command in commands_buffer.drain(..) {
                match command {
                    Command::InsertTimer { id, delay, cb } => {
                        let expire = now + delay;
                        let timer = Timer { id, expire };
                        let timer_cb = TimerCallback { cb, expire };

                        if delay <= HEAP_THRESHOLD {
                            heap.push(timer);
                        } else {
                            let ticks_from_now = delay.as_millis() as u64 / TICK_MS;
                            let max_wheel_ticks = WHEEL_SIZE as u64;

                            if ticks_from_now <= max_wheel_ticks {
                                let target_tick = current_tick.saturating_add(ticks_from_now);
                                let slot = (target_tick & WHEEL_MASK) as usize;
                                wheel[slot].push(timer);
                                wheel_timer_count += 1;

                                // 时间轮状态改变, 重置缓存
                                next_tick_wake = None;
                            } else {
                                heap.push(timer);
                            }
                        }

                        cbs.insert(id, timer_cb);

                        if matches!(next_wake, Some(wake) if expire < wake) || next_wake.is_none() {
                            next_wake = Some(expire);
                        }
                    }
                    Command::RemoveTimer { id } => {
                        cbs.remove(&id);
                    }
                }
            }
        }
    }
}

impl Default for TimerHeap {
    fn default() -> Self {
        Self::new()
    }
}
