use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;

use crate::{FramePacket, IRacingSDKError, Result};

/// Controls when the telemetry task may read a frame and how it publishes the result.
///
/// Live and recorded telemetry require different delivery semantics. A live
/// consumer generally wants the newest available snapshot, so reading can
/// continue independently of individual subscribers. An IBT replay is an
/// ordered sequence, so reading must wait until its consumer explicitly asks
/// for the next frame.
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

/// A request for exactly one replay frame.
///
/// The request itself is the on-demand delivery permit. The telemetry task
/// answers its one-shot channel with one frame, EOF, or an error before waiting
/// for another request.
pub(crate) struct ReplayDemand {
    /// Response channel paired with one consumer request.
    pub(crate) response: oneshot::Sender<Result<Option<FramePacket>>>,
}

/// Backpressured delivery for recorded telemetry.
///
/// Unlike [`LatestDelivery`], this policy does not authorize a provider read
/// until it receives a [`ReplayDemand`]. Consequently, an IBT cursor cannot
/// advance while there is no subscriber demand.
pub(crate) struct OnDemandDelivery {
    /// Queue of requests for individual replay frames.
    requests: mpsc::Receiver<ReplayDemand>,
}

impl OnDemandDelivery {
    /// Create a replay policy that obtains permits from a demand channel.
    pub(crate) fn new(requests: mpsc::Receiver<ReplayDemand>) -> Self {
        Self { requests }
    }
}

#[async_trait]
impl DeliveryPolicy for OnDemandDelivery {
    /// One demand is consumed by one provider-read attempt.
    type Permit = ReplayDemand;

    async fn acquire(&mut self, cancel: &CancellationToken) -> Option<Self::Permit> {
        // With no queued demand this await is the replay backpressure boundary:
        // the provider remains untouched until a consumer requests a frame.
        tokio::select! {
            _ = cancel.cancelled() => None,
            request = self.requests.recv() => request,
        }
    }

    async fn deliver(&mut self, permit: Self::Permit, frame: FramePacket) -> bool {
        // A dropped response receiver means the requesting consumer no longer
        // wants this frame, so the telemetry task should stop.
        permit.response.send(Ok(Some(frame))).is_ok()
    }

    async fn end(&mut self, permit: Self::Permit) {
        // EOF is returned only in response to a demand made after the final
        // frame; it cannot overwrite a previously delivered replay frame.
        let _ = permit.response.send(Ok(None));
    }

    async fn error(&mut self, permit: Self::Permit, error: IRacingSDKError) -> bool {
        // IBT read failures are deterministic for the completed file. Answer
        // the outstanding request with the error and stop this replay task.
        let _ = permit.response.send(Err(error));
        false
    }
}

#[cfg(test)]
mod tests {
    //! Characterization tests for live and on-demand frame delivery.
    //!
    //! These tests operate directly on delivery policies rather than spawning a
    //! provider task. That keeps each assertion focused on permit acquisition,
    //! channel publication, cancellation, EOF, and error semantics.
    //!
    //! Frames use empty schemas and payloads because delivery policies treat
    //! packets as opaque values. Tick numbers are sufficient to identify which
    //! frame a receiver observed.

    use std::{collections::HashMap, sync::Arc};

    use tokio::sync::{mpsc, oneshot, watch};
    use tokio_util::sync::CancellationToken;

    use crate::{FramePacket, IRacingSDKError, VariableSchema};

    use super::{DeliveryPolicy, LatestDelivery, OnDemandDelivery, ReplayDemand};

    /// Construct a minimal packet whose tick identifies it in assertions.
    fn frame(tick: u32) -> FramePacket {
        FramePacket::new(
            Vec::new(),
            tick,
            0,
            Arc::new(
                VariableSchema::new(HashMap::new(), 0)
                    .expect("an empty telemetry schema should be valid"),
            ),
        )
    }

