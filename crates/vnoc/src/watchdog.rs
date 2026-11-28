//! VS-D67 condition 4: async is for I/O only. A handler that holds a tokio worker for more than ~50 ms (parsing, hashing,
//! a blocking read) starves everything else on that worker. `timed` measures every poll of a handler's future and warns
//! when one poll runs too long, so a blocking call shows up in the log the first time it happens.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, Once};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use pin_project_lite::pin_project;

/// One poll longer than this means the handler held a worker (VS-D67).
pub const LIMIT: Duration = Duration::from_millis(50);

/// How many slow polls there have been in this process (for tests and for a status line).
pub static SLOW_POLLS: AtomicU32 = AtomicU32::new(0);

/// The handlers that have started and not ended: id → (name, when). A handler whose future is waiting holds one of the core's request slots (`ConcurrencyLayer`), and with all of them
/// held the core reads no more requests (not even a `vnoc/ping`), so what is still running is the first thing to know about a core that stopped answering.
static IN_FLIGHT: Mutex<BTreeMap<u64, (&'static str, Instant)>> = Mutex::new(BTreeMap::new());
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// A handler counts as in flight while this lives (as long as its `Timed` does).
struct InFlight(u64);

impl InFlight {
    fn start(name: &'static str) -> InFlight {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        IN_FLIGHT
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, (name, Instant::now()));
        InFlight(id)
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        IN_FLIGHT
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.0);
    }
}

/// The handlers that have been running for at least `age`, oldest first, as `name (seconds)`.
pub fn running_for(age: Duration) -> Vec<String> {
    let now = Instant::now();
    let mut held: Vec<(Duration, &'static str)> = IN_FLIGHT
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .values()
        .map(|(name, since)| (now - *since, *name))
        .filter(|(took, _)| *took >= age)
        .collect();
    held.sort_by(|a, b| b.0.cmp(&a.0));
    held.iter()
        .map(|(took, name)| format!("{name} ({}s)", took.as_secs()))
        .collect()
}

/// Starts the thread that says, every 5 s, which handlers have been running for 10 s or more. It is a thread of its own, not a task: it still runs when every worker of the runtime is
/// busy or every request slot is held, which is the case it is for. Once per process.
pub fn report_slow_requests() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = std::thread::Builder::new()
            .name("vnoc-watchdog".into())
            .spawn(|| {
                loop {
                    std::thread::sleep(Duration::from_secs(5));
                    let held = running_for(Duration::from_secs(10));
                    if !held.is_empty() {
                        tracing::warn!("requests running for 10 s or more: {}", held.join(", "));
                    }
                }
            });
    });
}

pin_project! {
    pub struct Timed<'c, F> {
        _in_flight: InFlight,
        name: &'static str,
        limit: Duration,
        counter: &'c AtomicU32,
        #[pin]
        inner: F,
    }
}

impl<F: Future> Future for Timed<'_, F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        let this = self.project();
        let started = Instant::now();
        let out = this.inner.poll(cx);
        let took = started.elapsed();
        if took > *this.limit {
            this.counter.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(
                "handler `{}` held a tokio worker for {took:?} in one poll (limit {:?}): move the blocking work to spawn_blocking",
                this.name,
                this.limit
            );
        }
        out
    }
}

/// Wrap a handler's future: `timed("structure", async move { … })`.
pub fn timed<F: Future>(name: &'static str, inner: F) -> Timed<'static, F> {
    Timed {
        _in_flight: InFlight::start(name),
        name,
        limit: LIMIT,
        counter: &SLOW_POLLS,
        inner,
    }
}

/// The same, with your own limit and counter (so a test does not share a global with other tests).
pub fn timed_with<'c, F: Future>(
    name: &'static str,
    limit: Duration,
    counter: &'c AtomicU32,
    inner: F,
) -> Timed<'c, F> {
    Timed {
        _in_flight: InFlight::start(name),
        name,
        limit,
        counter,
        inner,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_handler_that_blocks_the_worker_is_counted() {
        let counter = AtomicU32::new(0);
        timed_with("blocks", Duration::from_millis(50), &counter, async {
            std::thread::sleep(Duration::from_millis(90))
        })
        .await;
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn a_handler_that_has_not_ended_is_named_with_its_age_and_leaves_when_it_does() {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let running = tokio::spawn(timed_with(
            "stuck-for-the-test",
            LIMIT,
            &COUNTER,
            tokio::time::sleep(Duration::from_millis(400)),
        ));
        tokio::time::sleep(Duration::from_millis(200)).await;
        let held = running_for(Duration::from_millis(100));
        assert!(
            held.iter().any(|h| h.starts_with("stuck-for-the-test")),
            "{held:?}"
        );
        running.await.unwrap();
        assert!(
            !running_for(Duration::ZERO)
                .iter()
                .any(|h| h.starts_with("stuck-for-the-test"))
        );
    }

    #[tokio::test]
    async fn a_handler_that_waits_properly_is_not() {
        let counter = AtomicU32::new(0);
        // 120 ms of waiting, but every poll is short: the worker was free the whole time
        timed_with("waits", Duration::from_millis(50), &counter, async {
            tokio::time::sleep(Duration::from_millis(120)).await
        })
        .await;
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn blocking_work_moved_to_spawn_blocking_is_not() {
        let counter = AtomicU32::new(0);
        timed_with("offloaded", Duration::from_millis(50), &counter, async {
            tokio::task::spawn_blocking(|| std::thread::sleep(Duration::from_millis(90)))
                .await
                .unwrap();
        })
        .await;
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }
}
