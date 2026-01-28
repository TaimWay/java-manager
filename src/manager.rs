// Copyright 2026 TaimWay
//
// @file: manager.rs
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

use std::collections::HashMap;

use crate::errors::Result;
use crate::info::JavaInfo;

/// Manages multiple Java installations and provides convenient access methods.
///
/// The `JavaManager` struct provides functionality to:
/// - Store and organize multiple Java installations
/// - Filter installations by various criteria
/// - Set default Java installations
/// - Execute commands with specific Java versions
///
/// # Examples
///
/// ```rust
/// use java_manager::{JavaManager, JavaInfo};
///
/// fn main() -> java_manager::Result<()> {
///     // Create a manager and discover all Java installations
///     let mut manager = JavaManager::new();
///     manager.discover_installations()?;
///
///     // List all installations
///     println!("Found {} Java installations:", manager.len());
///     for java in manager.list() {
///         println!("  - {}", java);
///     }
///
///     // Get Java by version
///     if let Some(java_11) = manager.get_by_version(11) {
///         println!("Java 11: {}", java_11);
///     }
///
///     Ok(())
/// }
/// ```
pub struct JavaManager {
    /// Vector of Java installations
    java_installations: Vec<JavaInfo>,
    /// Default Java installation index
    default_index: Option<usize>,
    /// Map of version to installation indices for quick lookup
    version_map: HashMap<u32, Vec<usize>>,
}

impl JavaManager {
    /// Creates a new empty `JavaManager`.
    ///
    /// # Returns
    ///
    /// A new `JavaManager` instance
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// let manager = JavaManager::new();
    /// assert_eq!(manager.len(), 0);
    /// ```
    pub fn new() -> Self {
        JavaManager {
            java_installations: Vec::new(),
            default_index: None,
            version_map: HashMap::new(),
        }
    }

    /// Discovers and adds all Java installations on the system.
    ///
    /// # Returns
    ///
    /// - `Ok(())` if discovery succeeds
    /// - `Err(JavaLocatorError)` if an error occurs during discovery
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// fn main() -> java_manager::Result<()> {
    ///     let mut manager = JavaManager::new();
    ///     manager.discover_installations()?;
    ///     println!("Discovered {} Java installations", manager.len());
    ///     Ok(())
    /// }
    /// ```
    pub fn discover_installations(&mut self) -> Result<()> {
        let installations = crate::local::find_all_java_installations()?;
        
        for installation in installations {
            self.add(installation);
        }
        
        // Set the first installation as default if any exist
        if !self.java_installations.is_empty() {
            self.default_index = Some(0);
        }
        
        Ok(())
    }

    /// Adds a Java installation to the manager.
    ///
    /// # Arguments
    ///
    /// * `java_info` - Java installation information to add
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::{JavaManager, JavaInfo};
    ///
    /// let mut manager = JavaManager::new();
    /// let java_info = JavaInfo::new("java", "/usr/bin/java", "11.0.12", "64-bit", "OpenJDK");
    /// manager.add(java_info);
    /// assert_eq!(manager.len(), 1);
    /// ```
    pub fn add(&mut self, java_info: JavaInfo) {
        let index = self.java_installations.len();
        self.java_installations.push(java_info.clone());
        
        // Update version map for quick lookup
        if let Some(version) = java_info.get_major_version() {
            self.version_map
                .entry(version)
                .or_insert_with(Vec::new)
                .push(index);
        }
        
        // Set as default if this is the first installation
        if self.default_index.is_none() {
            self.default_index = Some(index);
        }
    }

    /// Gets a Java installation by index.
    ///
    /// # Arguments
    ///
    /// * `index` - Index of the Java installation to retrieve
    ///
    /// # Returns
    ///
    /// - `Some(&JavaInfo)` if the index is valid
    /// - `None` if the index is out of bounds
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// let manager = JavaManager::new();
    /// // Add some installations first...
    /// // let java = manager.get(0);
    /// ```
    pub fn get(&self, index: usize) -> Option<&JavaInfo> {
        self.java_installations.get(index)
    }

