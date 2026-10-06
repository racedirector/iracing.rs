use crate::{
    command::Command,
    error::{Error, Result},
    message_format::FormattedMessage,
};
use iracing_irsdk::constants::IRSDK_BROADCASTMSGNAME;
use widestring::U16CString;
use windows::{
    Win32::{
        Foundation::{LPARAM, WPARAM},
        UI::WindowsAndMessaging::{HWND_BROADCAST, RegisterWindowMessageW, SendNotifyMessageW},
    },
    core::PCWSTR,
};

#[derive(Debug)]
struct WindowsMessage {
    wparam: WPARAM,
    lparam: LPARAM,
}

impl From<FormattedMessage> for WindowsMessage {
    fn from(message: FormattedMessage) -> Self {
        let wparam_value = (i32::from(message.0) as usize) | ((message.1 as usize) << 16);
        let lparam_value = i32::from(message.2) | (i32::from(message.3) << 16);

        Self {
            wparam: WPARAM(wparam_value),
            lparam: LPARAM(lparam_value as isize),
        }
    }
}

impl TryFrom<Command> for WindowsMessage {
    type Error = crate::error::Error;

    fn try_from(command: Command) -> Result<Self> {
        let message = FormattedMessage::try_from(command)?;
        Ok(WindowsMessage::from(message))
    }
}

#[derive(Debug)]
struct MessageDispatcher {
    message_id: u32,
}

impl MessageDispatcher {
    fn new() -> Result<Self> {
        let message = U16CString::from_str(IRSDK_BROADCASTMSGNAME)?;

        let id = unsafe { RegisterWindowMessageW(PCWSTR::from_raw(message.as_ptr())) };

        if id == 0 {
            return Err(Error::Connection {
                reason: format!(
                    "Failed to register broadcast window message '{IRSDK_BROADCASTMSGNAME}'"
                ),
            });
        }

        Ok(Self { message_id: id })
    }

    fn send_message(&self, message: WindowsMessage) -> Result<()> {
        tracing::debug!("Sending message: {:?}", message);
        unsafe {
            // Safety: iRacing expects these messages to be delivered to
            // HWND_BROADCAST using the ID obtained from RegisterWindowMessageW.
            // All parameter packing matches the documented protocol, so the
            // Win32 API receives well-formed data.
            SendNotifyMessageW(
                HWND_BROADCAST,
                self.message_id,
                message.wparam,
                message.lparam,
            )
            .map_err(Error::from)
        }
    }
}

/// Client for sending iRacing broadcast commands over the Win32 broadcast channel.
#[derive(Debug)]
pub struct Client {
    dispatch: MessageDispatcher,
}

impl Client {
    /// Create a new broadcast client by registering the iRacing message ID.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Connection`] if `RegisterWindowMessageW` fails.
    pub fn new() -> Result<Self> {
        Ok(Self {
            dispatch: MessageDispatcher::new()?,
        })
    }

    /// Send a typed broadcast message to iRacing.
    ///
    /// The command is packed into the `WPARAM`/`LPARAM` format expected by the
    /// official iRacing SDK and dispatched via `HWND_BROADCAST`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the command cannot be encoded or
    /// [`Error::Windows`] if `SendNotifyMessageW` reports a Win32 error.
    pub fn send_message(&self, message: Command) -> Result<()> {
        self.dispatch
            .send_message(WindowsMessage::try_from(message)?)
    }
}
