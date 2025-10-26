//! Test suite filters separate tests by their attributes and configurations.
//!
//! Filters are applied to [test suites][suite] on creation and when tests are
//! added or modified.
//!
//! [suite]: crate::suite::Suite

use std::convert::Infallible;

use crate::project::ProjectContext;
use crate::test::Test;

/// A filter for test suites.
///
/// Such a filter may record which tests it has seen to emit errors when
/// `finish` is called.
pub trait Filter: Sized {
    /// An error type that may occur during filtering.
    type Error: std::error::Error;

    /// Whether the test should be included in the test run.
    ///
    /// This should be called once for each test in a suite.
    fn filter(&mut self, ctx: &ProjectContext, test: &Test) -> Result<bool, Self::Error>;

    /// Finishes the filter, emitting any delayed errors if there are any.
    fn finish(self, ctx: &ProjectContext) -> Result<(), Self::Error> {
        let _ctx = ctx;

        Ok(())
    }
}

/// A filter that includes no tests.
#[derive(Debug, Default, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct NoneFilter;

impl Filter for NoneFilter {
    type Error = Infallible;

    fn filter(&mut self, _ctx: &ProjectContext, _test: &Test) -> Result<bool, Self::Error> {
        Ok(false)
    }
}

/// A filter that includes all tests.
#[derive(Debug, Default, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct AllFilter;

impl Filter for AllFilter {
    type Error = Infallible;

    fn filter(&mut self, _ctx: &ProjectContext, _test: &Test) -> Result<bool, Self::Error> {
        Ok(true)
    }
}
