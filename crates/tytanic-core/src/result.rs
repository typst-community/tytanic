//! Stages represent the units of work done per test.
//!
//! # Stages
//! When a runner runs a test suite it may decide to run test in any order, but
//! it must run the stages of individual tests in a pre-determined order.
//!
//! ## `prepare`
//! The prepare stage is the first stage of every test and is always run. If it
//! fails the test is likewise considered failed.
//!
//! ## `compilation`
//! The compilation stages are run right after the `prepare` stage and produce
//! the first result. These come in two kinds, one of which is optional and
//! depend on the test kind.
//!
//! ### `reference compilation`
//! This compilation stage is run first if the test has ephemeral references,
//! i.e. if the reference output is compiled and rendered on the fly. This stage
//! is skipped for any other tests.
//!
//! ### `primary compilation`
//! This stage is always run.
//!
//! ## `comparison`
//! This stage is only run for tests with references, it is skipped for
//! compile-only tests.
//!
//! ## `update`
//! This stage is only run for tests with persistent references, it is skipped
//! for compile-only and ephemeral tests.
//!
//! ## `cleanup`
//! This stage is always run if the `prepare` stage passed, regardless of other
//! intermediate failures.
//!
//! # Traces
//! Traces store the results of test runs and provide convenience functions and
//! serialization into common test report formats.
//!
//! # Examples
//! Traces can be built while a test run is in progress.
//! ```
//! # use ecow::eco_vec;
//! # use typst::diag::SourceDiagnostic;
//! # use typst_syntax::Span;
//! # use tytanic_core::result::CompilationFailure;
//! # use tytanic_core::result::CompilationResult;
//! # use tytanic_core::result::PostCompileTestStage;
//! # use tytanic_core::result::SuiteTrace;
//! # use tytanic_core::result::TestTrace;
//! # use tytanic_core::test::Ident;
//! # use tytanic_core::test::Kind;
//! # use tytanic_core::test::UnitKind;
//! # use uuid::Uuid;
//! let mut trace = SuiteTrace::unfinished(Uuid::new_v4());
//! trace.record_start();
//!
//! // A filtered test recieves no trace, as opposed to a skipped trace which
//! // shoudl be initialized to be unfinished.
//! trace.record_test_trace(Ident::new("foo/test")?, None);
//!
//! // A test trace may be added for non-filtered test by recording the result
//! // for each stage until it is either done or failed.
//! {
//!     let mut test_trace = TestTrace::unfinished(
//!         Kind::Unit(UnitKind::CompileOnly),
//!         PostCompileTestStage::Cleanup,
//!     );
//!     test_trace.record_start();
//!     test_trace.record_prepare(true);
//!     test_trace.record_primary_compilation(CompilationResult::Failed(
//!         CompilationFailure::new(
//!             eco_vec![SourceDiagnostic::error(Span::detached(), "oops!")],
//!             eco_vec![],
//!         ),
//!     ));
//!     test_trace.record_cleanup(true);
//!     test_trace.record_end();
//!     trace.record_test_trace(
//!         Ident::new("foo/bar/test")?,
//!         Some(Box::new(test_trace)),
//!     );
//! }
//! trace.record_end();
//!
//! assert_eq!(trace.len(), 2);
//! assert_eq!(trace.filtered(), 1);
//! assert_eq!(trace.passed(), 0);
//! assert_eq!(trace.failed(), 1);
//! # Ok::<_, Box<dyn std::error::Error>>(())
//! ```

use std::collections::BTreeMap;
use std::collections::btree_map::Iter;

use chrono::DateTime;
use chrono::TimeDelta;
use chrono::Utc;
use ecow::EcoVec;
use typst::diag::SourceDiagnostic;
use typst::diag::SourceResult;
use typst::diag::Warned;
use typst::layout::Page;
use typst::layout::PagedDocument;
use uuid::Uuid;

use crate::analysis;
use crate::test::Ident;
use crate::test::Kind as TestKind;

/// The result of a suite run.
#[derive(Debug, Default, Clone)]
pub struct SuiteTrace {
    run_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,

    filtered: usize,
    skipped: usize,
    passed: usize,
    failed: usize,

    traces: BTreeMap<Ident, Option<Box<TestTrace>>>,
}

