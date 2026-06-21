//! Error types for Java environment detection and execution.

use std::fmt;
use std::io;

/// Errors that can occur when working with Java installations.
#[derive(Debug)]
pub enum JavaError {
    /// The provided Java path is invalid (does not exist or cannot be used).
    InvalidJavaPath(String),

    /// A required Java executable or file was not found.
    NotFound(String),

    /// A stale registry entry points to a path where `java` no longer exists.
    StaleRegistryEntry(String),

    /// An I/O error occurred (e.g., reading a file or spawning a process).
    IoError(io::Error),

    /// An error during command execution (e.g., `java -version` failed).
    ExecuteError(String),

    /// A runtime error, such as unexpected output format.
    RuntimeError(String),

    /// Execution of a Java process failed (non-zero exit code).
    ExecutionFailed(String),

    /// An error during Java download.
    DownloadError(String),

    /// An error during archive extraction.
    ExtractError(String),

    /// A generic error with a custom message.
    Other(String),
}

impl fmt::Display for JavaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            JavaError::InvalidJavaPath(msg) => write!(f, "Invalid Java path: {}", msg),
            JavaError::NotFound(msg) => write!(f, "Not found: {}", msg),
            JavaError::StaleRegistryEntry(msg) => write!(f, "Stale registry entry: {}", msg),
            JavaError::IoError(err) => write!(f, "IO error: {}", err),
            JavaError::ExecuteError(msg) => write!(f, "Execute error: {}", msg),
            JavaError::RuntimeError(msg) => write!(f, "Runtime error: {}", msg),
            JavaError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            JavaError::DownloadError(msg) => write!(f, "Download error: {}", msg),
            JavaError::ExtractError(msg) => write!(f, "Extraction error: {}", msg),
            JavaError::Other(msg) => write!(f, "Other error: {}", msg),
        }
    }
}

impl std::error::Error for JavaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            JavaError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for JavaError {
    fn from(err: io::Error) -> Self {
        JavaError::IoError(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_display_invalid_path() {
        let err = JavaError::InvalidJavaPath("bad/path".into());
        assert_eq!(err.to_string(), "Invalid Java path: bad/path");
    }

    #[test]
    fn test_display_not_found() {
        let err = JavaError::NotFound("missing.exe".into());
        assert_eq!(err.to_string(), "Not found: missing.exe");
    }

    #[test]
    fn test_display_stale_registry() {
        let err = JavaError::StaleRegistryEntry("HKLM\\..".into());
        assert_eq!(err.to_string(), "Stale registry entry: HKLM\\..");
    }

    #[test]
    fn test_display_execution_failed() {
        let err = JavaError::ExecutionFailed("exit code 1".into());
        assert_eq!(err.to_string(), "Execution failed: exit code 1");
    }

    #[test]
    fn test_display_extract_error() {
        let err = JavaError::ExtractError("corrupt archive".into());
        assert_eq!(err.to_string(), "Extraction error: corrupt archive");
    }

    #[test]
    fn test_display_download_error() {
        let err = JavaError::DownloadError("network".into());
        assert_eq!(err.to_string(), "Download error: network");
    }

    #[test]
    fn test_from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file missing");
        let java_err: JavaError = io_err.into();
        assert!(matches!(java_err, JavaError::IoError(_)));
        assert!(java_err.to_string().contains("IO error"));
    }

    #[test]
    fn test_error_source_io() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        let java_err = JavaError::IoError(io_err);
        assert!(java_err.source().is_some());
    }

    #[test]
    fn test_error_source_other_variants() {
        let err = JavaError::Other("msg".into());
        assert!(err.source().is_none());
    }

    #[test]
    fn test_debug_format() {
        let err = JavaError::ExecuteError("boom".into());
        let debug = format!("{err:?}");
        assert!(debug.contains("ExecuteError"));
        assert!(debug.contains("boom"));
    }
}
