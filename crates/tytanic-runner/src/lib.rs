//! # `tytanic-runner`
//! This crate provides a sequential implementation for the [`Runner`] trait,
//! this implementation is used in the Tytanic CLI.

use std::sync::RwLock;

use tiny_skia::Pixmap;
use typst::World;
use typst::diag::Warned;
use typst::ecow::EcoVec;
use typst::ecow::eco_vec;
use typst::layout::PagedDocument;
use tytanic_core::analysis;
use tytanic_core::config::TestConfig;
use tytanic_core::config::Warnings;
use tytanic_core::project::ProjectContext;
use tytanic_core::project::store::ArtifactKind;
use tytanic_core::project::store::PersistentReferencesError;
use tytanic_core::result::ComparisonFailure;
use tytanic_core::result::ComparisonOutput;
use tytanic_core::result::ComparisonResult;
use tytanic_core::result::CompilationFailure;
use tytanic_core::result::CompilationOutput;
use tytanic_core::result::CompilationResult;
use tytanic_core::result::PostCompileTestStage;
use tytanic_core::result::SuiteTrace;
use tytanic_core::result::TestTrace;
use tytanic_core::runner::CancellationReason;
use tytanic_core::runner::Error;
use tytanic_core::runner::Runner;
use tytanic_core::suite::Suite;
use tytanic_core::test::Test;
use tytanic_core::test::UnitTest;
use uuid::Uuid;

use crate::export::Exporter;
use crate::provide::Provider;
use crate::render::Renderer;
use crate::report::Reporter;

pub mod export;
pub mod provide;
pub mod render;
pub mod report;

// TODO(tinger): Parallelize test runs, many tests are single-region tests such
// that Typst can't parallelize everything and we'd be wasting cores running
// it sequentially.

/// The configuration of the default runner.
#[derive(Debug)]
pub struct DefaultRunnerConfig {
    /// Instructs the runner to fail on the first test failure.
    pub fail_fast: bool,
}

/// A runner for project test suites.
#[derive(Debug)]
pub struct DefaultRunner<'a> {
    provider: &'a dyn Provider,
    reporter: &'a dyn Reporter,
    renderer: &'a dyn Renderer,
    exporter: &'a dyn Exporter,
    cancellation: RwLock<Option<CancellationReason>>,
    config: DefaultRunnerConfig,
}

impl<'a> DefaultRunner<'a> {
    /// Create a new project runner with the given world provider and
    /// cancellation token.
    pub fn new(
        provider: &'a dyn Provider,
        reporter: &'a dyn Reporter,
        renderer: &'a dyn Renderer,
        exporter: &'a dyn Exporter,
        config: DefaultRunnerConfig,
    ) -> Self {
        Self {
            provider,
            reporter,
            renderer,
            exporter,
            cancellation: RwLock::new(None),
            config,
        }
    }
}

impl DefaultRunner<'_> {
    /// Resets the cancellation.
    fn reset_cancellation(&self) {
        *self.cancellation.write().unwrap() = None;
    }

    /// Updates the cancellation reason if it has a higher priority than the
    /// current one.
    fn update_cancellation(&self, reason: CancellationReason) {
        let mut cancellation = self.cancellation.write().unwrap();

        if cancellation.is_some_and(|current| current < reason) {
            *cancellation = Some(reason);
        }
    }

    fn is_cancellation_requested(&self) -> bool {
        self.cancellation
            .read()
            .unwrap()
            .is_some_and(|reason| match reason {
                CancellationReason::TestFailed => self.config.fail_fast,
                CancellationReason::Request => true,
            })
    }
}

