use std::path::PathBuf;

use thiserror::Error;

#[cfg(windows)]
use windows_core as core;

/// Result type alias for telemetry operations.
pub type Result<T, E = IRacingSDKError> = std::result::Result<T, E>;

/// Main error type for telemetry operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum IRacingSDKError {
    /// Failed to open or maintain a connection to the iRacing shared-memory API.
    #[error("Failed to connect to iRacing: {reason}")]
    Connection {
        /// Human-readable description of why the connection failed.
        reason: String,
        /// The source error
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// An I/O error occurred while reading or seeking an `.ibt` telemetry file.
    #[error("IBT file error: {path}")]
    File {
        /// Path of the file that triggered the error.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The iRacing SDK header version does not match the expected version.
    #[error("SDK version mismatch: expected {expected}, found {found}")]
    Version {
        /// The SDK version this library requires.
        expected: u32,
        /// The SDK version found in the data source.
        found: u32,
    },

    /// A memory access at the given offset was invalid or out of bounds.
    #[error("Memory access violation at offset {offset:#x}")]
    Memory {
        /// Byte offset at which the access violation occurred.
        offset: usize,
        /// Optional source error carrying additional context.
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// A structured or textual value could not be parsed.
    #[error("Parse error in {context}: {details}")]
    Parse {
        /// Human-readable description of the parsing stage that failed.
        context: String,
        /// Detailed description of the parse failure.
        details: String,
    },

    #[error("Field '{field}' not found in telemetry data")]
    FieldNotFound { field: String },

    /// A value could not be converted to the expected type.
    #[error("Type conversion error: {details}")]
    TypeConversion {
        /// Description of the failed conversion.
        details: String,
    },

    #[error("{feature} is only available on {required_platform}")]
    UnsupportedPlatform {
        feature: String,
        required_platform: String,
    },

    /// A Windows API call failed.
    #[error("Windows API error: {operation}")]
    #[cfg(windows)]
    WindowsApi {
        /// Name or description of the Windows operation that failed.
        operation: String,
        /// Underlying Windows error.
        #[source]
        source: core::Error,
    },

    #[error("Schema validation failed: {reason}")]
    SchemaValidation {
        reason: String,
        expected_version: Option<u32>,
        actual_version: Option<u32>,
    },

    /// A buffer read or write operation failed.
    #[error("Buffer operation failed: {context}")]
    Buffer {
        /// Description of the buffer operation that failed.
        context: String,
        /// Index of the buffer involved, if known.
        buffer_index: Option<usize>,
        /// Optional source error carrying additional context.
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl IRacingSDKError {
    /// Returns whether this error is potentially recoverable through retry.
    pub fn is_retryable(&self) -> bool {
        match self {
            IRacingSDKError::Connection { .. } => true,
            IRacingSDKError::Buffer { .. } => true,
            IRacingSDKError::File { .. } => false,
            IRacingSDKError::Memory { .. } => false,
            IRacingSDKError::Version { .. } => false,
            IRacingSDKError::Parse { .. } => false,
            IRacingSDKError::FieldNotFound { .. } => false,
            IRacingSDKError::TypeConversion { .. } => false,
            IRacingSDKError::UnsupportedPlatform { .. } => false,
            #[cfg(windows)]
            IRacingSDKError::WindowsApi { .. } => true,
            IRacingSDKError::SchemaValidation { .. } => false,
        }
    }

    /// Returns suggested recovery actions for this error.
    pub fn recovery_suggestions(&self) -> Vec<&'static str> {
        match self {
            IRacingSDKError::Connection { .. } => vec![
                "Ensure iRacing is running",
                "Check Windows permissions for shared memory access",
                "Verify iRacing SDK version compatibility",
                "Try restarting iRacing",
            ],
            IRacingSDKError::Memory { .. } => vec![
                "Check memory access bounds",
                "Verify shared memory is still valid",
                "Restart the application",
            ],
            IRacingSDKError::File { .. } => vec![
                "Check file exists and is readable",
                "Verify IBT file format and version",
                "Ensure sufficient disk space",
                "Check file permissions",
            ],
            IRacingSDKError::Version { .. } => vec![
                "Update iRacing to latest version",
                "Update library to compatible version",
                "Check SDK compatibility matrix",
            ],
            IRacingSDKError::Parse { .. } => vec![
                "Check data format compatibility",
                "Verify source data integrity",
                "Update parsing logic if needed",
            ],
            &IRacingSDKError::FieldNotFound { .. } => vec![
                "Check field name spelling",
                "Verify field exists in current iRacing version",
                "Use optional field access patterns",
            ],
            IRacingSDKError::TypeConversion { .. } => vec![
                "Check data type compatibility",
                "Verify expected vs actual data types",
                "Use appropriate conversion methods",
            ],
            IRacingSDKError::UnsupportedPlatform { .. } => vec![
                "Use platform-appropriate features",
                "Consider IBT file replay for cross-platform testing",
                "Check documentation for platform requirements",
            ],
            #[cfg(windows)]
            IRacingSDKError::WindowsApi { .. } => vec![
                "Check Windows API permissions",
                "Verify system resources availability",
                "Check Windows version compatibility",
            ],
            IRacingSDKError::SchemaValidation { .. } => vec![
                "Check schema version compatibility",
                "Update to compatible data format",
                "Verify data structure integrity",
            ],
            IRacingSDKError::Buffer { .. } => vec![
                "Check buffer synchronization",
                "Verify buffer access patterns",
                "Restart buffer management",
            ],
        }
    }

    /// Helper constructor for connection errors.
    pub fn connection_failed(reason: impl Into<String>) -> Self {
        IRacingSDKError::Connection {
            reason: reason.into(),
            source: None,
        }
    }

    /// Helper constructor for file errors with path context.
    pub fn file_error(path: PathBuf, source: std::io::Error) -> Self {
        IRacingSDKError::File { path, source }
    }

    /// Helper constructor for memory access errors.
    pub fn memory_access_error(offset: usize) -> Self {
        IRacingSDKError::Memory {
            offset,
            source: None,
        }
    }

    /// Helper constructor for Windows API errors.
    #[cfg(windows)]
    pub fn windows_api_error(operation: impl Into<String>, source: core::Error) -> Self {
        IRacingSDKError::WindowsApi {
            operation: operation.into(),
            source,
        }
    }

    /// Helper constructor for schema validation errors.
    pub fn schema_validation_error(
        reason: impl Into<String>,
        expected_version: Option<u32>,
        actual_version: Option<u32>,
    ) -> Self {
        IRacingSDKError::SchemaValidation {
            reason: reason.into(),
            expected_version,
            actual_version,
        }
    }

    /// Helper constructor for buffer operation errors.
    pub fn buffer_operation_error(context: impl Into<String>, buffer_index: Option<usize>) -> Self {
        IRacingSDKError::Buffer {
            context: context.into(),
            buffer_index,
            source: None,
        }
    }

    /// Helper constructor for unsupported platform errors.
    pub fn unsupported_platform(
        feature: impl Into<String>,
        required_platform: impl Into<String>,
    ) -> Self {
        IRacingSDKError::UnsupportedPlatform {
            feature: feature.into(),
            required_platform: required_platform.into(),
        }
    }
}

// Comprehensive From implementations
impl From<std::io::Error> for IRacingSDKError {
    fn from(err: std::io::Error) -> Self {
        IRacingSDKError::File {
            path: PathBuf::from("<unknown>"),
            source: err,
        }
    }
}

#[cfg(windows)]
impl From<core::Error> for IRacingSDKError {
    fn from(err: core::Error) -> Self {
        IRacingSDKError::WindowsApi {
            operation: "Unknown Windows operation".to_string(),
            source: err,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(test)]
    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn error_conversions_work_for_all_generated_variants(
                offset in 0usize..0x10000usize,
            ) {
                // Property: Error conversions work for all generated error variants

                // Test various error variant creations
                let memory_err = IRacingSDKError::memory_access_error(offset);

                // Property: All variants should be constructible and display correctly
                prop_assert!(!memory_err.to_string().is_empty());
            }

            #[test]
            fn error_messages_format_correctly_with_arbitrary_context(
                reason in ".*",
                field_name in "\\w+",
                offset in 0usize..0x10000usize,
                expected_version in 1u32..10u32,
                found_version in 1u32..10u32,
                details in ".*"
            ) {
                // Property: Error messages format correctly with arbitrary context strings
                let connection_error = IRacingSDKError::Connection { reason: reason.clone(), source: None };
                let field_error = IRacingSDKError::FieldNotFound { field: field_name.clone() };
                let memory_error = IRacingSDKError::Memory { offset, source: None };
                let version_error = IRacingSDKError::Version { expected: expected_version, found: found_version };
                let conversion_error = IRacingSDKError::TypeConversion { details: details.clone() };

                // Property: All error messages should contain their context
                let connection_msg = connection_error.to_string();
                prop_assert!(connection_msg.contains(&reason));

                let field_msg = field_error.to_string();
                prop_assert!(field_msg.contains(&field_name));

                let memory_msg = memory_error.to_string();
                let offset_hex = format!("{:#x}", offset);
                prop_assert!(memory_msg.contains(&offset_hex));

                let version_msg = version_error.to_string();
                prop_assert!(version_msg.contains(&expected_version.to_string()));
                prop_assert!(version_msg.contains(&found_version.to_string()));

                let conversion_msg = conversion_error.to_string();
                prop_assert!(conversion_msg.contains(&details));

                prop_assert!(!connection_msg.is_empty());
                prop_assert!(!field_msg.is_empty());
                prop_assert!(!memory_msg.is_empty());
                prop_assert!(!version_msg.is_empty());
                prop_assert!(!conversion_msg.is_empty());
            }

            #[test]
            fn error_source_chaining_preserves_information_through_nested_trees(
                chain_depth in 1usize..5usize,
                base_message in ".*",
                intermediate_reasons in prop::collection::vec(".*", 1..5)
            ) {
                // Property: Error source chaining preserves information through nested trees
                let mut current_error: Box<dyn std::error::Error + Send + Sync> =
                    Box::new(std::io::Error::other(base_message.clone()));

                // Add intermediate layers
                for (i, reason) in intermediate_reasons.iter().enumerate().take(chain_depth.saturating_sub(1)) {
                    current_error = Box::new(IRacingSDKError::Connection {
                    reason: format!("Level {}: {}", i, reason),
                    source: Some(current_error),
                    });
                }

                // Create top-level error
                let top_error = IRacingSDKError::Connection {
                    reason: "Top level".to_string(),
                    source: Some(current_error),
                };

                // Property: Should be able to traverse the entire chain
                let mut traversed_count = 0;
                let mut current = std::error::Error::source(&top_error);
                let mut found_base_message = false;

                while let Some(source) = current {
                    traversed_count += 1;

                    // Check if we found the base message
                    if source.to_string().contains(&base_message) {
                    found_base_message = true;
                    }

                    current = std::error::Error::source(source);

                    // Prevent infinite loops
                    if traversed_count > 10 {
                    break;
                    }
                }

                // Property: Chain depth should be reasonable (1 base + intermediate layers)
                let expected_depth = 1 + intermediate_reasons.len().min(chain_depth.saturating_sub(1));
                prop_assert_eq!(traversed_count, expected_depth);

                // Property: Base message should be preserved
                prop_assert!(found_base_message, "Base message '{}' not found in chain", base_message);
            }

            #[test]
            fn platform_error_handling_works_across_failure_modes(
                operation in ".*",
                _error_code in 0u32..1000u32
            ) {
                // Property: Platform error handling works across generated failure modes

                // Test cross-platform error creation
                let generic_error = IRacingSDKError::connection_failed(operation.clone());
                prop_assert!(generic_error.to_string().contains(&operation));

                #[cfg(not(windows))]
                {
                    // On non-Windows platforms, ensure graceful degradation
                    let fallback_error = IRacingSDKError::connection_failed(format!("Platform error: {}", _error_code));
                    prop_assert!(!fallback_error.to_string().is_empty());
                }
            }
        }
    }

    #[test]
    fn error_constructors_validation() {
        // Unit test: Simple error constructor validation
        let file_error = IRacingSDKError::file_error(
            PathBuf::from("/test"),
            std::io::Error::new(std::io::ErrorKind::NotFound, "test"),
        );
        assert!(matches!(file_error, IRacingSDKError::File { .. }));

        let conn_error = IRacingSDKError::connection_failed("test");
        assert!(matches!(conn_error, IRacingSDKError::Connection { .. }));

        let mem_error = IRacingSDKError::memory_access_error(0x1000);
        assert!(matches!(mem_error, IRacingSDKError::Memory { .. }));
    }

    #[test]
    fn error_traits_validation() {
        // Compile-time check: IRacingSDKError must be Send + Sync + 'static
        fn assert_send_sync_static<T: Send + Sync + 'static>() {}
        assert_send_sync_static::<IRacingSDKError>();

        // Runtime check: Error trait is implemented
        let error = IRacingSDKError::connection_failed("test");
        let _: &dyn std::error::Error = &error;
    }

    #[test]
    fn recovery_methods_work() {
        // Test that recovery methods provide actionable guidance
        let connection_error = IRacingSDKError::connection_failed("test");
        let memory_error = IRacingSDKError::memory_access_error(0x1000);
        let version_error = IRacingSDKError::Version {
            expected: 2,
            found: 1,
        };

        // Test is_retryable classification
        assert!(connection_error.is_retryable());
        assert!(!memory_error.is_retryable());
        assert!(!version_error.is_retryable());

        // Test recovery suggestions are provided
        let conn_suggestions = connection_error.recovery_suggestions();
        let mem_suggestions = memory_error.recovery_suggestions();
        let ver_suggestions = version_error.recovery_suggestions();

        assert!(!conn_suggestions.is_empty());
        assert!(!mem_suggestions.is_empty());
        assert!(!ver_suggestions.is_empty());

        // All suggestions should be actionable (non-empty strings)
        for suggestion in &conn_suggestions {
            assert!(!suggestion.is_empty());
            assert!(suggestion.len() > 5); // Should be descriptive
        }
    }

    #[test]
    fn from_conversions_work() {
        // Test From trait implementations
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test file");
        let telemetry_err: IRacingSDKError = io_err.into();

        match telemetry_err {
            IRacingSDKError::File { source, .. } => {
                assert_eq!(source.to_string(), "test file");
            }
            _ => panic!("Expected File error variant"),
        }
    }
}