impl SuiteTrace {
    /// Returns an unfinished suite trace during a run.
    pub fn unfinished(run_id: Uuid) -> SuiteTrace {
        let start = Utc::now();

        Self {
            run_id,
            start,
            end: start,
            filtered: 0,
            skipped: 0,
            passed: 0,
            failed: 0,
            traces: BTreeMap::new(),
        }
    }
}

impl SuiteTrace {
    /// The unique id of the suite run.
    pub fn run_id(&self) -> Uuid {
        self.run_id
    }

    /// The start timestamp of the suite run.
    pub fn start(&self) -> DateTime<Utc> {
        self.start
    }

    /// The end timestamp of the suite run.
    pub fn end(&self) -> DateTime<Utc> {
        self.end
    }

    /// The duration of the entire run.
    pub fn duration(&self) -> TimeDelta {
        self.end.signed_duration_since(self.start)
    }

    /// The total amount of test traces in this suite trace.
    ///
    /// This is equal to `filtered + unfinished + passed + failed`.
    pub fn len(&self) -> usize {
        self.traces.len()
    }

    /// Whether the suite trace had any tests traces at all.
    pub fn is_empty(&self) -> bool {
        self.traces.is_empty()
    }

    /// The amount of tests that which were filtered out.
    pub fn filtered(&self) -> usize {
        self.filtered
    }

    /// The amount of tests that which were skipped.
    pub fn skipped(&self) -> usize {
        self.skipped
    }

    /// The amount of tests which passed.
    pub fn passed(&self) -> usize {
        self.passed
    }

    /// The amount of tests which failed.
    pub fn failed(&self) -> usize {
        self.failed
    }

    /// Returns an iterator over the test traces.
    pub fn test_traces(&self) -> TestTraces<'_> {
        TestTraces {
            iter: self.traces.iter(),
        }
    }
}

impl SuiteTrace {
    /// Records the start time of the trace.
    pub fn record_start(&mut self) {
        tracing::trace!("recording trace suite start time");

        self.start = Utc::now();
    }

    /// Records a test trace for this suite result.
    pub fn record_test_trace(&mut self, ident: Ident, trace: Option<Box<TestTrace>>) {
        tracing::trace!(
            kind = ?trace.as_ref().map(|t| t.kind()),
            "recording test trace",
        );

        debug_assert!(
            !self.traces.contains_key(&ident),
            "tried to record test trace more than once",
        );

        if let Some(trace) = &trace {
            match trace.kind() {
                Kind::Unfinished => self.skipped += 1,
                Kind::Passed => self.passed += 1,
                Kind::Failed => self.failed += 1,
            }
        } else {
            self.filtered += 1;
        }

        self.traces.insert(ident, trace);
    }

    /// Records the end time of the trace.
    pub fn record_end(&mut self) {
        tracing::trace!("recording trace suite end time");

        self.end = Utc::now();
    }
}

/// An iterator returned by [`SuiteTrace::test_traces`]
#[derive(Debug)]
pub struct TestTraces<'a> {
    iter: Iter<'a, Ident, Option<Box<TestTrace>>>,
}

impl<'a> Iterator for TestTraces<'a> {
    type Item = Option<&'a TestTrace>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(_, trace)| trace.as_deref())
    }
}

/// A set of the results at all stages of a single test run.
///
/// This struct is used to record results in stages and to correctly chose the
/// next stage depending on the previous stages and results.
///
/// This struct is fairly large, it is recommended to use it behind some
/// indirection.
#[derive(Debug, Clone)]
pub struct TestTrace {
    start: DateTime<Utc>,
    end: DateTime<Utc>,

    prepare: Option<bool>,
    reference_compilation: Option<CompilationResult>,
    primary_compilation: Option<CompilationResult>,
    comparison: Option<ComparisonResult>,
    update: Option<bool>,
    cleanup: Option<bool>,

    expect_reference_compilation: bool,
    expect_comparison: bool,
    expect_update: bool,
}

