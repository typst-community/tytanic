//! Exact filter sets.
//!
//! Exact test filters are sets of test identifiers which must all be part of
//! the suite that is filtered. If a test is missing the filter fails unlike
//! test sets, which are purely additive.

use std::collections::HashSet;

use thiserror::Error;

use tytanic_core::filter::Filter;
use tytanic_core::filter::FilterState;
use tytanic_core::project::Project;
use tytanic_core::test::Id;
use tytanic_core::test::TestRef;

/// A filter that expects a specific set of tests.
///
/// This filter is _impure_, it requires a dedicated [`FilterState`].
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ExactFilter {
    expected: HashSet<Id>,
}

impl ExactFilter {
    /// Creates a new exact filter with no.
    pub fn new<I>(tests: I) -> Self
    where
        I: IntoIterator<Item = Id>,
    {
        let mut expected = HashSet::new();

        for test in tests {
            expected.insert(test);
        }

        Self { expected }
    }
}

impl ExactFilter {
    /// The identifiers of the expected tests.
    pub fn expected(&self) -> &HashSet<Id> {
        &self.expected
    }
}

impl Filter for ExactFilter {
    type State<'a>
        = ExactFilterState
    where
        Self: 'a;

    fn state(&self) -> Self::State<'_> {
        ExactFilterState {
            missing: self.expected.clone(),
            expected: self.expected.clone(),
        }
    }
}

/// Created by [`ExactFilter::state`] for [`ExactFilter`].
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ExactFilterState {
    missing: HashSet<Id>,
    expected: HashSet<Id>,
}

impl FilterState for ExactFilterState {
    type Error = Error;

    fn filter<'t, T>(&mut self, _project: &Project, test: T) -> Result<bool, Self::Error>
    where
        T: Into<TestRef<'t>>,
    {
        let test = test.into();

        Ok(if self.missing.remove(test.id().as_str()) {
            true
        } else {
            self.expected.contains(test.id().as_str())
        })
    }

    fn finish(self, _ctx: &Project) -> Result<(), Self::Error> {
        if self.missing.is_empty() {
            Ok(())
        } else {
            Err(Error {
                missing: self.missing,
            })
        }
    }
}

/// Returned by [`FilterState::finish`] for [`ExactFilterState`].
#[derive(Debug, Error)]
#[error("{} tests were missing", .missing.len())]
pub struct Error {
    /// The identifiers of the missing tests.
    pub missing: HashSet<Id>,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use tytanic_core::UnitTest;
    use tytanic_core::suite::Suite;
    use tytanic_core::test::{IdRef, UnitId, UnitKind};
    use tytanic_utils::fs::TempTestEnv;

    use super::*;

    #[test]
    fn test_all_no_missing() {
        TempTestEnv::run_no_check(
            |root| root,
            |root| {
                let project = Project::new(root);

                let filter = ExactFilter::new([
                    Id::new("foo").unwrap(),
                    Id::new("bar").unwrap(),
                    Id::new("qux").unwrap(),
                ]);

                let suite = Suite::from_tests(
                    [
                        UnitTest::new(UnitId::new("foo").unwrap(), UnitKind::CompileOnly),
                        UnitTest::new(UnitId::new("bar").unwrap(), UnitKind::CompileOnly),
                        UnitTest::new(UnitId::new("qux").unwrap(), UnitKind::CompileOnly),
                    ],
                    [],
                    None,
                );

                let suite = suite.filter(&project, filter).unwrap();

                assert_eq!(
                    suite
                        .matched()
                        .tests()
                        .map(TestRef::id)
                        .collect::<BTreeSet<_>>(),
                    BTreeSet::from_iter([
                        IdRef::Unit(&UnitId::new("foo").unwrap()),
                        IdRef::Unit(&UnitId::new("bar").unwrap()),
                        IdRef::Unit(&UnitId::new("qux").unwrap()),
                    ])
                );
                assert_eq!(
                    suite
                        .filtered()
                        .tests()
                        .map(TestRef::id)
                        .collect::<BTreeSet<_>>(),
                    BTreeSet::from_iter([])
                );
            },
        );
    }

    #[test]
    fn test_some_no_missing() {
        TempTestEnv::run_no_check(
            |root| root,
            |root| {
                let project = Project::new(root);

                let filter = ExactFilter::new([Id::new("foo").unwrap(), Id::new("bar").unwrap()]);

                let suite = Suite::from_tests(
                    [
                        UnitTest::new(UnitId::new("foo").unwrap(), UnitKind::CompileOnly),
                        UnitTest::new(UnitId::new("bar").unwrap(), UnitKind::CompileOnly),
                        UnitTest::new(UnitId::new("qux").unwrap(), UnitKind::CompileOnly),
                    ],
                    [],
                    None,
                );

                let suite = suite.filter(&project, filter).unwrap();

                assert_eq!(
                    suite
                        .matched()
                        .tests()
                        .map(TestRef::id)
                        .collect::<BTreeSet<_>>(),
                    BTreeSet::from_iter([
                        IdRef::Unit(&UnitId::new("foo").unwrap()),
                        IdRef::Unit(&UnitId::new("bar").unwrap())
                    ])
                );
                assert_eq!(
                    suite
                        .filtered()
                        .tests()
                        .map(TestRef::id)
                        .collect::<BTreeSet<_>>(),
                    BTreeSet::from_iter([IdRef::Unit(&UnitId::new("qux").unwrap())])
                );
            },
        );
    }

    #[test]
    fn test_missing() {
        TempTestEnv::run_no_check(
            |root| root,
            |root| {
                let project = Project::new(root);

                let filter = ExactFilter::new([
                    Id::new("foo").unwrap(),
                    Id::new("bar").unwrap(),
                    Id::new("zir").unwrap(),
                ]);

                let suite = Suite::from_tests(
                    [
                        UnitTest::new(UnitId::new("foo").unwrap(), UnitKind::CompileOnly),
                        UnitTest::new(UnitId::new("bar").unwrap(), UnitKind::CompileOnly),
                    ],
                    [],
                    None,
                );

                let missing = suite.filter(&project, filter).unwrap_err();
                assert_eq!(
                    missing.missing,
                    HashSet::from_iter([Id::new("zir").unwrap()])
                );
            },
        );
    }
}
