//! Default implementations for the provider traits.
//!
//! The following implementations and related types are provided:
//! - [`ProvideDatetime`]
//!   - [`FixedDateProvider`]: A fixed-date provider, this does not provide time.
//!   - [`SystemDateProvider`]: A system-date, this does not provide time.
//! - [`ProvideFile`]
//!   - [`TemplateFileProviderShim`]: A shim around file providers which
//!     re-routes specific template accesses for template tests
//!   - [`FilesystemFileProvider`]: A filesystem based file provider with
//!   - [`FilesystemFileSlot`]: A slot within a filesystem file provider.
//!   - [`VirtualFileProvider`]: An in-memory file provider without package
//!   - [`VirtualFileSlot`]: A slot within an in-memory file provider.
//! - [`ProvideFont`]
//!   - [`FilesystemFontProvider`]: A filesystem based file provider.
//!   - [`VirtualFontProvider`]: An in-memory file provider.
//! - [`ProvideLibrary`]
//!   - [`LibraryProvider`]: A simple provider for different versions of the
//!     library.
//! - [`ProvideWorld`]
//!   - [`WorldProvider`]: A default [`World`] provider that creates
//!     [`ComposedDynWorld`] instances from the other providers.

mod datetime;
mod file;
mod font;
mod library;

pub use crate::typst::world::default::datetime::FixedDateProvider;
pub use crate::typst::world::default::datetime::SystemDateProvider;
pub use crate::typst::world::default::file::FilesystemFileProvider;
pub use crate::typst::world::default::file::FilesystemSlot;
pub use crate::typst::world::default::file::FilesystemSlotCache;
pub use crate::typst::world::default::file::VirtualFileProvider;
pub use crate::typst::world::default::file::VirtualSlot;
pub use crate::typst::world::default::font::FilesystemFontProvider;
pub use crate::typst::world::default::font::VirtualFontProvider;
pub use crate::typst::world::default::library::LibraryProvider;
