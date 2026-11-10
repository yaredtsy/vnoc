//! VS-D67 condition 4: async is for I/O only. A handler that holds a tokio worker for more than ~50 ms (parsing, hashing,
//! a blocking read) starves everything else on that worker. `timed` measures every poll of a handler's future and warns
//! when one poll runs too long, so a blocking call shows up in the log the first time it happens.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use pin_project_lite::pin_project;

/// One poll longer than this means the handler held a worker (VS-D67).
pub const LIMIT: Duration = Duration::from_millis(50);

/// How many slow polls there have been in this process (for tests and for a status line).
pub static SLOW_POLLS: AtomicU32 = AtomicU32::new(0);

pin_project! {
    pub struct Timed<'c, F> {
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
            tracing::warn!("handler `{}` held a tokio worker for {took:?} in one poll (limit {:?}): move the blocking work to spawn_blocking", this.name, this.limit);
        }
        out
    }
}

/// Wrap a handler's future: `timed("structure", async move { … })`.
pub fn timed<F: Future>(name: &'static str, inner: F) -> Timed<'static, F> {
    Timed { name, limit: LIMIT, counter: &SLOW_POLLS, inner }
}

/// The same, with your own limit and counter (so a test does not share a global with other tests).
pub fn timed_with<'c, F: Future>(name: &'static str, limit: Duration, counter: &'c AtomicU32, inner: F) -> Timed<'c, F> {
    Timed { name, limit, counter, inner }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_handler_that_blocks_the_worker_is_counted() {
        let counter = AtomicU32::new(0);
        timed_with("blocks", Duration::from_millis(50), &counter, async { std::thread::sleep(Duration::from_millis(90)) }).await;
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn a_handler_that_waits_properly_is_not() {
        let counter = AtomicU32::new(0);
        // 120 ms of waiting, but every poll is short: the worker was free the whole time
        timed_with("waits", Duration::from_millis(50), &counter, async { tokio::time::sleep(Duration::from_millis(120)).await }).await;
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn blocking_work_moved_to_spawn_blocking_is_not() {
        let counter = AtomicU32::new(0);
        timed_with("offloaded", Duration::from_millis(50), &counter, async {
            tokio::task::spawn_blocking(|| std::thread::sleep(Duration::from_millis(90))).await.unwrap();
        })
        .await;
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }
}