impl Runner for DefaultRunner<'_> {
    fn cancel(&self, reason: CancellationReason) {
        self.update_cancellation(reason);
    }

    fn reset(&self) {
        self.reset_cancellation();
        self.provider.reset();
    }

    fn run_suite(
        &self,
        ctx: &ProjectContext,
        suite: &Suite,
        run_id: Uuid,
        update: bool,
    ) -> Result<SuiteTrace, Error> {
        let mut trace = SuiteTrace::unfinished(run_id);

        trace.record_start();
        tracing::info!("starting suite run");
        self.reporter
            .report_suite_started(ctx, suite, run_id, trace.start())
            .map_err(|error| Error::Other(error.into()))?;

        tracing::info_span!("run_suite").entered().in_scope(|| {
            self.run_suite_inner(ctx, suite, update, &mut trace)
                .map_err(|error| Error::Other(error.into()))
        })?;

        trace.record_end();
        tracing::info!(
            %run_id,
            tests = suite.matched_len(),
            filtered = suite.filtered_len(),
            duration = %{
                let duration = trace.duration();
                format!(
                    "{:02}:{:02}.{:09}",
                    duration.num_minutes(),
                    duration.num_seconds(),
                    duration.subsec_nanos(),
                )
            },
            "finished suite run",
        );
        self.reporter
            .report_suite_finished(ctx, suite, &trace)
            .map_err(|error| Error::Other(error.into()))?;

        Ok(trace)
    }

    fn run_test(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        run_id: Uuid,
        update: bool,
    ) -> Result<TestTrace, Error> {
        let compare =
            ctx.config()
                .get_test_config_member(test.config(), TestConfig::COMPARE, test.kind());

        let mut trace = TestTrace::unfinished(
            test.kind(),
            if update {
                PostCompileTestStage::PersistentUpdate
            } else if compare {
                PostCompileTestStage::Comparison
            } else {
                PostCompileTestStage::Cleanup
            },
        );

        trace.record_start();
        tracing::info!(test = %test.ident(), kind = ?test.kind(), "starting test run");
        self.reporter
            .report_test_started(ctx, test, trace.start())
            .map_err(|error| Error::Other(error.into()))?;

        tracing::info_span!("run_test").entered().in_scope(|| {
            self.run_test_inner(run_id, ctx, test, compare, update, &mut trace)
                .map_err(|error| match error {
                    Error::Other(error) => Error::Test {
                        test: test.ident(),
                        source: error,
                    },
                    other => other,
                })
        })?;

        trace.record_end();
        tracing::info!(test = %test.ident(), status = ?trace.kind(), "finished test run");
        self.reporter
            .report_test_finished(ctx, test, &trace)
            .map_err(|error| Error::Other(error.into()))?;

        Ok(trace)
    }
}

