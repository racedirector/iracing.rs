use crate::Result;

#[cfg(windows)]
use crate::providers::live::LiveProvider;

use super::LiveConnection;

/// Builder for a [`LiveConnection`].
///
/// On Windows, the default builder creates a [`LiveProvider`] with its default
/// connection policy. Use `with_provider` to supply a customized provider.
///
/// ```no_run
/// # #[cfg(windows)]
/// # fn example() -> iracing_sdk::Result<()> {
/// use iracing_sdk::connections::live::LiveConnection;
///
/// let connection = LiveConnection::builder().build()?;
/// # drop(connection);
/// # Ok(())
/// # }
/// ```
#[derive(Default)]
pub struct LiveConnectionBuilder {
    #[cfg(windows)]
    provider: Option<LiveProvider>,
}

impl LiveConnectionBuilder {
    /// Use an explicitly configured live telemetry provider.
    #[cfg(windows)]
    pub fn with_provider(mut self, provider: LiveProvider) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Build the live telemetry connection.
    ///
    /// On non-Windows platforms this returns
    /// [`crate::IRacingSDKError::UnsupportedPlatform`].
    pub fn build(self) -> Result<LiveConnection> {
        #[cfg(windows)]
        {
            tracing::info!("Connecting to iRacing live telemetry.");
            let provider = match self.provider {
                Some(provider) => provider,
                None => LiveProvider::builder().build()?,
            };
            Ok(LiveConnection::from_provider(provider))
        }

        #[cfg(not(windows))]
        {
            Err(crate::IRacingSDKError::unsupported_platform(
                "Live telemetry",
                "Windows",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_builder_can_be_created() {
        let _builder = LiveConnection::builder();
    }

    #[cfg(not(windows))]
    #[test]
    fn build_reports_unsupported_platform() {
        let error = match LiveConnection::builder().build() {
            Ok(_) => panic!("live telemetry should be unavailable off Windows"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            crate::IRacingSDKError::UnsupportedPlatform { .. }
        ));
    }
}
