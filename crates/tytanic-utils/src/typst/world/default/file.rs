//! Default implementations for [`ProvideFile`].

use std::collections::HashMap;
use std::fmt::Debug;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use ecow::eco_format;
use typst::diag::FileError;
use typst::diag::FileResult;
use typst::diag::PackageError;
use typst::foundations::Bytes;
use typst::syntax::FileId;
use typst::syntax::Source;
use typst_kit::download::ProgressSink;
use typst_kit::package::PackageStorage;

use crate::typst::world::ProvideFile;

/// Provides access to files from memory.
#[derive(Debug)]
pub struct VirtualFileProvider {
    slots: Mutex<HashMap<FileId, VirtualSlot>>,
}

impl VirtualFileProvider {
    /// Creates a new file provider with no files.
    pub fn new() -> Self {
        Self::from_slots(HashMap::new())
    }

    /// Creates a new file provider with the given file slots.
    pub fn from_slots(slots: HashMap<FileId, VirtualSlot>) -> Self {
        Self {
            slots: Mutex::new(slots),
        }
    }
}

impl VirtualFileProvider {
    /// The slots used to store file contents.
    pub fn slots(&self) -> MutexGuard<'_, HashMap<FileId, VirtualSlot>> {
        self.slots.lock().unwrap()
    }

    /// The slots used to store file contents.
    pub fn slots_mut(&mut self) -> &mut HashMap<FileId, VirtualSlot> {
        self.slots.get_mut().unwrap()
    }
}

impl VirtualFileProvider {
    /// Access the canonical slot for the given file id.
    pub fn slot<T, F>(&self, id: FileId, f: F) -> FileResult<T>
    where
        F: FnOnce(&VirtualSlot) -> T,
    {
        let map = self.slots.lock().unwrap();
        map.get(&id)
            .map(f)
            .ok_or_else(|| FileError::NotFound(id.vpath().as_rooted_path().to_owned()))
    }
}

impl Default for VirtualFileProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ProvideFile for VirtualFileProvider {
    fn provide_source(&self, id: FileId) -> FileResult<Source> {
        self.slot(id, |slot| slot.source())?
            .ok_or_else(|| FileError::NotSource)
    }

    fn provide_bytes(&self, id: FileId) -> FileResult<Bytes> {
        self.slot(id, |slot| slot.bytes())
    }

    fn reset_all(&self) {}
}

/// Holds the processed data for a file ID.
///
/// Is eagerly populated with data (unlike [`FilesystemSlot`]).
#[derive(Debug)]
pub struct VirtualSlot {
    id: FileId,
    source: Option<Source>,
    bytes: Bytes,
}

impl VirtualSlot {
    /// Create a new source file with the given source code.
    pub fn from_source(source: Source) -> Self {
        Self {
            id: source.id(),
            bytes: Bytes::new(source.text().as_bytes().to_vec()),
            source: Some(source),
        }
    }

    /// Create a new generic file with the given bytes.
    pub fn from_bytes<T>(id: FileId, bytes: T) -> Self
    where
        T: AsRef<[u8]> + Send + Sync + 'static,
    {
        Self {
            id,
            bytes: Bytes::new(bytes),
            source: None,
        }
    }
}

impl VirtualSlot {
    /// The file id of this file.
    pub fn id(&self) -> FileId {
        self.id
    }

    /// The optional source of this file.
    pub fn source(&self) -> Option<Source> {
        self.source.clone()
    }

    /// The bytes of this file.
    pub fn bytes(&self) -> Bytes {
        self.bytes.clone()
    }
}

/// A cache for file providers to share their files.
///
/// Files are identified by their [`FileId`], this means that this should only
/// be shared between file providers for which a file id has a unique file.
#[derive(Debug, Clone)]
pub struct FilesystemSlotCache(Arc<Mutex<HashMap<FileId, FilesystemSlot>>>);