impl DefaultRunner<'_> {
    fn run_suite_inner(
        &self,
        ctx: &ProjectContext,
        suite: &Suite,
        update: bool,
        trace: &mut SuiteTrace,
    ) -> Result<(), Error> {
        if self.is_cancellation_requested() {
            tracing::warn!("suite run cancelled before it started");
            return Ok(());
        }

        tracing::trace!(
            filtered = suite.filtered_len(),
            "pre-filling filtered test traces",
        );
        for test in suite.tests_filtered() {
            trace.record_test_trace(test.ident(), None);
        }

        tracing::trace!(test = suite.matched_len(), "running tests");
        let mut tests = suite.tests();
        for test in &mut tests {
            if self.is_cancellation_requested() {
                break;
            }

            let test_trace = self.run_test(ctx, test, trace.run_id(), update)?;

            if test_trace.kind().is_failed() {
                self.update_cancellation(CancellationReason::TestFailed);
            }

            trace.record_test_trace(test.ident(), Some(Box::new(test_trace)));
        }

        tracing::trace!(
            skipped = suite.matched_len() - trace.len(),
            "post-filling skipped test traces",
        );
        for test in &mut tests {
            let compare = ctx.config().get_test_config_member(
                test.config(),
                TestConfig::COMPARE,
                test.kind(),
            );

            let test_trace = TestTrace::unfinished(
                test.kind(),
                if update {
                    PostCompileTestStage::PersistentUpdate
                } else if compare {
                    PostCompileTestStage::Comparison
                } else {
                    PostCompileTestStage::Cleanup
                },
            );
            trace.record_test_trace(test.ident(), Some(Box::new(test_trace)));
        }

        Ok(())
    }

    fn run_test_inner(
        &self,
        run_id: Uuid,
        ctx: &ProjectContext,
        test: &Test,
        compare: bool,
        update: bool,
        trace: &mut TestTrace,
    ) -> Result<(), Error> {
        tracing::debug!("running test stage prepare");
        self.reporter
            .report_test_stage_prepare(ctx, test, true)
            .map_err(|error| Error::Other(error.into()))?;
        trace.record_prepare(true);

        'run: {
            match test {
                Test::Template(_) => {
                    if self.is_cancellation_requested() {
                        break 'run;
                    }

                    let result = run_compilation_stage(
                        self.provider,
                        self.reporter,
                        self.renderer,
                        self.exporter,
                        ctx,
                        test,
                        run_id,
                        true,
                    )?;

                    trace.record_primary_compilation(result);
                }
                Test::Unit(UnitTest::CompileOnly(_)) => {
                    if self.is_cancellation_requested() {
                        break 'run;
                    }

                    let result = run_compilation_stage(
                        self.provider,
                        self.reporter,
                        self.renderer,
                        self.exporter,
                        ctx,
                        test,
                        run_id,
                        true,
                    )?;

                    trace.record_primary_compilation(result);
                }
                Test::Unit(UnitTest::Ephemeral(_)) => {
                    if self.is_cancellation_requested() {
                        break 'run;
                    }

                    let reference = run_compilation_stage(
                        self.provider,
                        self.reporter,
                        self.renderer,
                        self.exporter,
                        ctx,
                        test,
                        run_id,
                        false,
                    )?;

                    if self.is_cancellation_requested() {
                        trace.record_reference_compilation(reference);
                        break 'run;
                    }

                    let primary = run_compilation_stage(
                        self.provider,
                        self.reporter,
                        self.renderer,
                        self.exporter,
                        ctx,
                        test,
                        run_id,
                        true,
                    )?;

                    if !compare || self.is_cancellation_requested() {
                        trace.record_reference_compilation(reference);
                        trace.record_primary_compilation(primary);
                        break 'run;
                    }

                    match (primary, reference) {
                        (
                            CompilationResult::Passed(primary),
                            CompilationResult::Passed(reference),
                        ) => {
                            let result = run_ephemeral_comparison_stage(
                                self.reporter,
                                self.renderer,
                                self.exporter,
                                ctx,
                                test,
                                run_id,
                                &primary,
                                &reference,
                            )?;

                            trace
                                .record_reference_compilation(CompilationResult::Passed(reference));
                            trace.record_primary_compilation(CompilationResult::Passed(primary));

                            trace.record_comparison(result);
                        }
                        (primary, reference) => {
                            tracing::debug!(
                                "skipping test stage comparison due to previous failure",
                            );
                            trace.record_reference_compilation(reference);
                            trace.record_primary_compilation(primary);
                        }
                    }
                }
                Test::Unit(UnitTest::Persistent(_)) => {
                    if self.is_cancellation_requested() {
                        break 'run;
                    }

                    let result = run_compilation_stage(
                        self.provider,
                        self.reporter,
                        self.renderer,
                        self.exporter,
                        ctx,
                        test,
                        run_id,
                        true,
                    )?;

                    if !compare || self.is_cancellation_requested() {
                        trace.record_primary_compilation(result);
                        break 'run;
                    }

                    if let CompilationResult::Passed(primary) = result {
                        let reference = read_persistent_artifacts(ctx, test)
                            .map_err(|error| Error::Other(error.into()))?;

                        let compare_result = run_persistent_comparison_stage(
                            self.reporter,
                            self.renderer,
                            self.exporter,
                            ctx,
                            test,
                            run_id,
                            &primary,
                            &reference,
                        )?;

                        self.reporter
                            .report_test_stage_comparison(ctx, test, &compare_result)
                            .map_err(|error| Error::Other(error.into()))?;

                        if !update || self.is_cancellation_requested() {
                            trace.record_primary_compilation(CompilationResult::Passed(primary));
                            trace.record_comparison(compare_result);
                            break 'run;
                        }

                        if compare_result.is_failed() {
                            let update_result = run_persistent_update_stage(
                                self.reporter,
                                self.renderer,
                                self.exporter,
                                ctx,
                                test,
                                run_id,
                                &primary,
                            )?;

                            trace.record_primary_compilation(CompilationResult::Passed(primary));
                            trace.record_comparison(compare_result);
                            trace.record_update(update_result);
                        } else {
                            tracing::debug!("skipping test stage update, output matches reference");
                            trace.record_primary_compilation(CompilationResult::Passed(primary));
                            trace.record_comparison(compare_result);
                            trace.record_update(false);
                        }
                    } else {
                        tracing::debug!("skipping test stage comparison due to previous failure");
                        trace.record_primary_compilation(result);
                    }
                }
                Test::Doc(_) => {
                    if self.is_cancellation_requested() {
                        break 'run;
                    }

                    let result = run_compilation_stage(
                        self.provider,
                        self.reporter,
                        self.renderer,
                        self.exporter,
                        ctx,
                        test,
                        run_id,
                        true,
                    )?;

                    trace.record_primary_compilation(result);
                }
            }
        }

        tracing::debug!("running test stage cleanup");
        self.reporter
            .report_test_stage_cleanup(ctx, test, true)
            .map_err(|error| Error::Other(error.into()))?;

        trace.record_cleanup(true);

        Ok(())
    }
}

