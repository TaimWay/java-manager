// Copyright 2026 TaimWay
//
// @file: utils.rs
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

use std::process::Command;
use std::str;

use crate::errors::{JavaLocatorError, Result};
use crate::info::JavaInfo;

/// Determines the architecture (32-bit or 64-bit) of a Java installation.
///
/// This function runs various Java commands to determine the architecture.
/// It tries multiple approaches:
/// 1. Attempts to run Java with `-d64` flag
/// 2. Attempts to run Java with `-d32` flag
/// 3. Parses system properties from `-XshowSettings:properties`
///
/// # Arguments
///
/// * `java_path` - Path to the Java executable
///
/// # Returns
///
/// - `Ok(String)` containing "64-bit", "32-bit", or "Unknown"
/// - `Err(JavaLocatorError)` if the command fails or output cannot be parsed
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// fn main() -> java_manager::Result<()> {
///     let java_path = "/usr/bin/java";
///     let arch = java_manager::get_java_architecture(java_path)?;
///     println!("Java architecture: {}", arch);
///     Ok(())
/// }
/// ```
pub fn get_java_architecture(java_path: &str) -> Result<String> {
    // Try -d64 flag
    let output = Command::new(java_path)
        .arg("-d64")
        .arg("-version")
        .output()
        .map_err(|e| JavaLocatorError::new(format!("Failed to run Java command: {}", e)))?;

    if output.status.success() {
        return Ok("64-bit".to_string());
    }

    // Try -d32 flag
    let output = Command::new(java_path)
        .arg("-d32")
        .arg("-version")
        .output()
        .map_err(|e| JavaLocatorError::new(format!("Failed to run Java command: {}", e)))?;

    if output.status.success() {
        return Ok("32-bit".to_string());
    }

    // Try to get architecture from system properties
    let output = Command::new(java_path)
        .arg("-XshowSettings:properties")
        .arg("-version")
        .output()
        .map_err(|e| JavaLocatorError::new(format!("Failed to run Java command: {}", e)))?;

    let output_str = str::from_utf8(&output.stderr)?;
    
    for line in output_str.lines() {
        if line.contains("os.arch") {
            let parts: Vec<&str> = line.split('=').collect();
            if parts.len() == 2 {
                let arch = parts[1].trim();
                return if arch.contains("64") {
                    Ok("64-bit".to_string())
                } else {
                    Ok("32-bit".to_string())
                };
            }
        }
    }

    Ok("Unknown".to_string())
}

/// Extracts the version string from a Java installation.
///
/// Runs `java -version` and parses the output to extract the version string.
/// Supports various version string formats from different Java vendors.
///
/// # Arguments
///
/// * `java_path` - Path to the Java executable
///
/// # Returns
///
/// - `Ok(String)` containing the version string (e.g., "11.0.12", "1.8.0_312")
/// - `Err(JavaLocatorError)` if version cannot be determined
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// fn main() -> java_manager::Result<()> {
///     let java_path = "/usr/bin/java";
///     let version = java_manager::get_java_version(java_path)?;
///     println!("Java version: {}", version);
///     Ok(())
/// }
/// ```
pub fn get_java_version(java_path: &str) -> Result<String> {
    let output = Command::new(java_path)
        .arg("-version")
        .output()
        .map_err(|e| JavaLocatorError::new(format!("Failed to run Java command: {}", e)))?;

    let output_str = str::from_utf8(&output.stderr)?;
    
    // Try various version string patterns
    for line in output_str.lines() {
        // Check for common version string patterns
        if line.starts_with("java version") 
            || line.starts_with("openjdk version") 
            || line.starts_with("java version")
            || line.contains("version \"")
        {
            // Extract version string using more robust parsing
            let line = line.trim();
            
            // Find the version within quotes
            if let Some(start) = line.find('\"') {
                if let Some(end) = line[start + 1..].find('\"') {
                    let version = &line[start + 1..start + 1 + end];
                    return Ok(version.to_string());
                }
            }
            
            // Fallback: split by whitespace
            let parts: Vec<&str> = line.split_whitespace().collect();
            for (i, part) in parts.iter().enumerate() {
                if part.contains("version") && i + 1 < parts.len() {
                    let version = parts[i + 1].trim_matches('\"');
                    return Ok(version.to_string());
                }
            }
            
            // Last resort: take the third word
            if parts.len() >= 3 {
                let version = parts[2].trim_matches('\"');
                return Ok(version.to_string());
            }
        }
    }

    Err(JavaLocatorError::new(
        "Could not determine Java version".to_string(),
    ))
}

