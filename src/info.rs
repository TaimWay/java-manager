// Copyright 2026 TaimWay
//
// @file: info.rs
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

use std::fmt;
use std::process::{Child, Command, Stdio};
use std::str;

/// Represents detailed information about a Java installation.
///
/// This struct contains all relevant information about a Java installation,
/// including version, architecture, supplier, and path information.
///
/// # Fields
///
/// - `name`: The name of the Java executable (e.g., "java", "javac")
/// - `path`: Full path to the Java executable
/// - `version`: Java version string (e.g., "11.0.12", "1.8.0_312")
/// - `architecture`: Architecture information (e.g., "64-bit", "32-bit")
/// - `suppliers`: Java supplier/vendor (e.g., "OpenJDK", "Oracle")
///
/// # Examples
///
/// ```rust
/// use java_manager::JavaInfo;
///
/// // Create a JavaInfo instance
/// let java_info = JavaInfo::new(
///     "java",
///     "/usr/bin/java",
///     "11.0.12",
///     "64-bit",
///     "OpenJDK"
/// );
///
/// println!("Java Info: {}", java_info);
/// println!("Major version: {:?}", java_info.get_major_version());
/// ```
#[derive(Debug, Clone)]
pub struct JavaInfo {
    /// Name of the Java executable
    pub name: String,
    /// Full path to the Java executable
    pub path: String,
    /// Java version string
    pub version: String,
    /// Architecture (32-bit or 64-bit)
    pub architecture: String,
    /// Java supplier/vendor
    pub suppliers: String,
}

impl JavaInfo {
    /// Creates a new `JavaInfo` instance.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the Java executable
    /// * `path` - Full path to the Java executable
    /// * `version` - Java version string
    /// * `architecture` - Architecture information
    /// * `suppliers` - Java supplier/vendor
    ///
    /// # Returns
    ///
    /// A new `JavaInfo` instance
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaInfo;
    ///
    /// let info = JavaInfo::new(
    ///     "java",
    ///     "/usr/bin/java",
    ///     "11.0.12",
    ///     "64-bit",
    ///     "OpenJDK"
    /// );
    /// ```
    pub fn new(name: &str, path: &str, version: &str, architecture: &str, suppliers: &str) -> Self {
        JavaInfo {
            name: name.to_string(),
            path: path.to_string(),
            version: version.to_string(),
            architecture: architecture.to_string(),
            suppliers: suppliers.to_string(),
        }
    }

    /// Executes a Java command asynchronously.
    ///
    /// This method spawns a new process and returns immediately without waiting
    /// for the command to complete.
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line arguments to pass to Java
    ///
    /// # Returns
    ///
    /// - `Ok(Child)` - A handle to the child process
    /// - `Err(std::io::Error)` - If the process cannot be spawned
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaInfo;
    ///
    /// let info = JavaInfo::new("java", "/usr/bin/java", "11.0.12", "64-bit", "OpenJDK");
    /// let child = info.execute(&["-version"]);
    /// if let Ok(mut child) = child {
    ///     let _ = child.wait();
    /// }
    /// ```
    pub fn execute(&self, args: &[&str]) -> std::io::Result<Child> {
        Command::new(&self.path)
            .args(args)
            .spawn()
    }

    /// Executes a Java command and waits for completion.
    ///
    /// This method runs the command and waits for it to complete, returning
    /// the exit status and output.
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line arguments to pass to Java
    ///
    /// # Returns
    ///
    /// - `Ok(Output)` - Command output including status, stdout, and stderr
    /// - `Err(std::io::Error)` - If the command fails to execute
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaInfo;
    ///
    /// let info = JavaInfo::new("java", "/usr/bin/java", "11.0.12", "64-bit", "OpenJDK");
    /// let output = info.execute_and_wait(&["-version"]);
    /// if let Ok(output) = output {
    ///     println!("Exit status: {}", output.status);
    /// }
    /// ```
    pub fn execute_and_wait(&self, args: &[&str]) -> std::io::Result<std::process::Output> {
        Command::new(&self.path)
            .args(args)
            .output()
    }

