use thiserror::Error;

/// Result type alias for telemetry operations.
pub type Result<T, E = TelemetryError> = std::result::Result<T, E>;

/// Main error type for telemetry operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum TelemetryError {
    #[error("SDK version mismatch: expected {expected}, found {found}")]
    Version { expected: u32, found: u32 },

    #[error("Memory access violation at offset {offset:#x}")]
    Memory {
        offset: usize,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Parse error in {context}: {details}")]
    Parse { context: String, details: String },

    #[error("Type conversion error: {details}")]
    TypeConversion { details: String },
}

impl TelemetryError {
    /// Returns whether this error is potentially recoverable through retry.
    pub fn is_retryable(&self) -> bool {
        match self {
            TelemetryError::Memory { .. } => false,
            TelemetryError::Version { .. } => false,
            TelemetryError::Parse { .. } => false,
            TelemetryError::TypeConversion { .. } => false,
        }
    }

    /// Returns suggested recovery actions for this error.
    pub fn recovery_suggestions(&self) -> Vec<&'static str> {
        match self {
            TelemetryError::Memory { .. } => vec![
                "Check memory access bounds",
                "Verify shared memory is still valid",
                "Restart the application",
            ],
            TelemetryError::Version { .. } => vec![
                "Update iRacing to latest version",
                "Update library to compatible version",
                "Check SDK compatibility matrix",
            ],
            TelemetryError::Parse { .. } => vec![
                "Check data format compatibility",
                "Verify source data integrity",
                "Update parsing logic if needed",
            ],
            TelemetryError::TypeConversion { .. } => vec![
                "Check data type compatibility",
                "Verify expected vs actual data types",
                "Use appropriate conversion methods",
            ],
        }
    }

    /// Helper constructor for memory access errors.
    pub fn memory_access_error(offset: usize) -> Self {
        TelemetryError::Memory {
            offset,
            source: None,
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use std::path::PathBuf;
//     use std::time::Duration;

//     #[cfg(test)]
//     mod property_tests {
//         use super::*;
//         use proptest::prelude::*;

//         proptest! {
//           #[test]
//           fn error_conversions_work_for_all_generated_variants(
//             offset in 0usize..0x10000usize,
//           ) {
//             // Property: Error conversions work for all generated error variants

//             // Test various error variant creations
//             let memory_err = TelemetryError::memory_access_error(offset);

//             // Property: All variants should be constructible and display correctly
//             prop_assert!(!memory_err.to_string().is_empty());
//           }

//           #[test]
//           fn error_messages_format_correctly_with_arbitrary_context(
//             reason in ".*",
//             field_name in "\\w+",
//             offset in 0usize..0x10000usize,
//             expected_version in 1u32..10u32,
//             found_version in 1u32..10u32,
//             details in ".*"
//           ) {
//             // Property: Error messages format correctly with arbitrary context strings
//             let memory_error = TelemetryError::Memory { offset, source: None };
//             let version_error = TelemetryError::Version { expected: expected_version, found: found_version };
//             let conversion_error = TelemetryError::TypeConversion { details: details.clone() };

//             // Property: All error messages should contain their context
//             let memory_msg = memory_error.to_string();
//             let offset_hex = format!("{:#x}", offset);
//             prop_assert!(memory_msg.contains(&offset_hex));

//             let version_msg = version_error.to_string();
//             prop_assert!(version_msg.contains(&expected_version.to_string()));
//             prop_assert!(version_msg.contains(&found_version.to_string()));

//             let conversion_msg = conversion_error.to_string();
//             prop_assert!(conversion_msg.contains(&details));

//             // Property: No error message should be empty
//             prop_assert!(!memory_msg.is_empty());
//             prop_assert!(!version_msg.is_empty());
//             prop_assert!(!conversion_msg.is_empty());
//           }

//           #[test]
//           fn error_source_chaining_preserves_information_through_nested_trees(
//             chain_depth in 1usize..5usize,
//             base_message in ".*",
//             intermediate_reasons in prop::collection::vec(".*", 1..5)
//           ) {
//             // Property: Error source chaining preserves information through nested trees
//             let mut current_error: Box<dyn std::error::Error + Send + Sync> =
//               Box::new(std::io::Error::other(base_message.clone()));

//             // Add intermediate layers
//             for (i, reason) in intermediate_reasons.iter().enumerate().take(chain_depth.saturating_sub(1)) {
//               current_error = Box::new(TelemetryError::Connection {
//                 reason: format!("Level {}: {}", i, reason),
//                 source: Some(current_error),
//               });
//             }

//             // Create top-level error
//             let top_error = TelemetryError::Connection {
//               reason: "Top level".to_string(),
//               source: Some(current_error),
//             };

//             // Property: Should be able to traverse the entire chain
//             let mut traversed_count = 0;
//             let mut current = std::error::Error::source(&top_error);
//             let mut found_base_message = false;

//             while let Some(source) = current {
//               traversed_count += 1;

//               // Check if we found the base message
//               if source.to_string().contains(&base_message) {
//                 found_base_message = true;
//               }

//               current = std::error::Error::source(source);

//               // Prevent infinite loops
//               if traversed_count > 10 {
//                 break;
//               }
//             }

//             // Property: Chain depth should be reasonable (1 base + intermediate layers)
//             let expected_depth = 1 + intermediate_reasons.len().min(chain_depth.saturating_sub(1));
//             prop_assert_eq!(traversed_count, expected_depth);

//             // Property: Base message should be preserved
//             prop_assert!(found_base_message, "Base message '{}' not found in chain", base_message);
//           }

//           #[test]
//           fn platform_error_handling_works_across_failure_modes(
//             operation in ".*",
//             _error_code in 0u32..1000u32
//           ) {
//             // Property: Platform error handling works across generated failure modes

//             // Test cross-platform error creation
//             let generic_error = TelemetryError::connection_failed(operation.clone());
//             prop_assert!(generic_error.to_string().contains(&operation));

//             #[cfg(not(windows))]
//             {
//               // On non-Windows platforms, ensure graceful degradation
//               let fallback_error = TelemetryError::connection_failed(format!("Platform error: {}", _error_code));
//               prop_assert!(!fallback_error.to_string().is_empty());
//             }
//           }
//         }
//     }

//     #[test]
//     fn error_constructors_validation() {
//         // Unit test: Simple error constructor validation
//         let file_error = TelemetryError::file_error(
//             PathBuf::from("/test"),
//             std::io::Error::new(std::io::ErrorKind::NotFound, "test"),
//         );
//         assert!(matches!(file_error, TelemetryError::File { .. }));

//         let conn_error = TelemetryError::connection_failed("test");
//         assert!(matches!(conn_error, TelemetryError::Connection { .. }));

//         let mem_error = TelemetryError::memory_access_error(0x1000);
//         assert!(matches!(mem_error, TelemetryError::Memory { .. }));
//     }

//     #[test]
//     fn error_traits_validation() {
//         // Compile-time check: TelemetryError must be Send + Sync + 'static
//         fn assert_send_sync_static<T: Send + Sync + 'static>() {}
//         assert_send_sync_static::<TelemetryError>();

//         // Runtime check: Error trait is implemented
//         let error = TelemetryError::connection_failed("test");
//         let _: &dyn std::error::Error = &error;
//     }

//     #[test]
//     fn recovery_methods_work() {
//         // Test that recovery methods provide actionable guidance
//         let connection_error = TelemetryError::connection_failed("test");
//         let memory_error = TelemetryError::memory_access_error(0x1000);
//         let version_error = TelemetryError::Version {
//             expected: 2,
//             found: 1,
//         };

//         // Test is_retryable classification
//         assert!(connection_error.is_retryable());
//         assert!(!memory_error.is_retryable());
//         assert!(!version_error.is_retryable());

//         // Test recovery suggestions are provided
//         let conn_suggestions = connection_error.recovery_suggestions();
//         let mem_suggestions = memory_error.recovery_suggestions();
//         let ver_suggestions = version_error.recovery_suggestions();

//         assert!(!conn_suggestions.is_empty());
//         assert!(!mem_suggestions.is_empty());
//         assert!(!ver_suggestions.is_empty());

//         // All suggestions should be actionable (non-empty strings)
//         for suggestion in &conn_suggestions {
//             assert!(!suggestion.is_empty());
//             assert!(suggestion.len() > 5); // Should be descriptive
//         }
//     }

//     #[test]
//     fn from_conversions_work() {
//         // Test From trait implementations
//         let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test file");
//         let telemetry_err: TelemetryError = io_err.into();

//         match telemetry_err {
//             TelemetryError::File { source, .. } => {
//                 assert_eq!(source.to_string(), "test file");
//             }
//             _ => panic!("Expected File error variant"),
//         }
//     }
// }
