// Copyright 2026 TaimWay
//
// @file: errors.rs
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::error::Error;
use std::{fmt, result};

use glob;

/// Result type alias for Java locator operations.
///
/// This is a convenience type alias for `Result<T, JavaLocatorError>`.
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// fn locate_java() -> java_manager::Result<String> {
///     java_manager::locate_java_home()
/// }
/// ```
pub type Result<T> = result::Result<T, JavaLocatorError>;

/// Error type for Java locator operations.
///
/// This error type encapsulates various errors that can occur
/// when locating or working with Java installations.
///
/// # Examples
///
/// ```rust
/// use java_manager::JavaLocatorError;
///
/// let error = JavaLocatorError::new("Java not found".to_string());
/// println!("Error: {}", error);
/// ```
#[derive(Debug)]
pub struct JavaLocatorError {
    /// Human-readable error description
    description: String,
}

impl JavaLocatorError {
    /// Creates a new `JavaLocatorError` with the given description.
    ///
    /// # Arguments
    ///
    /// * `description` - Error description
    ///
    /// # Returns
    ///
    /// A new `JavaLocatorError` instance
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaLocatorError;
    ///
    /// let error = JavaLocatorError::new("Failed to locate Java".to_string());
    /// ```
    pub(crate) fn new(description: String) -> JavaLocatorError {
        JavaLocatorError { description }
    }

    /// Returns the error description.
    ///
    /// # Returns
    ///
    /// Error description string
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaLocatorError;
    ///
    /// let error = JavaLocatorError::new("Test error".to_string());
    /// assert_eq!(error.description(), "Test error");
    /// ```
    pub fn description(&self) -> &str {
        self.description.as_str()
    }

    /// Creates an error indicating Java is not installed or not in PATH.
    ///
    /// # Returns
    ///
    /// A `JavaLocatorError` with appropriate message
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaLocatorError;
    ///
    /// let error = JavaLocatorError::java_not_found();
    /// ```
    pub fn java_not_found() -> Self {
        JavaLocatorError::new(
            "Java is not installed or not in the system PATH".to_string()
        )
    }

    /// Creates an error indicating a file was not found in the Java installation.
    ///
    /// # Arguments
    ///
    /// * `file_name` - Name of the file that was not found
    /// * `java_home` - Java home directory where the file was searched
    ///
    /// # Returns
    ///
    /// A `JavaLocatorError` with appropriate message
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaLocatorError;
    ///
    /// let error = JavaLocatorError::file_not_found("libjsig.so", "/usr/lib/jvm/java-11");
    /// ```
    pub fn file_not_found(file_name: &str, java_home: &str) -> Self {
        JavaLocatorError::new(
            format!(
                "Could not find '{}' in any subdirectory of {}",
                file_name, java_home
            )
        )
    }

    /// Creates an error indicating a command execution failure.
    ///
    /// # Arguments
    ///
    /// * `command` - Command that failed
    /// * `error` - Underlying error
    ///
    /// # Returns
    ///
    /// A `JavaLocatorError` with appropriate message
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaLocatorError;
    ///
    /// let error = JavaLocatorError::command_failed("java -version", "Permission denied");
    /// ```
    pub fn command_failed(command: &str, error: &str) -> Self {
        JavaLocatorError::new(
            format!("Failed to execute command '{}': {}", command, error)
        )
    }

    /// Creates an error indicating an invalid Java installation.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the invalid Java installation
    /// * `reason` - Reason why the installation is invalid
    ///
    /// # Returns
    ///
    /// A `JavaLocatorError` with appropriate message
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaLocatorError;
    ///
    /// let error = JavaLocatorError::invalid_installation("/invalid/path", "Executable not found");
    /// ```
    pub fn invalid_installation(path: &str, reason: &str) -> Self {
        JavaLocatorError::new(
            format!("Invalid Java installation at '{}': {}", path, reason)
        )
    }

    /// Creates an error indicating an invalid UTF-8 sequence in a path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path that contains invalid UTF-8
    ///
    /// # Returns
    ///
    /// A `JavaLocatorError` with appropriate message
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaLocatorError;
    ///
    /// let error = JavaLocatorError::invalid_utf8_path("<invalid-utf8-path>");
    /// ```
    pub fn invalid_utf8_path(path: &str) -> Self {
        JavaLocatorError::new(
            format!("Path contains invalid UTF-8: {}", path)
        )
    }
}

impl fmt::Display for JavaLocatorError {
    /// Formats the error for display.
    ///
    /// # Arguments
    ///
    /// * `f` - Formatter
    ///
    /// # Returns
    ///
    /// Formatter result
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "JavaLocatorError: {}", self.description)
    }
}

impl Error for JavaLocatorError {
    /// Returns the error description (for compatibility with std::error::Error).
    ///
    /// # Returns
    ///
    /// Error description
    fn description(&self) -> &str {
        self.description.as_str()
    }

    /// Returns the cause of the error (if any).
    ///
    /// # Returns
    ///
    /// `None` (this error doesn't wrap other errors)
    fn cause(&self) -> Option<&dyn Error> {
        None
    }

