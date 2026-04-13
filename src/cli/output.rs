//! Human-facing CLI output helpers.
//!
//! This module centralizes terminal messaging so the crate can keep a consistent
//! tone and formatting across commands.
//!
//! Design goals:
//!
//! - concise output
//! - stable wording
//! - no accidental leaking of debug internals
//! - easy future switch to structured / JSON output modes

use std::fmt;
use std::io::{self, Write};

use crate::error::AgentMemoryError;

/// Output mode used by the CLI.
///
/// Future versions may add:
///
/// - Json
/// - Quiet
/// - Verbose
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Normal human-readable terminal output.
    Standard,
}

/// Severity level for a rendered message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageLevel {
    /// Informational message.
    Info,

    /// Success message.
    Success,

    /// Warning message.
    Warning,

    /// Error message.
    Error,
}

/// Writes an informational message to stdout.
pub fn print_info(message: impl AsRef<str>) {
    let _ = write_message(
        io::stdout(),
        MessageLevel::Info,
        message.as_ref(),
        OutputMode::Standard,
    );
}

/// Writes a success message to stdout.
pub fn print_success(message: impl AsRef<str>) {
    let _ = write_message(
        io::stdout(),
        MessageLevel::Success,
        message.as_ref(),
        OutputMode::Standard,
    );
}

/// Writes a warning message to stderr.
pub fn print_warning(message: impl AsRef<str>) {
    let _ = write_message(
        io::stderr(),
        MessageLevel::Warning,
        message.as_ref(),
        OutputMode::Standard,
    );
}

/// Writes an error message to stderr.
pub fn print_error(error: &AgentMemoryError) {
    let _ = write_message(
        io::stderr(),
        MessageLevel::Error,
        &format_error(error),
        OutputMode::Standard,
    );
}

/// Writes a plain line to stdout.
///
/// Useful when output should remain script-friendly.
pub fn print_line(message: impl AsRef<str>) {
    let _ = writeln!(io::stdout(), "{}", message.as_ref());
}

/// Writes a key/value pair aligned for terminal readability.
pub fn print_field(label: impl AsRef<str>, value: impl AsRef<str>) {
    let _ = writeln!(io::stdout(), "{:<18} {}", label.as_ref(), value.as_ref());
}

/// Writes a section heading.
pub fn print_heading(title: impl AsRef<str>) {
    let title = title.as_ref();

    let _ = writeln!(io::stdout(), "{title}");
    let _ = writeln!(io::stdout(), "{}", "-".repeat(title.len()));
}

/// Writes a blank line.
pub fn print_blank_line() {
    let _ = writeln!(io::stdout());
}

/// Internal renderer for terminal messages.
fn write_message<W>(
    mut writer: W,
    level: MessageLevel,
    message: &str,
    mode: OutputMode,
) -> io::Result<()>
where
    W: Write,
{
    match mode {
        OutputMode::Standard => {
            writeln!(writer, "{} {}", prefix(level), message)
        }
    }
}

/// Stable textual prefixes.
///
/// Intentionally ASCII-only and portable.
#[must_use]
const fn prefix(level: MessageLevel) -> &'static str {
    match level {
        MessageLevel::Info => "[info]",
        MessageLevel::Success => "[ok]",
        MessageLevel::Warning => "[warn]",
        MessageLevel::Error => "[error]",
    }
}

/// Converts an error into a concise user-facing string.
///
/// This intentionally avoids dumping backtraces or noisy nested formatting
/// unless a future verbose mode is explicitly enabled.
#[must_use]
pub fn format_error(error: &AgentMemoryError) -> String {
    error.to_string()
}

/// Renders a list of values line-by-line.
///
/// Useful for keys, paths, namespaces, etc.
pub fn print_list<I, T>(items: I)
where
    I: IntoIterator<Item = T>,
    T: fmt::Display,
{
    for item in items {
        let _ = writeln!(io::stdout(), "{item}");
    }
}
