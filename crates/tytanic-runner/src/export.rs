//! Exporting of ephemeral artifacts and persistent references.

use std::collections::HashSet;
use std::fmt::Debug;
use std::io;
use std::sync::Mutex;

use thiserror::Error;
use tiny_skia::Pixmap;
use tytanic_core::project::ProjectContext;
use tytanic_core::project::store::ArtifactKind;
use tytanic_core::project::store::UnsupportedError;
use tytanic_core::test::Ident;
use tytanic_core::test::Test;
use tytanic_utils::forward_trait;
use tytanic_utils::result::PathError;
use tytanic_utils::result::ResultEx;
use uuid::Uuid;

/// A trait for exporting the artifacts of tests in the default runner.
///
/// The exporter must store persistent reference artifacts in the artifact store
/// using the pattern `{n}.png` where `n` is the 1-based page index. It may
/// optimize the persistent references.
pub trait Exporter: Debug + Send + Sync {
    /// Exports the temporary artifacts of the given test.
    ///
    /// This is used to store the temporary output of a test in the store. The
    /// export maybe cached.
    fn export_temporary_artifacts(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        run_id: Uuid,
        kind: ArtifactKind,
        pages: &[Pixmap],
    ) -> Result<(), Error>;

    /// Exports the persistent references of the given test output.
    ///
    /// This is used to update the persistent references of a test in the store,
    /// and because of this, should receive the primary artifacts of a test.
    /// These references may be optimized when exporting and the export maybe
    /// cached.
    fn export_persistent_references(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        run_id: Uuid,
        pages: &[Pixmap],
    ) -> Result<(), Error>;
}

forward_trait! {
    impl<R> Exporter for [std::boxed::Box<R>, std::sync::Arc<R>, &R, &mut R] {
        fn export_temporary_artifacts(
            &self,
            ctx: &ProjectContext,
            test: &Test,
            run_id: Uuid,
            kind: ArtifactKind,
            pages: &[Pixmap],
        ) -> Result<(), Error> {
            R::export_temporary_artifacts(self, ctx, test, run_id, kind, pages)
        }

        fn export_persistent_references(
            &self,
            ctx: &ProjectContext,
            test: &Test,
            run_id: Uuid,
            pages: &[Pixmap],
        ) -> Result<(), Error> {
            R::export_persistent_references(self, ctx, test, run_id, pages)
        }
    }
}

/// An implementation of [`Exporter`] which caches exporting artifacts by their
/// test and run id.
#[derive(Debug)]
pub struct CachingExporter {
    cache: Mutex<ExportCache>,
    export_temporary_artifacts: bool,
    optimize_options: Option<Box<oxipng::Options>>,
}

// TODO(tinger): This should be a hash of the inputs that influence the cache,
// currently this is fine because we operate under the assumption that the
// config of a test cannot change during a run. This may not be true in the
// future with `tytanic watch`.
type CacheKey = (Uuid, Ident);

#[derive(Debug)]
struct ExportCache {
    primary: HashSet<CacheKey>,
    reference: HashSet<CacheKey>,
    difference: HashSet<CacheKey>,
}

impl CachingExporter {
    /// Creates a new exporter.
    pub fn new<I>(export_temporary_artifacts: bool, optimize_options: I) -> Self
    where
        I: Into<Option<Box<oxipng::Options>>>,
    {
        Self {
            cache: Mutex::new(ExportCache {
                primary: HashSet::new(),
                reference: HashSet::new(),
                difference: HashSet::new(),
            }),
            export_temporary_artifacts,
            optimize_options: optimize_options.into(),
        }
    }
}

impl CachingExporter {
    /// Reset the cache.
    ///
    /// This is useful when the input to the associated runner changed.
    pub fn reset(&self) {
        let mut cache = self.cache.lock().unwrap();

        cache.primary.clear();
        cache.reference.clear();
        cache.difference.clear();
    }
}

