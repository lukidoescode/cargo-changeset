pub(crate) mod error_format;
mod formatter;
mod plain;
mod status;
pub(crate) mod writer;

pub(crate) use formatter::OutputFormatter;
pub(crate) use plain::PlainTextFormatter;
pub(crate) use status::{PlainTextStatusFormatter, StatusFormatter};
pub(crate) use writer::{CliWriter, MessageLevel, StdoutCliWriter};

#[cfg(test)]
pub(crate) use writer::test_support::{BufferCliWriter, OutputEntry};