impl TestTrace {
    /// Returns an unfinished test trace
    ///
    /// `post_compile` configures whether the `comparison`, `update` or
    /// `cleanup` stage should be run for test kinds that expect it.
    pub fn unfinished(kind: TestKind, post_compile: PostCompileTestStage) -> Self {
        let (expect_reference_compilation, expect_comparison, expect_update) = match kind {
            TestKind::Unit(unit) => {
                let (expect_comparison, expect_update) = match post_compile {
                    PostCompileTestStage::Comparison => (!unit.is_compile_only(), false),
                    PostCompileTestStage::PersistentUpdate => {
                        (unit.is_persistent(), unit.is_persistent())
                    }
                    PostCompileTestStage::Cleanup => (false, false),
                };

                (unit.is_ephemeral(), expect_comparison, expect_update)
            }
            _ => (false, false, false),
        };

        let start = Utc::now();

        Self {
            start,
            end: start,

            prepare: None,
            reference_compilation: None,
            primary_compilation: None,
            comparison: None,
            update: None,
            cleanup: None,
            expect_reference_compilation,
            expect_comparison,
            expect_update,
        }
    }
}

impl TestTrace {
    /// The start timestamp of the test run.
    pub fn start(&self) -> DateTime<Utc> {
        self.start
    }

    /// The end timestamp of the test run.
    pub fn end(&self) -> DateTime<Utc> {
        self.end
    }

    /// The duration of the test run.
    pub fn duration(&self) -> TimeDelta {
        self.end.signed_duration_since(self.start)
    }
}

impl TestTrace {
    /// The result of the `prepare` stage.
    pub fn prepare(&self) -> Option<bool> {
        self.prepare
    }

    /// The result of the `reference compilation` stage.
    pub fn reference_compilation(&self) -> Option<&CompilationResult> {
        self.reference_compilation.as_ref()
    }

    /// The result of the `primary compilation` stage.
    pub fn primary_compilation(&self) -> Option<&CompilationResult> {
        self.primary_compilation.as_ref()
    }

    /// The result of the `comparison` stage.
    pub fn comparison(&self) -> Option<&ComparisonResult> {
        self.comparison.as_ref()
    }

    /// The result of the `update` stage, whether it was run (`true`) or skipped
    /// (`false`).
    pub fn update(&self) -> Option<bool> {
        self.update
    }

    /// The result of the `cleanup` stage.
    pub fn cleanup(&self) -> Option<bool> {
        self.cleanup
    }

    /// Whether the `reference compilation` stage was expected to run.
    pub fn expect_reference_compilation(&self) -> bool {
        self.expect_reference_compilation
    }

    /// Whether the `comparison` stage was expected to run.
    pub fn expect_comparison(&self) -> bool {
        self.expect_comparison
    }

    /// Whether the `update` stage was expected to run.
    pub fn expect_update(&self) -> bool {
        self.expect_update
    }
}

impl TestTrace {
    /// Returns the number of stages expected in the test run.
    ///
    /// This is always in the range `3..=6`.
    pub fn total_stages(&self) -> usize {
        let mut total = 3;

        if self.expect_reference_compilation {
            total += 1;
        }

        if self.expect_update {
            total += 1;
        }

        if self.expect_comparison {
            total += 1;
        }

        total
    }

    /// Returns the number of stages the test run as completed.
    pub fn finished_stages(&self) -> usize {
        let mut finished = 0;

        let Self {
            start: _,
            end: _,
            prepare,
            reference_compilation,
            primary_compilation,
            comparison,
            update,
            cleanup,
            expect_reference_compilation: _,
            expect_comparison: _,
            expect_update: _,
        } = self;

        if cleanup.is_some() {
            finished += 1;
        }

        if update.is_some() {
            finished += 1;
        }

        if comparison.is_some() {
            finished += 1;
        }

        if primary_compilation.is_some() {
            finished += 1;
        }

        if reference_compilation.is_some() {
            finished += 1;
        }

        if prepare.is_some() {
            finished += 1;
        }

        finished
    }

    /// The last completed stage of the test run.
    ///
    /// Returns `None` if the test hasn't started yet.
    pub fn last_stage(&self) -> Option<TestStage> {
        let Self {
            start: _,
            end: _,
            prepare,
            reference_compilation,
            primary_compilation,
            comparison,
            update,
            cleanup,
            expect_reference_compilation: _,
            expect_comparison: _,
            expect_update: _,
        } = self;

        if cleanup.is_some() {
            return Some(TestStage::Cleanup);
        }

        if update.is_some() {
            return Some(TestStage::Update);
        }

        if comparison.is_some() {
            return Some(TestStage::Comparison);
        }

        if primary_compilation.is_some() {
            return Some(TestStage::PrimaryCompilation);
        }

        if reference_compilation.is_some() {
            return Some(TestStage::ReferenceCompilation);
        }

        if prepare.is_some() {
            return Some(TestStage::Prepare);
        }

        None
    }

