use std::{
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures::Stream;
use tokio::sync::{mpsc, watch};
use tokio_stream::wrappers::WatchStream;

use crate::{AdapterValidation, FrameAdapter, FramePacket};

use super::coordinator::ReplayControl;

/// A coordinated IBT subscription that acknowledges a frame when polled again.
pub(super) struct IbtSubscription<T> {
    subscriber_id: u64,
    frames: WatchStream<Option<Arc<FramePacket>>>,
    controls: mpsc::UnboundedSender<ReplayControl>,
    validation: AdapterValidation,
    acknowledgement_owed: bool,
    _adapter: PhantomData<fn() -> T>,
}

impl<T> IbtSubscription<T> {
    pub(super) fn new(
        subscriber_id: u64,
        frames: watch::Receiver<Option<Arc<FramePacket>>>,
        controls: mpsc::UnboundedSender<ReplayControl>,
        validation: AdapterValidation,
    ) -> Self {
        Self {
            subscriber_id,
            frames: WatchStream::new(frames),
            controls,
            validation,
            acknowledgement_owed: false,
            _adapter: PhantomData,
        }
    }
}

impl<T> Stream for IbtSubscription<T>
where
    T: FrameAdapter,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if this.acknowledgement_owed {
            let _ = this.controls.send(ReplayControl::Ack {
                subscriber_id: this.subscriber_id,
            });
            this.acknowledgement_owed = false;
        }

        loop {
            match Pin::new(&mut this.frames).poll_next(cx) {
                Poll::Ready(Some(Some(packet))) => {
                    this.acknowledgement_owed = true;
                    return Poll::Ready(Some(T::adapt(&packet, &this.validation)));
                }
                Poll::Ready(Some(None)) => {
                    // `None` is the connected, pre-first-frame watch value.
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl<T> Drop for IbtSubscription<T> {
    fn drop(&mut self) {
        let _ = self.controls.send(ReplayControl::Leave {
            subscriber_id: self.subscriber_id,
        });
    }
}

// The subscription has no pin-sensitive fields.
impl<T> Unpin for IbtSubscription<T> {}
