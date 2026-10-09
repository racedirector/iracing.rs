//! Telemetry and metadata provider traits for data sources.
use crate::{Result, SessionInfoBytes, VariableHeaders, schema::SessionInfo};

/// Trait indicating that an implementation can provide session information
/// bytes.
pub trait SessionInformationBytesProvider {
    /// Returns owned bytes, or `Ok(None)` when session information is absent.
    /// Acquisition failures return an error; bytes need not be valid YAML.
    fn session_info_snapshot(&self) -> Result<Option<SessionInfoBytes>>;
}

/// Trait indicating that an implementor can provide SessionInfo
pub trait SessionInformationProvider {
    /// Decodes, sanitizes, and parses the session snapshot.
    /// Returns `Ok(None)` on absence and an error on acquisition or parsing failure.
    fn session_info(&self) -> Result<Option<SessionInfo>>;
}

impl<T> SessionInformationProvider for T
where
    T: SessionInformationBytesProvider + ?Sized,
{
    fn session_info(&self) -> Result<Option<SessionInfo>> {
        let Some(bytes) = self.session_info_snapshot()? else {
            return Ok(None);
        };

        Ok(Some(SessionInfo::try_from(bytes)?))
    }
}

/// Trait indicating an implementor can provide variable headers.
pub trait VariableHeadersProvider {
    /// Returns owned headers, or an empty snapshot when metadata is absent.
    /// Acquisition and decoding failures return an error. Schema validation
    /// remains the responsibility of the caller.
    fn variable_headers(&self) -> Result<VariableHeaders>;
}

// Re-export for backwards compatibility
pub use crate::telemetry::provider::Provider;

#[cfg(test)]
mod tests {
    use super::*;

    struct BytesProvider(Option<SessionInfoBytes>);

    impl SessionInformationBytesProvider for BytesProvider {
        fn session_info_snapshot(&self) -> Result<Option<SessionInfoBytes>> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn parsed_session_supports_trait_objects_and_absence() {
        let source = BytesProvider(None);
        let provider: &dyn SessionInformationBytesProvider = &source;
        assert!(provider.session_info().unwrap().is_none());
    }

    #[test]
    fn parsed_session_propagates_invalid_yaml() {
        let source = BytesProvider(Some(SessionInfoBytes::from_checked_region(b"[")));
        assert!(source.session_info().is_err());
    }

    #[test]
    fn parsed_session_decodes_captured_session() {
        let source = BytesProvider(Some(SessionInfoBytes::from_checked_region(include_bytes!(
            "../../../test-data/session-yaml/utf-8-snapshot.yml"
        ))));
        assert!(source.session_info().unwrap().is_some());
    }
}
