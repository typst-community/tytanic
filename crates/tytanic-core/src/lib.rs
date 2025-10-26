//! # `tytanic-core`
//! This crate contains core data types for Tytanic such as test identifiers,
//! test suites, strongly typed paths, and VCS types.
//!
//! # Features
//! The following features are provided:
//! - `terminal-diagnostics` to include terminal diagnostic formatting.
//! - `default-world-builder` to include default implementations in the
//!   [`world`] module.

pub mod analysis;
pub mod config;
#[cfg(feature = "terminal-diagnostics")]
pub mod diag;
pub mod filter;
pub mod project;
pub mod result;
pub mod runner;
pub mod suite;
pub mod test;

/// The tool name, this is used in various places like config file directories,
/// manifest tool sections, and more.
pub const TOOL_NAME: &str = "tytanic";
