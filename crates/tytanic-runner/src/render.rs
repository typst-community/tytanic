//! Rendering of test documents.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::sync::Mutex;
use std::sync::MutexGuard;

use tiny_skia::Pixmap;
use typst::ecow::EcoVec;
use typst::layout::Page;
use tytanic_core::analysis;
use tytanic_core::analysis::Origin;
use tytanic_core::analysis::ppi_to_ppp;
use tytanic_core::config::Direction;
use tytanic_core::config::TestConfig;
use tytanic_core::project::ProjectContext;
use tytanic_core::result::CompilationOutput;
use tytanic_core::test::Ident;
use tytanic_core::test::Test;
use tytanic_utils::forward_trait;
use uuid::Uuid;

/// A trait for rendering the artifacts of tests in the default runner.
///
/// The renderer must respect the tests rendering configuration.
pub trait Renderer: Debug + Send + Sync {
    /// Renders the primary document of the given test compilation output.
    ///
    /// The rendered output may be cached.
    fn render_primary_document(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        run_id: Uuid,
        primary: &CompilationOutput,
    ) -> EcoVec<Pixmap>;

    /// Renders the reference document of the given test compilation output.
    ///
    /// The rendered output may be cached.
    fn render_reference_document(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        run_id: Uuid,
        reference: &CompilationOutput,
    ) -> EcoVec<Pixmap>;

    /// Renders difference artifacts of the given test compilation outputs.
    ///
    /// The rendered output may be cached.
    fn render_difference_document_ephemeral(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        run_id: Uuid,
        primary: &CompilationOutput,
        reference: &CompilationOutput,
    ) -> EcoVec<Pixmap>;

    /// Renders difference artifacts of the given test compilation output and persistent
    /// reference.
    ///
    /// The rendered output may be cached.
    fn render_difference_document_persistent(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        run_id: Uuid,
        primary: &CompilationOutput,
        reference: &[Pixmap],
    ) -> EcoVec<Pixmap>;
}

forward_trait! {
    impl<R> Renderer for [std::boxed::Box<R>, std::sync::Arc<R>, &R, &mut R] {
        fn render_primary_document(
            &self,
            ctx: &ProjectContext,
            test: &Test,
            run_id: Uuid,
            primary: &CompilationOutput,
        ) -> EcoVec<Pixmap> {
            R::render_primary_document(self, ctx, test, run_id, primary)
        }

        fn render_reference_document(
            &self,
            ctx: &ProjectContext,
            test: &Test,
            run_id: Uuid,
            reference: &CompilationOutput,
        ) -> EcoVec<Pixmap> {
            R::render_reference_document(self, ctx, test, run_id, reference)
        }

        fn render_difference_document_ephemeral(
            &self,
            ctx: &ProjectContext,
            test: &Test,
            run_id: Uuid,
            primary: &CompilationOutput,
            reference: &CompilationOutput,
        ) -> EcoVec<Pixmap> {
            R::render_difference_document_ephemeral(self, ctx, test, run_id, primary, reference)
        }

        fn render_difference_document_persistent(
            &self,
            ctx: &ProjectContext,
            test: &Test,
            run_id: Uuid,
            primary: &CompilationOutput,
            reference: &[Pixmap],
        ) -> EcoVec<Pixmap> {
            R::render_difference_document_persistent(self, ctx, test, run_id, primary, reference)
        }
    }
}

/// An implementation of [`Renderer`] which caches rendered documents by their
/// test and run id.
#[derive(Debug)]
pub struct CachingRenderer {
    cache: Mutex<RenderCache>,
}

// TODO(tinger): This should be a hash of the inputs that influence the
// cache, currently this is fine because we operate under the assumption that
// the config of a test cannot change during a run. This may not be true in the
// future with `tytanic watch`.
type CacheKey = (Uuid, Ident);

#[derive(Debug)]
struct RenderCache {
    primary: HashMap<CacheKey, EcoVec<Pixmap>>,
    reference: HashMap<CacheKey, EcoVec<Pixmap>>,
    difference: HashMap<CacheKey, EcoVec<Pixmap>>,
}

impl CachingRenderer {
    /// Creates a new caching renderer.
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(RenderCache {
                primary: HashMap::new(),
                reference: HashMap::new(),
                difference: HashMap::new(),
            }),
        }
    }
}