    /// Provides source of the error (for compatibility with Rust 1.30+).
    ///
    /// # Returns
    ///
    /// `None` (this error doesn't wrap other errors)
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl From<std::io::Error> for JavaLocatorError {
    /// Converts a `std::io::Error` to a `JavaLocatorError`.
    ///
    /// # Arguments
    ///
    /// * `err` - IO error to convert
    ///
    /// # Returns
    ///
    /// A `JavaLocatorError` with the IO error description
    fn from(err: std::io::Error) -> JavaLocatorError {
        JavaLocatorError::new(format!("IO error: {}", err))
    }
}

impl From<std::str::Utf8Error> for JavaLocatorError {
    /// Converts a `std::str::Utf8Error` to a `JavaLocatorError`.
    ///
    /// # Arguments
    ///
    /// * `err` - UTF-8 error to convert
    ///
    /// # Returns
    ///
    /// A `JavaLocatorError` with the UTF-8 error description
    fn from(err: std::str::Utf8Error) -> JavaLocatorError {
        JavaLocatorError::new(format!("UTF-8 error: {}", err))
    }
}

impl From<glob::PatternError> for JavaLocatorError {
    /// Converts a `glob::PatternError` to a `JavaLocatorError`.
    ///
    /// # Arguments
    ///
    /// * `err` - Glob pattern error to convert
    ///
    /// # Returns
    ///
    /// A `JavaLocatorError` with the glob error description
    fn from(err: glob::PatternError) -> JavaLocatorError {
        JavaLocatorError::new(format!("Glob pattern error: {}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests creating a new JavaLocatorError
    #[test]
    fn test_new_error() {
        let error = JavaLocatorError::new("Test error".to_string());
        assert_eq!(error.description(), "Test error");
    }

    /// Tests the Display trait implementation
    #[test]
    fn test_display() {
        let error = JavaLocatorError::new("Test error message".to_string());
        let display_output = format!("{}", error);
        assert!(display_output.contains("JavaLocatorError"));
        assert!(display_output.contains("Test error message"));
    }

    /// Tests the Error trait implementation
    #[test]
    fn test_error_trait() {
        let error = JavaLocatorError::new("Test error".to_string());
        
        assert_eq!(error.description(), "Test error");
        assert!(error.source().is_none());
        assert!(error.cause().is_none());
    }

    /// Tests the java_not_found helper method
    #[test]
    fn test_java_not_found() {
        let error = JavaLocatorError::java_not_found();
        assert_eq!(error.description(), "Java is not installed or not in the system PATH");
    }

    /// Tests the file_not_found helper method
    #[test]
    fn test_file_not_found() {
        let error = JavaLocatorError::file_not_found("libjsig.so", "/usr/lib/jvm/java-11");
        let description = error.description();
        assert!(description.contains("libjsig.so"));
        assert!(description.contains("/usr/lib/jvm/java-11"));
    }

    /// Tests the command_failed helper method
    #[test]
    fn test_command_failed() {
        let error = JavaLocatorError::command_failed("java -version", "Permission denied");
        let description = error.description();
        assert!(description.contains("java -version"));
        assert!(description.contains("Permission denied"));
    }

    /// Tests the invalid_installation helper method
    #[test]
    fn test_invalid_installation() {
        let error = JavaLocatorError::invalid_installation("/invalid/path", "Executable not found");
        let description = error.description();
        assert!(description.contains("/invalid/path"));
        assert!(description.contains("Executable not found"));
    }

    /// Tests the invalid_utf8_path helper method
    #[test]
    fn test_invalid_utf8_path() {
        let error = JavaLocatorError::invalid_utf8_path("<invalid-utf8-path>");
        let description = error.description();
        assert!(description.contains("<invalid-utf8-path>"));
    }

    /// Tests conversion from std::io::Error
    #[test]
    fn test_from_io_error() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let java_error: JavaLocatorError = io_error.into();
        
        let description = java_error.description();
        assert!(description.contains("IO error"));
        assert!(description.contains("File not found"));
    }

    /// Tests conversion from std::str::Utf8Error
    #[test]
    fn test_from_utf8_error() {
        // Create an invalid UTF-8 sequence
        let invalid_utf8: &[u8] = &[0xff, 0xff, 0xff];
        let utf8_error = std::str::from_utf8(invalid_utf8).unwrap_err();
        
        let java_error: JavaLocatorError = utf8_error.into();
        let description = java_error.description();
        assert!(description.contains("UTF-8 error"));
    }

    /// Tests conversion from glob::PatternError
    #[test]
    fn test_from_glob_error() {
        // Create an invalid glob pattern
        let pattern_result = glob::Pattern::new("**[invalid");
        assert!(pattern_result.is_err());
        
        if let Err(glob_error) = pattern_result {
            let java_error: JavaLocatorError = glob_error.into();
            let description = java_error.description();
            assert!(description.contains("Glob pattern error"));
        }
    }

    /// Tests the Result type alias
    #[test]
    fn test_result_type() {
        // Test Ok variant
        let ok_result: Result<String> = Ok("Success".to_string());
        assert!(ok_result.is_ok());
        assert_eq!(ok_result.unwrap(), "Success");
        
        // Test Err variant
        let err_result: Result<String> = Err(JavaLocatorError::new("Error".to_string()));
        assert!(err_result.is_err());
        
        if let Err(e) = err_result {
            assert_eq!(e.description(), "Error");
        }
    }

    /// Tests error chaining (source method)
    #[test]
    fn test_error_source() {
        let error = JavaLocatorError::new("Wrapper error".to_string());
        // JavaLocatorError doesn't wrap other errors, so source should be None
        assert!(error.source().is_none());
    }
}