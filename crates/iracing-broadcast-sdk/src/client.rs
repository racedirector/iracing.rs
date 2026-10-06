use crate::{
    command::Command,
    error::{Error, Result},
};
use iracing_irsdk::{BroadcastMessage, constants::IRSDK_BROADCASTMSGNAME};
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

impl From<BroadcastMessage> for WindowsMessage {
    /// Pack the message ID and three parameter words into Win32 message parameters.
    ///
    /// The ID and first word occupy `WPARAM`; the remaining words occupy `LPARAM`.
    fn from(message: BroadcastMessage) -> Self {
        let wparam_value = (i32::from(message.kind) as usize) | ((message.var1 as usize) << 16);
        let lparam_value = i32::from(message.var2) | (i32::from(message.var3) << 16);

        Self {
            wparam: WPARAM(wparam_value),
            lparam: LPARAM(lparam_value as isize),
        }
    }
}

#[derive(Debug)]
struct MessageDispatcher {
    message_id: u32,
}

impl MessageDispatcher {
    /// Register the iRacing broadcast window message and retain its identifier.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Connection`] if `RegisterWindowMessageW` fails.
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

    /// Dispatch a packed message through `HWND_BROADCAST` using the registered ID.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Windows`] if `SendNotifyMessageW` fails.
    fn send_message(&self, message: WindowsMessage) -> Result<()> {
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
        let broadcast_message = BroadcastMessage::try_from(message)?;
        let windows_message = WindowsMessage::from(broadcast_message);

        self.dispatch.send_message(windows_message)
    }
}
