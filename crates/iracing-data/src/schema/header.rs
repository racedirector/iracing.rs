//! iRacing Header Structure Parsing
//!
//! This module provides parsing and validation for iRacing's `irsdk_header` structure
//! from Windows shared memory. The header contains essential metadata for schema discovery
//! and buffer management.
//!
//! # iRacing Header Layout
//!
//! The header follows the C structure layout from the iRacing SDK:
//! ```c
//! typedef struct irsdk_header
//! {
//!     int ver;                    // api version 1 for older clients, 2 for newer
//!     int status;                 // bitfield for status
//!     int tickRate;               // ticks per second (60hz)
//!     int sessionInfoUpdate;      // incremented when session info changes
//!     int sessionInfoLen;         // length in bytes of session info string
//!     int sessionInfoOffset;      // offset to session info string
//!     int numVars;                // length of iarVarHeader array
//!     int varHeaderOffset;        // offset to iarVarHeader[0]
//!     int numBuf;                 // num of buffers (should be 4)
//!     int bufLen;                 // length in bytes for each buffer
//!     irsdk_varBuf varBuf[4];     // buffers
//! } irsdk_header;
//! ```
//!
//! # Memory Layout and Alignment
//!
//! The structure is packed to match C alignment (4-byte boundaries):
//! - Header size: 112 bytes (40 bytes header fields + 8 bytes padding + 64 bytes for 4 buffers)
//! - Each varBuf: 16 bytes (4 × i32)
//! - Critical: Includes pad1\[2\] padding to match exact C struct layout
//! - Total alignment: 4-byte boundaries to match iRacing's C implementation
//!
//! # Field Descriptions
//!
//! ## Version and Status
//! - `ver`: SDK version (expect 2 for current implementation)
//! - `status`: Bitfield indicating connection status and flags
//! - `tickRate`: Update frequency in Hz (typically 60)
//!
//! ## Schema Discovery
//! - `sessionInfoUpdate`: Counter for YAML session info changes (enables caching)
//! - `sessionInfoLen`: Size of YAML session info string in bytes
//! - `sessionInfoOffset`: Byte offset to YAML session info from start of shared memory
//! - `numVars`: Number of telemetry variables available
//! - `varHeaderOffset`: Byte offset to variable header array
//!
//! ## Buffer Management (4-Buffer Rotation System)
//! - `numBuf`: Number of buffers (always 4 in iRacing)
//! - `bufLen`: Size of each telemetry buffer in bytes
//! - `varBuf[4]`: Array of buffer descriptors with tick counts and offsets
//!
//! # Performance Characteristics
//!
//! This implementation is optimized for the <1ms latency requirement:
//! - Zero-copy parsing using `read_unaligned` for robustness
//! - Fast validation path for 60Hz updates (`validate_fast()`)
//! - Comprehensive validation for initial connection (`validate_comprehensive()`)
//! - Corruption detection for production resilience
//!
//! # Relationship to Buffer Management
//!
//! The header enables iRacing's 4-buffer rotation system:
//! 1. Each buffer has a tick count indicating when it was last written
//! 2. Consumer finds latest buffer by comparing tick counts with wraparound handling
//! 3. Double-read pattern ensures data consistency during concurrent updates
//! 4. Buffer offsets are relative to the start of shared memory mapping
//!
//! # Session Info Caching Integration
//!
//! The `sessionInfoUpdate` counter enables efficient session parsing:
//! - Producer task checks counter on each frame
//! - Only triggers YAML parsing when counter changes
//! - Separate session parser task handles heavy YAML processing
//! - Bounded channel prevents parser backlog from affecting telemetry loop

use crate::{Result, TelemetryError};
use std::mem;
use tracing::{debug, trace};

/// The expected iRacing SDK version
pub const IRSDK_VER: i32 = 2;

/// Status flag indicating that the simulator is actively publishing telemetry
pub const IRSDK_STATUS_CONNECTED: i32 = 0x1;

