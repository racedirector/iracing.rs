use crate::{
    command::Command,
    error::{BroadcastError, Result},
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

/// Client for sending iRacing broadcast commands over the Win32 broadcast channel.
#[derive(Debug)]
pub struct Client {
    message_id: u32,
}

impl Client {
    fn send_windows_message(message_id: u32, wparam: WPARAM, lparam: LPARAM) -> Result<()> {
        unsafe {
            // Safety: iRacing expects these messages to be delivered to
            // HWND_BROADCAST using the ID obtained from RegisterWindowMessageW.
            // All parameter packing matches the documented protocol, so the
            // Win32 API receives well-formed data.
            SendNotifyMessageW(HWND_BROADCAST, message_id, wparam, lparam)
                .map_err(BroadcastError::from)
        }
    }

    /// Create a new broadcast client by registering the iRacing message ID.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Connection`] if `RegisterWindowMessageW` fails.
    pub fn new() -> Result<Self> {
        let message = U16CString::from_str(IRSDK_BROADCASTMSGNAME)?;

        let id = unsafe { RegisterWindowMessageW(PCWSTR::from_raw(message.as_ptr())) };

        if id == 0 {
            return Err(BroadcastError::Connection {
                reason: format!(
                    "Failed to register broadcast window message '{IRSDK_BROADCASTMSGNAME}'"
                ),
            });
        }

        Ok(Self { message_id: id })
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
        self.send_formatted_message(message.try_into()?)
    }

    fn send_formatted_message(&self, message: FormattedMessage) -> Result<()> {
        // Pack the low/high words to match the Windows broadcast contract.
        let wparam_value = (i32::from(message.0) as usize) | ((message.1 as usize) << 16);
        let lparam_value = i32::from(message.2) | (i32::from(message.3) << 16);

        Self::send_windows_message(
            self.message_id,
            WPARAM(wparam_value),
            LPARAM(lparam_value as isize),
        )
    }
}