    /// Create one replay permit and the consumer side of its response channel.
    fn demand() -> (
        ReplayDemand,
        oneshot::Receiver<crate::Result<Option<FramePacket>>>,
    ) {
        let (response, receiver) = oneshot::channel();
        (ReplayDemand { response }, receiver)
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

    /// Deliver `frame_count` replay frames through individual demand permits.
    ///
    /// Each loop iteration creates one request, acquires exactly that request
    /// as a permit, and verifies that its response contains the matching tick.
    async fn assert_on_demand_delivery(frame_count: u32) {
        let (requests, request_receiver) = mpsc::channel(1);
        let mut delivery = OnDemandDelivery::new(request_receiver);
        let cancel = CancellationToken::new();

        for tick in 0..frame_count {
            let (request, response) = demand();
            requests
                .send(request)
                .await
                .expect("replay demand channel should remain open");

            let permit = delivery
                .acquire(&cancel)
                .await
                .expect("one replay demand should issue one permit");
            assert!(delivery.deliver(permit, frame(tick)).await);

            let delivered = response
                .await
                .expect("delivery should answer the replay demand")
                .expect("frame delivery should succeed")
                .expect("frame demand should produce a frame");
            assert_eq!(delivered.tick, tick);
        }
    }

    #[tokio::test]
    async fn on_demand_delivery_handles_one_frame() {
        // The smallest replay still requires one explicit demand.
        assert_on_demand_delivery(1).await;
    }

    #[tokio::test]
    async fn on_demand_delivery_handles_two_frames() {
        // Consecutive demands must remain paired with consecutive frames.
        assert_on_demand_delivery(2).await;
    }

    #[tokio::test]
    async fn on_demand_delivery_handles_many_frames() {
        // A longer replay must preserve all ticks rather than coalescing them.
        assert_on_demand_delivery(48).await;
    }

    #[tokio::test]
    async fn on_demand_delivery_reports_end_to_the_requester() {
        // Arrange one outstanding demand made after the replay is exhausted.
        let (requests, request_receiver) = mpsc::channel(1);
        let mut delivery = OnDemandDelivery::new(request_receiver);
        let cancel = CancellationToken::new();
        let (request, response) = demand();
        requests
            .send(request)
            .await
            .expect("replay demand channel should remain open");

        let permit = delivery
            .acquire(&cancel)
            .await
            .expect("replay demand should issue a permit");
        // Act: answer that specific permit with EOF.
        delivery.end(permit).await;

        // Assert: the requester receives successful end-of-stream, not channel
        // cancellation or a stale frame.
        assert!(
            response
                .await
                .expect("delivery should answer the replay demand")
                .expect("end-of-stream delivery should succeed")
                .is_none()
        );
    }

    #[tokio::test]
    async fn on_demand_delivery_reports_errors_and_stops() {
        // Arrange one outstanding demand whose provider read will fail.
        let (requests, request_receiver) = mpsc::channel(1);
        let mut delivery = OnDemandDelivery::new(request_receiver);
        let cancel = CancellationToken::new();
        let (request, response) = demand();
        requests
            .send(request)
            .await
            .expect("replay demand channel should remain open");

        let permit = delivery
            .acquire(&cancel)
            .await
            .expect("replay demand should issue a permit");
        // Act: report the deterministic replay read error.
        assert!(
            !delivery
                .error(
                    permit,
                    IRacingSDKError::connection_failed("replay read failed"),
                )
                .await
        );

        // Assert: the policy stops and the same requester receives the error.
        assert!(
            response
                .await
                .expect("delivery should answer the replay demand")
                .is_err()
        );
    }

    #[tokio::test]
    async fn on_demand_delivery_stops_acquiring_after_cancellation() {
        // A pre-cancelled replay policy must not consume or wait for demand.
        let (_requests, request_receiver) = mpsc::channel(1);
        let mut delivery = OnDemandDelivery::new(request_receiver);
        let cancel = CancellationToken::new();
        cancel.cancel();

        assert!(delivery.acquire(&cancel).await.is_none());
    }
}
