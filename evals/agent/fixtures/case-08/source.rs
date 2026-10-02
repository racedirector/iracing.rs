// workflow.yml: pull_request paths: ["src/old-reader/**", "benches/reader.rs"]
// Current reader is src/reader.rs and calls source.read_into(&mut reused).
// Benchmark benches/reader.rs only times parse_header(&captured_bytes).
// Report: "The new reader is 30% faster"; no run IDs or comparable timings attached.
