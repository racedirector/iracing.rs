use std::time::Duration;

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
/// use iracing::telemetry::Connection;
///
/// let sampler = Connection::new()?.block()?;
///
/// let twice = Fps { value: 30 };
/// let _ = sampler.sample_fps(twice.to_duration())?;
/// let _ = sampler.sample_fps(Fps::MAX.to_duration())?;
/// let _ = sampler.sample_fps(Fps::MIN.to_duration())?;
/// ```
#[derive(Debug)]
pub struct Fps {
    //
    // A value between 1 and 60 representing the number of times to check for updates per second.
    pub value: u32,
}

impl Fps {
    ///
    /// The maximum number of times to check per second.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use iracing::fps::Fps;
    /// assert_eq!(Fps::MAX.value, 60);
    /// assert_eq!(Fps::MAX.to_ms().floor(), 16.0);
    /// assert_eq!(Fps::MAX.to_duration(), Duration::from_millis(16));
    /// ```
    pub const MAX: Fps = Fps::new(60);

    ///
    /// The smallest number of times to check per second.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use iracing::fps::Fps;
    /// assert_eq!(Fps::MIN.value, 1);
    /// assert_eq!(Fps::MIN.to_ms().floor(), 1000.0);
    /// assert_eq!(Fps::MIN.to_duration(), Duration::from_millis(1000));
    ///
    pub const MIN: Fps = Fps::new(1);

    pub const fn new(value: u32) -> Fps {
        if value < 1 || value > 60 {
            panic!("FPS value must be between 1 and 60.");
        }

        Fps { value }
    }

    pub fn to_ms(&self) -> f64 {
        1000.0 / self.value as f64
    }

    pub fn to_duration(&self) -> Duration {
        Duration::from_millis(self.to_ms().floor() as u64)
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
