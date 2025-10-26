//! Test suites types.
//!
//! Test suites are collections of filtered and parsed tests, ready to be passed
//! to a runner.
//!
//! # Examples
//! ```
//! # use tytanic_core::suite::Suite;
//! # use tytanic_core::test::Test;
//! # use tytanic_core::test::UnitKind;
//! let mut suite = Suite::from_tests(
//!     [
//!         Test::try_new_unit("foo/test", UnitKind::CompileOnly)?,
//!         Test::try_new_unit("foo/bar/test1", UnitKind::CompileOnly)?,
//!         Test::try_new_unit("foo/bar/test2", UnitKind::CompileOnly)?,
//!     ],
//!     [],
//! );
//! # Ok::<_, Box<dyn std::error::Error>>(())
//! ```

use std::borrow::Borrow;
use std::collections::HashMap;
use std::collections::hash_map;
use std::fmt::Debug;
use std::hash::Hash;
use std::io;
use std::ops::Index;
use std::ops::IndexMut;
use std::sync::mpsc;
use std::thread;

use ignore::DirEntry;
use ignore::WalkBuilder;
use ignore::WalkState;
use thiserror::Error;

use crate::config::SettingsConfig;
use crate::filter::Filter;
use crate::project::ProjectContext;
use crate::project::store::Store;
use crate::test::DocTest;
use crate::test::Ident;
use crate::test::ParseIdentError;
use crate::test::TemplateTest;
use crate::test::Test;
use crate::test::UnitIdent;
use crate::test::UnitKind;
use crate::test::UnitTest;

/// A test suite contains all tests in a project separated into tests to run,
/// skipped tests and filtered tests.
///
/// See the [module-level documentation][self] for more info.
#[derive(Debug)]
pub struct Suite {
    tests: HashMap<Ident, (Test, bool)>,
    matched: usize,
}

impl Suite {
    /// Creates a new empty suite.
    pub fn new() -> Self {
        Self {
            tests: HashMap::new(),
            matched: 0,
        }
    }

    /// Creates a new suite with the given tests.
    pub fn from_tests<I, J>(matched: I, filtered: J) -> Self
    where
        I: IntoIterator<Item = Test>,
        J: IntoIterator<Item = Test>,
    {
        let mut combined = HashMap::new();
        let mut matched_len = 0;

        for test in matched {
            combined.insert(test.ident().clone(), (test, true));
            matched_len += 1;
        }

        for test in filtered {
            combined.insert(test.ident().clone(), (test, false));
        }

        Self {
            tests: combined,
            matched: matched_len,
        }
    }

    /// Collects a suite from the given context and filter.
    pub fn collect(ctx: &ProjectContext) -> Result<Self, CollectError> {
        let mut tests = HashMap::new();

        for unit_test in discover_unit_tests(ctx)? {
            tests.insert(
                unit_test.ident().into_ident(),
                (Test::Unit(unit_test), true),
            );
        }

        let len = tests.len();

        // TODO: add template tests

        // TODO(tinger): add doc tests

        Ok(Self {
            tests,
            matched: len,
        })
    }

    /// Applies the filter to this suite.
    ///
    /// The filter is applied once to each test in the suite such that
    /// [`Filter`] implementations can ensure that a suite contains all
    /// expected tests in a filter.
    ///
    /// # Errors
    /// Returns an error if the filter returns an error, in this case the filter
    /// may not be applied in its entirety.
    pub fn apply_filter<F>(&mut self, ctx: &ProjectContext, mut filter: F) -> Result<(), F::Error>
    where
        F: Filter,
    {
        for (test, is_matched) in self.tests.values_mut() {
            *is_matched = filter.filter(ctx, test)?;
        }

        filter.finish(ctx)?;

        Ok(())
    }
}

impl Suite {
    /// The amount of tests in this suite.
    pub fn len(&self) -> usize {
        self.tests.len()
    }

    /// Whether this suite has no tests.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The amount of tests in this suite that were not filtered out.
    pub fn matched_len(&self) -> usize {
        self.matched
    }

    /// The amount of tests in this suite that were filtered out.
    pub fn filtered_len(&self) -> usize {
        self.tests.len() - self.matched
    }
}

impl Suite {
    /// Inserts a new test into this suite and whether it is matched or not.
    ///
    /// # Returns
    /// Returns the old test and whether it was matched if there was a test or
    /// `None` if it wasn't found.
    pub fn insert(&mut self, test: Test, matched: bool) -> Option<(Test, bool)> {
        let old = self.tests.insert(test.ident().clone(), (test, matched));

        if matched && old.as_ref().is_none_or(|(_, is_matched)| !is_matched) {
            self.matched += 1;
        }

        old
    }