    /// Executes a Java command and returns the output as a string.
    ///
    /// This method captures both stdout and stderr, returning them as a string.
    /// If the command succeeds, stdout is returned. If it fails, stderr is returned.
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line arguments to pass to Java
    ///
    /// # Returns
    ///
    /// - `Ok(String)` - Command output as a string
    /// - `Err(std::io::Error)` - If the command fails to execute
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaInfo;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let info = JavaInfo::new("java", "/usr/bin/java", "11.0.12", "64-bit", "OpenJDK");
    ///     let output = info.execute_with_output(&["-version"])?;
    ///     println!("Java version:\n{}", output);
    ///     Ok(())
    /// }
    /// ```
    pub fn execute_with_output(&self, args: &[&str]) -> std::io::Result<String> {
        let output = Command::new(&self.path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Ok(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    /// Executes a Java command and returns both stdout and stderr as separate strings.
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line arguments to pass to Java
    ///
    /// # Returns
    ///
    /// - `Ok((String, String))` - Tuple containing (stdout, stderr)
    /// - `Err(std::io::Error)` - If the command fails to execute
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaInfo;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let info = JavaInfo::new("java", "/usr/bin/java", "11.0.12", "64-bit", "OpenJDK");
    ///     let (stdout, stderr) = info.execute_with_separate_output(&["-version"])?;
    ///     println!("Stdout:\n{}", stdout);
    ///     println!("Stderr:\n{}", stderr);
    ///     Ok(())
    /// }
    /// ```
    pub fn execute_with_separate_output(&self, args: &[&str]) -> std::io::Result<(String, String)> {
        let output = Command::new(&self.path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        
        Ok((stdout, stderr))
    }

    /// Returns the major version number of Java.
    ///
    /// Parses the version string to extract the major version.
    /// Handles both old (1.8) and new (9+) version formats.
    ///
    /// # Returns
    ///
    /// - `Some(u32)` - Major version number
    /// - `None` - If version cannot be parsed
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaInfo;
    ///
    /// let info1 = JavaInfo::new("java", "/usr/bin/java", "1.8.0_312", "64-bit", "OpenJDK");
    /// assert_eq!(info1.get_major_version(), Some(8));
    ///
    /// let info2 = JavaInfo::new("java", "/usr/bin/java", "11.0.12", "64-bit", "OpenJDK");
    /// assert_eq!(info2.get_major_version(), Some(11));
    /// ```
    pub fn get_major_version(&self) -> Option<u32> {
        let version_parts: Vec<&str> = self.version.split('.').collect();
        
        if version_parts.is_empty() {
            return None;
        }

        if version_parts[0] == "1" && version_parts.len() > 1 {
            version_parts[1].parse::<u32>().ok()
        } else {
            version_parts[0].parse::<u32>().ok()
        }
    }

    /// Checks if the Java version is at least the specified minimum version.
    ///
    /// # Arguments
    ///
    /// * `min_version` - Minimum major version required
    ///
    /// # Returns
    ///
    /// - `true` if Java version >= min_version
    /// - `false` if Java version < min_version or version cannot be parsed
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaInfo;
    ///
    /// let info = JavaInfo::new("java", "/usr/bin/java", "11.0.12", "64-bit", "OpenJDK");
    /// assert!(info.is_at_least_version(8));  // Java 11 >= 8
    /// assert!(info.is_at_least_version(11)); // Java 11 >= 11
    /// assert!(!info.is_at_least_version(17)); // Java 11 < 17
    /// ```
    pub fn is_at_least_version(&self, min_version: u32) -> bool {
        match self.get_major_version() {
            Some(version) => version >= min_version,
            None => false,
        }
    }

    /// Extracts the Java home directory from the executable path.
    ///
    /// Removes the "bin" directory from the path to get the JAVA_HOME.
    ///
    /// # Returns
    ///
    /// Java home directory path
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaInfo;
    ///
    /// let info = JavaInfo::new("java", "/usr/lib/jvm/java-11-openjdk/bin/java", "11.0.12", "64-bit", "OpenJDK");
    /// assert_eq!(info.get_java_home(), "/usr/lib/jvm/java-11-openjdk");
    /// ```
    pub fn get_java_home(&self) -> String {
        let path = std::path::Path::new(&self.path);
        if let Some(parent) = path.parent() {
            if parent.ends_with("bin") {
                if let Some(java_home) = parent.parent() {
                    return java_home.to_string_lossy().to_string();
                }
            }
        }
        self.path.clone()
    }

    /// Returns a display-friendly string with key Java information.
    ///
    /// # Returns
    ///
    /// Formatted string with Java information
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaInfo;
    ///
    /// let info = JavaInfo::new("java", "/usr/bin/java", "11.0.12", "64-bit", "OpenJDK");
    /// println!("Display: {}", info.to_display_string());
    /// ```
    pub fn to_display_string(&self) -> String {
        format!(
            "{} {} ({}, {}) at {}",
            self.suppliers, self.version, self.architecture, self.name, self.path
        )
    }

    /// Validates that the Java executable exists and is executable.
    ///
    /// # Returns
    ///
    /// - `true` if the executable exists and is accessible
    /// - `false` otherwise
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaInfo;
    ///
    /// let info = JavaInfo::new("java", "/usr/bin/java", "11.0.12", "64-bit", "OpenJDK");
    /// if info.is_valid() {
    ///     println!("Java installation is valid");
    /// }
    /// ```
    pub fn is_valid(&self) -> bool {
        let path = std::path::Path::new(&self.path);
        path.exists()
    }
}

impl fmt::Display for JavaInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "JavaInfo {{ name: {}, version: {}, architecture: {}, supplier: {}, path: {} }}",
            self.name, self.version, self.architecture, self.suppliers, self.path
        )
    }
}

impl PartialEq for JavaInfo {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version && self.path == other.path
    }
}

