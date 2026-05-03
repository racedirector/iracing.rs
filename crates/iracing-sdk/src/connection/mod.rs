//! Connection types for live and replay telemetry

pub mod live;

pub mod ibt;

/// Unified entry point for iRacing SDK telemetry connections.
///
/// This factory provides a consistent API for creating connections iRacing telemetry.
///
/// # Examples
///
/// ## Live Telemetry (Windows)
/// ```rust,no_run
/// use iracing_sdk::IRacingSDKConnection;
///
/// #[tokio::main]
/// async fn main() -> iracing_sdk::Result<()> {
///     let connection = IRacingSDKConnection::connect().await?;
///     // Use connection...
///     Ok(())
/// }
/// ```
pub struct IRacingSDKConnection;

impl IRacingSDKConnection {
    /// Connect to live iRacing telemetry.
    ///
    /// Establishes a connection to iRacing's shared memory on Windows.
    /// This method waits for iRacing to be running and telemetry to be available.
    ///
    /// # Platform
    ///
    /// This method is only available on Windows where iRacing runs.
    /// On other platforms, this method returns an `UnsupportedPlatform` error.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Platform is not Windows
    /// - iRacing is not running
    /// - Shared memory is not accessible
    /// - Connection timeout is reached
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use iracing_sdk::IRacingSDKConnection;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> iracing_sdk::Result<()> {
    /// let connection = IRacingSDKConnection::connect().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect() -> crate::Result<live::LiveConnection> {
        live::LiveConnection::connect().await
    }
}
