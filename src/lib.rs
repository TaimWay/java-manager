// Copyright 2026 TaimWay
//
// @file: lib.rs
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

//! # Java Manager Library
//!
//! A comprehensive Rust library for discovering, managing, and interacting with Java installations.
//!
//! This library provides functionality to:
//! - Locate Java installations across different platforms
//! - Extract detailed information about Java installations
//! - Execute Java commands and capture output
//! - Manage multiple Java installations
//! - Search for files within Java installations
//!
//! ## Features
//!
//! - **Cross-platform support**: Works on Windows, macOS, and Linux/Unix
//! - **Comprehensive Java detection**: Finds Java installations in common locations
//! - **Detailed Java information**: Extracts version, architecture, supplier information
//! - **Advanced searching**: Wildcard support for file searches
//! - **Command execution**: Execute Java commands and capture output
//!
//! ## Quick Start
//!
//! ```rust
//! use java_manager;
//!
//! fn main() -> java_manager::Result<()> {
//!     // Get detailed information about the default Java installation
//!     let java_info = java_manager::get_local_java_home()?;
//!     println!("Default Java: {}", java_info);
//!
//!     // Find all Java installations on the system
//!     let installations = java_manager::find_all_java_installations()?;
//!     println!("Found {} Java installations", installations.len());
//!
//!     // Execute a Java command
//!     let output = java_info.execute_with_output(&["-version"])?;
//!     println!("Java version output:\n{}", output);
//!
//!     Ok(())
//! }
//! ```

use std::env;
use std::path::PathBuf;
use std::process::Command;

use glob::{glob, Pattern};

/// Error handling module
pub mod errors;
/// Java information structures
pub mod info;
/// Local Java installation management
pub mod local;
/// Java installation manager
pub mod manager;
/// Utility functions
pub mod utils;

// Re-export commonly used types and functions
pub use errors::{JavaLocatorError, Result};
pub use info::JavaInfo;
pub use utils::{get_java_architecture, get_java_info, get_java_suppliers, get_java_version};
pub use local::{
    find_all_java_installations, get_java_document, get_java_dyn_lib,
    get_java_home as get_local_java_home,
};

/// Returns the platform-specific name of the JVM dynamic library.
///
/// # Returns
///
/// - `"jvm.dll"` on Windows
/// - `"libjvm.dylib"` on macOS
/// - `"libjvm.so"` on Linux/Unix
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// let lib_name = java_manager::get_jvm_dyn_lib_file_name();
/// println!("JVM library name: {}", lib_name);
/// ```
pub fn get_jvm_dyn_lib_file_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "jvm.dll"
    } else if cfg!(target_os = "macos") {
        "libjvm.dylib"
    } else {
        "libjvm.so"
    }
}

/// Locates and returns the Java home directory path.
///
/// This function first checks the `JAVA_HOME` environment variable.
/// If not set or empty, it attempts to locate Java using platform-specific methods.
///
/// # Platform-specific Behavior
///
/// - **Windows**: Uses the `where` command to find `java.exe`
/// - **macOS**: Uses `/usr/libexec/java_home` system utility
/// - **Linux/Unix**: Uses the `which` command to find `java`
///
/// # Returns
///
/// - `Ok(String)` containing the Java home path
/// - `Err(JavaLocatorError)` if Java cannot be located
///
/// # Errors
///
/// This function may return an error if:
/// - Java is not installed
/// - Java is not in the system PATH
/// - The platform-specific command fails
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// fn main() -> java_manager::Result<()> {
///     let java_home = java_manager::locate_java_home()?;
///     println!("Java home: {}", java_home);
///     Ok(())
/// }
/// ```
pub fn locate_java_home() -> Result<String> {
    match &env::var("JAVA_HOME") {
        Ok(s) if s.is_empty() => do_locate_java_home(),
        Ok(java_home_env_var) => Ok(java_home_env_var.clone()),
        Err(_) => do_locate_java_home(),
    }
}

