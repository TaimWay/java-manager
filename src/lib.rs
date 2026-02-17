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

pub mod error;
pub mod info;
pub mod search;
pub mod local;
pub mod execute;

pub use info::JavaInfo;
pub use error::JavaError;
pub use search::quick_search;
pub use search::deep_search;
pub use local::java_home;
pub use execute::JavaRunner;
pub use execute::JavaRedirect;