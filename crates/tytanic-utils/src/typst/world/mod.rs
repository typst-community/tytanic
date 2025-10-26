//! World implementation helpers.
//!
//! This module contains various `Provide*` traits and one or more default
//! implementations for each of them if the `default-world-builder` feature is
//! enabled.
//!
//! These components make it easier to share resources between many short lived
//! world implementations.

use std::fmt::Debug;
use std::sync::Arc;

use typst::Library;
use typst::World;
use typst::diag::FileResult;
use typst::foundations::Bytes;
use typst::foundations::Datetime;
use typst::syntax::FileId;
use typst::syntax::Source;
use typst::text::Font;
use typst::text::FontBook;
use typst::utils::LazyHash;

use crate::forward_trait;

#[cfg(feature = "typst-world-builder-default")]
pub mod default;

/// A trait for providing access to files.
pub trait ProvideFile: Debug + Send + Sync {
    /// Provides a Typst source with the given file id.
    ///
    /// This may download a package, for which the progress callbacks will be
    /// used.
    fn provide_source(&self, id: FileId) -> FileResult<Source>;

    /// Provides a generic file with the given file id.
    ///
    /// This may download a package, for which the progress callbacks will be
    /// used.
    fn provide_bytes(&self, id: FileId) -> FileResult<Bytes>;

    /// Reset the cached files for the next compilation.
    fn reset_all(&self);
}

forward_trait! {
    impl<W> ProvideFile for [std::boxed::Box<W>, std::sync::Arc<W>, &W] {
        fn provide_source(&self, id: FileId) -> FileResult<Source> {
            W::provide_source(self, id)
        }

        fn provide_bytes(&self, id: FileId) -> FileResult<Bytes> {
            W::provide_bytes(self, id)
        }

        fn reset_all(&self) {
            W::reset_all(self)
        }
    }
}

/// A trait for providing access to fonts.
pub trait ProvideFont: Debug + Send + Sync {
    /// Provides the font book which stores metadata about fonts.
    fn provide_font_book(&self) -> &LazyHash<FontBook>;

    /// Provides a font with the given index.
    fn provide_font(&self, index: usize) -> Option<Font>;
}

forward_trait! {
    impl<W> ProvideFont for [std::boxed::Box<W>, std::sync::Arc<W>, &W] {
        fn provide_font_book(&self) -> &LazyHash<FontBook> {
            W::provide_font_book(self)
        }

        fn provide_font(&self, index: usize) -> Option<Font> {
            W::provide_font(self, index)
        }
    }
}

/// A trait for providing access to libraries.
pub trait ProvideLibrary: Debug + Send + Sync {
    /// Provides the library.
    fn provide_library(&self) -> &LazyHash<Library>;
}

forward_trait! {
    impl<W> ProvideLibrary for [std::boxed::Box<W>, std::sync::Arc<W>, &W] {
        fn provide_library(&self) -> &LazyHash<Library> {
            W::provide_library(self)
        }
    }
}

/// A trait for providing access to the current date and time.
pub trait ProvideDatetime: Debug + Send + Sync {
    /// Provides the current date.
    ///
    /// If no offset is specified, the local date should be chosen. Otherwise,
    /// the UTC date should be chosen with the corresponding offset in hours.
    ///
    /// If this function returns `None`, Typst's `datetime` function will
    /// return an error.
    ///
    /// Note that most implementations should provide a date only or only very
    /// course time increments to ensure Typst's incremental compilation cache
    /// is not disrupted too much.
    fn provide_today(&self, offset: Option<i64>) -> Option<Datetime>;

    /// Reset the current date for the next compilation.
    ///
    /// Note that this is only relevant for those providers which actually
    /// provide the current date.
    fn reset_today(&self);
}

forward_trait! {
    impl<W> ProvideDatetime for [std::boxed::Box<W>, std::sync::Arc<W>, &W] {
        fn provide_today(&self, offset: Option<i64>) -> Option<Datetime> {
            W::provide_today(self, offset)
        }

        fn reset_today(&self) {
            W::reset_today(self)
        }
    }
}