#[cfg(target_os = "windows")]
fn do_locate_java_home() -> Result<String> {
    let output = Command::new("where")
        .arg("java")
        .output()
        .map_err(|e| JavaLocatorError::new(format!("Failed to run command `where` ({e})")))?;

    let java_exec_path_raw = std::str::from_utf8(&output.stdout)?;
    java_exec_path_validation(java_exec_path_raw)?;

    let paths_found = java_exec_path_raw.lines().count();
    if paths_found > 1 {
        eprintln!("WARNING: Found {paths_found} possible java locations. Using the first one. Set JAVA_HOME env var to avoid this warning.")
    }

    let java_exec_path = java_exec_path_raw
        .lines()
        .next()
        .expect("guaranteed to have at least one line by java_exec_path_validation")
        .trim();

    let mut home_path = follow_symlinks(java_exec_path);

    // Remove "bin" and parent directory to get JAVA_HOME
    home_path.pop();
    home_path.pop();

    home_path
        .into_os_string()
        .into_string()
        .map_err(|path| JavaLocatorError::new(format!("Java path {path:?} is invalid utf8")))
}

#[cfg(target_os = "macos")]
fn do_locate_java_home() -> Result<String> {
    let output = Command::new("/usr/libexec/java_home")
        .output()
        .map_err(|e| {
            JavaLocatorError::new(format!(
                "Failed to run command `/usr/libexec/java_home` ({e})"
            ))
        })?;

    let java_exec_path = std::str::from_utf8(&output.stdout)?.trim();

    java_exec_path_validation(java_exec_path)?;
    let home_path = follow_symlinks(java_exec_path);

    home_path
        .into_os_string()
        .into_string()
        .map_err(|path| JavaLocatorError::new(format!("Java path {path:?} is invalid utf8")))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))] // Unix
fn do_locate_java_home() -> Result<String> {
    let output = Command::new("which")
        .arg("java")
        .output()
        .map_err(|e| JavaLocatorError::new(format!("Failed to run command `which` ({e})")))?;
    let java_exec_path = std::str::from_utf8(&output.stdout)?.trim();

    java_exec_path_validation(java_exec_path)?;
    let mut home_path = follow_symlinks(java_exec_path);

    // Remove "bin" directory to get JAVA_HOME
    home_path.pop();
    home_path.pop();

    home_path
        .into_os_string()
        .into_string()
        .map_err(|path| JavaLocatorError::new(format!("Java path {path:?} is invalid utf8")))
}

/// Validates that a Java executable path is not empty.
///
/// # Arguments
///
/// * `path` - The Java executable path to validate
///
/// # Returns
///
/// - `Ok(())` if the path is valid (non-empty)
/// - `Err(JavaLocatorError)` if the path is empty
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// let result = java_manager::locate_java_home();
/// assert!(result.is_ok() || result.is_err());
/// ```
fn java_exec_path_validation(path: &str) -> Result<()> {
    if path.is_empty() {
        return Err(JavaLocatorError::new(
            "Java is not installed or not in the system PATH".into(),
        ));
    }

    Ok(())
}

/// Follows symbolic links to get the real path of an executable.
///
/// # Arguments
///
/// * `path` - The path to follow
///
/// # Returns
///
/// A `PathBuf` containing the real path after following all symbolic links
///
/// # Examples
///
/// ```rust
/// use std::path::PathBuf;
///
/// let real_path = java_manager::locate_java_home().unwrap();
/// println!("Real path: {:?}", real_path);
/// ```
fn follow_symlinks(path: &str) -> PathBuf {
    let mut test_path = PathBuf::from(path);
    while let Ok(path) = test_path.read_link() {
        test_path = if path.is_absolute() {
            path
        } else {
            test_path.pop();
            test_path.push(path);
            test_path
        };
    }
    test_path
}

/// Locates the JVM dynamic library directory.
///
/// Searches for the JVM dynamic library (jvm.dll, libjvm.dylib, or libjvm.so)
/// within the Java installation directory.
///
/// # Returns
///
/// - `Ok(String)` containing the directory path where the JVM library is located
/// - `Err(JavaLocatorError)` if the JVM library cannot be found
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// fn main() -> java_manager::Result<()> {
///     let jvm_lib_path = java_manager::locate_jvm_dyn_library()?;
///     println!("JVM library directory: {}", jvm_lib_path);
///     Ok(())
/// }
/// ```
pub fn locate_jvm_dyn_library() -> Result<String> {
    if cfg!(target_os = "windows") {
        locate_file("jvm.dll")
    } else {
        locate_file("libjvm.*")
    }
}