impl FilesystemSlotCache {
    /// Creates a new empty cache for file system slots.
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }
}

impl FilesystemSlotCache {
    /// Reset all slots in the cache in preparation of the next compilation.
    pub fn reset_slots(&self) {
        for slot in self.0.lock().unwrap().values_mut() {
            slot.reset();
        }
    }
}

impl Default for FilesystemSlotCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Provides access to files from the filesystem.
#[derive(Debug)]
pub struct FilesystemFileProvider {
    root: PathBuf,
    cache: FilesystemSlotCache,
    package_storage: Option<Arc<PackageStorage>>,
}

impl FilesystemFileProvider {
    /// Creates a new file provider for the given project root.
    ///
    /// The package storage will be used to download and prepare packages.
    pub fn new<P>(
        root: P,
        cache: FilesystemSlotCache,
        package_storage: Option<Arc<PackageStorage>>,
    ) -> Self
    where
        P: Into<PathBuf>,
    {
        Self {
            root: root.into(),
            cache,
            package_storage,
        }
    }
}

impl FilesystemFileProvider {
    /// The project root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The cache used to store file contents.
    pub fn cache(&self) -> &FilesystemSlotCache {
        &self.cache
    }

    /// The package storage if one is given.
    pub fn package_storage(&self) -> Option<&PackageStorage> {
        self.package_storage.as_deref()
    }
}

impl FilesystemFileProvider {
    /// Reset the slots in the provider's shared cache in preparation of the
    /// next compilation.
    pub fn reset_slots(&self) {
        self.cache.reset_slots();
    }

    /// Access the canonical slot for the given file id.
    pub fn slot<F, T>(&self, id: FileId, f: F) -> T
    where
        F: FnOnce(&mut FilesystemSlot) -> T,
    {
        let mut map = self.cache.0.lock().unwrap();
        f(map.entry(id).or_insert_with(|| FilesystemSlot::new(id)))
    }
}

impl ProvideFile for FilesystemFileProvider {
    fn provide_source(&self, id: FileId) -> FileResult<Source> {
        self.slot(id, |slot| slot.source(self.root(), self.package_storage()))
    }

    fn provide_bytes(&self, id: FileId) -> FileResult<Bytes> {
        self.slot(id, |slot| slot.bytes(self.root(), self.package_storage()))
    }

    fn reset_all(&self) {
        self.reset_slots();
    }
}

/// Holds the processed data for a file ID.
///
/// Both fields can be populated if the file is both imported and read().
#[derive(Debug)]
pub struct FilesystemSlot {
    /// The slot's file id.
    id: FileId,
    /// The lazily loaded and incrementally updated source file.
    source: SlotCell<Source>,
    /// The lazily loaded raw byte buffer.
    file: SlotCell<Bytes>,
}

impl FilesystemSlot {
    /// Create a new file slot.
    pub fn new(id: FileId) -> Self {
        Self {
            id,
            file: SlotCell::new(),
            source: SlotCell::new(),
        }
    }

    /// Marks the file as not yet accessed in preparation of the next
    /// compilation.
    pub fn reset(&mut self) {
        self.source.reset();
        self.file.reset();
    }

    /// Retrieve the source for this file.
    pub fn source(
        &mut self,
        root: &Path,
        package_storage: Option<&PackageStorage>,
    ) -> FileResult<Source> {
        self.source.get_or_init(
            || read(self.id, root, package_storage),
            |data, prev| {
                let text = decode_utf8(&data)?;
                if let Some(mut prev) = prev {
                    prev.replace(text);
                    Ok(prev)
                } else {
                    Ok(Source::new(self.id, text.into()))
                }
            },
        )
    }

    /// Retrieve the file's bytes.
    pub fn bytes(
        &mut self,
        root: &Path,
        package_storage: Option<&PackageStorage>,
    ) -> FileResult<Bytes> {
        self.file.get_or_init(
            || read(self.id, root, package_storage),
            |data, _| Ok(Bytes::new(data)),
        )
    }
}