/// Runs a compilation stage for the given test, renders the output and stores
/// the temporary artifacts.
#[expect(
    clippy::too_many_arguments,
    reason = "this is a low level generic helper implementation"
)]
pub fn run_compilation_stage(
    provider: &dyn Provider,
    reporter: &dyn Reporter,
    renderer: &dyn Renderer,
    exporter: &dyn Exporter,
    ctx: &ProjectContext,
    test: &Test,
    run_id: Uuid,
    is_primary: bool,
) -> Result<CompilationResult, Error> {
    let (name, kind) = if is_primary {
        ("primary", ArtifactKind::Primary)
    } else {
        ("reference", ArtifactKind::Reference)
    };

    tracing::debug!("running test stage {name} compilation");

    let world = provider.provide(ctx, test, is_primary);
    let result = compile(ctx, test, &*world);

    if let CompilationResult::Passed(output) = &result {
        let output = if is_primary {
            renderer.render_primary_document(ctx, test, run_id, output)
        } else {
            renderer.render_reference_document(ctx, test, run_id, output)
        };

        exporter
            .export_temporary_artifacts(ctx, test, run_id, kind, &output)
            .map_err(|error| Error::Other(error.into()))?;
    }

    if is_primary {
        reporter
            .report_test_stage_primary_compilation(ctx, test, &result)
            .map_err(|error| Error::Other(error.into()))?;
    } else {
        reporter
            .report_test_stage_reference_compilation(ctx, test, &result)
            .map_err(|error| Error::Other(error.into()))?;
    }

    Ok(result)
}

/// Runs a comparison stage for the given ephemeral test, creates stores the
/// diff artifacts.
#[expect(
    clippy::too_many_arguments,
    reason = "this is a low level generic helper implementation"
)]
pub fn run_ephemeral_comparison_stage(
    reporter: &dyn Reporter,
    renderer: &dyn Renderer,
    exporter: &dyn Exporter,
    ctx: &ProjectContext,
    test: &Test,
    run_id: Uuid,
    primary: &CompilationOutput,
    reference: &CompilationOutput,
) -> Result<ComparisonResult, Error> {
    tracing::debug!("running test stage comparison");

    let difference =
        renderer.render_difference_document_ephemeral(ctx, test, run_id, primary, reference);

    exporter
        .export_temporary_artifacts(ctx, test, run_id, ArtifactKind::Difference, &difference)
        .map_err(|error| Error::Other(error.into()))?;

    let primary = renderer.render_primary_document(ctx, test, run_id, primary);
    let reference = renderer.render_reference_document(ctx, test, run_id, reference);

    let result = compare(ctx, test, &primary, &reference);

    reporter
        .report_test_stage_comparison(ctx, test, &result)
        .map_err(|error| Error::Other(error.into()))?;

    Ok(result)
}

/// Runs a comparison stage for the given persistent test, creates stores the
/// diff artifacts.
#[expect(
    clippy::too_many_arguments,
    reason = "this is a low level generic helper implementation"
)]
pub fn run_persistent_comparison_stage(
    reporter: &dyn Reporter,
    renderer: &dyn Renderer,
    exporter: &dyn Exporter,
    ctx: &ProjectContext,
    test: &Test,
    run_id: Uuid,
    primary: &CompilationOutput,
    reference: &[Pixmap],
) -> Result<ComparisonResult, Error> {
    tracing::debug!("running test stage comparison");

    let difference =
        renderer.render_difference_document_persistent(ctx, test, run_id, primary, reference);

    exporter
        .export_temporary_artifacts(ctx, test, run_id, ArtifactKind::Difference, &difference)
        .map_err(|error| Error::Other(error.into()))?;

    let primary = renderer.render_primary_document(ctx, test, run_id, primary);

    let result = compare(ctx, test, &primary, reference);

    reporter
        .report_test_stage_comparison(ctx, test, &result)
        .map_err(|error| Error::Other(error.into()))?;

    Ok(result)
}

