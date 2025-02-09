//! Utilities for working with [`typst`] and creating test [`World`]
//! implementations.

use std::collections::HashMap;
use std::sync::Mutex;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};

/// A virtual file slot, unlike the typst-cli implementation, this will not read
/// from disk.
#[derive(Debug, Clone)]
pub struct VirtualFile {
    /// The file id of this slot.
    pub id: FileId,

    /// The optional source of this file, this is only set when constructed
    /// through [`VirtualFile::new`].
    pub source: Option<Source>,

    /// The bytes of this file.
    pub bytes: Bytes,
}

impl VirtualFile {
    /// Create a new Typst source file with the given source code.
    pub fn new(id: FileId, source: &str) -> Self {
        Self {
            id,
            bytes: source.as_bytes().into(),
            source: Some(Source::new(id, source.to_owned())),
        }
    }

    /// Create a new virtual file from a [`typst_dev_assets`] entry, this will
    /// have no `source` field set.
    pub fn asset(id: FileId) -> Option<Self> {
        typst_dev_assets::get(&id.vpath().as_rootless_path().to_string_lossy()).map(|content| {
            Self {
                id,
                source: None,
                bytes: content.into(),
            }
        })
    }
}

/// A minimal implementation of [`World`] for running tests.
#[derive(Debug)]
pub struct VirtualWorld {
    /// The optional main file, defaults to `None`. If this is `None`, then this
    /// is only useful as a base implementation for pther worlds. Similar to the
    /// tests in `tytanic-core`.
    pub main: Option<FileId>,

    /// The standard library provided by this world, defaults
    /// [`Library::default`].
    pub lib: LazyHash<Library>,

    /// The fonts provided by this world, defaults to assets from
    /// [`typst_assets`] and [`typst_dev_assets`].
    pub book: LazyHash<FontBook>,

    /// The loaded fonts of the font book.
    pub fonts: Vec<Font>,

    /// The virtual file slots.
    ///
    /// This will not resolve any files from disk, all file slots are purely virtual
    /// in-memory files.
    pub slots: Mutex<HashMap<FileId, VirtualFile>>,
}

impl VirtualWorld {
    /// Creates a new test world with the given standard library.
    pub fn new(library: Library) -> Self {
        let fonts: Vec<_> = typst_assets::fonts()
            .chain(typst_dev_assets::fonts())
            .flat_map(|data| Font::iter(Bytes::from_static(data)))
            .collect();

        VirtualWorld {
            main: None,
            lib: LazyHash::new(library),
            book: LazyHash::new(FontBook::from_fonts(&fonts)),
            fonts,
            slots: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for VirtualWorld {
    fn default() -> Self {
        Self::new(Library::default())
    }
}

impl World for VirtualWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.lib
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main.expect("TestWorld did not contain a main file")
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        match self.slots.lock().unwrap().get(&id) {
            Some(slot) => slot
                .source
                .as_ref()
                .cloned()
                .ok_or_else(|| FileError::NotSource),
            None => Err(FileError::NotFound(id.vpath().as_rooted_path().to_owned())),
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        match self.slots.lock().unwrap().get(&id) {
            Some(slot) => Ok(slot.bytes.clone()),
            None => Err(FileError::NotFound(id.vpath().as_rooted_path().to_owned())),
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        Some(self.fonts[index].clone())
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        Some(Datetime::from_ymd(1970, 1, 1).unwrap())
    }
}