    /// Gets a Java installation by major version.
    ///
    /// If multiple installations have the same version, returns the first one.
    ///
    /// # Arguments
    ///
    /// * `version` - Major version to look for (e.g., 8, 11, 17)
    ///
    /// # Returns
    ///
    /// - `Some(&JavaInfo)` if a matching installation is found
    /// - `None` if no installation with the specified version exists
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// let manager = JavaManager::new();
    /// // Discover installations first...
    /// // if let Some(java_11) = manager.get_by_version(11) {
    /// //     println!("Found Java 11: {}", java_11);
    /// // }
    /// ```
    pub fn get_by_version(&self, version: u32) -> Option<&JavaInfo> {
        self.version_map
            .get(&version)
            .and_then(|indices| indices.first())
            .and_then(|&index| self.get(index))
    }

    /// Gets all Java installations of a specific major version.
    ///
    /// # Arguments
    ///
    /// * `version` - Major version to look for
    ///
    /// # Returns
    ///
    /// Vector of references to Java installations with the specified version
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// let manager = JavaManager::new();
    /// // Discover installations first...
    /// // let java_11_installations = manager.get_all_by_version(11);
    /// // println!("Found {} Java 11 installations", java_11_installations.len());
    /// ```
    pub fn get_all_by_version(&self, version: u32) -> Vec<&JavaInfo> {
        self.version_map
            .get(&version)
            .map(|indices| {
                indices.iter()
                    .filter_map(|&index| self.get(index))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Gets the default Java installation.
    ///
    /// # Returns
    ///
    /// - `Some(&JavaInfo)` if a default installation is set
    /// - `None` if no installations exist or no default is set
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// let manager = JavaManager::new();
    /// // Discover installations first...
    /// // if let Some(default_java) = manager.get_default() {
    /// //     println!("Default Java: {}", default_java);
    /// // }
    /// ```
    pub fn get_default(&self) -> Option<&JavaInfo> {
        self.default_index.and_then(|index| self.get(index))
    }

    /// Sets the default Java installation by index.
    ///
    /// # Arguments
    ///
    /// * `index` - Index of the Java installation to set as default
    ///
    /// # Returns
    ///
    /// - `true` if the index is valid and default was set
    /// - `false` if the index is out of bounds
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// let mut manager = JavaManager::new();
    /// // Add installations first...
    /// // let success = manager.set_default(0);
    /// ```
    pub fn set_default(&mut self, index: usize) -> bool {
        if index < self.java_installations.len() {
            self.default_index = Some(index);
            true
        } else {
            false
        }
    }

    /// Sets the default Java installation by version.
    ///
    /// If multiple installations have the same version, sets the first one as default.
    ///
    /// # Arguments
    ///
    /// * `version` - Major version to set as default
    ///
    /// # Returns
    ///
    /// - `true` if a matching installation was found and set as default
    /// - `false` if no installation with the specified version exists
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// let mut manager = JavaManager::new();
    /// // Discover installations first...
    /// // let success = manager.set_default_by_version(11);
    /// ```
    pub fn set_default_by_version(&mut self, version: u32) -> bool {
        if let Some(&index) = self.version_map
            .get(&version)
            .and_then(|indices| indices.first())
        {
            self.default_index = Some(index);
            true
        } else {
            false
        }
    }

    /// Returns a reference to all Java installations.
    ///
    /// # Returns
    ///
    /// Reference to vector of all Java installations
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// let manager = JavaManager::new();
    /// // Discover installations first...
    /// // let all_java = manager.list();
    /// // for java in all_java {
    /// //     println!("{}", java);
    /// // }
    /// ```
    pub fn list(&self) -> &Vec<JavaInfo> {
        &self.java_installations
    }

    /// Returns the number of Java installations in the manager.
    ///
    /// # Returns
    ///
    /// Number of Java installations
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// let manager = JavaManager::new();
    /// assert_eq!(manager.len(), 0);
    /// ```
    pub fn len(&self) -> usize {
        self.java_installations.len()
    }

    /// Checks if the manager has any Java installations.
    ///
    /// # Returns
    ///
    /// - `true` if there are no Java installations
    /// - `false` if there is at least one Java installation
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// let manager = JavaManager::new();
    /// assert!(manager.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.java_installations.is_empty()
    }

    /// Filters Java installations by supplier/vendor.
    ///
    /// # Arguments
    ///
    /// * `supplier` - Supplier name to filter by (case-insensitive)
    ///
    /// # Returns
    ///
    /// Vector of references to Java installations from the specified supplier
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// let manager = JavaManager::new();
    /// // Discover installations first...
    /// // let openjdk_installations = manager.filter_by_supplier("OpenJDK");
    /// ```
    pub fn filter_by_supplier(&self, supplier: &str) -> Vec<&JavaInfo> {
        let supplier_lower = supplier.to_lowercase();
        self.java_installations
            .iter()
            .filter(|info| info.suppliers.to_lowercase().contains(&supplier_lower))
            .collect()
    }

    /// Filters Java installations by architecture.
    ///
    /// # Arguments
    ///
    /// * `architecture` - Architecture to filter by (e.g., "64-bit", "32-bit")
    ///
    /// # Returns
    ///
    /// Vector of references to Java installations with the specified architecture
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// let manager = JavaManager::new();
    /// // Discover installations first...
    /// // let x64_installations = manager.filter_by_architecture("64-bit");
    /// ```
    pub fn filter_by_architecture(&self, architecture: &str) -> Vec<&JavaInfo> {
        self.java_installations
            .iter()
            .filter(|info| info.architecture == architecture)
            .collect()
    }

    /// Executes a Java command using the default Java installation.
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line arguments to pass to Java
    ///
    /// # Returns
    ///
    /// - `Ok(String)` - Command output as a string
    /// - `Err(std::io::Error)` - If the command fails to execute or no default Java is set
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut manager = JavaManager::new();
    ///     manager.discover_installations()?;
    ///     
    ///     let output = manager.execute_default(&["-version"])?;
    ///     println!("Output:\n{}", output);
    ///     Ok(())
    /// }
    /// ```
    pub fn execute_default(&self, args: &[&str]) -> std::io::Result<String> {
        self.get_default()
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No default Java installation set"
            ))?
            .execute_with_output(args)
    }

