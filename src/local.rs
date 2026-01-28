// Copyright 2026 TaimWay
//
// @file: local.rs
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

use crate::errors::{JavaLocatorError, Result};
use crate::info::JavaInfo;

/// Gets detailed information about the current Java installation.
///
/// This function locates the Java home directory and creates a comprehensive
/// `JavaInfo` object with all available information about the installation.
///
/// # Returns
///
/// - `Ok(JavaInfo)` containing detailed Java information
/// - `Err(JavaLocatorError)` if Java cannot be located or information cannot be gathered
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// fn main() -> java_manager::Result<()> {
///     let java_info = java_manager::get_local_java_home()?;
///     println!("Current Java: {}", java_info);
///     println!("Version: {}", java_info.version);
///     println!("Architecture: {}", java_info.architecture);
///     println!("Supplier: {}", java_info.suppliers);
///     Ok(())
/// }
/// ```
pub fn get_java_home() -> Result<JavaInfo> {
    let java_home = crate::locate_java_home()?;
    
    let java_exec_path = if cfg!(target_os = "windows") {
        format!("{}\\bin\\java.exe", java_home)
    } else {
        format!("{}/bin/java", java_home)
    };

    if !std::path::Path::new(&java_exec_path).exists() {
        return Err(JavaLocatorError::new(
            format!("Java executable not found at: {}", java_exec_path)
        ));
    }

    crate::utils::get_java_info(&java_exec_path)
}

/// Gets the directory containing the JVM dynamic library.
///
/// # Returns
///
/// - `Ok(String)` containing the directory path
/// - `Err(JavaLocatorError)` if the JVM library cannot be found
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// fn main() -> java_manager::Result<()> {
///     let jvm_lib_dir = java_manager::get_java_dyn_lib()?;
///     println!("JVM library directory: {}", jvm_lib_dir);
///     Ok(())
/// }
/// ```
pub fn get_java_dyn_lib() -> Result<String> {
    crate::locate_jvm_dyn_library()
}

/// Gets the Java documentation directory.
///
/// Searches for Java documentation in common locations within the Java installation.
///
/// # Returns
///
/// - `Ok(String)` containing the documentation directory path
/// - `Err(JavaLocatorError)` if documentation cannot be found
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// fn main() -> java_manager::Result<()> {
///     let doc_dir = java_manager::get_java_document()?;
///     println!("Java documentation directory: {}", doc_dir);
///     Ok(())
/// }
/// ```
pub fn get_java_document() -> Result<String> {
    let java_home = crate::locate_java_home()?;
    
    // Common documentation directory names across different Java distributions
    let possible_doc_paths = vec![
        format!("{}/docs", java_home),
        format!("{}/doc", java_home),
        format!("{}/legal", java_home),
        format!("{}/man", java_home),
        format!("{}/man/man1", java_home),
        format!("{}/../docs", java_home), // Some distributions install docs in parent directory
        format!("{}/../legal", java_home),
    ];

    for path in possible_doc_paths {
        if std::path::Path::new(&path).exists() {
            return Ok(path);
        }
    }

    // If no documentation directory found, return Java home
    Ok(java_home)
}

/// Discovers all Java installations on the system.
///
/// Searches for Java installations in common locations and environment variables.
/// The results are sorted by version (highest first).
///
/// # Returns
///
/// - `Ok(Vec<JavaInfo>)` containing all found Java installations
/// - `Err(JavaLocatorError)` if an error occurs during discovery
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// fn main() -> java_manager::Result<()> {
///     let installations = java_manager::find_all_java_installations()?;
///     println!("Found {} Java installations:", installations.len());
///     for (i, java) in installations.iter().enumerate() {
///         println!("{}. {}", i + 1, java);
///     }
///     Ok(())
/// }
/// ```
pub fn find_all_java_installations() -> Result<Vec<JavaInfo>> {
    let mut java_installations = Vec::new();

    // Check JAVA_HOME environment variable first
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        if !java_home.is_empty() {
            let java_exec = if cfg!(target_os = "windows") {
                format!("{}\\bin\\java.exe", java_home)
            } else {
                format!("{}/bin/java", java_home)
            };
            
            if std::path::Path::new(&java_exec).exists() {
                if let Ok(info) = crate::utils::get_java_info(&java_exec) {
                    java_installations.push(info);
                }
            }
        }
    }

    // Search in platform-specific common installation directories
    let common_paths = get_platform_specific_java_paths();

    for base_path in common_paths {
        if let Ok(entries) = std::fs::read_dir(base_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Try to find Java executable in this directory
                    if let Some(java_info) = try_get_java_info_from_dir(&path) {
                        if !java_installations.iter().any(|i| i.path == java_info.path) {
                            java_installations.push(java_info);
                        }
                    }
                }
            }
        }
    }

    // Also check PATH for Java executables
    find_java_in_path(&mut java_installations);

    // Sort installations by version (highest first)
    java_installations.sort_by(|a: &JavaInfo, b: &JavaInfo| {
        let ver_a = a.get_major_version().unwrap_or(0);
        let ver_b = b.get_major_version().unwrap_or(0);
        ver_b.cmp(&ver_a).then_with(|| a.path.cmp(&b.path))
    });

    Ok(java_installations)
}

