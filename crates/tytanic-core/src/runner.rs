//! Runners are types which manage the resources necessary to execute tests.

use std::fmt::Debug;
use std::io;

use thiserror::Error;
use tytanic_utils::forward_trait;
use uuid::Uuid;

use crate::project::ProjectContext;
use crate::result::SuiteTrace;
use crate::result::TestTrace;
use crate::suite::Suite;
use crate::test::Ident;
use crate::test::Test;

/// A runner for Tytanic tests.
///
/// # Cancellation
/// In case of cancellations the runner must ensure the that the unfinished
/// suite traces are filled up with skipped tests before exiting such that an
/// interrupted run can still produce a full test report.
///
/// # Reuse
/// The runner may be reused across test runs to facilitate efficient
/// incremental running. The caller should call [`Runner::reset`] in between
/// test runs to ensure states can be reset.
pub trait Runner: Debug + Send + Sync {
    /// Instructs the runner to cancel the current run.
    fn cancel(&self, reason: CancellationReason);

    /// Instructs the runner to reset state between runs.
    ///
    /// This should be called before running the same runner again for a
    /// watch session, indicating to the runner to reset states that must stay
    /// consistent across compilations.
    fn reset(&self);

    /// Runs all non-filtered tests in a test suite and creates a test suite
    /// trace.
    fn run_suite(
        &self,
        ctx: &ProjectContext,
        suite: &Suite,
        run_id: Uuid,
        update: bool,
    ) -> Result<SuiteTrace, Error>;

    /// Runs a single test and creates a test trace.
    fn run_test(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        run_id: Uuid,
        update: bool,
    ) -> Result<TestTrace, Error>;
}

forward_trait! {
    impl<R> Runner for [std::boxed::Box<R>, std::sync::Arc<R>, &R, &mut R] {
        fn cancel(&self, reason: CancellationReason) {
            R::cancel(self, reason)
        }

        fn reset(&self) {
            R::reset(self)
        }

        fn run_suite(
            &self,
            ctx: &ProjectContext,
            suite: &Suite,
            run_id: Uuid,
            update: bool,
        ) -> Result<SuiteTrace, Error> {
            R::run_suite(self, ctx, suite, run_id, update)
        }

        fn run_test(
            &self,
            ctx: &ProjectContext,
            test: &Test,
            run_id: Uuid,
            update: bool,
        ) -> Result<TestTrace, Error> {
            R::run_test(self, ctx, test, run_id, update)
        }
    }
}

/// The reason for a cancellation.
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum CancellationReason {
    /// A test failed.
    ///
    /// This _may_ cancel the run, but still run any cleanup necessary for a
    /// complete test result to be created.
    ///
    /// This maybe ignored by a test runner.
    TestFailed,

    /// A soft cancellation was requested externally.
    ///
    /// This _must_ cancel the run as soon as possible, but still run any
    /// cleanup necessary for a complete test result to be created.
    ///
    /// This must not be ignored by the runner.
    Request,
}

impl CancellationReason {
    /// Whether this is [`CancellationReason::TestFailed`].
    pub fn is_test_failed(&self) -> bool {
        matches!(self, CancellationReason::TestFailed)
    }

    /// Whether this is [`CancellationReason::Request`].
    pub fn is_request(&self) -> bool {
        matches!(self, CancellationReason::Request)
    }
}

/// Returned by the methods on [`Runner`].
#[derive(Debug, Error)]
pub enum Error {
    /// An IO error occurred.
    #[error("an IO error occured")]
    Io(#[from] io::Error),

    /// A catch-all variant for test specific errors.
    #[error("an error occured for test {test}")]
    Test {
        /// The identifier of the test for which the error occurred.
        test: Ident,

        /// The inner error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// A catch-all variant for user implementations.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}
