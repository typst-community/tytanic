//! The core library of tytanic.

pub mod config;
pub mod doc;
pub mod library;
pub mod project;
pub mod stdx;
pub mod test;
pub mod test_set;

/// The tool name, this is used in various places like config file directories,
/// manifest tool sections, , and more.
pub const TOOL_NAME: &str = "tytanic";

#[cfg(test)]
pub mod _dev;
