//! Functions for accessing Java from the local environment.

use crate::JavaInfo;
use std::env;

/// Returns a `JavaInfo` for the Java installation pointed to by the `JAVA_HOME`
/// environment variable, if set and valid.
///
/// This function reads the `JAVA_HOME` variable, treats it as a path, and attempts
/// to create a `JavaInfo` from it. If the variable is not set or if the resulting
/// `JavaInfo` cannot be created (e.g., the path does not contain a valid Java),
/// `None` is returned.
///
/// # Examples
///
/// ```no_run
/// use java_manager::java_home;
///
/// if let Some(java) = java_home() {
///     println!("JAVA_HOME points to Java version {}", java.version);
/// } else {
///     println!("JAVA_HOME is not set or invalid");
/// }
/// ```
pub fn java_home() -> Option<JavaInfo> {
    env::var("JAVA_HOME")
        .ok()
        .and_then(|path| JavaInfo::new(path).ok())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_java_home_not_set() {
        let previous = std::env::var_os("JAVA_HOME");
        // SAFETY: single-threaded test, no other code depends on JAVA_HOME
        unsafe {
            std::env::remove_var("JAVA_HOME");
        }
        let result = super::java_home();
        assert!(result.is_none());
        if let Some(val) = previous {
            // SAFETY: restoring original value
            unsafe {
                std::env::set_var("JAVA_HOME", &val);
            }
        }
    }

    #[test]
    fn test_java_home_invalid_path() {
        let previous = std::env::var_os("JAVA_HOME");
        // SAFETY: single-threaded test
        unsafe {
            std::env::set_var("JAVA_HOME", "/nonexistent/java/home");
        }
        let result = super::java_home();
        assert!(result.is_none());
        if let Some(val) = previous {
            unsafe {
                std::env::set_var("JAVA_HOME", &val);
            }
        } else {
            unsafe {
                std::env::remove_var("JAVA_HOME");
            }
        }
    }
}
