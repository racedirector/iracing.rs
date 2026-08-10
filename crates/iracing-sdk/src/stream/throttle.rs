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
                    return match this.pending.take() {
                        Some(item) => Poll::Ready(Some(item)),
                        None => Poll::Pending,
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ThrottleExt;
    use futures::{StreamExt, stream};
    use std::task::Poll;
    use std::time::Duration;
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::UnboundedReceiverStream;

    #[tokio::test(start_paused = true)]
    async fn first_tick_emits_the_latest_item_that_is_immediately_available() {
        let mut throttled = stream::iter([1, 2, 3]).throttle(Duration::from_millis(100));

        assert_eq!(throttled.next().await, Some(3));
        assert_eq!(throttled.next().await, None);
    }

    #[tokio::test(start_paused = true)]
    async fn items_arriving_between_ticks_are_coalesced_to_the_latest_value() {
        let (sender, receiver) = mpsc::unbounded_channel();
        sender.send(1).unwrap();

        let mut throttled =
            UnboundedReceiverStream::new(receiver).throttle(Duration::from_millis(100));

        assert_eq!(throttled.next().await, Some(1));

        let send_in_between_values = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(25)).await;
            sender.send(2).unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;
            sender.send(3).unwrap();
        });

        assert_eq!(throttled.next().await, Some(3));
        send_in_between_values.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn upstream_pending_does_not_end_the_throttled_stream() {
        let (_sender, receiver) = mpsc::unbounded_channel::<i32>();
        let mut throttled =
            UnboundedReceiverStream::new(receiver).throttle(Duration::from_millis(100));

        assert_eq!(futures::poll!(throttled.next()), Poll::Pending);
    }

    #[tokio::test(start_paused = true)]
    async fn item_after_upstream_pending_is_eventually_emitted() {
        let (sender, receiver) = mpsc::unbounded_channel();
        let mut throttled =
            UnboundedReceiverStream::new(receiver).throttle(Duration::from_millis(100));

        assert_eq!(futures::poll!(throttled.next()), Poll::Pending);

        sender.send(42).unwrap();

        assert_eq!(throttled.next().await, Some(42));
    }

    #[tokio::test(start_paused = true)]
    async fn upstream_end_emits_pending_item_then_ends() {
        let (sender, receiver) = mpsc::unbounded_channel();
        sender.send(1).unwrap();
        sender.send(2).unwrap();
        drop(sender);

        let mut throttled =
            UnboundedReceiverStream::new(receiver).throttle(Duration::from_millis(100));

        assert_eq!(throttled.next().await, Some(2));
        assert_eq!(throttled.next().await, None);
    }

    #[tokio::test(start_paused = true)]
    async fn pending_poll_duplicates() {
        let (sender, receiver) = mpsc::unbounded_channel();
        let mut throttled =
            UnboundedReceiverStream::new(receiver).throttle(Duration::from_millis(100));

        // Stream is in pending state
        assert_eq!(futures::poll!(throttled.next()), Poll::Pending);

        // Send value to stream
        sender.send(42).unwrap();

        // Stream received value
        assert_eq!(throttled.next().await, Some(42));

        // Stream goes to pending
        assert_eq!(futures::poll!(throttled.next()), Poll::Pending);

        // Send two values
        sender.send(43).unwrap();
        sender.send(44).unwrap();

        // Stream received latest
        assert_eq!(throttled.next().await, Some(44));

        // Stream goes to pending
        assert_eq!(futures::poll!(throttled.next()), Poll::Pending);

        // Await the next value, but send a value after the throttle duration
        tokio::select! {
            // Wait for a duration longer than the throttle
            _ = tokio::time::sleep(Duration::from_millis(150)) => {
                // After the duration, send a value
                sender.send(45).unwrap();
            },
            // Wait for a value to come back on the stream
            _ = throttled.next() => {}
        };

        assert_eq!(throttled.next().await, Some(45));

        drop(sender);

        assert_eq!(throttled.next().await, None);
    }
}