    /// The next expected stage of the test run.
    ///
    /// Returns `None` if the test completed.
    pub fn next_stage(&self) -> Option<TestStage> {
        let Self {
            start: _,
            end: _,
            prepare,
            reference_compilation,
            primary_compilation,
            comparison,
            update,
            cleanup,
            expect_reference_compilation,
            expect_comparison,
            expect_update,
        } = self;

        if cleanup.is_some() {
            return None;
        }

        if prepare.is_some_and(|prepare| !prepare) {
            return None;
        }

        if update.is_some() {
            return Some(TestStage::Cleanup);
        }

        if comparison.is_some() {
            return Some(if *expect_update {
                TestStage::Update
            } else {
                TestStage::Cleanup
            });
        }

        if let Some(primary_compilation) = primary_compilation {
            // Skip to clean up if this or the reference compilation failed.
            if primary_compilation.is_failed()
                || reference_compilation
                    .as_ref()
                    .is_some_and(|s| s.is_failed())
            {
                return Some(TestStage::Cleanup);
            }

            return Some(if *expect_comparison {
                TestStage::Comparison
            } else {
                TestStage::Cleanup
            });
        }

        if let Some(reference_compilation) = reference_compilation {
            return Some(if reference_compilation.is_passed() {
                TestStage::PrimaryCompilation
            } else {
                TestStage::Cleanup
            });
        }

        if prepare.is_some() {
            return Some(if *expect_reference_compilation {
                TestStage::ReferenceCompilation
            } else {
                TestStage::PrimaryCompilation
            });
        }

        Some(TestStage::Prepare)
    }

    /// The kind of a test result.
    pub fn kind(&self) -> Kind {
        let Self {
            start: _,
            end: _,
            prepare,
            reference_compilation,
            primary_compilation,
            comparison,
            update,
            cleanup,
            expect_reference_compilation,
            expect_comparison,
            expect_update,
        } = self;

        // If cleanup ran and passed then we may still have a failure in another
        // stage, only if it failed can we exit early. If it didn't run then
        // it is unfinished iff prepare also didn't run or passed.
        if let Some(cleanup) = *cleanup {
            if !cleanup {
                return Kind::Failed;
            };
        } else if prepare.is_some_and(|prepare| prepare) {
            return Kind::Unfinished;
        }

        if update.is_some() {
            return Kind::Passed;
        } else if *expect_update {
            return Kind::Unfinished;
        }

        if let Some(comparison) = comparison {
            return if comparison.is_passed() {
                Kind::Passed
            } else {
                Kind::Failed
            };
        } else if *expect_comparison {
            return Kind::Unfinished;
        }

        if let Some(primary_compilation) = primary_compilation {
            return if primary_compilation.is_passed() {
                Kind::Passed
            } else {
                Kind::Failed
            };
        }

        if let Some(reference_compilation) = reference_compilation {
            return if reference_compilation.is_passed() {
                Kind::Passed
            } else {
                Kind::Failed
            };
        } else if *expect_reference_compilation {
            return Kind::Unfinished;
        }

        if let Some(prepare) = *prepare {
            return if prepare { Kind::Passed } else { Kind::Failed };
        }

        Kind::Unfinished
    }
}

impl TestTrace {
    /// Records the start time of the trace.
    pub fn record_start(&mut self) {
        tracing::trace!("recording trace test start time");

        self.start = Utc::now();
    }

    /// Records the result of the prepare stage.
    pub fn record_prepare(&mut self, result: bool) {
        tracing::trace!(success = result, "recording stage prepare");

        debug_assert!(
            self.prepare().is_none(),
            "tried to record stage prepare compilation more than once",
        );

        self.prepare = Some(result);
    }

    /// Records the result of the reference compilation stage.
    pub fn record_reference_compilation(&mut self, result: CompilationResult) {
        tracing::trace!(
            success = result.is_passed(),
            "recording stage reference compilation"
        );

        debug_assert!(
            self.reference_compilation().is_none(),
            "tried to record stage reference compilation more than once",
        );

        self.reference_compilation = Some(result);
    }

    /// Records the result of the primary compilation stage.
    pub fn record_primary_compilation(&mut self, result: CompilationResult) {
        tracing::trace!(
            success = result.is_passed(),
            "recording stage primary compilation"
        );

        debug_assert!(
            self.primary_compilation().is_none(),
            "tried to record stage primary compilation more than once",
        );

        self.primary_compilation = Some(result);
    }