/// A type alias for [`ComposedWorld`]s which use `Arc<dyn Trait>` for each
/// provider.
pub type ComposedDynWorld = ComposedWorld<
    Arc<dyn ProvideFile>,
    Arc<dyn ProvideFont>,
    Arc<dyn ProvideLibrary>,
    Arc<dyn ProvideDatetime>,
>;

/// A builder for [`ComposedWorld`].
#[derive(Debug)]
pub struct ComposedWorldBuilder<Fi, Fo, L, D> {
    files: Option<Fi>,
    fonts: Option<Fo>,
    library: Option<L>,
    datetime: Option<D>,
}

impl<Fi, Fo, L, D> ComposedWorldBuilder<Fi, Fo, L, D> {
    /// Creates a new world builder.
    pub fn new() -> Self {
        Self {
            files: None,
            fonts: None,
            library: None,
            datetime: None,
        }
    }
}

impl<Fi, Fo, L, D> ComposedWorldBuilder<Fi, Fo, L, D> {
    /// Configure the file provider.
    pub fn file_provider(self, value: Fi) -> Self {
        Self {
            files: Some(value),
            ..self
        }
    }

    /// Configure the font provider.
    pub fn font_provider(self, value: Fo) -> Self {
        Self {
            fonts: Some(value),
            ..self
        }
    }

    /// Configure the library provider.
    pub fn library_provider(self, value: L) -> Self {
        Self {
            library: Some(value),
            ..self
        }
    }

    /// Configure the datetime provider.
    pub fn datetime_provider(self, value: D) -> Self {
        Self {
            datetime: Some(value),
            ..self
        }
    }

    /// Build the world with the configured providers.
    ///
    /// Panics if a provider is missing.
    pub fn build(self, id: FileId) -> ComposedWorld<Fi, Fo, L, D> {
        self.try_build(id).unwrap()
    }

    /// Build the world with the configured providers.
    ///
    /// Returns `None` if a provider is missing.
    pub fn try_build(self, id: FileId) -> Option<ComposedWorld<Fi, Fo, L, D>> {
        Some(ComposedWorld {
            files: self.files?,
            fonts: self.fonts?,
            library: self.library?,
            datetime: self.datetime?,
            id,
        })
    }
}

impl<Fi, Fo, L, D> Default for ComposedWorldBuilder<Fi, Fo, L, D> {
    fn default() -> Self {
        Self::new()
    }
}

/// A shim around the various provider traits which together implement a whole
/// [`World`].
///
/// This can be built from a set of individual providers using
/// [`ComposedWorldBuilder`]. It implements [`World`] if its providers implement
/// the following traits:
/// - `Fi`: [`ProvideFile`]
/// - `Fo`: [`ProvideFont`]
/// - `L`: [`ProvideLibrary`]
/// - `D`: [`ProvideDatetime`]
#[derive(Debug)]
pub struct ComposedWorld<Fi, Fo, L, D> {
    files: Fi,
    fonts: Fo,
    library: L,
    datetime: D,
    id: FileId,
}

impl<Fi, Fo, L, D> ComposedWorld<Fi, Fo, L, D> {
    /// Creates a new builder.
    pub fn builder() -> ComposedWorldBuilder<Fi, Fo, L, D> {
        ComposedWorldBuilder::new()
    }
}

impl<Fi, Fo, L, D> ComposedWorld<Fi, Fo, L, D>
where
    Fi: ProvideFile,
    D: ProvideDatetime,
{
    /// Resets the inner providers for the next compilation.
    pub fn reset(&self) {
        self.files.reset_all();
        self.datetime.reset_today();
    }
}

impl<Fi, Fo, L, D> World for ComposedWorld<Fi, Fo, L, D>
where
    Fi: ProvideFile,
    Fo: ProvideFont,
    L: ProvideLibrary,
    D: ProvideDatetime,
{
    fn library(&self) -> &LazyHash<Library> {
        self.library.provide_library()
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.provide_font_book()
    }

    fn main(&self) -> FileId {
        self.id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.files.provide_source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.provide_bytes(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.provide_font(index)
    }

    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        self.datetime.provide_today(offset)
    }
}
