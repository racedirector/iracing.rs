//! Session info caching and parsing
//!
//! This module provides session info caching and YAML parsing utilities with
//! support for iRacing's non-standard YAML format.

use super::SessionInfo;
use crate::error::TelemetryError;
use anyhow::Result;
use tracing::debug;

/// Session info cache entry with version tracking
#[derive(Debug, Clone)]
pub struct SessionInfoCache {
    /// Cached session info
    pub session_info: SessionInfo,
    /// Version counter when this was cached
    pub version: u32,
    /// Parse timestamp for cache validity
    pub parsed_at: std::time::SystemTime,
}

impl SessionInfoCache {
    /// Create new cache entry
    pub fn new(session_info: SessionInfo, version: u32) -> Self {
        Self { session_info, version, parsed_at: std::time::SystemTime::now() }
    }

    /// Check if cache is valid for given version
    pub fn is_valid(&self, current_version: u32) -> bool {
        self.version == current_version
    }
}

/// Session info parser with YAML preprocessing for iRacing compatibility
#[derive(Debug, Clone)]
pub struct SessionInfoParser {
    /// Current cached session info
    cache: Option<SessionInfoCache>,
}

impl Default for SessionInfoParser {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionInfoParser {
    /// Create new session info parser
    pub fn new() -> Self {
        Self { cache: None }
    }

    /// Parse session info from shared memory with caching
    pub fn parse_from_memory(
        &mut self,
        memory: &[u8],
        session_info_offset: i32,
        session_info_len: i32,
        session_version: u32,
    ) -> Result<SessionInfo> {
        // Check cache validity first
        if let Some(cached) = &self.cache {
            if cached.is_valid(session_version) {
                debug!(version = session_version, "Using cached session info");
                return Ok(cached.session_info.clone());
            }
        }

        debug!(
            version = session_version,
            offset = session_info_offset,
            length = session_info_len,
            "Parsing fresh session info from memory"
        );

        // Extract YAML from memory
        let raw_yaml =
            self.extract_yaml_from_memory(memory, session_info_offset, session_info_len)?;

        // Parse YAML to SessionInfo (preprocessing happens inside parse())
        let session_info = self.parse(&raw_yaml)?;

        // Update cache
        self.cache = Some(SessionInfoCache::new(session_info.clone(), session_version));

        // Return the session info
        Ok(session_info)
    }

    /// Extract YAML string from shared memory
    pub fn extract_yaml_from_memory(
        &self,
        memory: &[u8],
        offset: i32,
        length: i32,
    ) -> Result<String> {
        if offset < 0 || length <= 0 {
            return Err(TelemetryError::Parse {
                context: "Session info extraction".to_string(),
                details: format!("Invalid offset {} or length {}", offset, length),
            }
            .into());
        }

        let start = offset as usize;
        let end = start + (length as usize);

        if end > memory.len() {
            return Err(TelemetryError::Memory { offset: end, source: None }.into());
        }

        // Extract bytes and convert to string
        let yaml_bytes = &memory[start..end];

        // Find null terminator or use full length
        let null_pos = yaml_bytes.iter().position(|&b| b == 0).unwrap_or(yaml_bytes.len());

        // Convert to UTF-8 string
        let yaml_str = String::from_utf8_lossy(&yaml_bytes[..null_pos]).to_string();

        if yaml_str.trim().is_empty() {
            return Err(TelemetryError::Parse {
                context: "Session YAML extraction".to_string(),
                details: "Extracted YAML string is empty".to_string(),
            }
            .into());
        }

        Ok(yaml_str)
    }

    /// Preprocess iRacing YAML to fix compatibility issues with unescaped characters
    /// Based on iRacing forum discussion: <https://forums.iracing.com/discussion/comment/374646#Comment_374646>
    pub fn preprocess_iracing_yaml(&self, yaml: &str) -> Result<String> {
        const PROBLEMATIC_KEYS: &[&str] = &[
            "AbbrevName:",
            "TeamName:",
            "UserName:",
            "Initials:",
            "DriverSetupName:",
            "CarDesignStr:", // Car livery color codes - can start with comma
        ];

        // First pass: Remove all control characters (except \n, \r, \t)
        let mut cleaned = String::with_capacity(yaml.len());
        for ch in yaml.chars() {
            if ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t' {
                // Skip control characters except basic whitespace
                continue;
            }
            cleaned.push(ch);
        }

        let lines: Vec<&str> = cleaned.lines().collect();
        let mut result_lines = Vec::with_capacity(lines.len());

        for line in lines {
            let mut processed_line = line.to_string();

            // Check if this line contains any problematic keys
            for &key in PROBLEMATIC_KEYS {
                if let Some(colon_pos) = line.find(key) {
                    let after_colon = colon_pos + key.len();
                    if let Some(value_start) =
                        line[after_colon..].find(|c: char| !c.is_whitespace())
                    {
                        let actual_value_start = after_colon + value_start;
                        let value = line[actual_value_start..].trim();

                        if !value.is_empty() && !value.starts_with('\'') && !value.starts_with('"')
                        {
                            // Need to quote this value
                            let escaped_value = value.replace('\'', "''");
                            processed_line = format!(
                                "{}{} '{}'",
                                &line[..after_colon],
                                &line[after_colon..actual_value_start],
                                escaped_value
                            );
                        }
                    }
                    break; // Only process first match per line
                }
            }

            result_lines.push(processed_line);
        }

        let result = result_lines.join("\n");

        // Handle edge case where input is just whitespace/newlines
        if yaml.trim().is_empty() {
            return Ok(yaml.to_string());
        }

        Ok(result)
    }

    /// Parse YAML to SessionInfo struct (with automatic preprocessing)
    pub fn parse(&self, yaml: &str) -> Result<SessionInfo> {
        // Preprocess the YAML to handle iRacing's quirks (control characters, unquoted values)
        let preprocessed = self.preprocess_iracing_yaml(yaml)?;

        match serde_yaml_ng::from_str::<SessionInfo>(&preprocessed) {
            Ok(session_info) => {
                self.validate_session_info(&session_info)?;
                Ok(session_info)
            }
            Err(e) => Err(TelemetryError::Parse {
                context: "Session YAML deserialization".to_string(),
                details: format!("YAML parsing failed: {}", e),
            }
            .into()),
        }
    }

    /// Validate parsed session info for completeness
    pub fn validate_session_info(&self, session_info: &SessionInfo) -> Result<()> {
        if session_info.weekend_info.track_name.is_empty() {
            return Err(TelemetryError::Parse {
                context: "Session validation".to_string(),
                details: "Missing track name".to_string(),
            }
            .into());
        }

        if session_info.weekend_info.track_display_name.is_empty() {
            return Err(TelemetryError::Parse {
                context: "Session validation".to_string(),
                details: "Missing track display name".to_string(),
            }
            .into());
        }

        if session_info.session_info.sessions.is_empty() {
            return Err(TelemetryError::Parse {
                context: "Session validation".to_string(),
                details: "No sessions found".to_string(),
            }
            .into());
        }

        Ok(())
    }

    /// Get cached session info if valid for version
    pub fn get_cached(&self, version: u32) -> Option<SessionInfo> {
        self.cache
            .as_ref()
            .filter(|cache| cache.is_valid(version))
            .map(|cache| cache.session_info.clone())
    }

    /// Clear session info cache
    pub fn clear_cache(&mut self) {
        self.cache = None;
    }
}
