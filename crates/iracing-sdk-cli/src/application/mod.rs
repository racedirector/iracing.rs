//! Composition root for the `iracing-sdk` executable, constructed by `main`.
//!
//! This application is specific to this CLI; it does not wrap the standalone
//! broadcast application's composition object. It implements the broadcast CLI's
//! consumer-owned `BroadcastCommands` and this CLI's live dependency traits.
//!
//! Every simulator-facing resource starts absent. First use initializes only the
//! requested resource, retaining success until application drop. Initialization
//! errors leave that slot absent and a later call retries. Operation errors retain
//! initialized resources; no reconnection policy is added. Broadcast and live
//! slots are independent. Unused resources have no initialization side effects.
//!
//! Mutable injection matches sequential command execution on the executable's
//! current-thread runtime: initialization cannot race and requires no locks.
//! The live adapter preserves current direct shared-memory capture, including its
//! connected-at-initialization check and bounded waits, rather than spawning a
//! `LiveConnection` background task. Drop releases its mapping and event; canceled
//! async waits retain the SDK's bounded native-wait lifetime. Libraries create no
//! runtime. Non-Windows adapters return unsupported errors; live CLI syntax stays
//! Windows-only while dependency contracts and fake tests are portable.

mod live;

use crate::dependencies::{LiveFrames, LiveHeaders, LiveSessions, LiveVariables};
use anyhow::Result;
use iracing_broadcast_cli::{BroadcastClient, BroadcastCommands};
use iracing_broadcast_sdk::Command;
use iracing_irsdk::Header;
use iracing_sdk::{FieldLayout, FramePacket, VariableHeaders, schema::SessionInfo};
use live::LiveTelemetry;

// Private factory/resource parameters let tests count initialization without Win32.
// Production uses the concrete defaults; this is not a service registry.
pub(crate) struct Application<
    B = BroadcastClient,
    L = LiveTelemetry,
    F = fn() -> Result<B>,
    G = fn() -> Result<L>,
> {
    broadcast: Option<B>,
    live: Option<L>,
    initialize_broadcast: F,
    initialize_live: G,
}

impl Application {
    pub(crate) fn new() -> Self {
        Self::with_initializers(BroadcastClient::new, LiveTelemetry::try_connect)
    }
}

impl<B, L, F: FnMut() -> Result<B>, G: FnMut() -> Result<L>> Application<B, L, F, G> {
    fn with_initializers(initialize_broadcast: F, initialize_live: G) -> Self {
        Self {
            broadcast: None,
            live: None,
            initialize_broadcast,
            initialize_live,
        }
    }
}

impl<B, L, F, G: FnMut() -> Result<L>> Application<B, L, F, G> {
    fn live(&mut self) -> Result<&mut L> {
        match &mut self.live {
            Some(live) => Ok(live),
            slot @ None => Ok(slot.insert((self.initialize_live)()?)),
        }
    }
}