impl Eq for JavaInfo {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests JavaInfo creation and basic properties
    #[test]
    fn test_java_info_creation() {
        let info = JavaInfo::new(
            "java",
            "/usr/bin/java",
            "11.0.12",
            "64-bit",
            "OpenJDK"
        );

        assert_eq!(info.name, "java");
        assert_eq!(info.path, "/usr/bin/java");
        assert_eq!(info.version, "11.0.12");
        assert_eq!(info.architecture, "64-bit");
        assert_eq!(info.suppliers, "OpenJDK");
    }

    /// Tests major version extraction
    #[test]
    fn test_get_major_version() {
        // Test old version format (Java 8)
        let info_8 = JavaInfo::new("java", "/path", "1.8.0_312", "64-bit", "Oracle");
        assert_eq!(info_8.get_major_version(), Some(8));

        // Test new version format (Java 11)
        let info_11 = JavaInfo::new("java", "/path", "11.0.12", "64-bit", "OpenJDK");
        assert_eq!(info_11.get_major_version(), Some(11));

        // Test invalid version
        let info_invalid = JavaInfo::new("java", "/path", "invalid", "64-bit", "Unknown");
        assert_eq!(info_invalid.get_major_version(), None);
    }

    /// Tests version comparison
    #[test]
    fn test_is_at_least_version() {
        let info = JavaInfo::new("java", "/path", "11.0.12", "64-bit", "OpenJDK");
        
        assert!(info.is_at_least_version(8));
        assert!(info.is_at_least_version(11));
        assert!(!info.is_at_least_version(17));
        
        // Test with invalid version
        let info_invalid = JavaInfo::new("java", "/path", "invalid", "64-bit", "Unknown");
        assert!(!info_invalid.is_at_least_version(8));
    }