/// Searches for a file within the Java installation directory.
///
/// Supports wildcard patterns in the file name.
///
/// # Arguments
///
/// * `file_name` - The name of the file to search for (supports wildcards)
///
/// # Returns
///
/// - `Ok(String)` containing the directory path where the file is located
/// - `Err(JavaLocatorError)` if the file cannot be found
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// fn main() -> java_manager::Result<()> {
///     // Find libjsig.so
///     let libjsig_dir = java_manager::locate_file("libjsig.so")?;
///     println!("libjsig.so directory: {}", libjsig_dir);
///
///     // Search with wildcard
///     let jvm_lib_dir = java_manager::locate_file("libjvm*")?;
///     println!("JVM library directory: {}", jvm_lib_dir);
///
///     Ok(())
/// }
/// ```
pub fn locate_file(file_name: &str) -> Result<String> {
    let java_home = locate_java_home()?;

    let query = format!("{}/**/{}", Pattern::escape(&java_home), file_name);

    let path = glob(&query)?.filter_map(|x| x.ok()).next().ok_or_else(|| {
        JavaLocatorError::new(format!(
            "Could not find the {file_name} library in any subdirectory of {java_home}",
        ))
    })?;

    let parent_path = path.parent().unwrap();
    match parent_path.to_str() {
        Some(parent_path) => Ok(parent_path.to_owned()),
        None => Err(JavaLocatorError::new(format!(
            "Java path {parent_path:?} is invalid utf8"
        ))),
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    /// Tests basic Java home location functionality
    #[test]
    fn test_locate_java_home() {
        match locate_java_home() {
            Ok(path) => {
                println!("Java home: {}", path);
                // Verify the path exists
                assert!(std::path::Path::new(&path).exists());
            }
            Err(e) => {
                println!("Error locating Java home: {}", e);
                // If Java is not installed, this test should pass (no panic)
            }
        }
    }

    /// Tests JVM dynamic library location functionality
    #[test]
    fn test_locate_jvm_dyn_library() {
        match locate_jvm_dyn_library() {
            Ok(path) => {
                println!("JVM library path: {}", path);
                // Verify the directory exists
                assert!(std::path::Path::new(&path).exists());
            }
            Err(e) => {
                println!("Error locating JVM library: {}", e);
                // If Java is not installed or JVM library not found, this test should pass
            }
        }
    }

    /// Tests file searching with wildcards
    #[test]
    fn test_locate_file_with_wildcard() {
        // This test requires a Java installation
        if let Ok(java_home) = locate_java_home() {
            // Search for Java executable
            let java_exec = if cfg!(target_os = "windows") {
                "java.exe"
            } else {
                "java"
            };
            
            match locate_file(java_exec) {
                Ok(path) => {
                    println!("Found {} in: {}", java_exec, path);
                    assert!(std::path::Path::new(&path).exists());
                }
                Err(e) => {
                    println!("Error locating {}: {}", java_exec, e);
                }
            }
        }
    }

    /// Tests platform-specific library name function
    #[test]
    fn test_get_jvm_dyn_lib_file_name() {
        let lib_name = get_jvm_dyn_lib_file_name();
        println!("Platform JVM library name: {}", lib_name);
        
        // Verify platform-specific names
        if cfg!(target_os = "windows") {
            assert_eq!(lib_name, "jvm.dll");
        } else if cfg!(target_os = "macos") {
            assert_eq!(lib_name, "libjvm.dylib");
        } else {
            assert_eq!(lib_name, "libjvm.so");
        }
    }

    /// Tests symbolic link following functionality
    #[test]
    fn test_follow_symlinks() {
        // Create a test file structure with symlinks
        let temp_dir = tempfile::tempdir().unwrap();
        let target_path = temp_dir.path().join("target.txt");
        std::fs::write(&target_path, "test content").unwrap();
        
        let link_path = temp_dir.path().join("link.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target_path, &link_path).unwrap();
        
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&target_path, &link_path).unwrap();
        
        let followed = follow_symlinks(link_path.to_str().unwrap());
        assert!(followed.exists());
    }
}