    /// Executes a Java command using a specific Java version.
    ///
    /// # Arguments
    ///
    /// * `version` - Major version of Java to use
    /// * `args` - Command-line arguments to pass to Java
    ///
    /// # Returns
    ///
    /// - `Ok(String)` - Command output as a string
    /// - `Err(std::io::Error)` - If the command fails to execute or the version is not found
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut manager = JavaManager::new();
    ///     manager.discover_installations()?;
    ///     
    ///     let output = manager.execute_with_version(11, &["-version"])?;
    ///     println!("Java 11 output:\n{}", output);
    ///     Ok(())
    /// }
    /// ```
    pub fn execute_with_version(&self, version: u32, args: &[&str]) -> std::io::Result<String> {
        self.get_by_version(version)
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Java version {} not found", version)
            ))?
            .execute_with_output(args)
    }

    /// Returns a summary of Java installations by version.
    ///
    /// # Returns
    ///
    /// HashMap mapping major versions to counts of installations
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// let manager = JavaManager::new();
    /// // Discover installations first...
    /// // let summary = manager.get_version_summary();
    /// // for (version, count) in summary {
    /// //     println!("Java {}: {} installations", version, count);
    /// // }
    /// ```
    pub fn get_version_summary(&self) -> HashMap<u32, usize> {
        let mut summary = HashMap::new();
        
        for info in &self.java_installations {
            if let Some(version) = info.get_major_version() {
                *summary.entry(version).or_insert(0) += 1;
            }
        }
        
        summary
    }

    /// Clears all Java installations from the manager.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use java_manager::JavaManager;
    ///
    /// let mut manager = JavaManager::new();
    /// // Add some installations...
    /// manager.clear();
    /// assert!(manager.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.java_installations.clear();
        self.version_map.clear();
        self.default_index = None;
    }
}