impl CachingRenderer {
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

impl CachingRenderer {
    fn render_primary_document_inner(
        guard: &mut MutexGuard<'_, RenderCache>,
        ctx: &ProjectContext,
        test: &Test,
        run_id: Uuid,
        primary: &CompilationOutput,
    ) -> EcoVec<Pixmap> {
        match guard.primary.entry((run_id, test.ident())) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let pages = render(ctx, test, primary.pages());

                entry.insert(pages.clone());
                pages
            }
        }
    }

    fn render_reference_document_inner(
        guard: &mut MutexGuard<'_, RenderCache>,
        ctx: &ProjectContext,
        test: &Test,
        run_id: Uuid,
        reference: &CompilationOutput,
    ) -> EcoVec<Pixmap> {
        match guard.reference.entry((run_id, test.ident())) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let pages = render(ctx, test, reference.pages());

                entry.insert(pages.clone());
                pages
            }
        }
    }
}

impl Renderer for CachingRenderer {
    fn render_primary_document(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        run_id: Uuid,
        primary: &CompilationOutput,
    ) -> EcoVec<Pixmap> {
        Self::render_primary_document_inner(
            &mut self.cache.lock().unwrap(),
            ctx,
            test,
            run_id,
            primary,
        )
    }

    fn render_reference_document(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        run_id: Uuid,
        reference: &CompilationOutput,
    ) -> EcoVec<Pixmap> {
        Self::render_reference_document_inner(
            &mut self.cache.lock().unwrap(),
            ctx,
            test,
            run_id,
            reference,
        )
    }

    fn render_difference_document_ephemeral(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        run_id: Uuid,
        primary: &CompilationOutput,
        reference: &CompilationOutput,
    ) -> EcoVec<Pixmap> {
        let mut cache = self.cache.lock().unwrap();

        let primary = Self::render_primary_document_inner(&mut cache, ctx, test, run_id, primary);
        let reference =
            Self::render_reference_document_inner(&mut cache, ctx, test, run_id, reference);

        match cache.difference.entry((run_id, test.ident())) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let pages = render_diff(ctx, test, &primary, &reference);

                entry.insert(pages.clone());
                pages
            }
        }
    }

    fn render_difference_document_persistent(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        run_id: Uuid,
        primary: &CompilationOutput,
        reference: &[Pixmap],
    ) -> EcoVec<Pixmap> {
        let mut cache = self.cache.lock().unwrap();

        let primary = Self::render_primary_document_inner(&mut cache, ctx, test, run_id, primary);

        match cache.difference.entry((run_id, test.ident())) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let pages = render_diff(ctx, test, &primary, reference);

                entry.insert(pages.clone());
                pages
            }
        }
    }
}

impl Default for CachingRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Renders the output for the given test.
///
/// This function respects the test's configured [ppi].
///
/// [PPI]: tytanic_core::config::TestConfig::pixel_per_inch
#[tracing::instrument(skip_all, fields(test = %test.ident()))]
pub fn render<'a, P>(ctx: &ProjectContext, test: &Test, pages: P) -> EcoVec<Pixmap>
where
    P: IntoIterator<Item = &'a Page>,
{
    tracing::debug!("rendering document");

    let ppi = ctx.config().get_test_config_member(
        test.as_unit().and_then(|t| t.config()),
        TestConfig::PIXEL_PER_INCH,
        (),
    );

    pages
        .into_iter()
        .map(|page| typst_render::render(page, ppi_to_ppp(ppi)))
        .collect()
}

/// Renders the difference pages for the given test.
///
/// This function respects the test's configured [ppi] and [direction].
///
/// [PPI]: tytanic_core::config::TestConfig::pixel_per_inch
/// [direction]: tytanic_core::config::TestConfig::direction
#[tracing::instrument(skip_all, fields(test = %test.ident()))]
pub fn render_diff<'a, P, R>(
    ctx: &ProjectContext,
    test: &Test,
    primary: P,
    reference: R,
) -> EcoVec<Pixmap>
where
    R: IntoIterator<Item = &'a Pixmap>,
    P: IntoIterator<Item = &'a Pixmap>,
{
    tracing::debug!("rendering difference document");

    let dir = ctx.config().get_test_config_member(
        test.as_unit().and_then(|t| t.config()),
        TestConfig::DIRECTION,
        (),
    );

    analysis::render_pages_diff(
        primary,
        reference,
        match dir {
            Direction::Ltr => Origin::TopLeft,
            Direction::Rtl => Origin::TopRight,
        },
    )
}