    /// Records the result of the comparison stage.
    pub fn record_comparison(&mut self, result: ComparisonResult) {
        tracing::trace!(success = result.is_passed(), "recording stage comparison");

        debug_assert!(
            self.comparison().is_none(),
            "tried to record stage comparison more than once",
        );

        self.comparison = Some(result);
    }

    /// Records the result of the update stage.
    pub fn record_update(&mut self, result: bool) {
        tracing::trace!(success = result, "recording stage update");

        debug_assert!(
            self.update().is_none(),
            "tried to record stage update more than once",
        );

        self.update = Some(result);
    }

    /// Records the result of the cleanup stage.
    pub fn record_cleanup(&mut self, result: bool) {
        tracing::trace!(success = result, "recording stage cleanup");

        debug_assert!(
            self.cleanup().is_none(),
            "tried to record stage cleanup more than once",
        );

        self.cleanup = Some(result);
    }

    /// Records the end time of the trace.
    pub fn record_end(&mut self) {
        tracing::trace!("recording trace test end time");

        self.end = Utc::now();
    }
}

/// The stages of a test run, see the [module-level documentation][self] for
/// more info.
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum TestStage {
    /// The parsing and preparation stage.
    Prepare,

    /// The optional reference compilation stage.
    ReferenceCompilation,

    /// The primary compilation stage.
    PrimaryCompilation,

    /// The comparison stage.
    Comparison,

    /// The update stage.
    Update,

    /// The cleanup stage.
    Cleanup,
}

impl TestStage {
    /// Whether this is the prepare stage.
    pub fn is_prepare(&self) -> bool {
        matches!(self, Self::Prepare)
    }

    /// Whether this is the reference compilation stage.
    pub fn is_reference_compilation(&self) -> bool {
        matches!(self, Self::ReferenceCompilation)
    }

    /// Whether this is the primary compilation stage.
    pub fn is_primary_compilation(&self) -> bool {
        matches!(self, Self::PrimaryCompilation)
    }

    /// Whether this is the comparison stage.
    pub fn is_comparison(&self) -> bool {
        matches!(self, Self::Comparison)
    }

    /// Whether this is the update stage.
    pub fn is_update(&self) -> bool {
        matches!(self, Self::Update)
    }

    /// Whether this is the cleanup stage.
    pub fn is_cleanup(&self) -> bool {
        matches!(self, Self::Cleanup)
    }
}

/// The post-compile stage of a test run, this is used to decide what a runner
/// should do with compiled and rendered documents.
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum PostCompileTestStage {
    /// The comparison stage.
    Comparison,

    /// The comparison and update stages if the test is persistent.
    PersistentUpdate,

    /// The cleanup stage.
    Cleanup,
}

impl PostCompileTestStage {
    /// Whether this is the comparison only post-compile stage.
    pub fn is_comparison(&self) -> bool {
        matches!(self, Self::Comparison)
    }

    /// Whether this is the comparison and update post-compile stage.
    pub fn is_persistent_update(&self) -> bool {
        matches!(self, Self::PersistentUpdate)
    }

    /// Whether this is the cleanup post-compile stage.
    pub fn is_cleanup(&self) -> bool {
        matches!(self, Self::Cleanup)
    }
}

/// The kind of a suite or test result.
#[derive(Debug, Default, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum Kind {
    /// The test was not run to completion.
    #[default]
    Unfinished,

    /// The test result was for a passed test.
    Passed,

    /// The test result was for a failed test.
    Failed,
}

impl Kind {
    /// Whether this is [`Kind::Unfinished`].
    pub fn is_unfinished(&self) -> bool {
        matches!(self, Self::Unfinished)
    }

    /// Whether this is [`Kind::Passed`].
    pub fn is_passed(&self) -> bool {
        matches!(self, Self::Passed)
    }

    /// Whether this is [`Kind::Failed`].
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed)
    }
}

/// The result of compiling a test or its reference.
#[derive(Debug, Clone)]
pub enum CompilationResult {
    /// The passed compilation result.
    Passed(CompilationOutput),

    /// The failed compilation result.
    Failed(CompilationFailure),
}

impl CompilationResult {
    /// Whether this is [`CompilationResult::Passed`].
    pub fn is_passed(&self) -> bool {
        matches!(self, Self::Passed(_))
    }