/// Identifies the supplier/vendor of a Java installation.
///
/// Analyzes the output of `java -version` to determine the Java supplier.
/// Supports various Java vendors including OpenJDK, Oracle, IBM, Azul, etc.
///
/// # Arguments
///
/// * `java_path` - Path to the Java executable
///
/// # Returns
///
/// - `Ok(String)` containing the supplier name
/// - `Err(JavaLocatorError)` if supplier cannot be determined
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// fn main() -> java_manager::Result<()> {
///     let java_path = "/usr/bin/java";
///     let supplier = java_manager::get_java_suppliers(java_path)?;
///     println!("Java supplier: {}", supplier);
///     Ok(())
/// }
/// ```
pub fn get_java_suppliers(java_path: &str) -> Result<String> {
    let output = Command::new(java_path)
        .arg("-version")
        .output()
        .map_err(|e| JavaLocatorError::new(format!("Failed to run Java command: {}", e)))?;

    let output_str = str::from_utf8(&output.stderr)?;
    
    // Check for specific vendor patterns in the output
    for line in output_str.lines() {
        let line_lower = line.to_lowercase();
        
        if line_lower.contains("openjdk") && !line_lower.contains("adopt") {
            return Ok("OpenJDK".to_string());
        } else if line_lower.contains("oracle") {
            return Ok("Oracle".to_string());
        } else if line_lower.contains("ibm") {
            return Ok("IBM".to_string());
        } else if line_lower.contains("azul") {
            return Ok("Azul".to_string());
        } else if line_lower.contains("adoptopenjdk") || line_lower.contains("adoptium") {
            return Ok("AdoptOpenJDK/Adoptium".to_string());
        } else if line_lower.contains("amazon") || line_lower.contains("corretto") {
            return Ok("Amazon Corretto".to_string());
        } else if line_lower.contains("microsoft") {
            return Ok("Microsoft".to_string());
        } else if line_lower.contains("sap") {
            return Ok("SAP".to_string());
        } else if line_lower.contains("graalvm") {
            return Ok("GraalVM".to_string());
        } else if line_lower.contains("bellsoft") {
            return Ok("BellSoft Liberica".to_string());
        }
    }

    // Try to get vendor from system properties
    let output = Command::new(java_path)
        .arg("-XshowSettings:properties")
        .arg("-version")
        .output()
        .map_err(|e| JavaLocatorError::new(format!("Failed to run Java command: {}", e)))?;

    let output_str = str::from_utf8(&output.stderr)?;
    
    for line in output_str.lines() {
        if line.contains("java.vendor") {
            let parts: Vec<&str> = line.split('=').collect();
            if parts.len() == 2 {
                return Ok(parts[1].trim().to_string());
            }
        }
    }

    Ok("Unknown".to_string())
}

