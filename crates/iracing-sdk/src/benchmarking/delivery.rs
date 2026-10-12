//! Historical acknowledged delivery workload, available only with `benchmark`.
use crate::{FramePacket, IRacingSDKError, Result, telemetry::delivery_policy::DeliveryPolicy};
use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

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
/// Unlike the live latest-value policy, this policy does not authorize a provider read
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
    use super::*;
    use std::sync::Arc;
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
    fn demand() -> (
        ReplayDemand,
        oneshot::Receiver<crate::Result<Option<FramePacket>>>,
    ) {
        let (response, receiver) = oneshot::channel();
        (ReplayDemand { response }, receiver)
    }
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
