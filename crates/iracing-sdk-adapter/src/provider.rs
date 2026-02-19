use crate::{FramePacket, Result};

pub trait Provider: Send + 'static {
    fn next_frame(&mut self) -> Result<Option<FramePacket>>;

    fn session_yaml(&mut self, version: u32) -> Result<Option<String>>;
}
