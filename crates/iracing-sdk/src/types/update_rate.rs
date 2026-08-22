//! Update rate control for telemetry streams

use std::{num::NonZeroU32, time::Duration};

use serde::{Deserialize, Serialize};

use crate::IRacingSDKError;

/// Update rate for telemetry streams
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum UpdateRate {
    /// Full speed from source (typically 60Hz)
    Native,

    /// Throttled to maximum Hz
    /// If the requested rate exceeds source rate, Native is used
    Max(NonZeroU32),
}

impl UpdateRate {
    /// Create an update rate from a provided hz
    pub fn max(hz: u32) -> Result<Self, IRacingSDKError> {
        NonZeroU32::new(hz)
            .map(Self::Max)
            .ok_or(IRacingSDKError::parse_error(
                "Update Rate".to_string(),
                format!("{} could not be parsed to NonZeroU32", hz),
            ))
    }

    /// Normalize rate against source frequency
    /// Returns effective rate to use
    pub fn normalize(self, source_hz: f64) -> Self {
        match self {
            UpdateRate::Native => UpdateRate::Native,
            UpdateRate::Max(hz) if f64::from(hz.get()) >= source_hz => UpdateRate::Native,
            UpdateRate::Max(hz) => UpdateRate::Max(hz),
        }
    }

    /// Check if throttling is needed
    ///
    // An `UpdateRate` needs throttling if it's requested rate is less than the source hz
    // of the file.
    pub fn needs_throttle(self, source_hz: f64) -> bool {
        matches!(self.normalize(source_hz), Self::Max(_))
    }

    /// Get throttle interval if needed
    pub fn throttle_interval(self, source_hz: f64) -> Option<std::time::Duration> {
        match self.normalize(source_hz) {
            UpdateRate::Native => None,
            UpdateRate::Max(hz) => Some(Duration::from_secs_f64(1.0 / f64::from(hz.get()))),
        }
    }
}

// ???: Is this useful?
// impl TryFrom<u32> for UpdateRate {
//     type Error = IRacingSDKError;

//     fn try_from(hz: u32) -> Result<Self, Self::Error> {
//         NonZeroU32::new(hz)
//             .map(Self::Max)
//             .ok_or(IRacingSDKError::Parse {
//                 context: "UpdateRate".to_string(),
//                 details: format!("{} could not be parsed to NonZeroU32", hz),
//             })
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn try_from_u32() {
    //     let target_hz = 32;
    //     let non_zero_target_hz = NonZeroU32::new(target_hz).unwrap();
    //     let target_rate = UpdateRate::Max(non_zero_target_hz);

    //     assert_eq!(UpdateRate::try_from(32).unwrap(), target_rate);

    //     let check_rate: UpdateRate = target_hz.try_into().unwrap();
    //     assert_eq!(check_rate, target_rate);
    // }

    #[test]
    fn max() {
        // The minimum value of a NonZeroU32
        let non_zero_min: NonZeroU32 = NonZeroU32::new(1).unwrap();

        // The maximum value of a NonZeroU32
        let non_zero_max: NonZeroU32 = NonZeroU32::new(u32::MAX).unwrap();

        assert!(UpdateRate::max(0).is_err());

        assert_eq!(UpdateRate::max(1).unwrap(), UpdateRate::Max(non_zero_min));

        assert_eq!(
            UpdateRate::max(u32::MAX).unwrap(),
            UpdateRate::Max(non_zero_max)
        );
    }

    #[test]
    fn normalize() -> Result<(), IRacingSDKError> {
        let source_hz = 60.0;

        assert_eq!(UpdateRate::Native.normalize(source_hz), UpdateRate::Native);

        assert_eq!(
            UpdateRate::max(30)?.normalize(source_hz),
            UpdateRate::max(30)?
        );

        assert_eq!(
            UpdateRate::max(60)?.normalize(source_hz),
            UpdateRate::Native
        );

        assert_eq!(
            UpdateRate::max(120)?.normalize(source_hz),
            UpdateRate::Native
        );

        Ok(())
    }

    #[test]
    fn needs_throttle() -> Result<(), IRacingSDKError> {
        let source_hz = 60.0;

        assert!(UpdateRate::max(1)?.needs_throttle(source_hz));
        assert!(UpdateRate::max(59)?.needs_throttle(source_hz));

        assert!(!UpdateRate::max(60)?.needs_throttle(source_hz));
        assert!(!UpdateRate::max(61)?.needs_throttle(source_hz));
        assert!(!UpdateRate::Native.needs_throttle(source_hz));

        Ok(())
    }

    #[test]
    fn throttle_interval() -> Result<(), IRacingSDKError> {
        let source_hz = 60.0;

        assert_eq!(UpdateRate::Native.throttle_interval(source_hz), None);

        assert_eq!(UpdateRate::max(60)?.throttle_interval(source_hz), None);

        assert_eq!(UpdateRate::max(120)?.throttle_interval(source_hz), None);

        assert_eq!(
            UpdateRate::max(1)?.throttle_interval(source_hz),
            Some(Duration::from_secs(1))
        );

        assert_eq!(
            UpdateRate::max(2)?.throttle_interval(source_hz),
            Some(Duration::from_millis(500))
        );

        Ok(())
    }

    #[test]
    fn duration() -> Result<(), IRacingSDKError> {
        // 60hz is the iRacing SDK default output rate.
        let source_hz = 60_f64;

        assert_eq!(
            UpdateRate::max(1)?.throttle_interval(source_hz).unwrap(),
            Duration::from_secs(1)
        );

        Ok(())
    }
}
