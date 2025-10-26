//! Live reporting of test run progress.

use std::fmt::Debug;
use std::io;

use chrono::DateTime;
use chrono::Utc;
use thiserror::Error;
use tytanic_core::project::ProjectContext;
use tytanic_core::result::ComparisonResult;
use tytanic_core::result::CompilationResult;
use tytanic_core::result::SuiteTrace;
use tytanic_core::result::TestTrace;
use tytanic_core::suite::Suite;
use tytanic_core::test::Test;
use uuid::Uuid;

/// A trait for reporting the progress of the default runner.
///
/// `()` can be used as a no-op reporter.
pub trait Reporter: Debug + Send + Sync {
    /// Reports the start of a suite run.
    fn report_suite_started(
        &self,
        ctx: &ProjectContext,
        suite: &Suite,
        run_id: Uuid,
        start: DateTime<Utc>,
    ) -> Result<(), Error>;

    /// Reports the end of a suite run.
    fn report_suite_finished(
        &self,
        ctx: &ProjectContext,
        suite: &Suite,
        trace: &SuiteTrace,
    ) -> Result<(), Error>;

    /// Reports the start of a test run.
    fn report_test_started(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        start: DateTime<Utc>,
    ) -> Result<(), Error>;

    /// Reports the completion of a test's prepare stage.
    fn report_test_stage_prepare(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        result: bool,
    ) -> Result<(), Error>;

    /// Reports the completion of a test's reference compilation stage.
    fn report_test_stage_reference_compilation(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        result: &CompilationResult,
    ) -> Result<(), Error>;

    /// Reports the completion of a test's primary compilation stage.
    fn report_test_stage_primary_compilation(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        result: &CompilationResult,
    ) -> Result<(), Error>;

    /// Reports the completion of a test's comparison stage.
    fn report_test_stage_comparison(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        result: &ComparisonResult,
    ) -> Result<(), Error>;

    /// Reports the completion of a test's update stage.
    fn report_test_stage_update(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        result: bool,
    ) -> Result<(), Error>;

    /// Reports the completion of a test's prepare stage.
    fn report_test_stage_cleanup(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        result: bool,
    ) -> Result<(), Error>;

    /// Reports the end of a test run.
    fn report_test_finished(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        trace: &TestTrace,
    ) -> Result<(), Error>;
}

impl Reporter for () {
    fn report_suite_started(
        &self,
        _ctx: &ProjectContext,
        _suite: &Suite,
        _run_id: Uuid,
        _start: DateTime<Utc>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn report_suite_finished(
        &self,
        _ctx: &ProjectContext,
        _suite: &Suite,
        _trace: &SuiteTrace,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn report_test_started(
        &self,
        _ctx: &ProjectContext,
        _test: &Test,
        _start: DateTime<Utc>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn report_test_stage_prepare(
        &self,
        _ctx: &ProjectContext,
        _test: &Test,
        _result: bool,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn report_test_stage_reference_compilation(
        &self,
        _ctx: &ProjectContext,
        _test: &Test,
        _result: &CompilationResult,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn report_test_stage_primary_compilation(
        &self,
        _ctx: &ProjectContext,
        _test: &Test,
        _result: &CompilationResult,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn report_test_stage_comparison(
        &self,
        _ctx: &ProjectContext,
        _test: &Test,
        _result: &ComparisonResult,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn report_test_stage_update(
        &self,
        _ctx: &ProjectContext,
        _test: &Test,
        _result: bool,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn report_test_stage_cleanup(
        &self,
        _ctx: &ProjectContext,
        _test: &Test,
        _result: bool,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn report_test_finished(
        &self,
        _ctx: &ProjectContext,
        _test: &Test,
        _trace: &TestTrace,
    ) -> Result<(), Error> {
        Ok(())
    }
}

/// Returned by the methods on [`Reporter`].
#[derive(Debug, Error)]
pub enum Error {
    /// An IO error occurred.
    #[error("an IO error occured")]
    Io(#[from] io::Error),

    /// A catch-all variant for user implementations.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}
