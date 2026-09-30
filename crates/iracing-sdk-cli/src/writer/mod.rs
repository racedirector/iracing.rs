mod document;
mod output_sink;
mod record_stream;

pub(crate) use document::{DocumentFormat, DocumentWriter, OutputTarget};
pub(crate) use record_stream::{RecordStreamFormat, RecordStreamWriter, TelemetrySnapshot};
