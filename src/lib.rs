//! A library for locating Java installations on the local system and executing Java programs.
//!
//! This crate provides functionality to:
//! - Discover Java runtimes via `PATH`, `JAVA_HOME`, or deep system scans.
//! - Extract detailed metadata (version, vendor, architecture) from each installation.
//! - Execute Java applications with configurable arguments, memory settings, and I/O redirection.
//!
//! # Examples
//!
//! ```no_run
//! use java_manager::{java_home, JavaRunner};
//!
//! // Find all Java installations in PATH
//! let java = java_home().unwrap();
//! // Run a JAR file
//! JavaRunner::new()
//!     .java(java)
//!     .arg("--version")
//!     .execute()?;
//! # Ok::<_, java_manager::JavaError>(())
//! ```

pub mod cache;
pub mod error;
pub mod execute;
pub mod info;
pub mod local;
pub mod search;

#[cfg(feature = "download")]
pub mod download;

pub use cache::JavaCache;
pub use error::JavaError;
pub use execute::JavaRedirect;
pub use execute::JavaRunner;
pub use info::JavaInfo;
pub use info::JavaVersion;
pub use local::java_home;
pub use search::deep_search;
pub use search::full_search;
pub use search::quick_search;

#[cfg(feature = "parallel")]
pub use search::parallel_full_search;

/// Filter a list of `JavaInfo` by a version requirement.
///
/// See [`JavaInfo::matches_version`] for the supported requirement formats.
pub fn filter_by_version(javas: Vec<JavaInfo>, req: &str) -> Vec<JavaInfo> {
    javas
        .into_iter()
        .filter(|j| j.matches_version(req))
        .collect()
}

/// Pick the best (highest version) match from a list of `JavaInfo`.
///
/// Returns `None` if no installation matches the requirement.
pub fn best_match(javas: Vec<JavaInfo>, req: &str) -> Option<JavaInfo> {
    javas
        .into_iter()
        .filter(|j| j.matches_version(req))
        .max_by(|a, b| a.parsed_version.cmp(&b.parsed_version))
}
