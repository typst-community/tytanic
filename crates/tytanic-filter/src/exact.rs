//! Exact filter sets.
//!
//! Exact test filters are sets of test identifiers which must all be part of
//! the suite that is filtered. If a test is missing the filter fails unlike
//! test sets, which are purely additive.

use std::collections::HashSet;

use thiserror::Error;

use tytanic_core::filter::Filter;
use tytanic_core::project::ProjectContext;
use tytanic_core::test::Ident;
use tytanic_core::test::Test;

/// A filter that expects a specific set of tests.
///
/// This filter will emit an error on [`ExactFilter::finish`] if tests are
/// missing.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ExactFilter {
    missing: HashSet<Ident>,
    expected: HashSet<Ident>,
}

impl ExactFilter {
    /// Creates a new exact filter with no.
    pub fn new<I>(tests: I) -> Self
    where
        I: IntoIterator<Item = Ident>,
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

impl Filter for ExactFilter {
    type Error = Error;

    fn filter(&mut self, _ctx: &ProjectContext, test: &Test) -> Result<bool, Self::Error> {
        Ok(if self.missing.remove(&test.ident()) {
            true
        } else {
            self.expected.contains(&test.ident())
        })
    }

    fn finish(self, _ctx: &ProjectContext) -> Result<(), Self::Error> {
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
    pub missing: HashSet<Ident>,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use tytanic_core::config::LayeredConfig;
    use tytanic_core::project::store::Kind;
    use tytanic_core::suite::Suite;
    use tytanic_core::test::UnitKind;
    use tytanic_utils::fs::TempTestEnv;
    use tytanic_utils::typst::manifest::PackageManifestBuilder;

    use super::*;

    #[test]
    fn test_all_no_missing() {
        TempTestEnv::run_no_check(
            |root| root,
            |root| {
                let manifest = Some(Box::new(PackageManifestBuilder::new().build()));
                let config = Box::new(LayeredConfig::new());

                let ctx = ProjectContext::new(root, None, manifest, config, Kind::V1);

                let filter = ExactFilter::new([
                    Ident::new("foo").unwrap(),
                    Ident::new("bar").unwrap(),
                    Ident::new("qux").unwrap(),
                ]);

                let suite = Suite::from_tests_filter(
                    [
                        Test::try_new_unit("foo", UnitKind::CompileOnly).unwrap(),
                        Test::try_new_unit("bar", UnitKind::CompileOnly).unwrap(),
                        Test::try_new_unit("qux", UnitKind::CompileOnly).unwrap(),
                    ],
                    &ctx,
                    filter,
                )
                .unwrap();

                assert_eq!(
                    suite.iter_tests().map(Test::ident).collect::<BTreeSet<_>>(),
                    BTreeSet::from_iter([
                        Ident::new("foo").unwrap(),
                        Ident::new("bar").unwrap(),
                        Ident::new("qux").unwrap(),
                    ])
                );
                assert_eq!(
                    suite
                        .iter_filtered()
                        .map(Test::ident)
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
                let manifest = Some(Box::new(PackageManifestBuilder::new().build()));
                let config = Box::new(LayeredConfig::new());

                let ctx = ProjectContext::new(root, None, manifest, config, Kind::V1);

                let filter =
                    ExactFilter::new([Ident::new("foo").unwrap(), Ident::new("bar").unwrap()]);

                let suite = Suite::from_tests_filter(
                    [
                        Test::try_new_unit("foo", UnitKind::CompileOnly).unwrap(),
                        Test::try_new_unit("bar", UnitKind::CompileOnly).unwrap(),
                        Test::try_new_unit("qux", UnitKind::CompileOnly).unwrap(),
                    ],
                    &ctx,
                    filter,
                )
                .unwrap();

                assert_eq!(
                    suite.iter_tests().map(Test::ident).collect::<BTreeSet<_>>(),
                    BTreeSet::from_iter([Ident::new("foo").unwrap(), Ident::new("bar").unwrap()])
                );
                assert_eq!(
                    suite
                        .iter_filtered()
                        .map(Test::ident)
                        .collect::<BTreeSet<_>>(),
                    BTreeSet::from_iter([Ident::new("qux").unwrap()])
                );
            },
        );
    }

    #[test]
    fn test_missing() {
        TempTestEnv::run_no_check(
            |root| root,
            |root| {
                let manifest = Some(Box::new(PackageManifestBuilder::new().build()));
                let config = Box::new(LayeredConfig::new());

                let ctx = ProjectContext::new(root, None, manifest, config, Kind::V1);

                let filter = ExactFilter::new([
                    Ident::new("foo").unwrap(),
                    Ident::new("bar").unwrap(),
                    Ident::new("zir").unwrap(),
                ]);

                let missing = Suite::from_tests_filter(
                    [
                        Test::try_new_unit("foo", UnitKind::CompileOnly).unwrap(),
                        Test::try_new_unit("bar", UnitKind::CompileOnly).unwrap(),
                    ],
                    &ctx,
                    filter,
                )
                .unwrap_err();

                assert_eq!(
                    missing.missing,
                    HashSet::from_iter([Ident::new("zir").unwrap(),])
                );
            },
        );
    }
}
