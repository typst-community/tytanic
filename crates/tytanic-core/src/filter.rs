//! Test suite filters separate tests by their attributes and configurations.
//!
//! Filters are applied to [test suites][suite] on creation and when tests are
//! added or modified.
//!
//! [suite]: crate::suite::Suite

use std::convert::Infallible;

use crate::project::Project;
use crate::test::Test;

/// A filter from which a [`FilterState`] can be created.
///
/// To use a filter create its state and apply it to a set of tests.
/// ```
/// # use tytanic_core::project::Project;
/// # use tytanic_core::test::Id;
/// # use tytanic_core::test::Test;
/// # use tytanic_core::test::UnitTest;
/// # use tytanic_core::test::unit::Kind as UnitKind;
/// # let project = Project::new(".");
/// use tytanic_core::filter::Filter as _;
/// use tytanic_core::filter::FilterState as _;
/// use tytanic_core::filter::FnFilter;
///
/// let tests = [
///     Test::Unit(UnitTest::new(Id::new("foo")?, UnitKind::CompileOnly)),
///     Test::Unit(UnitTest::new(Id::new("bar")?, UnitKind::CompileOnly)),
///     Test::Unit(UnitTest::new(Id::new("qux")?, UnitKind::CompileOnly)),
/// ];
///
/// let filter = FnFilter(|_, t| t.id() != "foo");
/// let mut state = filter.state();
///
/// let mut filtered = vec![];
/// for test in &tests {
///     if state.filter(&project, test)? {
///         filtered.push(test.id());
///     }
/// }
///
/// state.finish(&project)?;
///
/// assert_eq!(filtered, [
///     &Id::new("bar")?,
///     &Id::new("qux")?,
/// ]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub trait Filter {
    /// The state type for the filter.
    ///
    /// For pure filters this can be the filter itself or a reference to it. For
    /// impure filters this may be another type which potentially references the
    /// filter itself.
    type State<'a>: FilterState
    where
        Self: 'a;

    /// Creates a state for this filter which is consumed by a [`Suite`].
    ///
    /// [`Suite`]: crate::suite::Suite
    fn state(&self) -> Self::State<'_>;
}

/// A type which allows storing state during filter application.
///
/// See [`Filter`] for more info.
pub trait FilterState: Sized {
    /// An error type that may occur during filtering.
    type Error: std::error::Error;

    /// Whether the test should be included in the test run.
    ///
    /// This must be called exactly once for each test in a suite.
    fn filter(&mut self, project: &Project, test: &Test) -> Result<bool, Self::Error>;

    /// Finishes the filter, emitting any delayed errors if there are any.
    ///
    /// This must be called after each test was applied with `filter`.
    fn finish(self, project: &Project) -> Result<(), Self::Error> {
        let _project = project;

        Ok(())
    }
}

/// A filter that includes no tests.
///
/// This filter is pure, it returns itself as a [`FilterState`].
#[derive(Debug, Default, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct NoneFilter;

impl Filter for NoneFilter {
    type State<'a>
        = Self
    where
        Self: 'a;

    fn state(&self) -> Self::State<'_> {
        *self
    }
}

impl FilterState for NoneFilter {
    type Error = Infallible;

    fn filter(&mut self, _project: &Project, _test: &Test) -> Result<bool, Self::Error> {
        Ok(false)
    }
}

/// A filter that includes all tests.
///
/// This filter is pure, it returns itself as a [`FilterState`].
#[derive(Debug, Default, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct AllFilter;

impl Filter for AllFilter {
    type State<'a>
        = Self
    where
        Self: 'a;

    fn state(&self) -> Self::State<'_> {
        *self
    }
}

impl FilterState for AllFilter {
    type Error = Infallible;

    fn filter(&mut self, _project: &Project, _test: &Test) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// A filter that forwards the filter decision to a an infallible closure.
///
/// See [`TryFnFilter`] for a fallible version.
///
/// This filter is pure, it returns a reference to itself as a [`FilterState`].
#[derive(Debug, Default, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct FnFilter<F>(pub F)
where
    F: Fn(&Project, &Test) -> bool;

impl<F> Filter for FnFilter<F>
where
    F: Fn(&Project, &Test) -> bool,
{
    type State<'a>
        = &'a Self
    where
        Self: 'a;

    fn state(&self) -> Self::State<'_> {
        self
    }
}

impl<F> FilterState for &FnFilter<F>
where
    F: Fn(&Project, &Test) -> bool,
{
    type Error = Infallible;

    fn filter(&mut self, project: &Project, test: &Test) -> Result<bool, Self::Error> {
        Ok((self.0)(project, test))
    }
}

/// A filter that forwards the filter decision to a fallible closure.
///
/// See [`FnFilter`] for an infallible version.
///
/// This filter is pure, it returns a reference to itself as a [`FilterState`].
#[derive(Debug, Default, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct TryFnFilter<F, E>(pub F)
where
    F: Fn(&Project, &Test) -> Result<bool, E>,
    E: std::error::Error;

impl<F, E> Filter for TryFnFilter<F, E>
where
    F: Fn(&Project, &Test) -> Result<bool, E>,
    E: std::error::Error,
{
    type State<'a>
        = &'a Self
    where
        Self: 'a;

    fn state(&self) -> Self::State<'_> {
        self
    }
}

impl<F, E> FilterState for &TryFnFilter<F, E>
where
    F: Fn(&Project, &Test) -> Result<bool, E>,
    E: std::error::Error,
{
    type Error = E;

    fn filter(&mut self, project: &Project, test: &Test) -> Result<bool, Self::Error> {
        (self.0)(project, test)
    }
}
