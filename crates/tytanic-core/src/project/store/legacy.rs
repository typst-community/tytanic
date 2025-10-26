//! The legacy store.
//!
//! This is going to be deprecated in the next version.

use std::fs;
use std::io;
use std::path::PathBuf;

use crate::test::Ident;
use crate::test::UnitTest;

use super::ArtifactKind;

/// Represents the legacy store in which artifacts are stored alongside their
/// test sources.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LegacyStore {
    pub(super) test_root: PathBuf,
}

impl LegacyStore {}
