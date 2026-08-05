use std::path::PathBuf;

use crate::{Result, providers::ibt::IbtProvider};

use super::IbtConnection;

/// Type state indicating that an [`IbtConnectionBuilder`] has no source yet.
#[derive(Debug, Default)]
pub struct NoSource;

/// Type state containing the path from which an [`IbtConnection`] will be opened.
#[derive(Debug)]
pub struct PathSource {
    path: PathBuf,
}

/// Type state containing an explicit provider for an [`IbtConnection`].
pub struct ProviderSource {
    provider: IbtProvider,
}

/// Type-state builder for an [`IbtConnection`].
///
/// The initial [`NoSource`] state has no `build` method. Select either a path
/// with [`Self::with_path`] or an existing provider with
/// [`Self::with_provider`] before building the connection.
///
/// ```compile_fail
/// use iracing_sdk::connections::ibt::IbtConnection;
///
/// # async fn example() {
/// let _connection = IbtConnection::builder().build().await;
/// # }
/// ```
pub struct IbtConnectionBuilder<Source = NoSource> {
    source: Source,
}

impl Default for IbtConnectionBuilder<NoSource> {
    fn default() -> Self {
        Self { source: NoSource }
    }
}

impl IbtConnectionBuilder<NoSource> {
    /// Select an `.ibt` file path as the connection source.
    pub fn with_path<P: Into<PathBuf>>(self, path: P) -> IbtConnectionBuilder<PathSource> {
        IbtConnectionBuilder {
            source: PathSource { path: path.into() },
        }
    }

    /// Select an existing [`IbtProvider`] as the connection source.
    pub fn with_provider(self, provider: IbtProvider) -> IbtConnectionBuilder<ProviderSource> {
        IbtConnectionBuilder {
            source: ProviderSource { provider },
        }
    }
}

impl IbtConnectionBuilder<PathSource> {
    /// Open the configured path and build an [`IbtConnection`].
    pub async fn build(self) -> Result<IbtConnection> {
        tracing::info!("Opening IBT file: {}", self.source.path.display());
        let provider = IbtProvider::open(self.source.path)?;
        IbtConnection::from_provider(provider).await
    }
}

impl IbtConnectionBuilder<ProviderSource> {
    /// Build an [`IbtConnection`] from the configured provider.
    pub async fn build(self) -> Result<IbtConnection> {
        IbtConnection::from_provider(self.source.provider).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SchemaProvider;
    use crate::test_utils::require_smallest_ibt_fixture;

    #[tokio::test]
    async fn path_source_builds_connection() -> Result<()> {
        let path = require_smallest_ibt_fixture()
            .expect("generated IBT fixture should be available for connection tests");

        let connection = IbtConnection::builder().with_path(path).build().await?;

        assert!(connection.schema().variable_count() > 0);
        assert!(connection.source_hz() > 0.0);
        Ok(())
    }

    #[tokio::test]
    async fn explicit_provider_builds_connection() -> Result<()> {
        let path = require_smallest_ibt_fixture()
            .expect("generated IBT fixture should be available for connection tests");
        let provider = IbtProvider::open(path)?;

        let connection = IbtConnection::builder()
            .with_provider(provider)
            .build()
            .await?;

        assert!(connection.schema().variable_count() > 0);
        Ok(())
    }
}