    /// Tests Java home extraction
    #[test]
    fn test_get_java_home() {
        // Test standard path
        let info1 = JavaInfo::new("java", "/usr/lib/jvm/java-11-openjdk/bin/java", "11.0.12", "64-bit", "OpenJDK");
        assert_eq!(info1.get_java_home(), "/usr/lib/jvm/java-11-openjdk");

        // Test Windows path
        let info2 = JavaInfo::new("java.exe", "C:\\Program Files\\Java\\jdk-11\\bin\\java.exe", "11.0.12", "64-bit", "Oracle");
        assert_eq!(info2.get_java_home(), "C:\\Program Files\\Java\\jdk-11");

        // Test path without bin directory
        let info3 = JavaInfo::new("java", "/usr/bin/java", "11.0.12", "64-bit", "OpenJDK");
        assert_eq!(info3.get_java_home(), "/usr/bin/java");
    }

    /// Tests display formatting
    #[test]
    fn test_display_formatting() {
        let info = JavaInfo::new("java", "/usr/bin/java", "11.0.12", "64-bit", "OpenJDK");
        
        let display_str = info.to_display_string();
        assert!(display_str.contains("OpenJDK"));
        assert!(display_str.contains("11.0.12"));
        assert!(display_str.contains("64-bit"));
        assert!(display_str.contains("/usr/bin/java"));
    }

    /// Tests equality comparison
    #[test]
    fn test_equality() {
        let info1 = JavaInfo::new("java", "/usr/bin/java", "11.0.12", "64-bit", "OpenJDK");
        let info2 = JavaInfo::new("java", "/usr/bin/java", "11.0.12", "64-bit", "OpenJDK");
        let info3 = JavaInfo::new("java", "/usr/bin/java", "17.0.1", "64-bit", "OpenJDK");
        let info4 = JavaInfo::new("java", "/usr/local/bin/java", "11.0.12", "64-bit", "OpenJDK");
        
        assert_eq!(info1, info2);
        assert_ne!(info1, info3);
        assert_ne!(info1, info4);
    }

    /// Tests the Display trait implementation
    #[test]
    fn test_display_trait() {
        let info = JavaInfo::new("java", "/usr/bin/java", "11.0.12", "64-bit", "OpenJDK");
        let display_output = format!("{}", info);
        
        assert!(display_output.contains("JavaInfo"));
        assert!(display_output.contains("java"));
        assert!(display_output.contains("11.0.12"));
        assert!(display_output.contains("64-bit"));
        assert!(display_output.contains("OpenJDK"));
        assert!(display_output.contains("/usr/bin/java"));
    }

    /// Tests validation of Java executable
    #[test]
    fn test_is_valid() {
        // Test with a path that should exist (current executable)
        let current_exe = std::env::current_exe().unwrap();
        let info_valid = JavaInfo::new(
            "test",
            current_exe.to_str().unwrap(),
            "1.0.0",
            "64-bit",
            "Test"
        );
        assert!(info_valid.is_valid());

        // Test with non-existent path
        let info_invalid = JavaInfo::new(
            "nonexistent",
            "/path/that/does/not/exist",
            "1.0.0",
            "64-bit",
            "Test"
        );
        assert!(!info_invalid.is_valid());
    }

    /// Tests command execution (if Java is available)
    #[test]
    fn test_execute_with_output() {
        // Only run this test if Java is available
        if let Ok(java_home) = crate::locate_java_home() {
            let java_exec = format!("{}/bin/java", java_home);
            if std::path::Path::new(&java_exec).exists() {
                let info = JavaInfo::new("java", &java_exec, "unknown", "unknown", "unknown");
                
                // Test version command
                let result = info.execute_with_output(&["-version"]);
                assert!(result.is_ok());
                
                let output = result.unwrap();
                assert!(!output.is_empty());
                println!("Java version output:\n{}", output);
            }
        }
    }
}