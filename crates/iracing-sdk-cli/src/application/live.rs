//! Direct shared-memory adapter used by the current CLI capture commands.
//! Retained by `Application`; no background `LiveConnection` task is introduced.

use crate::dependencies::{LiveFrames, LiveHeaders, LiveSessions, LiveVariables};
use anyhow::Result;
use iracing_irsdk::Header;
use iracing_sdk::{FieldLayout, FramePacket, VariableHeaders, schema::SessionInfo};
#[cfg(windows)]
use iracing_sdk::{
    LayoutProvider, TelemetryLayout, WindowsConnection,
    provider::{
        SessionInformationBytesProvider, SessionInformationProvider, VariableHeadersProvider,
    },
};
#[cfg(windows)]
use std::{sync::Arc, time::Duration};

#[cfg(windows)]
pub(crate) struct LiveTelemetry {
    connection: WindowsConnection,
    layout: Arc<TelemetryLayout>,
}

#[cfg(windows)]
impl LiveTelemetry {
    /// Open live shared memory and validate its telemetry layout without waiting
    /// for the simulator to become connected.
    ///
    /// # Errors
    ///
    /// Returns an error if telemetry is not connected. Propagates shared-memory
    /// setup, header access, frame-size conversion, variable header, and layout
    /// validation errors.
    pub(super) fn try_connect() -> Result<Self> {
        let connection = match WindowsConnection::try_connect() {
            Ok(c) if c.is_connected() => c,
            Ok(_) => {
                return Err(anyhow::anyhow!(
                    "Shared memory opened but telemetry is not connected yet"
                ));
            }
            Err(e) => return Err(anyhow::anyhow!(e)),
        };

        Self::from_connection(connection)
    }

    fn from_connection(connection: WindowsConnection) -> Result<Self> {
        let frame_size = usize::try_from(connection.header_snapshot()?.buffer_length)?;
        let headers = connection.variable_headers()?;
        let layout = TelemetryLayout::try_from_headers(&headers, frame_size)?;

        Ok(Self {
            connection,
            layout: Arc::new(layout),
        })
    }

    /// Wait cooperatively for a frame, or finish when the simulator disconnects.
    /// Canceling this future leaves at most one bounded native wait in progress.
    async fn next_frame_async(&mut self) -> Result<Option<FramePacket>> {
        loop {
            if !self.connection.is_connected() {
                return Ok(None);
            }
            if let Some(frame) = self.connection.get_new_data()? {
                return Ok(Some(FramePacket::new(
                    frame.data,
                    u32::try_from(frame.tick)?,
                    u32::try_from(frame.session_info_update)?,
                    Arc::clone(&self.layout),
                )?));
            }
            self.connection
                .wait_for_update_async(Duration::from_millis(500))
                .await?;
        }
    }

    /// Block until a new live frame can be returned with the retained layout.
    ///
    /// The first observed tick establishes a baseline without yielding a frame.
    /// Waits retry after each 500 ms timeout while connected; there is no overall
    /// timeout. A disconnect ends capture with an error.
    ///
    /// # Errors
    ///
    /// Returns an error if the source disconnects. Propagates acquisition and
    /// wait errors. Also returns an error if the tick or session update counter
    /// cannot fit in `u32`, or the frame size differs from the retained layout.
    fn next_frame(&mut self) -> Result<FramePacket> {
        loop {
            if !self.connection.is_connected() {
                anyhow::bail!("Live telemetry disconnected before a frame was available");
            }
            if let Some(frame) = self.connection.get_new_data()? {
                return Ok(FramePacket::new(
                    frame.data,
                    u32::try_from(frame.tick)?,
                    u32::try_from(frame.session_info_update)?,
                    Arc::clone(&self.layout),
                )?);
            }

            // Wait up to 500ms for an update
            self.connection
                .wait_for_update(Duration::from_millis(500))?;
        }
    }
}

#[cfg(windows)]
impl SessionInformationBytesProvider for LiveTelemetry {
    fn session_info_snapshot(&self) -> iracing_sdk::Result<Option<iracing_sdk::SessionInfoBytes>> {
        self.connection.session_info_snapshot()
    }
}

#[cfg(windows)]
impl VariableHeadersProvider for LiveTelemetry {
    fn variable_headers(&self) -> iracing_sdk::Result<iracing_sdk::VariableHeaders> {
        self.connection.variable_headers()
    }
}

#[cfg(windows)]
impl LayoutProvider for LiveTelemetry {
    fn layout(&self) -> &std::sync::Arc<TelemetryLayout> {
        &self.layout
    }
}

#[cfg(not(windows))]
pub(crate) struct LiveTelemetry;

#[cfg(not(windows))]
impl LiveTelemetry {
    pub(super) fn try_connect() -> Result<Self> {
        anyhow::bail!("Live telemetry only runs on Windows")
    }
}

impl LiveHeaders for LiveTelemetry {
    fn live_header(&mut self) -> Result<Header> {
        #[cfg(windows)]
        {
            Ok(self.connection.header_snapshot()?)
        }
        #[cfg(not(windows))]
        {
            anyhow::bail!("Live telemetry only runs on Windows")
        }
    }
}
impl LiveSessions for LiveTelemetry {
    fn live_session(&mut self) -> Result<Option<SessionInfo>> {
        #[cfg(windows)]
        {
            Ok(self.session_info()?)
        }
        #[cfg(not(windows))]
        {
            anyhow::bail!("Live telemetry only runs on Windows")
        }
    }
}
impl LiveVariables for LiveTelemetry {
    fn live_variable_headers(&mut self) -> Result<VariableHeaders> {
        #[cfg(windows)]
        {
            Ok(self.variable_headers()?)
        }
        #[cfg(not(windows))]
        {
            anyhow::bail!("Live telemetry only runs on Windows")
        }
    }
}
impl LiveFrames for LiveTelemetry {
    fn live_fields(&mut self) -> Result<Vec<FieldLayout>> {
        #[cfg(windows)]
        {
            Ok(self.fields_owned())
        }
        #[cfg(not(windows))]
        {
            anyhow::bail!("Live telemetry only runs on Windows")
        }
    }
    fn next_live_frame(&mut self) -> Result<FramePacket> {
        #[cfg(windows)]
        {
            self.next_frame()
        }
        #[cfg(not(windows))]
        {
            anyhow::bail!("Live telemetry only runs on Windows")
        }
    }
    async fn next_live_frame_async(&mut self) -> Result<Option<FramePacket>> {
        #[cfg(windows)]
        {
            self.next_frame_async().await
        }
        #[cfg(not(windows))]
        {
            anyhow::bail!("Live telemetry only runs on Windows")
        }
    }
}
