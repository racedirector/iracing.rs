use std::{num::NonZeroU8, time::Duration};

///
/// FPS for telemetry updates.
///
/// Represents the FPS at which telemetry updates can be received.
/// The data is updated every ~16ms, or 60 FPS.
/// Data can be retrieved every frame, or once a second.
///
/// # Examples
///
/// ```
/// use iracing::fps::Fps;
/// use std::num::NonZeroU8;
/// let _ = Fps::new(30);
/// let _ = Fps(NonZeroU8::new(30).unwrap());
/// ```
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Fps(pub NonZeroU8);

impl Fps {
    ///
    /// The maximum number of times to check per second.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::{time::Duration, num::NonZeroU8};
    /// use iracing::fps::Fps;
    /// assert_eq!(Fps::MAX.0, NonZeroU8::new(60).unwrap());
    /// assert_eq!(Fps::MAX.to_duration(), Duration::from_millis(16));
    /// ```
    pub const MAX: Fps = Fps(NonZeroU8::new(60).unwrap());

    ///
    /// The smallest number of times to check per second.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::{time::Duration, num::NonZeroU8};
    /// use iracing::fps::Fps;
    /// assert_eq!(Fps::MIN.0, NonZeroU8::new(1).unwrap());
    /// assert_eq!(Fps::MIN.to_duration(), Duration::from_millis(1000));
    /// ```
    pub const MIN: Fps = Fps(NonZeroU8::new(1).unwrap());

    #[track_caller]
    pub const fn new(value: u8) -> Fps {
        match NonZeroU8::new(value) {
            Some(v) if value <= 60 => Fps(v),
            _ => panic!("FPS must be between 1 and 60"),
        }
    }

    #[inline]
    pub fn to_duration(&self) -> Duration {
        Duration::from_millis(1000 / self.0.get() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn over_expected_fps() {
        Fps::new(61);
    }

    #[test]
    #[should_panic]
    fn under_expected_fps() {
        Fps::new(0);
    }

    #[test]
    fn validate_duration() {
        assert_eq!(Fps::MIN.to_duration(), Duration::from_millis(1000));
        assert_eq!(Fps::MAX.to_duration(), Duration::from_millis(16));
        assert_eq!(Fps::new(30).to_duration(), Duration::from_millis(33));
    }
}
