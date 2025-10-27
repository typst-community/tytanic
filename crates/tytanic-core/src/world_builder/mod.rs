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
use typst_kit::download::Progress;
use typst_kit::download::ProgressSink;

pub mod datetime;
pub mod file;
pub mod font;
pub mod library;

macro_rules! forward_trait {
    (impl<$pointee:ident> $trait:ident for [$($pointer:ty),+] $funcs:tt) => {
        $(impl<$pointee: $trait> $trait for $pointer $funcs)+
    };
}

/// A trait for providing access to files.
pub trait ProvideFile: Send + Sync {
    /// Provides a Typst source with the given file id.
    ///
    /// This may download a package, for which the progress callbacks will be
    /// used.
    fn provide_source(&self, id: FileId, progress: &mut dyn Progress) -> FileResult<Source>;

    /// Provides a generic file with the given file id.
    ///
    /// This may download a package, for which the progress callbacks will be
    /// used.
    fn provide_bytes(&self, id: FileId, progress: &mut dyn Progress) -> FileResult<Bytes>;

    /// Reset the cached files for the next compilation.
    fn reset_all(&self);
}

forward_trait! {
    impl<W> ProvideFile for [std::boxed::Box<W>, std::sync::Arc<W>, &W] {
        fn provide_source(&self, id: FileId, progress: &mut dyn Progress) -> FileResult<Source> {
            W::provide_source(self, id, progress)
        }

        fn provide_bytes(&self, id: FileId, progress: &mut dyn Progress) -> FileResult<Bytes> {
            W::provide_bytes(self, id, progress)
        }

        fn reset_all(&self) {
            W::reset_all(self)
        }
    }
}

/// A trait for providing access to fonts.
pub trait ProvideFont: Send + Sync {
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
pub trait ProvideLibrary: Send + Sync {
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

/// A trait for providing access to date.
pub trait ProvideDatetime: Send + Sync {
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

/// A builder for [`ComposedWorld`].
pub struct ComposedWorldBuilder<'w> {
    files: Option<&'w dyn ProvideFile>,
    fonts: Option<&'w dyn ProvideFont>,
    library: Option<&'w dyn ProvideLibrary>,
    datetime: Option<&'w dyn ProvideDatetime>,
}

impl ComposedWorldBuilder<'_> {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            files: None,
            fonts: None,
            library: None,
            datetime: None,
        }
    }
}

impl<'w> ComposedWorldBuilder<'w> {
    /// Configure the file provider.
    pub fn file_provider(self, value: &'w dyn ProvideFile) -> Self {
        Self {
            files: Some(value),
            ..self
        }
    }

    /// Configure the font provider.
    pub fn font_provider(self, value: &'w dyn ProvideFont) -> Self {
        Self {
            fonts: Some(value),
            ..self
        }
    }

    /// Configure the library provider.
    pub fn library_provider(self, value: &'w dyn ProvideLibrary) -> Self {
        Self {
            library: Some(value),
            ..self
        }
    }

    /// Configure the datetime provider.
    pub fn datetime_provider(self, value: &'w dyn ProvideDatetime) -> Self {
        Self {
            datetime: Some(value),
            ..self
        }
    }

    /// Build the world with the configured providers.
    ///
    /// Panics if a provider is missing.
    pub fn build(self, id: FileId) -> ComposedWorld<'w> {
        self.try_build(id).unwrap()
    }

    /// Build the world with the configured providers.
    ///
    /// Returns `None` if a provider is missing.
    pub fn try_build(self, id: FileId) -> Option<ComposedWorld<'w>> {
        Some(ComposedWorld {
            files: self.files?,
            fonts: self.fonts?,
            library: self.library?,
            datetime: self.datetime?,
            id,
        })
    }
}

impl Default for ComposedWorldBuilder<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// A shim around the various provider traits which together implement a whole
/// [`World`].
pub struct ComposedWorld<'w> {
    files: &'w dyn ProvideFile,
    fonts: &'w dyn ProvideFont,
    library: &'w dyn ProvideLibrary,
    datetime: &'w dyn ProvideDatetime,
    id: FileId,
}

impl<'w> ComposedWorld<'w> {
    /// Creates a new builder.
    pub fn builder() -> ComposedWorldBuilder<'w> {
        ComposedWorldBuilder::new()
    }
}

impl ComposedWorld<'_> {
    /// Resets the inner providers for the next compilation.
    pub fn reset(&self) {
        // TODO(tinger): We probably really want exclusive access here, no
        // provider should be used while it's being reset.
        self.files.reset_all();
        self.datetime.reset_today();
    }
}

impl World for ComposedWorld<'_> {
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
        self.files.provide_source(id, &mut ProgressSink)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.provide_bytes(id, &mut ProgressSink)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.provide_font(index)
    }

    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        self.datetime.provide_today(offset)
    }
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) mod test_utils {
    use std::collections::HashMap;
    use std::sync::LazyLock;

    use chrono::DateTime;
    use datetime::FixedDateProvider;
    use file::VirtualFileProvider;
    use font::VirtualFontProvider;
    use library::LibraryProvider;

    use super::file::VirtualFileSlot;
    use super::*;
    use crate::library::augmented_default_library;

    pub(crate) fn test_file_provider(source: Source) -> VirtualFileProvider {
        let mut map = HashMap::new();
        map.insert(source.id(), VirtualFileSlot::from_source(source.clone()));

        VirtualFileProvider::from_slots(map)
    }

    pub(crate) static TEST_FONT_PROVIDER: LazyLock<VirtualFontProvider> = LazyLock::new(|| {
        let fonts: Vec<_> = typst_assets::fonts()
            .flat_map(|data| Font::iter(Bytes::new(data)))
            .collect();

        let book = FontBook::from_fonts(&fonts);
        VirtualFontProvider::new(book, fonts)
    });

    pub(crate) static TEST_DEFAULT_LIBRARY_PROVIDER: LazyLock<LibraryProvider> =
        LazyLock::new(LibraryProvider::new);

    pub(crate) static TEST_AUGMENTED_LIBRARY_PROVIDER: LazyLock<LibraryProvider> =
        LazyLock::new(|| LibraryProvider::with_library(augmented_default_library()));

    pub(crate) static TEST_DATETIME_PROVIDER: LazyLock<FixedDateProvider> =
        LazyLock::new(|| FixedDateProvider::new(DateTime::from_timestamp(0, 0).unwrap()));

    pub(crate) fn virtual_world<'w>(
        source: Source,
        files: &'w mut VirtualFileProvider,
        library: &'w LibraryProvider,
    ) -> ComposedWorld<'w> {
        files
            .slots_mut()
            .insert(source.id(), VirtualFileSlot::from_source(source.clone()));

        ComposedWorld::builder()
            .file_provider(files)
            .font_provider(&*TEST_FONT_PROVIDER)
            .library_provider(library)
            .datetime_provider(&*TEST_DATETIME_PROVIDER)
            .build(source.id())
    }
}
