use typst::Library;
use typst::World;
use typst::diag::FileResult;
use typst::foundations::Bytes;
use typst::foundations::Datetime;
use typst::foundations::Duration;
use typst::syntax::FileId;
use typst::syntax::Source;
use typst::text::Font;
use typst::text::FontBook;
use typst::utils::LazyHash;
use typst_kit::datetime::Time;
use typst_kit::diagnostics::DiagnosticWorld;
use typst_kit::fonts::FontStore;
use tytanic_utils::forward_trait;

pub mod file;
pub mod font;

/// A trait for providing access to files.
pub trait ProvideFile: Send + Sync {
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

impl ProvideFont for FontStore {
    fn provide_font_book(&self) -> &LazyHash<FontBook> {
        self.book()
    }

    fn provide_font(&self, index: usize) -> Option<Font> {
        self.font(index)
    }
}

/// A builder for [`ComposedWorld`].
pub struct ComposedWorldBuilder<'w> {
    files: Option<&'w dyn ProvideFile>,
    fonts: Option<&'w dyn ProvideFont>,
    library: Option<&'w LazyHash<Library>>,
    datetime: Option<&'w Time>,
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

    /// Configure the library.
    pub fn library_provider(self, value: &'w LazyHash<Library>) -> Self {
        Self {
            library: Some(value),
            ..self
        }
    }

    /// Configure the datetime.
    pub fn datetime_provider(self, value: &'w Time) -> Self {
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
    library: &'w LazyHash<Library>,
    datetime: &'w Time,
    id: FileId,
}

impl<'w> ComposedWorld<'w> {
    /// Creates a new builder.
    pub fn builder() -> ComposedWorldBuilder<'w> {
        ComposedWorldBuilder::new()
    }
}

impl World for ComposedWorld<'_> {
    fn library(&self) -> &LazyHash<Library> {
        self.library
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

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.datetime.today(offset)
    }
}

impl DiagnosticWorld for ComposedWorld<'_> {
    fn name(&self, id: FileId) -> String {
        format!("{id:?}")
    }
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) mod test_utils {
    use std::collections::HashMap;
    use std::sync::LazyLock;

    use file::VirtualFileProvider;
    use font::VirtualFontProvider;
    use typst::LibraryExt;

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

    pub(crate) static TEST_DEFAULT_LIBRARY_PROVIDER: LazyLock<Library> =
        LazyLock::new(Library::default);

    pub(crate) static TEST_AUGMENTED_LIBRARY_PROVIDER: LazyLock<Library> =
        LazyLock::new(augmented_default_library);

    pub(crate) static TEST_DATETIME_PROVIDER: LazyLock<Time> =
        LazyLock::new(|| Time::fixed_timestamp(0).unwrap());

    pub(crate) fn virtual_world<'w>(
        source: Source,
        files: &'w mut VirtualFileProvider,
        library: &'w LazyHash<Library>,
    ) -> ComposedWorld<'w> {
        files
            .slots_mut()
            .insert(source.id(), VirtualFileSlot::from_source(source.clone()));

        ComposedWorld::builder()
            .file_provider(files)
            .font_provider(&*TEST_FONT_PROVIDER)
            .library_provider(library)
            .datetime_provider(&TEST_DATETIME_PROVIDER)
            .build(source.id())
    }
}
