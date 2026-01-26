//! # `tytanic-filter`
//! Filtering Tytanic test suites effectively.
//!
//! This crates provides default implementations for the [`Filter`] trait, these
//! filters are used in the Tytanic CLI.
//!
//! ## Exact Filter Sets
//! Exact filter sets can be turned into [`ExactFilter`], being an exact filter
//! means they emit errors when expected tests are missing once
//! [`Filter::finish`] is called.
//!
//! ## Test Set Expressions
//! Tests sets are expressions in a set-based DSL for filtering tests according
//! to their identifiers, attributes, and annotations.
//!
//! Test sets can be parsed and evaluated into an [`ExpressionFilter`], unlike
//! explicit filters these are purely additive, i.e. missing tests do not cause
//! an error.

use thiserror::Error;

use tytanic_core::filter::FilterState;
use tytanic_core::project::Project;
use tytanic_core::test::Test;

use crate::exact::ExactFilter;
use crate::test_set::ExpressionFilter;
use crate::test_set::eval;

pub mod exact;
pub mod test_set;

/// A combined exact and test set expression filter.
///
/// The exact filter (if set) is applied first, if it does not match, or isn't
/// set, then the expression filter is applied.
#[derive(Debug, Default)]
pub struct CombinedFilter {
    test_set: Option<ExpressionFilter>,
    exact: Option<ExactFilter>,
}

impl CombinedFilter {
    /// Creates a new combined filter from the given test set and exact filters.
    pub fn new(test_set: Option<ExpressionFilter>, exact: Option<ExactFilter>) -> Self {
        Self { test_set, exact }
    }

    /// Creates a new empty combined filter.
    pub fn empty() -> Self {
        Self {
            test_set: None,
            exact: None,
        }
    }

    /// Adds the test set filter.
    pub fn with_test_set(&mut self, test_set: ExpressionFilter) -> &mut Self {
        self.test_set = Some(test_set);
        self
    }

    /// Adds the exact filter.
    pub fn with_exact(&mut self, exact: ExactFilter) -> &mut Self {
        self.exact = Some(exact);
        self
    }

    /// Maps the test set in place.
    pub fn map_test_set<F>(&mut self, f: F)
    where
        F: FnOnce(eval::Set) -> eval::Set,
    {
        self.test_set = self.test_set.take().map(|set| set.map(f));
    }
}

impl CombinedFilter {
    /// The test set filter.
    pub fn test_set(&self) -> Option<&ExpressionFilter> {
        self.test_set.as_ref()
    }

    /// The exact filter.
    pub fn exact(&self) -> Option<&ExactFilter> {
        self.exact.as_ref()
    }
}

impl FilterState for CombinedFilter {
    type Error = Error;

    fn filter(&mut self, project: &Project, test: &Test) -> Result<bool, Self::Error> {
        if let Some(exact) = &mut self.exact
            && exact.filter(project, test)?
        {
            return Ok(true);
        }

        if let Some(test_set) = &mut self.test_set {
            return Ok(test_set.filter(project, test)?);
        }

        Ok(false)
    }

    fn finish(mut self, project: &Project) -> Result<(), Self::Error> {
        self.test_set
            .take()
            .map(|f| f.finish(project))
            .transpose()?;
        self.exact.take().map(|f| f.finish(project)).transpose()?;

        Ok(())
    }
}

/// Returned by [`CombinedFilter::filter`] or [`CombinedFilter::finish`].
#[derive(Debug, Error)]
pub enum Error {
    /// The exact filter emitted an error.
    #[error("a exact filter error occurred")]
    Exact(#[from] exact::Error),

    /// The test set expression filter emitted an error.
    #[error("a test set expression filter evaluation error occurred")]
    TestSet(#[from] test_set::eval::Error),
}