    /// Whether this is [`CompilationResult::Failed`].
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

/// Converts the output of [`typst::compile`] into a
/// [`CompilationResult`].
pub fn from_typst_compilation(result: Warned<SourceResult<PagedDocument>>) -> CompilationResult {
    match result.output {
        Ok(doc) => CompilationResult::Passed(CompilationOutput::new(doc.pages, result.warnings)),
        Err(err) => CompilationResult::Failed(CompilationFailure::new(err, result.warnings)),
    }
}

/// The result of a passed test compilation.
///
/// This contains the emitted warnings and the output document of the
/// compilation.
#[derive(Debug, Clone)]
pub struct CompilationOutput {
    pages: Vec<Page>,
    warnings: EcoVec<SourceDiagnostic>,
}

impl CompilationOutput {
    /// Creates a new compilation output result from the given document and
    /// warnings.
    pub fn new<P, W>(pages: P, warnings: W) -> Self
    where
        P: Into<Vec<Page>>,
        W: Into<EcoVec<SourceDiagnostic>>,
    {
        Self {
            pages: pages.into(),
            warnings: warnings.into(),
        }
    }
}

impl CompilationOutput {
    /// The pages of the compiled output document.
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// The warnings emitted during compilation.
    pub fn warnings(&self) -> &[SourceDiagnostic] {
        &self.warnings
    }
}

/// The result of a failed test compilation.
///
/// This contains the emitted errors and warnings of the failed compilation.
#[derive(Debug, Clone)]
pub struct CompilationFailure {
    errors: EcoVec<SourceDiagnostic>,
    warnings: EcoVec<SourceDiagnostic>,
}

impl CompilationFailure {
    /// Creates a new compilation failure result from the given errors and
    /// warnings.
    pub fn new<E, W>(errors: E, warnings: W) -> Self
    where
        E: Into<EcoVec<SourceDiagnostic>>,
        W: Into<EcoVec<SourceDiagnostic>>,
    {
        let errors = errors.into();
        let warnings = warnings.into();

        debug_assert!(!errors.is_empty());

        Self { errors, warnings }
    }
}

impl CompilationFailure {
    /// The errors emitted during compilation.
    pub fn errors(&self) -> &[SourceDiagnostic] {
        &self.errors
    }

    /// The warnings emitted during compilation.
    pub fn warnings(&self) -> &[SourceDiagnostic] {
        &self.warnings
    }
}

/// The result of comparing a test and its reference.
#[derive(Debug, Clone)]
pub enum ComparisonResult {
    /// The passed comparison result.
    Passed(ComparisonOutput),

    /// The failed comparison result.
    Failed(ComparisonFailure),
}

impl ComparisonResult {
    /// Whether this is [`ComparisonResult::Passed`].
    pub fn is_passed(&self) -> bool {
        matches!(self, Self::Passed(_))
    }

    /// Whether this is [`ComparisonResult::Failed`].
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

/// The result of a passed test comparison.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ComparisonOutput {}

impl ComparisonOutput {
    /// Creates a new comparison output.
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ComparisonOutput {
    fn default() -> Self {
        Self::new()
    }
}

/// The result of a failed test comparison.
#[derive(Debug, Clone)]
pub struct ComparisonFailure {
    errors: analysis::Error,
}

impl ComparisonFailure {
    /// Creates a new comparison failure from the given errors.
    pub fn new(errors: analysis::Error) -> Self {
        Self { errors }
    }
}

impl ComparisonFailure {
    /// The inner analysis error.
    pub fn error(&self) -> &analysis::Error {
        &self.errors
    }
}

#[cfg(test)]
mod tests {
    use ecow::eco_vec;

    use crate::result::ComparisonOutput;
    use crate::result::CompilationOutput;
    use crate::test::UnitKind;

    use super::*;

    fn passed_compilation_result() -> CompilationOutput {
        CompilationOutput {
            pages: vec![],
            warnings: eco_vec![],
        }
    }

    fn passed_comparison_result() -> ComparisonOutput {
        ComparisonOutput {}
    }

