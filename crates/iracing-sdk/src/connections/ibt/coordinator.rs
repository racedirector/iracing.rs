use std::{collections::HashSet, sync::Arc};

use tokio::sync::{mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;

use crate::{FramePacket, Result, telemetry::delivery_policy::ReplayDemand};

/// Commands sent by subscriptions and the connection to the replay coordinator.
#[derive(Debug)]
pub(super) enum ReplayControl {
    Start,
    Join { subscriber_id: u64 },
    Ack { subscriber_id: u64 },
    Leave { subscriber_id: u64 },
}

/// Spawn the task that turns request/response IBT delivery into a coordinated watch stream.
pub(super) fn spawn(
    demands: mpsc::Sender<ReplayDemand>,
    cancel: CancellationToken,
) -> (
    watch::Receiver<Option<Arc<FramePacket>>>,
    mpsc::UnboundedSender<ReplayControl>,
) {
    let (frames, frame_receiver) = watch::channel(None);
    let (controls, control_receiver) = mpsc::unbounded_channel();

    tokio::spawn(run(demands, frames, control_receiver, cancel));

    (frame_receiver, controls)
}

async fn run(
    demands: mpsc::Sender<ReplayDemand>,
    frames: watch::Sender<Option<Arc<FramePacket>>>,
    mut controls: mpsc::UnboundedReceiver<ReplayControl>,
    cancel: CancellationToken,
) {
    let mut started = false;
    let mut active = HashSet::new();
    let mut pending = HashSet::new();
    let mut has_current_frame = false;
    let mut response: Option<oneshot::Receiver<Result<Option<FramePacket>>>> = None;

    loop {
        if started && !active.is_empty() && pending.is_empty() && response.is_none() {
            let (response_tx, response_rx) = oneshot::channel();
            match demands.try_send(ReplayDemand {
                response: response_tx,
            }) {
                Ok(()) => response = Some(response_rx),
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    tracing::debug!("IBT replay demand channel closed");
                    break;
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    tracing::error!(
                        "IBT replay demand channel was full without a request in flight"
                    );
                    break;
                }
            }
        }

        tokio::select! {
            _ = cancel.cancelled() => {
                tracing::debug!("IBT replay coordinator cancelled");
                break;
            }
            command = controls.recv() => {
                let Some(command) = command else {
                    break;
                };

                match command {
                    ReplayControl::Start => {
                        started = true;
                    }
                    ReplayControl::Join { subscriber_id } => {
                        if active.insert(subscriber_id) && has_current_frame {
                            pending.insert(subscriber_id);
                        }
                    }
                    ReplayControl::Ack { subscriber_id } => {
                        pending.remove(&subscriber_id);
                    }
                    ReplayControl::Leave { subscriber_id } => {
                        active.remove(&subscriber_id);
                        pending.remove(&subscriber_id);
                    }
                }
            }
            result = async {
                response
                    .as_mut()
                    .expect("response branch requires an in-flight demand")
                    .await
            }, if response.is_some() => {
                response = None;

                match result {
                    Ok(Ok(Some(frame))) => {
                        has_current_frame = true;
                        pending.clone_from(&active);
                        frames.send_replace(Some(Arc::new(frame)));
                    }
                    Ok(Ok(None)) => {
                        tracing::debug!("IBT replay coordinator reached EOF");
                        break;
                    }
                    Ok(Err(error)) => {
                        tracing::error!(%error, "IBT replay coordinator stopped after provider error");
                        break;
                    }
                    Err(_) => {
                        tracing::debug!("IBT replay response channel closed");
                        break;
                    }
                }
            }
        }
    }
}