/// Returns platform-specific common Java installation paths.
///
/// # Returns
///
/// Vector of directory paths where Java is commonly installed
fn get_platform_specific_java_paths() -> Vec<&'static str> {
    if cfg!(target_os = "windows") {
        vec![
            "C:\\Program Files\\Java",
            "C:\\Program Files (x86)\\Java",
            "C:\\java",
            "C:\\jdk",
            "C:\\jre",
        ]
    } else if cfg!(target_os = "macos") {
        vec![
            "/Library/Java/JavaVirtualMachines",
            "/System/Library/Java/JavaVirtualMachines",
            "/usr/local/opt", // Homebrew installations
            "/opt",
        ]
    } else {
        // Linux/Unix
        vec![
            "/usr/lib/jvm",
            "/usr/java",
            "/opt/java",
            "/usr/local/java",
            "/opt",
            "/usr/lib",
        ]
    }
}

/// Attempts to get JavaInfo from a directory that might contain a Java installation.
///
/// # Arguments
///
/// * `dir_path` - Directory path that might contain a Java installation
///
/// # Returns
///
/// `Some(JavaInfo)` if a valid Java installation is found, `None` otherwise
fn try_get_java_info_from_dir(dir_path: &std::path::Path) -> Option<JavaInfo> {
    // Try different possible executable paths
    let possible_exec_paths = if cfg!(target_os = "windows") {
        vec![
            dir_path.join("bin").join("java.exe"),
            dir_path.join("jre").join("bin").join("java.exe"),
            dir_path.join("bin").join("javaw.exe"),
        ]
    } else {
        vec![
            dir_path.join("bin").join("java"),
            dir_path.join("jre").join("bin").join("java"),
            dir_path.join("Contents").join("Home").join("bin").join("java"), // macOS .app bundles
        ]
    };

    for exec_path in possible_exec_paths {
        if exec_path.exists() {
            if let Ok(info) = crate::utils::get_java_info(exec_path.to_str().unwrap()) {
                return Some(info);
            }
        }
    }

    None
}

/// Searches for Java installations in the system PATH.
///
/// # Arguments
///
/// * `java_installations` - Mutable reference to vector to add found installations
fn find_java_in_path(java_installations: &mut Vec<JavaInfo>) {
    if let Ok(path_var) = std::env::var("PATH") {
        for path_dir in path_var.split(std::path::MAIN_SEPARATOR) {
            let java_exec = if cfg!(target_os = "windows") {
                std::path::Path::new(path_dir).join("java.exe")
            } else {
                std::path::Path::new(path_dir).join("java")
            };

            if java_exec.exists() {
                if let Ok(info) = crate::utils::get_java_info(java_exec.to_str().unwrap()) {
                    if !java_installations.iter().any(|i| i.path == info.path) {
                        java_installations.push(info);
                    }
                }
            }
        }
    }
}

/// Gets information about a specific Java installation by version.
///
/// # Arguments
///
/// * `major_version` - Major version of Java to find (e.g., 8, 11, 17)
///
/// # Returns
///
/// - `Ok(JavaInfo)` if a matching Java installation is found
/// - `Err(JavaLocatorError)` if no matching installation is found
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// fn main() -> java_manager::Result<()> {
///     // Find Java 11 installation
///     let java_11 = java_manager::get_java_by_version(11)?;
///     println!("Java 11: {}", java_11);
///     Ok(())
/// }
/// ```
pub fn get_java_by_version(major_version: u32) -> Result<JavaInfo> {
    let installations = find_all_java_installations()?;
    
    for installation in installations {
        if let Some(version) = installation.get_major_version() {
            if version == major_version {
                return Ok(installation);
            }
        }
    }
    
    Err(JavaLocatorError::new(
        format!("No Java installation found for version {}", major_version)
    ))
}

