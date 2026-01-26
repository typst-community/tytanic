//! Exact filter sets.
//!
//! Exact test filters are sets of test identifiers which must all be part of
//! the suite that is filtered. If a test is missing the filter fails unlike
//! test sets, which are purely additive.

use std::collections::HashSet;

use thiserror::Error;

use tytanic_core::filter::FilterState;
use tytanic_core::project::Project;
use tytanic_core::test::Id;
use tytanic_core::test::Test;

/// A filter that expects a specific set of tests.
///
/// This filter's state will emit an error on `finish` if tests are missing.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ExactFilter {
    missing: HashSet<Id>,
    expected: HashSet<Id>,
}

impl ExactFilter {
    /// Creates a new exact filter with no.
    pub fn new<I>(tests: I) -> Self
    where
        I: IntoIterator<Item = Id>,
    {
        let mut missing = HashSet::new();
        let mut expected = HashSet::new();

        for test in tests {
            missing.insert(test.clone());
            expected.insert(test);
        }

        Self { missing, expected }
    }
}

impl ExactFilter {
    /// The identifiers of the expected tests.
    ///
    /// Note that this may be accurate if the [`ExactFilter::filter`] has been
    /// called already.
    pub fn expected(&self) -> &HashSet<Id> {
        &self.expected
    }
}

impl FilterState for ExactFilter {
    type Error = Error;

    fn filter(&mut self, _ctx: &Project, test: &Test) -> Result<bool, Self::Error> {
        Ok(if self.missing.remove(test.id()) {
            true
        } else {
            self.expected.contains(test.id())
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

/// Returned by [`ExactFilter::finish`].
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
    use tytanic_core::test::unit::Kind as UnitKind;
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

                let suite = Suite::from_tests([
                    Test::Unit(UnitTest::new(
                        Id::new("foo").unwrap(),
                        UnitKind::CompileOnly,
                    )),
                    Test::Unit(UnitTest::new(
                        Id::new("bar").unwrap(),
                        UnitKind::CompileOnly,
                    )),
                    Test::Unit(UnitTest::new(
                        Id::new("qux").unwrap(),
                        UnitKind::CompileOnly,
                    )),
                ]);

                let suite = suite.filter(&project, filter).unwrap();

                assert_eq!(
                    suite
                        .matched()
                        .tests()
                        .map(Test::id)
                        .cloned()
                        .collect::<BTreeSet<_>>(),
                    BTreeSet::from_iter([
                        Id::new("foo").unwrap(),
                        Id::new("bar").unwrap(),
                        Id::new("qux").unwrap(),
                    ])
                );
                assert_eq!(
                    suite
                        .filtered()
                        .tests()
                        .map(Test::id)
                        .cloned()
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

                let suite = Suite::from_tests([
                    Test::Unit(UnitTest::new(
                        Id::new("foo").unwrap(),
                        UnitKind::CompileOnly,
                    )),
                    Test::Unit(UnitTest::new(
                        Id::new("bar").unwrap(),
                        UnitKind::CompileOnly,
                    )),
                    Test::Unit(UnitTest::new(
                        Id::new("qux").unwrap(),
                        UnitKind::CompileOnly,
                    )),
                ]);

                let suite = suite.filter(&project, filter).unwrap();

                assert_eq!(
                    suite
                        .matched()
                        .tests()
                        .map(Test::id)
                        .cloned()
                        .collect::<BTreeSet<_>>(),
                    BTreeSet::from_iter([Id::new("foo").unwrap(), Id::new("bar").unwrap()])
                );
                assert_eq!(
                    suite
                        .filtered()
                        .tests()
                        .map(Test::id)
                        .cloned()
                        .collect::<BTreeSet<_>>(),
                    BTreeSet::from_iter([Id::new("qux").unwrap()])
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

                let suite = Suite::from_tests([
                    Test::Unit(UnitTest::new(
                        Id::new("foo").unwrap(),
                        UnitKind::CompileOnly,
                    )),
                    Test::Unit(UnitTest::new(
                        Id::new("bar").unwrap(),
                        UnitKind::CompileOnly,
                    )),
                ]);

                let missing = suite.filter(&project, filter).unwrap_err();
                assert_eq!(
                    missing.missing,
                    HashSet::from_iter([Id::new("zir").unwrap()])
                );
            },
        );
    }
}