impl Default for JavaManager {
    /// Creates a new `JavaManager` with default settings.
    ///
    /// # Returns
    ///
    /// A new `JavaManager` instance
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests creating a new JavaManager
    #[test]
    fn test_new_manager() {
        let manager = JavaManager::new();
        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);
        assert!(manager.get_default().is_none());
    }

    /// Tests adding Java installations to the manager
    #[test]
    fn test_add_java() {
        let mut manager = JavaManager::new();
        
        let java1 = JavaInfo::new("java", "/usr/bin/java1", "11.0.12", "64-bit", "OpenJDK");
        let java2 = JavaInfo::new("java", "/usr/bin/java2", "1.8.0_312", "64-bit", "Oracle");
        
        manager.add(java1.clone());
        manager.add(java2.clone());
        
        assert_eq!(manager.len(), 2);
        assert!(!manager.is_empty());
        
        // First added should be default
        assert!(manager.get_default().is_some());
        assert_eq!(manager.get_default().unwrap().path, java1.path);
    }

    /// Tests getting Java installations by index
    #[test]
    fn test_get_by_index() {
        let mut manager = JavaManager::new();
        
        let java1 = JavaInfo::new("java", "/usr/bin/java1", "11.0.12", "64-bit", "OpenJDK");
        let java2 = JavaInfo::new("java", "/usr/bin/java2", "1.8.0_312", "64-bit", "Oracle");
        
        manager.add(java1);
        manager.add(java2);
        
        assert!(manager.get(0).is_some());
        assert!(manager.get(1).is_some());
        assert!(manager.get(2).is_none());
    }

    /// Tests getting Java installations by version
    #[test]
    fn test_get_by_version() {
        let mut manager = JavaManager::new();
        
        let java11 = JavaInfo::new("java", "/usr/bin/java11", "11.0.12", "64-bit", "OpenJDK");
        let java8 = JavaInfo::new("java", "/usr/bin/java8", "1.8.0_312", "64-bit", "Oracle");
        
        manager.add(java11);
        manager.add(java8);
        
        // Test getting Java 11
        let java_11 = manager.get_by_version(11);
        assert!(java_11.is_some());
        assert_eq!(java_11.unwrap().get_major_version(), Some(11));
        
        // Test getting Java 8
        let java_8 = manager.get_by_version(8);
        assert!(java_8.is_some());
        assert_eq!(java_8.unwrap().get_major_version(), Some(8));
        
        // Test getting non-existent version
        let java_17 = manager.get_by_version(17);
        assert!(java_17.is_none());
    }

    /// Tests getting all Java installations by version
    #[test]
    fn test_get_all_by_version() {
        let mut manager = JavaManager::new();
        
        // Add multiple Java 11 installations
        let java11_1 = JavaInfo::new("java", "/usr/bin/java11_1", "11.0.12", "64-bit", "OpenJDK");
        let java11_2 = JavaInfo::new("java", "/usr/bin/java11_2", "11.0.13", "64-bit", "Oracle");
        let java8 = JavaInfo::new("java", "/usr/bin/java8", "1.8.0_312", "64-bit", "OpenJDK");
        
        manager.add(java11_1);
        manager.add(java11_2);
        manager.add(java8);
        
        // Get all Java 11 installations
        let java11_installations = manager.get_all_by_version(11);
        assert_eq!(java11_installations.len(), 2);
        
        // Get all Java 8 installations
        let java8_installations = manager.get_all_by_version(8);
        assert_eq!(java8_installations.len(), 1);
        
        // Get all Java 17 installations (none)
        let java17_installations = manager.get_all_by_version(17);
        assert_eq!(java17_installations.len(), 0);
    }

    /// Tests setting and getting default Java installation
    #[test]
    fn test_set_default() {
        let mut manager = JavaManager::new();
        
        let java1 = JavaInfo::new("java", "/usr/bin/java1", "11.0.12", "64-bit", "OpenJDK");
        let java2 = JavaInfo::new("java", "/usr/bin/java2", "1.8.0_312", "64-bit", "Oracle");
        
        manager.add(java1.clone());
        manager.add(java2.clone());
        
        // First added should be default
        assert_eq!(manager.get_default().unwrap().path, java1.path);
        
        // Set second as default
        assert!(manager.set_default(1));
        assert_eq!(manager.get_default().unwrap().path, java2.path);
        
        // Try to set invalid index
        assert!(!manager.set_default(5));
        assert_eq!(manager.get_default().unwrap().path, java2.path); // Should remain unchanged
    }

    /// Tests setting default by version
    #[test]
    fn test_set_default_by_version() {
        let mut manager = JavaManager::new();
        
        let java11 = JavaInfo::new("java", "/usr/bin/java11", "11.0.12", "64-bit", "OpenJDK");
        let java8 = JavaInfo::new("java", "/usr/bin/java8", "1.8.0_312", "64-bit", "Oracle");
        
        manager.add(java11.clone());
        manager.add(java8.clone());
        
        // Set default to Java 8
        assert!(manager.set_default_by_version(8));
        assert_eq!(manager.get_default().unwrap().path, java8.path);
        
        // Set default to Java 11
        assert!(manager.set_default_by_version(11));
        assert_eq!(manager.get_default().unwrap().path, java11.path);
        
        // Try to set non-existent version
        assert!(!manager.set_default_by_version(17));
        assert_eq!(manager.get_default().unwrap().path, java11.path); // Should remain unchanged
    }

    /// Tests filtering Java installations by supplier
    #[test]
    fn test_filter_by_supplier() {
        let mut manager = JavaManager::new();
        
        let java1 = JavaInfo::new("java", "/usr/bin/java1", "11.0.12", "64-bit", "OpenJDK");
        let java2 = JavaInfo::new("java", "/usr/bin/java2", "11.0.13", "64-bit", "Oracle");
        let java3 = JavaInfo::new("java", "/usr/bin/java3", "1.8.0_312", "64-bit", "OpenJDK");
        
        manager.add(java1);
        manager.add(java2);
        manager.add(java3);
        
        // Filter by OpenJDK (case-insensitive)
        let openjdk_installations = manager.filter_by_supplier("OpenJDK");
        assert_eq!(openjdk_installations.len(), 2);
        
        // Filter by Oracle
        let oracle_installations = manager.filter_by_supplier("Oracle");
        assert_eq!(oracle_installations.len(), 1);
        
        // Filter by non-existent supplier
        let ibm_installations = manager.filter_by_supplier("IBM");
        assert_eq!(ibm_installations.len(), 0);
    }

    /// Tests filtering Java installations by architecture
    #[test]
    fn test_filter_by_architecture() {
        let mut manager = JavaManager::new();
        
        let java64 = JavaInfo::new("java", "/usr/bin/java64", "11.0.12", "64-bit", "OpenJDK");
        let java32 = JavaInfo::new("java", "/usr/bin/java32", "1.8.0_312", "32-bit", "Oracle");
        let java64_2 = JavaInfo::new("java", "/usr/bin/java64_2", "17.0.1", "64-bit", "OpenJDK");
        
        manager.add(java64);
        manager.add(java32);
        manager.add(java64_2);
        
        // Filter 64-bit installations
        let x64_installations = manager.filter_by_architecture("64-bit");
        assert_eq!(x64_installations.len(), 2);
        
        // Filter 32-bit installations
        let x86_installations = manager.filter_by_architecture("32-bit");
        assert_eq!(x86_installations.len(), 1);
        
        // Filter non-existent architecture
        let unknown_installations = manager.filter_by_architecture("Unknown");
        assert_eq!(unknown_installations.len(), 0);
    }

    /// Tests getting version summary
    #[test]
    fn test_get_version_summary() {
        let mut manager = JavaManager::new();
        
        // Add multiple versions
        let java11_1 = JavaInfo::new("java", "/usr/bin/java11_1", "11.0.12", "64-bit", "OpenJDK");
        let java11_2 = JavaInfo::new("java", "/usr/bin/java11_2", "11.0.13", "64-bit", "Oracle");
        let java8 = JavaInfo::new("java", "/usr/bin/java8", "1.8.0_312", "64-bit", "OpenJDK");
        let java17 = JavaInfo::new("java", "/usr/bin/java17", "17.0.1", "64-bit", "OpenJDK");
        let java_invalid = JavaInfo::new("java", "/usr/bin/java_invalid", "invalid", "64-bit", "Unknown");
        
        manager.add(java11_1);
        manager.add(java11_2);
        manager.add(java8);
        manager.add(java17);
        manager.add(java_invalid);
        
        let summary = manager.get_version_summary();
        
        assert_eq!(summary.get(&11), Some(&2)); // Two Java 11 installations
        assert_eq!(summary.get(&8), Some(&1));  // One Java 8 installation
        assert_eq!(summary.get(&17), Some(&1)); // One Java 17 installation
        assert_eq!(summary.get(&99), None);     // No Java 99 installations
    }

    /// Tests clearing all installations
    #[test]
    fn test_clear() {
        let mut manager = JavaManager::new();
        
        let java1 = JavaInfo::new("java", "/usr/bin/java1", "11.0.12", "64-bit", "OpenJDK");
        let java2 = JavaInfo::new("java", "/usr/bin/java2", "1.8.0_312", "64-bit", "Oracle");
        
        manager.add(java1);
        manager.add(java2);
        
        assert_eq!(manager.len(), 2);
        assert!(!manager.is_empty());
        
        manager.clear();
        
        assert_eq!(manager.len(), 0);
        assert!(manager.is_empty());
        assert!(manager.get_default().is_none());
    }

    /// Tests the list method
    #[test]
    fn test_list() {
        let mut manager = JavaManager::new();
        
        let java1 = JavaInfo::new("java", "/usr/bin/java1", "11.0.12", "64-bit", "OpenJDK");
        let java2 = JavaInfo::new("java", "/usr/bin/java2", "1.8.0_312", "64-bit", "Oracle");
        
        manager.add(java1);
        manager.add(java2);
        
        let list = manager.list();
        assert_eq!(list.len(), 2);
        
        // Verify we can iterate over the list
        for java in list {
            assert!(java.is_valid() || !std::path::Path::new(&java.path).exists());
        }
    }

    /// Tests the Default trait implementation
    #[test]
    fn test_default() {
        let manager = JavaManager::default();
        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);
    }

    /// Tests discovering installations (if Java is available)
    #[test]
    fn test_discover_installations() {
        let mut manager = JavaManager::new();
        let result = manager.discover_installations();
        
        // Discovery might succeed or fail depending on whether Java is installed
        if result.is_ok() {
            // If discovery succeeded, there should be at least one installation
            // (unless the system has no Java at all)
            println!("Discovered {} Java installations", manager.len());
            
            if !manager.is_empty() {
                assert!(manager.get_default().is_some());
                
                // List all discovered installations
                for (i, java) in manager.list().iter().enumerate() {
                    println!("{}. {}", i + 1, java);
                }
            }
        } else {
            println!("Discovery failed (Java may not be installed)");
        }
    }
}