/// Gets the latest Java installation available on the system.
///
/// # Returns
///
/// - `Ok(JavaInfo)` for the Java installation with the highest version
/// - `Err(JavaLocatorError)` if no Java installations are found
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// fn main() -> java_manager::Result<()> {
///     let latest_java = java_manager::get_latest_java()?;
///     println!("Latest Java: {}", latest_java);
///     Ok(())
/// }
/// ```
pub fn get_latest_java() -> Result<JavaInfo> {
    let installations = find_all_java_installations()?;
    
    if installations.is_empty() {
        return Err(JavaLocatorError::new(
            "No Java installations found".to_string()
        ));
    }
    
    // Installations are already sorted by version (highest first)
    Ok(installations[0].clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests getting detailed Java home information
    #[test]
    fn test_get_java_home_info() {
        let result = get_java_home();
        match result {
            Ok(info) => {
                println!("Found Java: {}", info);
                assert!(!info.version.is_empty());
                assert!(!info.architecture.is_empty());
                assert!(!info.suppliers.is_empty());
                assert!(info.is_valid());
            }
            Err(e) => {
                println!("Error getting Java home info: {}", e);
                // If Java is not installed, this test should pass without panic
            }
        }
    }

    /// Tests getting JVM dynamic library directory
    #[test]
    fn test_get_java_dyn_lib() {
        let result = get_java_dyn_lib();
        match result {
            Ok(path) => {
                println!("JVM Dynamic Library: {}", path);
                assert!(std::path::Path::new(&path).exists());
            }
            Err(e) => {
                println!("Error getting JVM library: {}", e);
                // If Java JVM library not found, this test should pass
            }
        }
    }

    /// Tests getting Java documentation directory
    #[test]
    fn test_get_java_document() {
        let result = get_java_document();
        match result {
            Ok(path) => {
                println!("Java documentation: {}", path);
                // Documentation directory might not exist in all installations
                if std::path::Path::new(&path).exists() {
                    println!("Documentation directory exists");
                } else {
                    println!("Documentation directory does not exist, using Java home");
                }
            }
            Err(e) => {
                println!("Error getting Java document: {}", e);
            }
        }
    }

    /// Tests finding all Java installations
    #[test]
    fn test_find_all_java() {
        let result = find_all_java_installations();
        assert!(result.is_ok());
        let installations = result.unwrap();
        println!("Found {} Java installations", installations.len());
        
        for (i, java) in installations.iter().enumerate() {
            println!("{}. {}", i + 1, java);
            assert!(java.is_valid());
        }
        
        // If Java is installed, there should be at least one installation
        if installations.is_empty() {
            println!("No Java installations found");
        }
    }

    /// Tests getting Java by specific version
    #[test]
    fn test_get_java_by_version() {
        // First find all installations to see what versions are available
        if let Ok(installations) = find_all_java_installations() {
            if !installations.is_empty() {
                // Try to get the highest version available
                let highest_version = installations[0].get_major_version().unwrap_or(0);
                if highest_version > 0 {
                    let result = get_java_by_version(highest_version);
                    assert!(result.is_ok());
                    let java = result.unwrap();
                    assert_eq!(java.get_major_version().unwrap_or(0), highest_version);
                    println!("Found Java {}: {}", highest_version, java);
                }
                
                // Test with a version that likely doesn't exist
                let non_existent_version = 99;
                let result = get_java_by_version(non_existent_version);
                assert!(result.is_err());
            }
        }
    }

    /// Tests getting the latest Java installation
    #[test]
    fn test_get_latest_java() {
        let result = get_latest_java();
        match result {
            Ok(java) => {
                println!("Latest Java: {}", java);
                assert!(java.is_valid());
                
                // Verify it's actually the latest by comparing with all installations
                let all_java = find_all_java_installations().unwrap();
                if all_java.len() > 1 {
                    let latest_version = java.get_major_version().unwrap_or(0);
                    for other_java in &all_java[1..] {
                        let other_version = other_java.get_major_version().unwrap_or(0);
                        assert!(latest_version >= other_version);
                    }
                }
            }
            Err(e) => {
                println!("Error getting latest Java: {}", e);
                // If no Java is installed, this is expected
            }
        }
    }

    /// Tests platform-specific path detection
    #[test]
    fn test_platform_specific_paths() {
        let paths = get_platform_specific_java_paths();
        assert!(!paths.is_empty());
        
        println!("Platform-specific Java paths:");
        for path in &paths {
            println!("  - {}", path);
        }
        
        // Verify platform-specific paths
        if cfg!(target_os = "windows") {
            assert!(paths.contains(&"C:\\Program Files\\Java"));
        } else if cfg!(target_os = "macos") {
            assert!(paths.contains(&"/Library/Java/JavaVirtualMachines"));
        } else {
            assert!(paths.contains(&"/usr/lib/jvm"));
        }
    }

    /// Tests that Java installations are sorted correctly
    #[test]
    fn test_installation_sorting() {
        if let Ok(mut installations) = find_all_java_installations() {
            if installations.len() > 1 {
                // Verify sorting (highest version first)
                for i in 0..installations.len() - 1 {
                    let current_version = installations[i].get_major_version().unwrap_or(0);
                    let next_version = installations[i + 1].get_major_version().unwrap_or(0);
                    assert!(current_version >= next_version);
                }
            }
        }
    }

    /// Tests that duplicate installations are filtered
    #[test]
    fn test_no_duplicate_installations() {
        if let Ok(installations) = find_all_java_installations() {
            let mut seen_paths = std::collections::HashSet::new();
            
            for installation in &installations {
                let path = &installation.path;
                assert!(!seen_paths.contains(path), "Duplicate installation found: {}", path);
                seen_paths.insert(path);
            }
        }
    }
}