impl<B: BroadcastCommands, L, F: FnMut() -> Result<B>, G> BroadcastCommands
    for Application<B, L, F, G>
{
    fn send_broadcast(&mut self, command: Command) -> Result<()> {
        let client = match &mut self.broadcast {
            Some(client) => client,
            slot @ None => slot.insert((self.initialize_broadcast)()?),
        };
        client.send_broadcast(command)
    }
}
impl<B, L: LiveHeaders, F, G: FnMut() -> Result<L>> LiveHeaders for Application<B, L, F, G> {
    fn live_header(&mut self) -> Result<Header> {
        self.live()?.live_header()
    }
}
impl<B, L: LiveSessions, F, G: FnMut() -> Result<L>> LiveSessions for Application<B, L, F, G> {
    fn live_session(&mut self) -> Result<Option<SessionInfo>> {
        self.live()?.live_session()
    }
}
impl<B, L: LiveVariables, F, G: FnMut() -> Result<L>> LiveVariables for Application<B, L, F, G> {
    fn live_variable_headers(&mut self) -> Result<VariableHeaders> {
        self.live()?.live_variable_headers()
    }
}
impl<B, L: LiveFrames, F, G: FnMut() -> Result<L>> LiveFrames for Application<B, L, F, G> {
    fn live_fields(&mut self) -> Result<Vec<FieldLayout>> {
        self.live()?.live_fields()
    }
    fn next_live_frame(&mut self) -> Result<FramePacket> {
        self.live()?.next_live_frame()
    }
    async fn next_live_frame_async(&mut self) -> Result<Option<FramePacket>> {
        self.live()?.next_live_frame_async().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    struct FakeBroadcast {
        sends: Rc<Cell<usize>>,
        drops: Rc<Cell<usize>>,
    }
    impl BroadcastCommands for FakeBroadcast {
        fn send_broadcast(&mut self, _: Command) -> Result<()> {
            self.sends.set(self.sends.get() + 1);
            anyhow::bail!("dispatch failure")
        }
    }
    impl Drop for FakeBroadcast {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    struct FakeLive {
        calls: usize,
        drops: Rc<Cell<usize>>,
    }
    impl LiveSessions for FakeLive {
        fn live_session(&mut self) -> Result<Option<SessionInfo>> {
            self.calls += 1;
            if self.calls == 1 {
                anyhow::bail!("snapshot failure");
            }
            Ok(None)
        }
    }
    impl LiveHeaders for FakeLive {
        fn live_header(&mut self) -> Result<Header> {
            anyhow::bail!("fake header")
        }
    }
    impl LiveVariables for FakeLive {
        fn live_variable_headers(&mut self) -> Result<VariableHeaders> {
            Ok(VariableHeaders::default())
        }
    }
    impl LiveFrames for FakeLive {
        fn live_fields(&mut self) -> Result<Vec<FieldLayout>> {
            Ok(vec![])
        }
        fn next_live_frame(&mut self) -> Result<FramePacket> {
            anyhow::bail!("no frame")
        }
        async fn next_live_frame_async(&mut self) -> Result<Option<FramePacket>> {
            std::future::pending().await
        }
    }
    impl Drop for FakeLive {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    #[test]
    fn capabilities_initialize_independently_reuse_success_and_drop_with_application() {
        let broadcast_count = Cell::new(0);
        let live_count = Cell::new(0);
        let sends = Rc::new(Cell::new(0));
        let broadcast_drops = Rc::new(Cell::new(0));
        let live_drops = Rc::new(Cell::new(0));
        let mut application = Application::with_initializers(
            || {
                broadcast_count.set(broadcast_count.get() + 1);
                Ok(FakeBroadcast {
                    sends: sends.clone(),
                    drops: broadcast_drops.clone(),
                })
            },
            || {
                live_count.set(live_count.get() + 1);
                Ok(FakeLive {
                    calls: 0,
                    drops: live_drops.clone(),
                })
            },
        );
        assert_eq!((broadcast_count.get(), live_count.get()), (0, 0));
        for _ in 0..2 {
            assert!(
                application
                    .send_broadcast(Command::ReloadAllTextures)
                    .is_err()
            );
        }
        assert_eq!((broadcast_count.get(), live_count.get()), (1, 0));
        assert_eq!(sends.get(), 2);
        assert!(application.live_session().is_err());
        assert!(application.live_session().unwrap().is_none());
        assert!(application.live_header().is_err());
        assert!(application.live_variable_headers().unwrap().is_empty());
        assert!(application.live_fields().unwrap().is_empty());
        assert!(application.next_live_frame().is_err());
        assert_eq!((broadcast_count.get(), live_count.get()), (1, 1));
        assert_eq!((broadcast_drops.get(), live_drops.get()), (0, 0));
        drop(application);
        assert_eq!((broadcast_drops.get(), live_drops.get()), (1, 1));
    }

    #[test]
    fn failed_live_initialization_retries_without_poisoning_broadcast() {
        let attempts = Cell::new(0);
        let broadcasts = Cell::new(0);
        let mut application = Application::with_initializers(
            || {
                broadcasts.set(broadcasts.get() + 1);
                Ok(FakeBroadcast {
                    sends: Rc::new(Cell::new(0)),
                    drops: Rc::new(Cell::new(0)),
                })
            },
            || {
                attempts.set(attempts.get() + 1);
                if attempts.get() == 1 {
                    anyhow::bail!("simulator unavailable");
                }
                Ok(FakeLive {
                    calls: 1,
                    drops: Rc::new(Cell::new(0)),
                })
            },
        );
        assert!(application.live_session().is_err());
        assert_eq!((broadcasts.get(), attempts.get()), (0, 1));
        assert!(
            application
                .send_broadcast(Command::ReloadAllTextures)
                .is_err()
        );
        for _ in 0..2 {
            assert!(application.live_session().unwrap().is_none());
        }
        assert_eq!((broadcasts.get(), attempts.get()), (1, 2));
    }

    #[test]
    fn failed_broadcast_initialization_preserves_existing_live_resource() {
        let attempts = Cell::new(0);
        let live_count = Cell::new(0);
        let mut application = Application::with_initializers(
            || {
                attempts.set(attempts.get() + 1);
                if attempts.get() == 1 {
                    anyhow::bail!("registration failure");
                }
                Ok(FakeBroadcast {
                    sends: Rc::new(Cell::new(0)),
                    drops: Rc::new(Cell::new(0)),
                })
            },
            || {
                live_count.set(live_count.get() + 1);
                Ok(FakeLive {
                    calls: 1,
                    drops: Rc::new(Cell::new(0)),
                })
            },
        );
        assert!(application.live_session().unwrap().is_none());
        assert_eq!(attempts.get(), 0);
        for _ in 0..3 {
            assert!(
                application
                    .send_broadcast(Command::ReloadAllTextures)
                    .is_err()
            );
        }
        assert!(application.live_session().unwrap().is_none());
        assert_eq!((attempts.get(), live_count.get()), (2, 1));
    }

    #[tokio::test]
    async fn canceled_frame_request_retains_initialized_live_resource() {
        let live_count = Cell::new(0);
        let mut application = Application::with_initializers(
            || -> Result<FakeBroadcast> { panic!("broadcast must stay unused") },
            || {
                live_count.set(live_count.get() + 1);
                Ok(FakeLive {
                    calls: 1,
                    drops: Rc::new(Cell::new(0)),
                })
            },
        );
        {
            let frame = application.next_live_frame_async();
            tokio::pin!(frame);
            assert!(futures::poll!(&mut frame).is_pending());
        }
        assert_eq!(live_count.get(), 1);
        assert!(application.live_session().unwrap().is_none());
        assert_eq!(live_count.get(), 1);
    }

    #[test]
    fn production_construction_is_inert() {
        let application = Application::new();
        assert!(application.broadcast.is_none());
        assert!(application.live.is_none());
    }

    #[cfg(not(windows))]
    #[test]
    fn production_capabilities_report_unsupported_platform() {
        let mut application = Application::new();
        assert!(
            application
                .live_session()
                .unwrap_err()
                .to_string()
                .contains("only runs on Windows")
        );
        assert!(
            application
                .send_broadcast(Command::ReloadAllTextures)
                .unwrap_err()
                .to_string()
                .contains("only run on Windows")
        );
        assert!(application.broadcast.is_none());
        assert!(application.live.is_none());
    }
}
