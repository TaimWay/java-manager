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

/// TTL-based cache that avoids redundant full-disk scans of Java installations.
pub mod cache;

/// Error types returned by every fallible operation in this crate.
pub mod error;

/// Execute Java programs with configurable arguments, memory, and I/O.
pub mod execute;

/// [`JavaInfo`] and [`JavaVersion`] — structured metadata from a Java installation.
pub mod info;

/// Read the `JAVA_HOME` environment variable.
pub mod local;

/// Discover Java installations via `PATH`, Everything SDK, registry, BFS, and more.
pub mod search;

/// Async JDK download with resume support, parallel chunks, and archive extraction.
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
///
/// # Examples
///
/// ```
/// use java_manager::{JavaInfo, best_match};
///
/// let javas = vec![
///     JavaInfo { version: "11.0.2".into(), parsed_version: java_manager::JavaVersion::parse("11.0.2"), ..Default::default() },
///     JavaInfo { version: "17.0.1".into(), parsed_version: java_manager::JavaVersion::parse("17.0.1"), ..Default::default() },
/// ];
///
/// let best = best_match(javas, "11").unwrap();
/// assert_eq!(best.version, "11.0.2");
/// ```
pub fn best_match(javas: Vec<JavaInfo>, req: &str) -> Option<JavaInfo> {
    javas
        .into_iter()
        .filter(|j| j.matches_version(req))
        .max_by(|a, b| a.parsed_version.cmp(&b.parsed_version))
}