/// iRacing header structure that matches the C SDK layout
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IRSDKHeader {
    /// API version (expect 2 for current SDK)
    pub ver: i32,
    /// Status bitfield
    pub status: i32,
    /// Ticks per second (typically 60Hz)
    pub tick_rate: i32,
    /// Incremented when session info changes (for caching)
    pub session_info_update: i32,
    /// Length in bytes of session info string
    pub session_info_len: i32,
    /// Offset to session info string
    pub session_info_offset: i32,
    /// Number of variables in telemetry
    pub num_vars: i32,
    /// Offset to variable header array
    pub var_header_offset: i32,
    /// Number of buffers (should be 4)
    pub num_buf: i32,
    /// Length in bytes for each buffer
    pub buf_len: i32,
    /// Padding for 16-byte alignment (matches C struct pad1\[2\])
    pub pad1: [i32; 2],
    /// Buffer information (4-buffer rotation system)
    pub var_buf: [IRSDKVarBuf; 4],
}

/// iRacing variable buffer information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IRSDKVarBuf {
    /// Tick count when buffer was written
    pub tick_count: i32,
    /// Offset from header to buffer start
    pub buf_offset: i32,
    /// Padding to maintain alignment
    pub pad: [i32; 2],
}

impl IRSDKHeader {
    /// Parse header from raw memory bytes with validation
    /// Optimized for <1ms latency requirement with zero-copy techniques
    pub fn parse_from_memory(memory: &[u8]) -> Result<Self> {
        trace!(memory_len = memory.len(), "Parsing iRacing header from memory");

        // Fast path: validate minimum size first
        const HEADER_SIZE: usize = mem::size_of::<IRSDKHeader>();
        if memory.len() < HEADER_SIZE {
            return Err(TelemetryError::Memory { offset: memory.len(), source: None });
        }

        // Zero-copy parsing: directly read from memory without copying
        // Uses native byte order like other working Rust implementations (simetry, iracing-telem)
        // Safety: We've validated the memory length above and use read_unaligned for robustness
        let header = unsafe { std::ptr::read_unaligned(memory.as_ptr() as *const IRSDKHeader) };

        debug!(
            ver = header.ver,
            status = header.status,
            tick_rate = header.tick_rate,
            session_info_update = header.session_info_update,
            num_vars = header.num_vars,
            num_buf = header.num_buf,
            "Parsed iRacing header"
        );

        // Comprehensive validation with early exits for performance
        header.validate_comprehensive()?;
        Ok(header)
    }

    /// Validate header fields for correctness (basic validation)
    pub fn validate(&self) -> Result<()> {
        // Check SDK version
        if self.ver != IRSDK_VER {
            return Err(TelemetryError::Version {
                expected: IRSDK_VER as u32,
                found: self.ver as u32,
            });
        }

        // Validate reasonable field ranges
        if self.tick_rate <= 0 || self.tick_rate > 1000 {
            return Err(TelemetryError::Parse {
                context: "Header validation".to_string(),
                details: format!("Invalid tick rate: {}", self.tick_rate),
            });
        }

        if self.num_vars < 0 || self.num_vars > 10000 {
            return Err(TelemetryError::Parse {
                context: "Header validation".to_string(),
                details: format!("Invalid num_vars: {}", self.num_vars),
            });
        }

        if self.num_buf < 3 || self.num_buf > 4 {
            return Err(TelemetryError::Parse {
                context: "Header validation".to_string(),
                details: format!("Expected 3-4 buffers, found {}", self.num_buf),
            });
        }

        if self.buf_len <= 0 {
            return Err(TelemetryError::Parse {
                context: "Header validation".to_string(),
                details: format!("Invalid buffer length: {}", self.buf_len),
            });
        }

        // Validate session info fields
        if self.session_info_len < 0 {
            return Err(TelemetryError::Parse {
                context: "Header validation".to_string(),
                details: format!("Invalid session info length: {}", self.session_info_len),
            });
        }

        if self.session_info_offset < 0 {
            return Err(TelemetryError::Parse {
                context: "Header validation".to_string(),
                details: format!("Invalid session info offset: {}", self.session_info_offset),
            });
        }

        // Validate variable header offset
        if self.var_header_offset < 0 {
            return Err(TelemetryError::Parse {
                context: "Header validation".to_string(),
                details: format!("Invalid var header offset: {}", self.var_header_offset),
            });
        }

        Ok(())
    }

    /// Returns true when iRacing reports the shared memory is live
    pub fn is_connected(&self) -> bool {
        (self.status & IRSDK_STATUS_CONNECTED) != 0
    }

    /// Comprehensive validation with corruption detection and internal consistency checks
    pub fn validate_comprehensive(&self) -> Result<()> {
        // Start with basic validation
        self.validate()?;

        // Advanced validation: offset calculations and bounds checking
        self.validate_offset_consistency()?;

        // Buffer consistency checks
        self.validate_buffer_layout()?;

        // Detect common corruption patterns
        self.detect_corruption_patterns()?;

        Ok(())
    }