/// Lazily processes data for a file.
#[derive(Debug)]
struct SlotCell<T> {
    /// The processed data.
    data: Option<FileResult<T>>,
    /// A hash of the raw file contents / access error.
    fingerprint: u128,
    /// Whether the slot has been accessed in the current compilation.
    accessed: bool,
}

impl<T: Clone> SlotCell<T> {
    /// Creates a new, empty cell.
    fn new() -> Self {
        Self {
            data: None,
            fingerprint: 0,
            accessed: false,
        }
    }

    /// Marks the cell as not yet accessed in preparation of the next
    /// compilation.
    fn reset(&mut self) {
        self.accessed = false;
    }

    /// Gets the contents of the cell or initialize them.
    fn get_or_init(
        &mut self,
        load: impl FnOnce() -> FileResult<Vec<u8>>,
        f: impl FnOnce(Vec<u8>, Option<T>) -> FileResult<T>,
    ) -> FileResult<T> {
        // If we accessed the file already in this compilation, retrieve it.
        if std::mem::replace(&mut self.accessed, true) {
            if let Some(data) = &self.data {
                return data.clone();
            }
        }

        // Read and hash the file.
        let result = load();
        let fingerprint = typst::utils::hash128(&result);

        // If the file contents didn't change, yield the old processed data.
        if std::mem::replace(&mut self.fingerprint, fingerprint) == fingerprint {
            if let Some(data) = &self.data {
                return data.clone();
            }
        }

        let prev = self.data.take().and_then(Result::ok);
        let value = result.and_then(|data| f(data, prev));
        self.data = Some(value.clone());

        value
    }
}

/// Resolves the path of a file id on the system, downloading a package if
/// necessary.
fn system_path(
    root: &Path,
    id: FileId,
    package_storage: Option<&PackageStorage>,
) -> FileResult<PathBuf> {
    // Determine the root path relative to which the file path
    // will be resolved.
    let buf;
    let mut root = root;

    match (id.package(), package_storage) {
        (Some(spec), Some(storage)) => {
            tracing::trace!(?spec, "preparing package");
            // TODO(tinger): Wrap this in some way that makes sense to expose on
            // the traits without pulling all of `typst-kit`.
            buf = storage.prepare_package(spec, &mut ProgressSink)?;
            root = &buf;
        }
        (Some(spec), None) => {
            tracing::error!(?spec, "cannot prepare package, no package storage provided");
            return Err(FileError::Package(PackageError::Other(Some(eco_format!(
                "cannot access package {spec}"
            )))));
        }
        (None, _) => {}
    }

    tracing::trace!(?root, ?id, "resolving system path");

    // Join the path to the root. If it tries to escape, deny
    // access. Note: It can still escape via symlinks.
    id.vpath().resolve(root).ok_or(FileError::AccessDenied)
}

/// Reads a file's contents from a file id.
///
/// If the id represents `stdin` it will read from standard input, otherwise it
/// gets the filepath of the id and reads the file from disk.
fn read(id: FileId, root: &Path, package_storage: Option<&PackageStorage>) -> FileResult<Vec<u8>> {
    read_from_disk(&system_path(root, id, package_storage)?)
}

/// Read a file from disk.
fn read_from_disk(path: &Path) -> FileResult<Vec<u8>> {
    let f = |e| FileError::from_io(e, path);
    if std::fs::metadata(path).map_err(f)?.is_dir() {
        Err(FileError::IsDirectory)
    } else {
        std::fs::read(path).map_err(f)
    }
}

/// Decode UTF-8 with an optional BOM.
fn decode_utf8(buf: &[u8]) -> FileResult<&str> {
    // Remove UTF-8 BOM.
    Ok(std::str::from_utf8(
        buf.strip_prefix(b"\xef\xbb\xbf").unwrap_or(buf),
    )?)
}
