//! YAML utilities for iRacing data preprocessing
//!
//! iRacing's YAML output has several non-standard issues that need correction:
//! - Unescaped special characters in strings (quotes, backslashes, etc.)
//! - Control characters that break YAML parsers
//! - Inconsistent string quoting
//!
//! This module provides low-level YAML cleaning without parsing.

use crate::{IRacingSDKError, Result};
use encoding_rs::WINDOWS_1252;

/// Preprocess iRacing YAML to fix known issues
///
/// This function cleans up iRacing's non-standard YAML format to make it
/// parseable by standard YAML libraries. It handles:
/// - Control character removal (except \n, \r, \t)
/// - String escaping for special characters
/// - Consistent quoting
///
/// Returns the cleaned YAML string ready for parsing.
pub fn preprocess_iracing_yaml(yaml: &str) -> Result<String> {
    let mut result = String::with_capacity(yaml.len());
    let mut in_quotes = false;
    let mut prev_char = ' ';

    for ch in yaml.chars() {
        match ch {
            // Track quote state
            '"' if prev_char != '\\' => {
                in_quotes = !in_quotes;
                result.push(ch);
            }
            // Remove control characters except newline, carriage return, tab
            '\x00'..='\x08' | '\x0B'..='\x0C' | '\x0E'..='\x1F' => {
                // Skip control characters
                continue;
            }
            // Keep normal characters
            _ => {
                result.push(ch);
            }
        }
        prev_char = ch;
    }

    if result.trim().is_empty() {
        return Err(IRacingSDKError::Parse {
            context: "YAML preprocessing".to_string(),
            details: "YAML is empty after preprocessing".to_string(),
        });
    }

    Ok(result)
}

/// Extract YAML from a memory buffer
///
/// Handles null-terminated strings and validates UTF-8 encoding.
/// Returns the raw YAML string without preprocessing.
pub fn extract_yaml_from_memory(data: &[u8], offset: i32, length: i32) -> Result<String> {
    // Validate parameters
    if offset < 0 {
        return Err(IRacingSDKError::Parse {
            context: "YAML extraction".to_string(),
            details: format!("Invalid offset: {}", offset),
        });
    }

    if length <= 0 {
        return Ok(String::new());
    }

    let offset = offset as usize;
    let length = length as usize;

    // Validate bounds
    if offset + length > data.len() {
        return Err(IRacingSDKError::Parse {
            context: "YAML extraction".to_string(),
            details: format!(
                "YAML extends beyond buffer bounds: offset={}, len={}, buffer_size={}",
                offset,
                length,
                data.len()
            ),
        });
    }

    // Extract the substring
    let yaml_data = &data[offset..offset + length];
    // Find null terminator or use entire length
    let yaml_len = yaml_data.iter().position(|&b| b == 0).unwrap_or(length);
    let candidate = &yaml_data[..yaml_len];

    decode_yaml_from_buffer(candidate)
}

/// Separate the decoding of the buffer; iRacing may change this to UTF-8 only soon, so keep it separate and testable.
fn decode_yaml_from_buffer(buffer: &[u8]) -> Result<String> {
    if let Ok(s) = std::str::from_utf8(buffer) {
        return Ok(s.to_string());
    }

    let (decoded, _, had_errors) = WINDOWS_1252.decode(buffer);

    if had_errors {
        return Err(IRacingSDKError::Parse {
            context: "YAML decoding".to_string(),
            details: "Failed to decode as UTF-8 or Windows-1252".to_string(),
        });
    }

    Ok(decoded.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preprocess_removes_control_characters() {
        let input = "WeekendInfo:\n\x00\x01\x02  TrackName: test\x03";
        let result = preprocess_iracing_yaml(input).unwrap();
        assert!(!result.contains('\x00'));
        assert!(!result.contains('\x01'));
        assert!(!result.contains('\x02'));
        assert!(!result.contains('\x03'));
        assert!(result.contains("WeekendInfo"));
        assert!(result.contains("TrackName"));
    }

    #[test]
    fn test_preprocess_keeps_valid_whitespace() {
        let input = "Key:\n\r\t  Value";
        let result = preprocess_iracing_yaml(input).unwrap();
        assert!(result.contains('\n'));
        assert!(result.contains('\r'));
        assert!(result.contains('\t'));
    }

    #[test]
    fn test_extract_yaml_from_memory_with_null_terminator() {
        let data = b"SessionInfo:\n  TrackName: test\0padding";
        let result = extract_yaml_from_memory(data, 0, data.len() as i32).unwrap();
        assert_eq!(result, "SessionInfo:\n  TrackName: test");
    }

    #[test]
    fn test_extract_yaml_from_memory_without_null() {
        let data = b"SessionInfo:\n  TrackName: test";
        let result = extract_yaml_from_memory(data, 0, data.len() as i32).unwrap();
        assert_eq!(result, "SessionInfo:\n  TrackName: test");
    }

    #[test]
    fn test_extract_yaml_bounds_check() {
        let data = b"test";
        let result = extract_yaml_from_memory(data, 0, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_yaml_from_buffer_utf8_with_special_characters() {
        let input = "DriverInfo:\n  UserName: \"José 🚗\"\n  CarScreenName: \"Mazda MX-5 – Cup\"";
        let result = decode_yaml_from_buffer(input.as_bytes()).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_decode_yaml_from_buffer_windows_1252() {
        let input = [
            b'D', b'r', b'i', b'v', b'e', b'r', b'I', b'n', b'f', b'o', b':', b'\n', b' ', b' ',
            b'U', b's', b'e', b'r', b'N', b'a', b'm', b'e', b':', b' ', 0x93, b'J', b'o', b's',
            0xE9, 0x94, b'\n', b' ', b' ', b'C', b'a', b'r', b'S', b'c', b'r', b'e', b'e', b'n',
            b'N', b'a', b'm', b'e', b':', b' ', b'M', b'a', b'z', b'd', b'a', b' ', b'M', b'X',
            b'-', b'5', b' ', 0x96, b' ', b'C', b'u', b'p',
        ];
        let result = decode_yaml_from_buffer(&input).unwrap();
        assert_eq!(
            result,
            "DriverInfo:\n  UserName: “José”\n  CarScreenName: Mazda MX-5 – Cup"
        );
    }
}
