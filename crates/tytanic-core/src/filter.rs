//! Test suite filters separate tests by their attributes and configurations.
//!
//! Filters are applied to [test suites][suite] on creation and when tests are
//! added or modified.
//!
//! [suite]: crate::suite::Suite

use std::convert::Infallible;

use crate::project::Project;
use crate::test::Test;

/// A stateful filter for test suites.
///
/// Such a filter may record which tests it has seen to emit errors when
/// `finish` is called.
pub trait FilterState: Sized {
    /// An error type that may occur during filtering.
    type Error: std::error::Error;

    /// Whether the test should be included in the test run.
    ///
    /// This should be called once for each test in a suite.
    fn filter(&mut self, project: &Project, test: &Test) -> Result<bool, Self::Error>;

    /// Finishes the filter, emitting any delayed errors if there are any.
    fn finish(self, project: &Project) -> Result<(), Self::Error> {
        let _project = project;

        Ok(())
    }
}

/// A filter that includes no tests.
#[derive(Debug, Default, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct NoneFilter;

impl FilterState for NoneFilter {
    type Error = Infallible;

    fn filter(&mut self, _project: &Project, _test: &Test) -> Result<bool, Self::Error> {
        Ok(false)
    }
}

/// A filter that includes all tests.
#[derive(Debug, Default, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct AllFilter;

impl FilterState for AllFilter {
    type Error = Infallible;

    fn filter(&mut self, _project: &Project, _test: &Test) -> Result<bool, Self::Error> {
        Ok(true)
    }
}