    fn all_stages(mut trace: TestTrace) -> Vec<TestStage> {
        std::iter::from_fn(|| {
            let stage = trace.next_stage();

            match stage {
                Some(TestStage::Prepare) => trace.record_prepare(true),
                Some(TestStage::ReferenceCompilation) => trace.record_reference_compilation(
                    CompilationResult::Passed(passed_compilation_result()),
                ),
                Some(TestStage::PrimaryCompilation) => trace.record_primary_compilation(
                    CompilationResult::Passed(passed_compilation_result()),
                ),
                Some(TestStage::Comparison) => {
                    trace.record_comparison(ComparisonResult::Passed(passed_comparison_result()))
                }
                Some(TestStage::Update) => trace.record_update(true),
                Some(TestStage::Cleanup) => trace.record_cleanup(true),
                None => {}
            };

            stage
        })
        .collect()
    }

    #[test]
    fn test_test_trace_next_stage_template() {
        assert_eq!(
            all_stages(TestTrace::unfinished(
                TestKind::Template,
                PostCompileTestStage::PersistentUpdate
            )),
            [
                TestStage::Prepare,
                TestStage::PrimaryCompilation,
                TestStage::Cleanup
            ]
        );

        assert_eq!(
            all_stages(TestTrace::unfinished(
                TestKind::Template,
                PostCompileTestStage::Comparison
            )),
            [
                TestStage::Prepare,
                TestStage::PrimaryCompilation,
                TestStage::Cleanup
            ]
        );

        assert_eq!(
            all_stages(TestTrace::unfinished(
                TestKind::Template,
                PostCompileTestStage::Cleanup
            )),
            [
                TestStage::Prepare,
                TestStage::PrimaryCompilation,
                TestStage::Cleanup
            ]
        );
    }

    #[test]
    fn test_test_trace_next_stage_unit_compile_only() {
        assert_eq!(
            all_stages(TestTrace::unfinished(
                TestKind::Unit(UnitKind::CompileOnly),
                PostCompileTestStage::PersistentUpdate
            )),
            [
                TestStage::Prepare,
                TestStage::PrimaryCompilation,
                TestStage::Cleanup
            ]
        );

        assert_eq!(
            all_stages(TestTrace::unfinished(
                TestKind::Unit(UnitKind::CompileOnly),
                PostCompileTestStage::Comparison
            )),
            [
                TestStage::Prepare,
                TestStage::PrimaryCompilation,
                TestStage::Cleanup
            ]
        );

        assert_eq!(
            all_stages(TestTrace::unfinished(
                TestKind::Unit(UnitKind::CompileOnly),
                PostCompileTestStage::Cleanup
            )),
            [
                TestStage::Prepare,
                TestStage::PrimaryCompilation,
                TestStage::Cleanup
            ]
        );
    }

    #[test]
    fn test_test_trace_next_stage_unit_ephemeral() {
        assert_eq!(
            all_stages(TestTrace::unfinished(
                TestKind::Unit(UnitKind::Ephemeral),
                PostCompileTestStage::PersistentUpdate
            )),
            [
                TestStage::Prepare,
                TestStage::ReferenceCompilation,
                TestStage::PrimaryCompilation,
                TestStage::Cleanup
            ]
        );

        assert_eq!(
            all_stages(TestTrace::unfinished(
                TestKind::Unit(UnitKind::Ephemeral),
                PostCompileTestStage::Comparison
            )),
            [
                TestStage::Prepare,
                TestStage::ReferenceCompilation,
                TestStage::PrimaryCompilation,
                TestStage::Comparison,
                TestStage::Cleanup
            ]
        );

        assert_eq!(
            all_stages(TestTrace::unfinished(
                TestKind::Unit(UnitKind::Ephemeral),
                PostCompileTestStage::Cleanup
            )),
            [
                TestStage::Prepare,
                TestStage::ReferenceCompilation,
                TestStage::PrimaryCompilation,
                TestStage::Cleanup
            ]
        );
    }

    #[test]
    fn test_test_trace_next_stage_unit_persistent() {
        assert_eq!(
            all_stages(TestTrace::unfinished(
                TestKind::Unit(UnitKind::Persistent),
                PostCompileTestStage::PersistentUpdate
            )),
            [
                TestStage::Prepare,
                TestStage::PrimaryCompilation,
                TestStage::Comparison,
                TestStage::Update,
                TestStage::Cleanup
            ]
        );

        assert_eq!(
            all_stages(TestTrace::unfinished(
                TestKind::Unit(UnitKind::Persistent),
                PostCompileTestStage::Comparison
            )),
            [
                TestStage::Prepare,
                TestStage::PrimaryCompilation,
                TestStage::Comparison,
                TestStage::Cleanup
            ]
        );

        assert_eq!(
            all_stages(TestTrace::unfinished(
                TestKind::Unit(UnitKind::Persistent),
                PostCompileTestStage::Cleanup
            )),
            [
                TestStage::Prepare,
                TestStage::PrimaryCompilation,
                TestStage::Cleanup
            ]
        );
    }