impl Exporter for CachingExporter {
    fn export_temporary_artifacts(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        run_id: Uuid,
        kind: ArtifactKind,
        pages: &[Pixmap],
    ) -> Result<(), Error> {
        if self.export_temporary_artifacts {
            write_temporary_artifacts(ctx, test, run_id, kind, pages)?;
        } else {
            tracing::trace!("skipping writing temporary artifacts");
        }

        Ok(())
    }

    fn export_persistent_references(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        _run_id: Uuid,
        pages: &[Pixmap],
    ) -> Result<(), Error> {
        if let Some(optimize_options) = &self.optimize_options {
            write_persistent_references_optimized(ctx, test, pages, optimize_options)?;
        } else {
            write_persistent_references(ctx, test, pages)?;
        }

        Ok(())
    }
}

/// Writes the temporary artifacts for the given test and run id.
#[tracing::instrument(skip_all, fields(test = %test.ident(), ?kind))]
pub fn write_temporary_artifacts<'a, P>(
    ctx: &ProjectContext,
    test: &Test,
    run_id: Uuid,
    kind: ArtifactKind,
    pages: P,
) -> Result<(), Error>
where
    P: IntoIterator<Item = &'a Pixmap>,
{
    tracing::debug!("writing temporary artifacts");

    let dir = ctx.store().artifact_dir(run_id, test, kind)?;
    std::fs::create_dir_all(&dir).path_with(|| dir.to_path_buf())?;

    for (n, page) in pages.into_iter().enumerate() {
        page.save_png(dir.join(format!("{}.png", n + 1)))?;
    }

    Ok(())
}

/// Writes the persistent reference document for the given test without
/// optimizing it.
///
/// See [`write_persistent_references_optimized`] for a version which optimizes
/// the reference document.
#[tracing::instrument(skip_all, fields(test = %test.ident()))]
pub fn write_persistent_references<'a, P>(
    ctx: &ProjectContext,
    test: &Test,
    pages: P,
) -> Result<(), Error>
where
    P: IntoIterator<Item = &'a Pixmap>,
{
    tracing::debug!("writing unoptimized persistent references");

    let dir = ctx.store().persistent_reference_dir(test)?;
    std::fs::create_dir_all(&dir).path_with(|| dir.to_path_buf())?;

    for (n, page) in pages.into_iter().enumerate() {
        page.save_png(dir.join(format!("{}.png", n + 1)))?;
    }

    Ok(())
}

/// Writes the persistent references document for the given test and optimizes
/// it for minimal size.
///
/// See [`write_persistent_references`] for a version which doesn't optimize the
/// reference document.
#[tracing::instrument(skip_all, fields(test = %test.ident()))]
pub fn write_persistent_references_optimized<'a, P>(
    ctx: &ProjectContext,
    test: &Test,
    pages: P,
    optimize_options: &oxipng::Options,
) -> Result<(), Error>
where
    P: IntoIterator<Item = &'a Pixmap>,
{
    tracing::debug!("writing optimized persistent references");

    let dir = ctx.store().persistent_reference_dir(test)?;
    std::fs::create_dir_all(&dir).path_with(|| dir.to_path_buf())?;

    for (n, page) in pages.into_iter().enumerate() {
        let encoded = page.encode_png()?;
        let optimized = oxipng::optimize_from_memory(&encoded, optimize_options)?;
        let path = dir.join(format!("{}.png", n + 1));
        std::fs::write(&path, optimized).path_with(|| path)?;
    }

    Ok(())
}

/// Returned by the methods on [`Exporter`].
#[derive(Debug, Error)]
pub enum Error {
    /// The legacy store does not support the operation.
    #[error("the legacy store does not support the operation")]
    LegacyStore(#[from] UnsupportedError),

    /// A page could not be optimized.
    #[error("a page could not be optimized")]
    Optimize(#[from] oxipng::PngError),

    /// A page could not be encoded.
    #[error("a page could not be encoded")]
    Encode(#[from] png::EncodingError),

    /// An IO error occurred.
    #[error("an IO error occured")]
    Io(#[from] PathError<io::Error>),

    /// A catch-all variant for user implementations.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}