    /// Removes the test with the given id from this suite.
    ///
    /// # Returns
    /// Returns the test or `None` if it wasn't found.
    pub fn remove(&mut self, ident: &Ident) -> Option<(Test, bool)> {
        self.tests.remove(ident)
    }

    /// Gets the test with the given identifier.
    ///
    /// # Returns
    /// Returns the test and matched flag with the given identifier or `None` if
    /// it wasn't found.
    pub fn get<Q>(&self, ident: &Q) -> Option<(&Test, &bool)>
    where
        Q: ?Sized + Eq + Hash,
        Ident: Borrow<Q>,
    {
        self.tests
            .get(ident)
            .map(|(test, is_matched)| (test, is_matched))
    }

    /// Gets the test with the given identifier.
    ///
    /// # Returns
    /// Returns the test and matched flag with the given identifier or `None` if
    /// it wasn't found.
    pub fn get_mut<Q>(&mut self, ident: &Q) -> Option<(&mut Test, &mut bool)>
    where
        Q: ?Sized + Eq + Hash,
        Ident: Borrow<Q>,
    {
        self.tests
            .get_mut(ident)
            .map(|(test, is_matched)| (test, is_matched))
    }
}

impl Suite {
    /// Returns an iterator which yields all tests.
    pub fn tests(&self) -> Tests<'_> {
        Tests {
            iter: self.tests.values(),
            matched: true,
        }
    }

    /// Returns an iterator which yields all filtered tests.
    pub fn tests_filtered(&self) -> Tests<'_> {
        Tests {
            iter: self.tests.values(),
            matched: false,
        }
    }

    /// Returns an iterator which yields all unit tests.
    pub fn unit_tests(&self) -> UnitTests<'_> {
        UnitTests(self.tests())
    }

    /// Returns an iterator which yields all filtered unit tests.
    pub fn unit_tests_filtered(&self) -> UnitTests<'_> {
        UnitTests(self.tests_filtered())
    }

    /// Returns an iterator which yields all template tests.
    pub fn template_tests(&self) -> TemplateTests<'_> {
        TemplateTests(self.tests())
    }

    /// Returns an iterator which yields all filtered template tests.
    pub fn template_tests_filtered(&self) -> TemplateTests<'_> {
        TemplateTests(self.tests_filtered())
    }

    /// Returns an iterator which yields all doc tests.
    pub fn doc_tests(&self) -> DocTests<'_> {
        DocTests(self.tests())
    }

    /// Returns an iterator which yields all filtered doc tests.
    pub fn doc_tests_filtered(&self) -> DocTests<'_> {
        DocTests(self.tests_filtered())
    }
}

impl Default for Suite {
    fn default() -> Self {
        Self::new()
    }
}

impl<Q> Index<&Q> for Suite
where
    Q: ?Sized + Eq + Hash + Debug,
    Ident: Borrow<Q>,
{
    type Output = Test;

    fn index(&self, index: &Q) -> &Self::Output {
        let Some((test, _)) = self.get(index) else {
            panic!("test with identifier {index:?} not found in suite");
        };

        test
    }
}

impl<Q> IndexMut<&Q> for Suite
where
    Q: ?Sized + Eq + Hash + Debug,
    Ident: Borrow<Q>,
{
    fn index_mut(&mut self, index: &Q) -> &mut Self::Output {
        let Some((test, _)) = self.get_mut(index) else {
            panic!("test with identifier {index:?} not found in suite");
        };

        test
    }
}

/// An iterator returned by [`Suite::tests`] and [`Suite::tests_filtered`].
#[derive(Debug)]
pub struct Tests<'a> {
    iter: hash_map::Values<'a, Ident, (Test, bool)>,
    matched: bool,
}

impl<'a> Iterator for Tests<'a> {
    type Item = &'a Test;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter
            .find_map(|(t, m)| (*m == self.matched).then_some(t))
    }
}

/// An iterator returned by [`Suite::unit_tests`] and
/// [`Suite::unit_tests_filtered`].
#[derive(Debug)]
pub struct UnitTests<'a>(Tests<'a>);

impl<'a> Iterator for UnitTests<'a> {
    type Item = &'a UnitTest;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.find_map(Test::as_unit)
    }
}

/// An iterator returned by [`Suite::template_tests`] and
/// [`Suite::template_tests_filtered`].
#[derive(Debug)]
pub struct TemplateTests<'a>(Tests<'a>);

impl<'a> Iterator for TemplateTests<'a> {
    type Item = &'a TemplateTest;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.find_map(Test::as_template)
    }
}

/// An iterator returned by [`Suite::doc_tests`] and
/// [`Suite::doc_tests_filtered`].
#[derive(Debug)]
pub struct DocTests<'a>(Tests<'a>);

impl<'a> Iterator for DocTests<'a> {
    type Item = &'a DocTest;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.find_map(Test::as_doc)
    }
}