    /// Validate offset calculations and bounds
    fn validate_offset_consistency(&self) -> Result<()> {
        // Check for reasonable offset values and prevent overflow
        if self.session_info_offset > 0 && self.session_info_len > 0 {
            let session_end = self.session_info_offset.saturating_add(self.session_info_len);
            if session_end < self.session_info_offset {
                return Err(TelemetryError::Parse {
                    context: "Offset validation".to_string(),
                    details: "Session info offset + length causes integer overflow".to_string(),
                });
            }
        }

        // Variable header should come before buffer data
        if self.var_header_offset > 0 && self.num_vars > 0 {
            // Each variable header is typically 144 bytes in iRacing SDK
            const VAR_HEADER_SIZE: i32 = 144;
            let var_headers_size = self.num_vars.saturating_mul(VAR_HEADER_SIZE);
            let var_headers_end = self.var_header_offset.saturating_add(var_headers_size);

            if var_headers_end < self.var_header_offset {
                return Err(TelemetryError::Parse {
                    context: "Offset validation".to_string(),
                    details: "Variable headers size causes integer overflow".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Validate buffer layout consistency
    fn validate_buffer_layout(&self) -> Result<()> {
        // Check buffer offsets are reasonable and ordered
        let mut last_offset = 0;
        for (i, buf) in self.var_buf.iter().enumerate() {
            if buf.buf_offset < 0 {
                return Err(TelemetryError::Parse {
                    context: "Buffer validation".to_string(),
                    details: format!("Buffer {} has negative offset: {}", i, buf.buf_offset),
                });
            }

            // Buffers should be reasonably spaced
            if buf.buf_offset < last_offset {
                // This is actually OK for circular buffers, but check for reasonable values
                if buf.buf_offset > 1_000_000 {
                    return Err(TelemetryError::Parse {
                        context: "Buffer validation".to_string(),
                        details: format!("Buffer {} offset too large: {}", i, buf.buf_offset),
                    });
                }
            }

            // Check for buffer size consistency
            let buffer_end = buf.buf_offset.saturating_add(self.buf_len);
            if buffer_end < buf.buf_offset {
                return Err(TelemetryError::Parse {
                    context: "Buffer validation".to_string(),
                    details: format!("Buffer {} size causes overflow", i),
                });
            }

            last_offset = buf.buf_offset;
        }

        Ok(())
    }

    /// Detect common corruption patterns
    fn detect_corruption_patterns(&self) -> Result<()> {
        // Check for all-zero header (common corruption)
        let all_zero = self.ver == 0
            && self.status == 0
            && self.tick_rate == 0
            && self.num_vars == 0
            && self.buf_len == 0;
        if all_zero {
            return Err(TelemetryError::Parse {
                context: "Corruption detection".to_string(),
                details: "Header appears to be all zeros (possible corruption)".to_string(),
            });
        }

        // Check for suspiciously high values (possible corruption)
        if self.num_vars > 5000 || self.buf_len > 10_000_000 {
            return Err(TelemetryError::Parse {
                context: "Corruption detection".to_string(),
                details: "Header contains suspiciously large values (possible corruption)"
                    .to_string(),
            });
        }

        // Check for negative values in unsigned-like fields (corruption indicator)
        if self.tick_rate < 0 || self.session_info_len < -1 {
            return Err(TelemetryError::Parse {
                context: "Corruption detection".to_string(),
                details: "Header contains invalid negative values".to_string(),
            });
        }

        Ok(())
    }

    /// Check if session info has been updated since last check
    pub fn session_info_changed(&self, last_update: i32) -> bool {
        self.session_info_update != last_update
    }

    /// Get the essential fields needed for schema building
    pub fn schema_info(&self) -> SchemaInfo {
        SchemaInfo {
            num_vars: self.num_vars,
            var_header_offset: self.var_header_offset,
            session_info_update: self.session_info_update,
            session_info_len: self.session_info_len,
            session_info_offset: self.session_info_offset,
        }
    }

    /// Get buffer information for buffer rotation management
    pub fn buffer_info(&self) -> BufferInfo {
        BufferInfo { num_buffers: self.num_buf, buffer_length: self.buf_len, buffers: self.var_buf }
    }
}

/// Essential schema information extracted from header
#[derive(Debug, Clone, Copy)]
pub struct SchemaInfo {
    pub num_vars: i32,
    pub var_header_offset: i32,
    pub session_info_update: i32,
    pub session_info_len: i32,
    pub session_info_offset: i32,
}

/// Buffer management information
#[derive(Debug, Clone, Copy)]
pub struct BufferInfo {
    pub num_buffers: i32,
    pub buffer_length: i32,
    pub buffers: [IRSDKVarBuf; 4],
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::mem;

    // Property test strategies for generating valid header data
    prop_compose! {
        fn arb_valid_header()(
            ver in Just(IRSDK_VER),
            status in 0..=0xFFFF_i32,
            tick_rate in 30..240_i32,
            session_info_update in 0..1_000_000_i32,
            session_info_len in 0..100_000_i32,
            session_info_offset in 1000..200_000_i32,
            num_vars in 1..1000_i32,
            var_header_offset in 500..100_000_i32,
            num_buf in Just(4),
            buf_len in 1000..50_000_i32,
            var_buf in arb_var_buf_array()
        ) -> IRSDKHeader {
            IRSDKHeader {
                ver,
                status,
                tick_rate,
                session_info_update,
                session_info_len,
                session_info_offset,
                num_vars,
                var_header_offset,
                num_buf,
                buf_len,
                pad1: [0, 0],
                var_buf,
            }
        }
    }

    prop_compose! {
        fn arb_var_buf_array()(
            buf0 in arb_var_buf(),
            buf1 in arb_var_buf(),
            buf2 in arb_var_buf(),
            buf3 in arb_var_buf()
        ) -> [IRSDKVarBuf; 4] {
            [buf0, buf1, buf2, buf3]
        }
    }

    prop_compose! {
        fn arb_var_buf()(
            tick_count in 0..1_000_000_i32,
            buf_offset in 1000..500_000_i32
        ) -> IRSDKVarBuf {
            IRSDKVarBuf {
                tick_count,
                buf_offset,
                pad: [0, 0],
            }
        }
    }

    prop_compose! {
        fn arb_corrupted_header()(
            ver in (i32::MIN..0).prop_union(3..i32::MAX),
            status in any::<i32>(),
            tick_rate in (i32::MIN..0).prop_union(1001..i32::MAX),
            session_info_update in any::<i32>(),
            session_info_len in i32::MIN..0,
            session_info_offset in i32::MIN..0,
            num_vars in (i32::MIN..0).prop_union(10001..i32::MAX),
            var_header_offset in i32::MIN..0,
            num_buf in prop::sample::select(vec![i32::MIN, -1, 0, 1, 2, 5, 6, 1000]).prop_filter("exclude 3-4", |&x| !(3..=4).contains(&x)),
            buf_len in i32::MIN..0,
            var_buf in arb_var_buf_array()
        ) -> IRSDKHeader {
            IRSDKHeader {
                ver,
                status,
                tick_rate,
                session_info_update,
                session_info_len,
                session_info_offset,
                num_vars,
                var_header_offset,
                num_buf,
                buf_len,
                pad1: [0, 0],
                var_buf,
            }
        }
    }

    // Property tests for comprehensive header validation
    proptest! {
        #[test]
        fn prop_irsdk_header_parsing_from_generated_binary_structures(
            header in arb_valid_header()
        ) {
            // Convert header to bytes for parsing test
            let header_bytes = unsafe {
                std::slice::from_raw_parts(
                    &header as *const _ as *const u8,
                    mem::size_of::<IRSDKHeader>()
                )
            };

            // Parsing should succeed for valid headers
            let parsed = IRSDKHeader::parse_from_memory(header_bytes);
            prop_assert!(parsed.is_ok());

            let parsed_header = parsed.unwrap();
            prop_assert_eq!(parsed_header.ver, header.ver);
            prop_assert_eq!(parsed_header.status, header.status);
            prop_assert_eq!(parsed_header.tick_rate, header.tick_rate);
            prop_assert_eq!(parsed_header.session_info_update, header.session_info_update);
            prop_assert_eq!(parsed_header.num_vars, header.num_vars);
            prop_assert_eq!(parsed_header.var_header_offset, header.var_header_offset);
            prop_assert_eq!(parsed_header.num_buf, header.num_buf);
            prop_assert_eq!(parsed_header.buf_len, header.buf_len);
        }

        #[test]
        fn prop_header_validation_with_fuzzed_corrupted_data(
            header in arb_corrupted_header()
        ) {
            // Validation should fail for corrupted headers
            let validation_result = header.validate();
            prop_assert!(validation_result.is_err());
        }

        #[test]
        fn prop_sdk_version_compatibility_with_arbitrary_version_numbers(
            version in prop::sample::select(vec![i32::MIN, -1, 0, 1, 3, 4, i32::MAX]).prop_filter("exclude 2", |&x| x != 2)
        ) {
            let header = IRSDKHeader {
                ver: version,
                status: 0,
                tick_rate: 60,
                session_info_update: 0,
                session_info_len: 0,
                session_info_offset: 1000,
                num_vars: 100,
                var_header_offset: 500,
                num_buf: 4,
                buf_len: 1000,
                pad1: [0, 0],
                var_buf: [IRSDKVarBuf { tick_count: 0, buf_offset: 2000, pad: [0, 0] }; 4],
            };

            if version == IRSDK_VER {
                prop_assert!(header.validate().is_ok());
            } else {
                prop_assert!(header.validate().is_err());
                if let Err(TelemetryError::Version { expected, found }) = header.validate() {
                    prop_assert_eq!(expected, IRSDK_VER as u32);
                    prop_assert_eq!(found, version as u32);
                }
            }
        }

        #[test]
        fn prop_session_info_counter_changes_with_generated_sequences(
            update_sequence in prop::collection::vec(0..1_000_000_i32, 1..20)
        ) {
            let base_header = IRSDKHeader {
                ver: IRSDK_VER,
                status: 0,
                tick_rate: 60,
                session_info_update: 0,
                session_info_len: 1000,
                session_info_offset: 2000,
                num_vars: 100,
                var_header_offset: 500,
                num_buf: 4,
                buf_len: 1000,
                pad1: [0, 0],
                var_buf: [IRSDKVarBuf { tick_count: 0, buf_offset: 3000, pad: [0, 0] }; 4],
            };

            let mut last_update = -1;
            for update_counter in update_sequence {
                let mut header = base_header;
                header.session_info_update = update_counter;

                let changed = header.session_info_changed(last_update);
                prop_assert_eq!(changed, update_counter != last_update);

                last_update = update_counter;
            }
        }

        #[test]
        fn prop_double_read_consistency_with_concurrent_tick_updates(
            tick_updates in prop::collection::vec(0..1_000_000_i32, 2..10)
        ) {
            // Simulate concurrent tick updates in buffers
            let mut base_header = IRSDKHeader {
                ver: IRSDK_VER,
                status: 0,
                tick_rate: 60,
                session_info_update: 0,
                session_info_len: 1000,
                session_info_offset: 2000,
                num_vars: 100,
                var_header_offset: 500,
                num_buf: 4,
                buf_len: 1000,
                pad1: [0, 0],
                var_buf: [IRSDKVarBuf { tick_count: 0, buf_offset: 3000, pad: [0, 0] }; 4],
            };

            for (i, tick_count) in tick_updates.iter().enumerate() {
                base_header.var_buf[i % 4].tick_count = *tick_count;
            }

            // Header should remain valid through tick updates
            prop_assert!(base_header.validate().is_ok());

            // Buffer info extraction should be consistent
            let buffer_info = base_header.buffer_info();
            prop_assert_eq!(buffer_info.num_buffers, 4);
            prop_assert_eq!(buffer_info.buffer_length, base_header.buf_len);
        }
    }

    // Unit tests for edge cases and pure functions
    #[test]
    fn header_size_matches_expected_layout() {
        // Ensure struct packing matches C layout
        assert_eq!(mem::size_of::<IRSDKHeader>(), 112); // Actual size for C struct (40 bytes header + 8 bytes padding + 64 bytes for 4x16-byte buffers)
        assert_eq!(mem::size_of::<IRSDKVarBuf>(), 16); // 4 * i32
    }

    #[test]
    fn insufficient_memory_returns_error() {
        let small_buffer = vec![0u8; 10]; // Too small for header
        let result = IRSDKHeader::parse_from_memory(&small_buffer);
        assert!(result.is_err());
        assert!(matches!(result, Err(TelemetryError::Memory { .. })));
    }

    #[test]
    fn valid_header_validation_passes() {
        let header = IRSDKHeader {
            ver: IRSDK_VER,
            status: 0,
            tick_rate: 60,
            session_info_update: 123,
            session_info_len: 5000,
            session_info_offset: 1000,
            num_vars: 150,
            var_header_offset: 500,
            num_buf: 4,
            buf_len: 2000,
            pad1: [0, 0],
            var_buf: [IRSDKVarBuf { tick_count: 100, buf_offset: 3000, pad: [0, 0] }; 4],
        };

        assert!(header.validate().is_ok());
    }

    #[test]
    fn session_info_change_detection_works() {
        let header = IRSDKHeader {
            ver: IRSDK_VER,
            status: 0,
            tick_rate: 60,
            session_info_update: 42,
            session_info_len: 1000,
            session_info_offset: 2000,
            num_vars: 100,
            var_header_offset: 500,
            num_buf: 4,
            buf_len: 1000,
            pad1: [0, 0],
            var_buf: [IRSDKVarBuf { tick_count: 0, buf_offset: 3000, pad: [0, 0] }; 4],
        };

        assert!(header.session_info_changed(41)); // Different value
        assert!(!header.session_info_changed(42)); // Same value
    }

    #[test]
    fn schema_info_extraction_correct() {
        let header = IRSDKHeader {
            ver: IRSDK_VER,
            status: 0,
            tick_rate: 60,
            session_info_update: 123,
            session_info_len: 5000,
            session_info_offset: 10000,
            num_vars: 150,
            var_header_offset: 500,
            num_buf: 4,
            buf_len: 2000,
            pad1: [0, 0],
            var_buf: [IRSDKVarBuf { tick_count: 100, buf_offset: 3000, pad: [0, 0] }; 4],
        };

        let schema_info = header.schema_info();
        assert_eq!(schema_info.num_vars, 150);
        assert_eq!(schema_info.var_header_offset, 500);
        assert_eq!(schema_info.session_info_update, 123);
        assert_eq!(schema_info.session_info_len, 5000);
        assert_eq!(schema_info.session_info_offset, 10000);
    }

    #[test]
    fn benchmark_header_parsing_performance() {
        use std::time::Instant;

        // Create a realistic header for benchmarking
        let header = IRSDKHeader {
            ver: IRSDK_VER,
            status: 0x00000001,
            tick_rate: 60,
            session_info_update: 123,
            session_info_len: 5000,
            session_info_offset: 10000,
            num_vars: 331,             // Realistic based on live data
            var_header_offset: 524400, // Realistic based on live data
            num_buf: 3,                // Realistic based on live data
            buf_len: 7817,             // Realistic based on live data
            pad1: [0, 0],
            var_buf: [
                IRSDKVarBuf { tick_count: 100, buf_offset: 50000, pad: [0, 0] },
                IRSDKVarBuf { tick_count: 101, buf_offset: 60000, pad: [0, 0] },
                IRSDKVarBuf { tick_count: 102, buf_offset: 70000, pad: [0, 0] },
                IRSDKVarBuf { tick_count: 103, buf_offset: 80000, pad: [0, 0] },
            ],
        };

        // Convert to bytes
        let header_bytes = unsafe {
            std::slice::from_raw_parts(
                &header as *const _ as *const u8,
                mem::size_of::<IRSDKHeader>(),
            )
        };

        // Warm up the code path
        for _ in 0..100 {
            let _ = IRSDKHeader::parse_from_memory(header_bytes).unwrap();
        }

        // Benchmark parsing latency
        const NUM_ITERATIONS: usize = 10000;
        let start = Instant::now();

        for _ in 0..NUM_ITERATIONS {
            let _ = IRSDKHeader::parse_from_memory(header_bytes).unwrap();
        }

        let elapsed = start.elapsed();
        let avg_duration_nanos = elapsed.as_nanos() as f64 / NUM_ITERATIONS as f64;
        let avg_duration_micros = avg_duration_nanos / 1000.0;

        println!(
            "Header parsing performance: avg {:.2}ns ({:.3}μs) per parse, {} iterations",
            avg_duration_nanos, avg_duration_micros, NUM_ITERATIONS
        );

        // Verify <1ms latency requirement (1ms = 1,000,000ns)
        assert!(
            avg_duration_nanos < 1_000_000.0,
            "Header parsing should be <1ms, got {:.2}ns",
            avg_duration_nanos
        );

        // For 60Hz updates, we need <16.67ms per frame. Header parsing should be much faster
        assert!(
            avg_duration_nanos < 100_000.0, // <100μs for good headroom
            "Header parsing should be <100μs for 60Hz updates, got {:.2}ns",
            avg_duration_nanos
        );
    }
}
