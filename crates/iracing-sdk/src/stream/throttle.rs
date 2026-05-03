//! Stream throttling utilities

use futures::{Stream, ready};
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{Interval, interval};

/// Extension trait to add throttling to any Stream
pub trait ThrottleExt: Stream {
    /// Throttle the stream to emit at most once per interval
    ///
    /// Uses "latest-wins" semantics - if multiple items arrive
    /// during an interval, only the latest is emitted.
    fn throttle(self, duration: Duration) -> Throttle<Self>
    where
        Self: Sized,
    {
        Throttle::new(self, duration)
    }
}

impl<T: Stream> ThrottleExt for T {}

// Use pin_project_lite macro syntax
pin_project! {
    /// A stream combinator that throttles emission rate
    pub struct Throttle<S: Stream> {
        #[pin]
        stream: S,
        interval: Interval,
        pending: Option<S::Item>,
    }
}

impl<S: Stream> Throttle<S> {
    /// Create a new throttled stream
    pub fn new(stream: S, duration: Duration) -> Self {
        let mut interval = interval(duration);
        // Set missed tick behavior to delay (don't burst)
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        Self {
            stream,
            interval,
            pending: None,
        }
    }
}

impl<S: Stream> Stream for Throttle<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // Wait for interval tick
        ready!(this.interval.poll_tick(cx));

        // Drain all available items, keeping only the latest
        loop {
            match this.stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(item)) => {
                    *this.pending = Some(item);
                    // Continue draining
                }
                Poll::Ready(None) => {
                    // Stream ended
                    return Poll::Ready(this.pending.take());
                }
                Poll::Pending => {
                    // No more items available right now
                    return Poll::Ready(this.pending.take());
                }
            }
        }
    }
}
