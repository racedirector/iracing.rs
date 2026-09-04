//! Non-type constants defined by `irsdk_defines.h`.

/// Name of the Windows event signaled when telemetry data is available.
pub const IRSDK_DATAVALIDEVENTNAME: &str = r"Local\IRSDKDataValidEvent";

/// Name of the Windows shared-memory mapping published by iRacing.
pub const IRSDK_MEMMAPFILENAME: &str = r"Local\IRSDKMemMapFileName";

/// Name of the registered Windows message used for broadcast commands.
pub const IRSDK_BROADCASTMSGNAME: &str = "IRSDK_BROADCASTMSG";

/// Maximum number of rotating telemetry buffers described by an SDK header.
pub const IRSDK_MAX_BUFS: usize = 4;

/// Fixed byte length of SDK variable names and unit strings.
pub const IRSDK_MAX_STRING: usize = 32;

/// Fixed byte length of SDK variable descriptions.
pub const IRSDK_MAX_DESC: usize = 64;

/// Session lap-count marker meaning that the session has no lap limit.
pub const IRSDK_UNLIMITED_LAPS: i32 = 32_767;

/// Session time marker, in seconds, meaning that the session has no time limit.
pub const IRSDK_UNLIMITED_TIME: f32 = 604_800.0;

/// Current telemetry-header version defined by the SDK.
pub const IRSDK_VER: i32 = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_sdk_definitions() {
        assert_eq!(IRSDK_MAX_BUFS, 4);
        assert_eq!(IRSDK_MAX_STRING, 32);
        assert_eq!(IRSDK_MAX_DESC, 64);
        assert_eq!(IRSDK_UNLIMITED_LAPS, 32_767);
        assert_eq!(IRSDK_UNLIMITED_TIME, 604_800.0);
        assert_eq!(IRSDK_VER, 2);
    }
}