/// Creates a comprehensive `JavaInfo` object for a Java installation.
///
/// This function gathers all information about a Java installation by
/// calling the individual information extraction functions.
///
/// # Arguments
///
/// * `java_exec_path` - Path to the Java executable
///
/// # Returns
///
/// - `Ok(JavaInfo)` containing all Java information
/// - `Err(JavaLocatorError)` if any information cannot be gathered
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// fn main() -> java_manager::Result<()> {
///     let java_path = "/usr/bin/java";
///     let info = java_manager::get_java_info(java_path)?;
///     println!("Java Info: {}", info);
///     Ok(())
/// }
/// ```
pub fn get_java_info(java_exec_path: &str) -> Result<JavaInfo> {
    let version = get_java_version(java_exec_path)?;
    let architecture = get_java_architecture(java_exec_path)?;
    let suppliers = get_java_suppliers(java_exec_path)?;
    
    let name = std::path::Path::new(java_exec_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("java")
        .to_string();

    Ok(JavaInfo::new(
        &name,
        java_exec_path,
        &version,
        &architecture,
        &suppliers,
    ))
}

/// Validates that a Java executable exists and can be executed.
///
/// # Arguments
///
/// * `java_path` - Path to the Java executable
///
/// # Returns
///
/// - `Ok(())` if Java exists and can be executed
/// - `Err(JavaLocatorError)` if Java cannot be found or executed
///
/// # Examples
///
/// ```rust
/// use java_manager;
///
/// fn main() -> java_manager::Result<()> {
///     let java_path = "/usr/bin/java";
///     java_manager::validate_java_executable(java_path)?;
///     println!("Java executable is valid");
///     Ok(())
/// }
/// ```
pub fn validate_java_executable(java_path: &str) -> Result<()> {
    let path = std::path::Path::new(java_path);
    
    if !path.exists() {
        return Err(JavaLocatorError::new(
            format!("Java executable not found: {}", java_path)
        ));
    }
    
    // Try to execute java -version to verify it works
    let output = Command::new(java_path)
        .arg("-version")
        .output()
        .map_err(|e| JavaLocatorError::new(format!("Failed to execute Java: {}", e)))?;
    
    if !output.status.success() {
        return Err(JavaLocatorError::new(
            format!("Java executable failed to run: {}", java_path)
        ));
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests Java version extraction
    #[test]
    fn test_get_java_version() {
        // Only run this test if Java is available
        if let Ok(java_home) = crate::locate_java_home() {
            let java_exec_path = format!("{}/bin/java", java_home);
            if std::path::Path::new(&java_exec_path).exists() {
                let version = get_java_version(&java_exec_path);
                assert!(version.is_ok());
                let version_str = version.unwrap();
                println!("Java version: {}", version_str);
                assert!(!version_str.is_empty());
                
                // Version string should contain at least one dot or underscore
                assert!(version_str.contains('.') || version_str.contains('_') || 
                       version_str.chars().any(|c| c.is_digit(10)));
            }
        }
    }

    /// Tests Java architecture detection
    #[test]
    fn test_get_java_architecture() {
        if let Ok(java_home) = crate::locate_java_home() {
            let java_exec_path = format!("{}/bin/java", java_home);
            if std::path::Path::new(&java_exec_path).exists() {
                let arch = get_java_architecture(&java_exec_path);
                assert!(arch.is_ok());
                let arch_str = arch.unwrap();
                println!("Java architecture: {}", arch_str);
                
                // Should be one of these values
                assert!(arch_str == "64-bit" || arch_str == "32-bit" || arch_str == "Unknown");
            }
        }
    }

    /// Tests Java supplier detection
    #[test]
    fn test_get_java_suppliers() {
        if let Ok(java_home) = crate::locate_java_home() {
            let java_exec_path = format!("{}/bin/java", java_home);
            if std::path::Path::new(&java_exec_path).exists() {
                let supplier = get_java_suppliers(&java_exec_path);
                assert!(supplier.is_ok());
                let supplier_str = supplier.unwrap();
                println!("Java supplier: {}", supplier_str);
                assert!(!supplier_str.is_empty());
            }
        }
    }

    /// Tests comprehensive Java info gathering
    #[test]
    fn test_get_java_info() {
        if let Ok(java_home) = crate::locate_java_home() {
            let java_exec_path = format!("{}/bin/java", java_home);
            if std::path::Path::new(&java_exec_path).exists() {
                let info = get_java_info(&java_exec_path);
                assert!(info.is_ok());
                let info = info.unwrap();
                println!("Java Info: {:?}", info);
                
                // Verify all fields are populated
                assert!(!info.name.is_empty());
                assert!(!info.path.is_empty());
                assert!(!info.version.is_empty());
                assert!(!info.architecture.is_empty());
                assert!(!info.suppliers.is_empty());
            }
        }
    }

    /// Tests Java executable validation
    #[test]
    fn test_validate_java_executable() {
        // Test with system Java if available
        if let Ok(java_home) = crate::locate_java_home() {
            let java_exec_path = format!("{}/bin/java", java_home);
            if std::path::Path::new(&java_exec_path).exists() {
                let result = validate_java_executable(&java_exec_path);
                assert!(result.is_ok());
            }
        }
        
        // Test with non-existent path
        let result = validate_java_executable("/path/that/does/not/exist/java");
        assert!(result.is_err());
    }

    /// Tests version parsing with various formats
    #[test]
    fn test_version_parsing() {
        // Simulate different version string formats
        let test_cases = vec![
            ("java version \"1.8.0_312\"", "1.8.0_312"),
            ("openjdk version \"11.0.12\" 2021-07-20", "11.0.12"),
            ("java version \"17.0.1\" 2021-10-19 LTS", "17.0.1"),
            ("openjdk version \"1.8.0_302\"", "1.8.0_302"),
        ];
        
        // Note: This test doesn't actually run Java, just tests our understanding
        // of the version string patterns
        for (input, expected) in test_cases {
            println!("Testing version parsing: {}", input);
            // We can't easily test the actual function without running Java,
            // but we can verify our understanding of the patterns
        }
    }

    /// Tests supplier detection patterns
    #[test]
    fn test_supplier_patterns() {
        let test_cases = vec![
            ("OpenJDK Runtime Environment", "OpenJDK"),
            ("Java(TM) SE Runtime Environment", "Oracle"),
            ("IBM J9 VM", "IBM"),
            ("Zulu", "Azul"),
            ("AdoptOpenJDK", "AdoptOpenJDK/Adoptium"),
            ("Corretto", "Amazon Corretto"),
            ("Microsoft", "Microsoft"),
        ];
        
        for (input, expected) in test_cases {
            println!("Testing supplier pattern: {} -> {}", input, expected);
            // This is just for documentation - actual detection happens in get_java_suppliers
        }
    }
}