    #[test]
    fn test_test_trace_next_stage_doc() {
        assert_eq!(
            all_stages(TestTrace::unfinished(
                TestKind::Doc,
                PostCompileTestStage::PersistentUpdate
            )),
            [
                TestStage::Prepare,
                TestStage::PrimaryCompilation,
                TestStage::Cleanup
            ]
        );

        assert_eq!(
            all_stages(TestTrace::unfinished(
                TestKind::Doc,
                PostCompileTestStage::Comparison
            )),
            [
                TestStage::Prepare,
                TestStage::PrimaryCompilation,
                TestStage::Cleanup
            ]
        );

        assert_eq!(
            all_stages(TestTrace::unfinished(
                TestKind::Doc,
                PostCompileTestStage::Cleanup
            )),
            [
                TestStage::Prepare,
                TestStage::PrimaryCompilation,
                TestStage::Cleanup
            ]
        );
    }

    #[test]
    fn test_suite_trace_record_test_trace_unifinished() {
        let mut suite_trace = SuiteTrace::unfinished(Uuid::nil());

        suite_trace.record_start();
        {
            let mut trace =
                TestTrace::unfinished(TestKind::Template, PostCompileTestStage::Comparison);
            trace.record_start();
            trace.record_end();
            suite_trace.record_test_trace(Ident::new("@template").unwrap(), Some(Box::new(trace)));
        }
        suite_trace.record_end();

        assert_eq!(suite_trace.len(), 1);
        assert_eq!(suite_trace.skipped(), 1);
        assert_eq!(suite_trace.passed(), 0);
        assert_eq!(suite_trace.failed(), 0);
        assert_eq!(suite_trace.filtered(), 0);
    }

    #[test]
    fn test_suite_trace_record_test_trace_passed() {
        let mut suite_trace = SuiteTrace::unfinished(Uuid::nil());

        suite_trace.record_start();
        {
            let mut trace =
                TestTrace::unfinished(TestKind::Template, PostCompileTestStage::Comparison);
            trace.record_start();
            trace.record_prepare(true);
            trace
                .record_primary_compilation(CompilationResult::Passed(passed_compilation_result()));
            trace.record_cleanup(true);
            trace.record_end();
            suite_trace.record_test_trace(Ident::new("@template").unwrap(), Some(Box::new(trace)));
        }
        suite_trace.record_end();

        assert_eq!(suite_trace.len(), 1);
        assert_eq!(suite_trace.skipped(), 0);
        assert_eq!(suite_trace.passed(), 1);
        assert_eq!(suite_trace.failed(), 0);
        assert_eq!(suite_trace.filtered(), 0);
    }

    #[test]
    fn test_suite_trace_record_test_trace_failed() {
        let mut suite_trace = SuiteTrace::unfinished(Uuid::nil());

        suite_trace.record_start();
        {
            let mut trace =
                TestTrace::unfinished(TestKind::Template, PostCompileTestStage::Comparison);
            trace.record_start();
            trace.record_prepare(false);
            trace.record_end();
            suite_trace.record_test_trace(Ident::new("@template").unwrap(), Some(Box::new(trace)));
        }
        suite_trace.record_end();

        assert_eq!(suite_trace.len(), 1);
        assert_eq!(suite_trace.skipped(), 0);
        assert_eq!(suite_trace.passed(), 0);
        assert_eq!(suite_trace.failed(), 1);
        assert_eq!(suite_trace.filtered(), 0);
    }

    #[test]
    fn test_suite_trace_record_test_trace_filtered() {
        let mut suite_trace = SuiteTrace::unfinished(Uuid::nil());

        suite_trace.record_start();
        suite_trace.record_test_trace(Ident::new("@template").unwrap(), None);
        suite_trace.record_end();

        assert_eq!(suite_trace.len(), 1);
        assert_eq!(suite_trace.skipped(), 0);
        assert_eq!(suite_trace.passed(), 0);
        assert_eq!(suite_trace.failed(), 0);
        assert_eq!(suite_trace.filtered(), 1);
    }
}
