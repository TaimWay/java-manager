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
//! use java_locator::{quick_search, JavaRunner};
//!
//! // Find all Java installations in PATH
//! let javas = quick_search()?;
//! if let Some(java) = javas.first() {
//!     // Run a JAR file
//!     JavaRunner::new()
//!         .java(java.clone())
//!         .jar("app.jar")
//!         .arg("--verbose")
//!         .execute()?;
//! }
//! # Ok::<_, java_locator::JavaError>(())
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