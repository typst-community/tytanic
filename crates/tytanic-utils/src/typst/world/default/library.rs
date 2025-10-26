//! Default implementations for [`ProvideLibrary`]

use std::fmt::Debug;

use typst::Library;
use typst::LibraryBuilder;
use typst::LibraryExt;
use typst::utils::LazyHash;

use crate::typst::world::ProvideLibrary;

/// Provides access to a library.
pub struct LibraryProvider {
    library: LazyHash<Library>,
}

impl LibraryProvider {
    /// Creates a new library provider with the default library.
    pub fn new() -> Self {
        Self::with_library(Library::default())
    }

    /// Creates a new library provider with the given library.
    pub fn with_library(library: Library) -> Self {
        Self {
            library: LazyHash::new(library),
        }
    }

    /// Creates a new library provider with the given library builder callback.
    pub fn with_builder(f: impl FnOnce(&mut LibraryBuilder) -> &mut LibraryBuilder) -> Self {
        let mut builder = Library::builder();
        f(&mut builder);
        Self::with_library(builder.build())
    }
}

impl LibraryProvider {
    /// The library.
    pub fn library(&self) -> &LazyHash<Library> {
        &self.library
    }
}

impl Default for LibraryProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ProvideLibrary for LibraryProvider {
    fn provide_library(&self) -> &LazyHash<Library> {
        self.library()
    }
}

impl Debug for LibraryProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryProvider").finish_non_exhaustive()
    }
}
