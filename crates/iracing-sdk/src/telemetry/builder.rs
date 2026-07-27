use std::sync::Arc;

use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::{FramePacket, provider::Provider, schema::SessionInfo};

use super::{
    Telemetry, TelemetryChannels,
    delivery_policy::{DeliveryPolicy, LatestDelivery},
    session_policy::{LiveSessionPolicy, SessionPolicy},
};

/// Marker selecting the default latest-wins frame delivery policy.
pub(crate) struct DefaultDelivery;

/// Marker selecting the default live session policy.
pub(crate) struct DefaultSessions;

/// A concrete delivery policy paired with the handle returned to its consumer.
pub(crate) struct CustomDelivery<D, F> {
    pub(crate) policy: D,
    pub(crate) frames: F,
}

/// A concrete session policy paired with the receiver returned to its consumer.
pub(crate) struct CustomSessions<S, R> {
    pub(crate) policy: S,
    pub(crate) sessions: R,
}

pub(crate) trait BuildDelivery {
    type Policy: DeliveryPolicy;
    type Frames;

    fn build(self) -> (Self::Policy, Self::Frames);
}

impl BuildDelivery for DefaultDelivery {
    type Policy = LatestDelivery;
    type Frames = watch::Receiver<Option<Arc<FramePacket>>>;

    fn build(self) -> (Self::Policy, Self::Frames) {
        let (sender, receiver) = watch::channel(None);
        (LatestDelivery::new(sender), receiver)
    }
}

impl<D, F> BuildDelivery for CustomDelivery<D, F>
where
    D: DeliveryPolicy,
{
    type Policy = D;
    type Frames = F;

    fn build(self) -> (Self::Policy, Self::Frames) {
        (self.policy, self.frames)
    }
}

pub(crate) trait BuildSessions<P>
where
    P: Provider,
{
    type Policy: SessionPolicy<P>;
    type Sessions;

    fn build(self) -> (Self::Policy, Self::Sessions);
}

impl<P> BuildSessions<P> for DefaultSessions
where
    P: Provider,
{
    type Policy = LiveSessionPolicy;
    type Sessions = watch::Receiver<Option<Arc<SessionInfo>>>;

    fn build(self) -> (Self::Policy, Self::Sessions) {
        let (sender, receiver) = watch::channel(None);
        (LiveSessionPolicy::new(sender), receiver)
    }
}

impl<P, S, R> BuildSessions<P> for CustomSessions<S, R>
where
    P: Provider,
    S: SessionPolicy<P>,
{
    type Policy = S;
    type Sessions = R;

    fn build(self) -> (Self::Policy, Self::Sessions) {
        (self.policy, self.sessions)
    }
}

/// Builds a telemetry task from independently configurable delivery and session policies.
///
/// A newly-created builder uses [`LatestDelivery`] and [`LiveSessionPolicy`].
/// Calling [`Self::with_delivery_policy`] or [`Self::with_session_policy`]
/// replaces only that part of the task while preserving the other default.
pub(crate) struct TelemetryBuilder<P, D = DefaultDelivery, S = DefaultSessions> {
    pub(crate) provider: P,
    pub(crate) delivery: D,
    pub(crate) sessions: S,
}

impl<P> TelemetryBuilder<P> {
    pub(crate) fn new(provider: P) -> Self {
        Self {
            provider,
            delivery: DefaultDelivery,
            sessions: DefaultSessions,
        }
    }
}

impl<P, D, S> TelemetryBuilder<P, D, S> {
    /// Replace the frame delivery policy and its consumer-facing handle.
    ///
    /// The handle is returned as [`TelemetryChannels::frames`]. For latest-wins
    /// delivery it is a watch receiver; an on-demand policy can instead return
    /// a request handle that drives one provider read per request.
    pub(crate) fn with_delivery_policy<NextD, F>(
        self,
        policy: NextD,
        frames: F,
    ) -> TelemetryBuilder<P, CustomDelivery<NextD, F>, S> {
        TelemetryBuilder {
            provider: self.provider,
            delivery: CustomDelivery { policy, frames },
            sessions: self.sessions,
        }
    }

    /// Replace the session policy and its consumer-facing receiver.
    ///
    /// The supplied receiver is returned as [`TelemetryChannels::sessions`].
    pub(crate) fn with_session_policy<NextS, R>(
        self,
        policy: NextS,
        sessions: R,
    ) -> TelemetryBuilder<P, D, CustomSessions<NextS, R>> {
        TelemetryBuilder {
            provider: self.provider,
            delivery: self.delivery,
            sessions: CustomSessions { policy, sessions },
        }
    }
}

impl<P, D, S> TelemetryBuilder<P, D, S>
where
    P: Provider,
    D: BuildDelivery,
    S: BuildSessions<P>,
    D::Policy: 'static,
    S::Policy: 'static,
{
    /// Spawn the configured telemetry task and return its consumer handles.
    pub(crate) fn build(self) -> TelemetryChannels<D::Frames, S::Sessions> {
        let (delivery, frames) = self.delivery.build();
        let (sessions, session_receiver) = self.sessions.build();
        let cancel = CancellationToken::new();
        let task_cancel = cancel.clone();

        tokio::spawn(async move {
            Telemetry::read_task(self.provider, delivery, sessions, task_cancel).await;
        });

        TelemetryChannels {
            frames,
            sessions: session_receiver,
            cancel,
        }
    }
}
