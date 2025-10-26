//! The V1 store.
//!
//! The API is deliberately very restrictive until the [legacy store][legacy]
//! is removed.
//!
//! [legacy]: super::legacy

use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use uuid::Uuid;

use crate::project::vcs::IgnoreDirectoryError;
use crate::project::vcs::Vcs;
use crate::test::Ident;
use crate::test::UnitIdent;

use super::ArtifactKind;

/// Represents the new store in which artifacts are stored in a dedicated
/// directory.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct V1Store {
    pub(super) test_root: PathBuf,
    pub(super) store_root: PathBuf,
}
/// Store-wide artifact handling.
impl V1Store {
    /// Creates the ignore file in the directory for temporary artifacts.
    pub(super) fn ignore_tmp_dir(&self, vcs: &Vcs) -> Result<(), IgnoreDirectoryError> {
        let dir = self.tmp_dir_path();

        tracing::debug!(?vcs, ?dir, "ignoring `tmp` dir");
        vcs.ignore_directory(dir)?;

        Ok(())
    }
}

