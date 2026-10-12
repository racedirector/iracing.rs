use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::{FramePacket, IRacingSDKError};

/// Controls when the telemetry task may read a frame and how it publishes the result.
///
/// Live consumers want the newest snapshot, so acquisition continues independently
/// of individual subscribers. Alternate delivery experiments can provide a
/// request-specific permit through the source-independent policy contract.
///
/// The telemetry task uses a delivery policy in this order:
///
/// 1. [`DeliveryPolicy::acquire`] grants permission to perform one provider read.
/// 2. A successful frame is passed to [`DeliveryPolicy::deliver`] with that permit.
/// 3. Provider EOF is passed to [`DeliveryPolicy::end`] with that permit.
/// 4. Provider errors may be passed to [`DeliveryPolicy::error`] with that permit.
///
/// A permit belongs to exactly one provider-read attempt. Policies can use a
/// zero-sized permit for unconditional live reads or carry a response channel
/// that ties one replay request to one result.
#[async_trait]
pub(crate) trait DeliveryPolicy: Send {
    /// Permission and response state associated with one provider read.
    ///
    /// The telemetry task obtains this value before calling
    /// `Provider::next_frame` and consumes it when reporting the result.
    type Permit: Send;

    /// Wait until the policy allows one provider read.
    ///
    /// Returning `Some(permit)` authorizes exactly one call to
    /// `Provider::next_frame`. Returning `None` tells the telemetry task to
    /// stop, normally because cancellation was requested or the replay-demand
    /// channel was closed.
    async fn acquire(&mut self, cancel: &CancellationToken) -> Option<Self::Permit>;

    /// Publish one successfully read frame.
    ///
    /// The permit is the value returned by the preceding
    /// [`DeliveryPolicy::acquire`] call. Returning `true` allows the telemetry
    /// task to acquire another permit. Returning `false` stops the task,
    /// normally because the receiving side has been dropped.
    async fn deliver(&mut self, permit: Self::Permit, frame: FramePacket) -> bool;

    /// Report that the provider reached its permanent end.
    ///
    /// Live delivery clears its latest snapshot. On-demand replay answers the
    /// outstanding request with `None`. This consumes the final permit; the
    /// telemetry task does not perform another provider read afterward.
    async fn end(&mut self, permit: Self::Permit);

    /// Report an error produced by the permitted provider read.
    ///
    /// Returning `true` indicates that the policy can continue with another
    /// permit. Returning `false` treats the error as terminal. A policy that
    /// carries a request-specific response channel should answer that request
    /// before returning.
    async fn error(&mut self, permit: Self::Permit, error: IRacingSDKError) -> bool;
}

/// Latest-wins delivery for live telemetry.
///
/// Reads are always permitted while the task is active. Each delivered frame
/// replaces the previous value in a watch channel, so slow subscribers may
/// intentionally skip intermediate frames and observe only the newest snapshot.
pub(crate) struct LatestDelivery {
    /// Watch sender holding the current live frame, or `None` after live EOF.
    frames: watch::Sender<Option<Arc<FramePacket>>>,
}

impl LatestDelivery {
    /// Create a live delivery policy backed by the supplied watch channel.
    pub(crate) fn new(frames: watch::Sender<Option<Arc<FramePacket>>>) -> Self {
        Self { frames }
    }
}

#[async_trait]
impl DeliveryPolicy for LatestDelivery {
    /// Live reads need no request-specific state.
    type Permit = ();

    async fn acquire(&mut self, cancel: &CancellationToken) -> Option<()> {
        // LiveProvider supplies source pacing by waiting for shared-memory
        // updates, so this policy only needs to reject reads after cancellation.
        (!cancel.is_cancelled()).then_some(())
    }

    async fn deliver(&mut self, _permit: Self::Permit, frame: FramePacket) -> bool {
        // watch intentionally replaces any frame a slow subscriber has not yet
        // observed; live telemetry represents current state, not an event log.
        self.frames.send(Some(Arc::new(frame))).is_ok()
    }

    async fn end(&mut self, _permit: Self::Permit) {
        // None is the established live-channel signal that the current source
        // is no longer producing frames.
        let _ = self.frames.send(None);
    }

    async fn error(&mut self, _permit: Self::Permit, _error: IRacingSDKError) -> bool {
        // A transient live error does not erase the latest valid snapshot.
        // Continue only while at least one receiver still exists.
        !self.frames.is_closed()
    }
}

#[cfg(test)]
mod tests {
    //! Live latest-value delivery characterization.

    use std::sync::Arc;

    use tokio::sync::watch;
    use tokio_util::sync::CancellationToken;

    use crate::{FramePacket, IRacingSDKError};

    use super::{DeliveryPolicy, LatestDelivery};

    /// Construct a minimal packet whose tick identifies it in assertions.
    fn frame(tick: u32) -> FramePacket {
        FramePacket::new(
            vec![0],
            tick,
            0,
            Arc::new(
                crate::test_utils::layout([], 1)
                    .expect("an empty telemetry layout should be valid"),
            ),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn latest_delivery_retains_only_the_latest_frame() {
        // Arrange a live watch channel and an uncancelled policy.
        let (frames, receiver) = watch::channel(None);
        let mut delivery = LatestDelivery::new(frames);
        let cancel = CancellationToken::new();

        // Act: authorize and publish two live frames without reading watch
        // between them.
        delivery
            .acquire(&cancel)
            .await
            .expect("active live delivery should issue a permit");
        assert!(delivery.deliver((), frame(0)).await);

        delivery
            .acquire(&cancel)
            .await
            .expect("active live delivery should issue another permit");
        assert!(delivery.deliver((), frame(1)).await);

        // Assert: watch retains the newest snapshot rather than frame zero.
        assert_eq!(
            receiver
                .borrow()
                .as_ref()
                .expect("latest frame should be retained")
                .tick,
            1
        );
    }

    #[tokio::test]
    async fn latest_delivery_clears_the_frame_at_end() {
        // Arrange a live policy containing one current frame.
        let (frames, receiver) = watch::channel(None);
        let mut delivery = LatestDelivery::new(frames);

        assert!(delivery.deliver((), frame(0)).await);
        // Act: report permanent provider EOF.
        delivery.end(()).await;

        // Assert: live observers see that no current frame remains.
        assert!(receiver.borrow().is_none());
    }

    #[tokio::test]
    async fn latest_delivery_preserves_the_frame_after_an_error() {
        // Arrange a valid current frame before a transient provider error.
        let (frames, receiver) = watch::channel(None);
        let mut delivery = LatestDelivery::new(frames);

        assert!(delivery.deliver((), frame(0)).await);
        // Act: report an error while the watch receiver remains connected.
        assert!(
            delivery
                .error(
                    (),
                    IRacingSDKError::connection_failed("transient live error"),
                )
                .await
        );

        // Assert: retry is allowed and the last valid snapshot is retained.
        assert_eq!(
            receiver
                .borrow()
                .as_ref()
                .expect("transient errors should not clear the latest frame")
                .tick,
            0
        );
    }

    #[tokio::test]
    async fn latest_delivery_stops_acquiring_after_cancellation() {
        // A pre-cancelled live policy must not authorize another provider read.
        let (frames, _receiver) = watch::channel(None);
        let mut delivery = LatestDelivery::new(frames);
        let cancel = CancellationToken::new();
        cancel.cancel();

        assert!(delivery.acquire(&cancel).await.is_none());
    }
}