/// Runs update stage for the given persistent test.
pub fn run_persistent_update_stage(
    reporter: &dyn Reporter,
    renderer: &dyn Renderer,
    exporter: &dyn Exporter,
    ctx: &ProjectContext,
    test: &Test,
    run_id: Uuid,
    primary: &CompilationOutput,
) -> Result<bool, Error> {
    tracing::debug!("running test stage update");

    let primary = renderer.render_primary_document(ctx, test, run_id, primary);

    exporter
        .export_persistent_references(ctx, test, run_id, &primary)
        .map_err(|error| Error::Other(error.into()))?;

    reporter
        .report_test_stage_update(ctx, test, true)
        .map_err(|error| Error::Other(error.into()))?;

    Ok(true)
}

/// Runs a test compilation with the given test world.
///
/// Warnings may be ignored, emitted or promoted according to the test's
/// configuration. The `world` should likewise be resolved in accordance to the
/// test's other configuration.
pub fn compile(ctx: &ProjectContext, test: &Test, world: &dyn World) -> CompilationResult {
    tracing::debug!("compiling document");

    let Warned {
        output,
        mut warnings,
    } = typst::compile::<PagedDocument>(world);

    let config = ctx.config().get_test_config_member(
        test.as_unit().and_then(|t| t.config()),
        TestConfig::WARNINGS,
        (),
    );

    match config {
        Warnings::Ignore => {
            tracing::trace!(warnings = ?warnings.len(), "ignoring warnings");
            warnings.clear()
        }
        Warnings::Emit => {}
        Warnings::Promote => {
            return match output {
                Ok(document) => {
                    if warnings.is_empty() {
                        CompilationResult::Passed(CompilationOutput::new(document.pages, []))
                    } else {
                        tracing::trace!(
                            warnings = ?warnings.len(),
                            "promoting warnings and discarding document",
                        );
                        CompilationResult::Failed(CompilationFailure::new(warnings, []))
                    }
                }
                Err(mut errors) => CompilationResult::Failed(CompilationFailure::new(
                    {
                        tracing::trace!(
                            errors = ?errors.len(),
                            warnings = ?warnings.len(),
                            "promoting warnings to errors",
                        );
                        errors.extend(warnings);
                        errors
                    },
                    [],
                )),
            };
        }
    }

    match output {
        Ok(document) => CompilationResult::Passed(CompilationOutput::new(document.pages, warnings)),
        Err(errors) => CompilationResult::Failed(CompilationFailure::new(errors, warnings)),
    }
}

/// Runs a test comparison with the given primary and reference pages.
///
/// The comparison is done according to the test's configuration.
#[tracing::instrument(skip_all)]
pub fn compare<'a, P, R>(
    ctx: &ProjectContext,
    test: &Test,
    primary: P,
    reference: R,
) -> ComparisonResult
where
    R: IntoIterator<Item = &'a Pixmap>,
    P: IntoIterator<Item = &'a Pixmap>,
{
    tracing::debug!("comparing test documents");

    let max_delta = ctx.config().get_test_config_member(
        test.as_unit().and_then(|t| t.config()),
        TestConfig::MAX_DELTA,
        (),
    );

    let max_deviations = ctx.config().get_test_config_member(
        test.as_unit().and_then(|t| t.config()),
        TestConfig::MAX_DEVIATIONS,
        (),
    );

    match analysis::compare_pages_simple(primary, reference, max_delta, max_deviations) {
        Ok(()) => ComparisonResult::Passed(ComparisonOutput::new()),
        Err(error) => ComparisonResult::Failed(ComparisonFailure::new(error)),
    }
}

/// Reads the persistent artifacts for the given test.
#[tracing::instrument(skip_all, fields(test = %test.ident()))]
pub fn read_persistent_artifacts(
    ctx: &ProjectContext,
    test: &Test,
) -> Result<EcoVec<Pixmap>, ArtifactError> {
    let mut pages = eco_vec![];

    for page in ctx.store().unit_persistent_references(test)? {
        pages.push(Pixmap::load_png(page)?);
    }

    Ok(pages)
}

/// An error that may occur when trying to read or write artifacts.
#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    /// The export of artifacts failed.
    #[error("the export of artifacts failed")]
    Export(#[from] export::Error),

    /// The on-disk references could not be accessed.
    #[error("the on-disk references could not be accessed")]
    Store(#[from] PersistentReferencesError),

    /// The on-disk references could not be decoded.
    #[error("the on-disk references could not be decoded")]
    Decode(#[from] png::DecodingError),

    /// The on-disk references could not be encoded.
    #[error("the on-disk references could not be encoded")]
    Encode(#[from] png::EncodingError),
}

// TODO: tests
