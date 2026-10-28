//! Loading and filtering of test suites, suites contain test and supplementary
//! fields for test templates, custom test set bindings and other information
//! necessary for managing, filtering, and running tests.

use camino::Utf8Path;
use camino::Utf8PathBuf;
use chrono::DateTime;
use chrono::TimeDelta;
use chrono::Utc;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::btree_map;
use std::io;
use std::option;

use thiserror::Error;
use tytanic_utils::result::ResultEx;
use tytanic_utils::result::io_not_found;
use uuid::Uuid;

use crate::filter::Filter;
use crate::filter::FilterState;
use crate::project::Project;
use crate::test::DocId;
use crate::test::DocTest;
use crate::test::IdRef;
use crate::test::ParseIdError;
use crate::test::TemplateTest;
use crate::test::TemplateTestLoadError;
use crate::test::TestRef;
use crate::test::TestResult;
use crate::test::UnitId;
use crate::test::UnitTest;
use crate::test::UnitTestLoadError;

/// A suite of tests.
#[derive(Debug, Clone)]
pub struct Suite {
    unit_tests: BTreeMap<UnitId, UnitTest>,
    nested_unit_tests: BTreeMap<UnitId, UnitTest>,
    doc_tests: BTreeMap<DocId, DocTest>,
    template_test: Option<TemplateTest>,
}

impl Suite {
    /// Creates a new empty suite.
    pub fn new() -> Self {
        Self {
            unit_tests: BTreeMap::new(),
            nested_unit_tests: BTreeMap::new(),
            doc_tests: BTreeMap::new(),
            template_test: None,
        }
    }

    /// Creates a new suite with the given tests.
    pub fn from_tests<U, D, T>(unit_tests: U, doc_tests: D, template_test: T) -> Self
    where
        U: IntoIterator<Item = UnitTest>,
        D: IntoIterator<Item = DocTest>,
        T: Into<Option<TemplateTest>>,
    {
        Self {
            unit_tests: unit_tests
                .into_iter()
                .map(|t| (t.id().clone(), t))
                .collect(),
            nested_unit_tests: BTreeMap::new(),
            doc_tests: doc_tests.into_iter().map(|t| (t.id().clone(), t)).collect(),
            template_test: template_test.into(),
        }
    }

    /// Recursively collects entries in the given directory.
    #[tracing::instrument(skip_all)]
    pub fn collect(project: &Project) -> Result<Self, Error> {
        let mut this = Self::new();

        if let Some(test) =
            TemplateTest::load(project).ignore(|e| matches!(e, TemplateTestLoadError::NotFound))?
        {
            tracing::debug!("found template test");
            this.template_test = Some(test);
        }

        let root = project.unit_tests_root();
        let Some(read_dir) = root.read_dir_utf8().ignore(io_not_found)? else {
            tracing::debug!(?root, "test root not found, ignoring");
            return Ok(this);
        };

        tracing::debug!(?root, "test root found, collecting top level entries");
        for entry in read_dir {
            let entry = entry?;

            if entry.metadata()?.is_dir() {
                let abs = entry.path();
                let rel = abs
                    .strip_prefix(project.unit_tests_root())
                    .expect("entry must be in full");

                this.collect_dir(project, rel)?;
            }
        }

        let without_leaves: BTreeSet<_> = this
            .unit_tests
            .keys()
            // TODO(id): Will this work on windows and Linux?
            .flat_map(|test| Utf8Path::new(test).ancestors().skip(1))
            .map(|test| test.to_owned())
            .collect();

        let all: BTreeSet<_> = this
            .unit_tests
            .keys()
            // TODO(id): Will this work on windows and Linux?
            .map(|test| Utf8PathBuf::from(test.as_str()))
            .collect();

        for id in all.intersection(&without_leaves) {
            if let Some((id, test)) = this.unit_tests.remove_entry(id.as_str()) {
                this.nested_unit_tests.insert(id, test);
            }
        }

        if !this.nested_unit_tests.is_empty() {
            tracing::trace!(nested = ?this.nested_unit_tests, "found nested tests");
        }

        Ok(this)
    }

    /// Recursively collect tests in the given directory.
    fn collect_dir(&mut self, project: &Project, dir: &Utf8Path) -> Result<(), Error> {
        let abs = project.unit_tests_root().join(dir);

        if dir.file_name().is_some_and(|p| p.starts_with('.')) {
            tracing::debug!(?dir, "skipping hidden directory");
            return Ok(());
        }

        let id = match UnitId::new_from_path(dir) {
            Ok(id) => id,
            Err(err) => {
                tracing::error!(?dir, ?err, "ignoring test with invalid id");
                return Ok(());
            }
        };

        tracing::trace!(?dir, "checking for test");
        if let Some(test) = UnitTest::load(project, id.clone())
            .ignore(|e| matches!(e, UnitTestLoadError::NotFound(_)))?
        {
            tracing::debug!(id = %test.id(), "collected test");
            self.unit_tests.insert(id, test);
        }

        tracing::trace!(?dir, "collecting sub directories");
        for entry in abs.read_dir_utf8()? {
            let entry = entry?;

            if entry.metadata()?.is_dir() {
                let abs = entry.path();
                let rel = abs
                    .strip_prefix(project.unit_tests_root())
                    .expect("entry must be in full");

                self.collect_dir(project, rel)?;
            }
        }

        Ok(())
    }
}

impl Suite {
    /// All tests in this suite.
    pub fn tests(&self) -> Tests<'_> {
        Tests {
            unit_iter: self.unit_tests.values(),
            doc_iter: self.doc_tests.values(),
            template_iter: self.template_test.iter(),
        }
    }

    /// The unit tests in this suite.
    pub fn unit_tests(&self) -> UnitTests<'_> {
        UnitTests {
            iter: self.unit_tests.values(),
        }
    }

    /// The unit tests in this suite.
    pub fn doc_tests(&self) -> DocTests<'_> {
        DocTests {
            iter: self.doc_tests.values(),
        }
    }

    /// The template test, if it exists.
    pub fn template_test(&self) -> Option<&TemplateTest> {
        self.template_test.as_ref()
    }

    /// The nested tests, those which contain other tests and need to be
    /// migrated.
    ///
    /// This is a temporary method and will be removed in a future release.
    pub fn nested(&self) -> &BTreeMap<UnitId, UnitTest> {
        &self.nested_unit_tests
    }

    /// Returns the test with the given id.
    pub fn get<'id, I>(&self, id: I) -> Option<TestRef<'_>>
    where
        I: Into<IdRef<'id>>,
    {
        fn inner<'s>(this: &'s Suite, id: IdRef<'_>) -> Option<TestRef<'s>> {
            match id {
                IdRef::Template(_) => this.template_test.as_ref().map(TestRef::Template),
                IdRef::Unit(id) => this.unit_tests.get(id).map(TestRef::Unit),
                IdRef::Doc(id) => this.doc_tests.get(id).map(TestRef::Doc),
            }
        }

        inner(self, id.into())
    }

    /// Returns true if a test is contained in this suite.
    pub fn contains<'id, I>(&self, id: I) -> bool
    where
        I: Into<IdRef<'id>>,
    {
        self.get(id).is_some()
    }

    /// Returns the total number of tests in this suite.
    pub fn len(&self) -> usize {
        self.unit_tests.len()
    }

    /// Whether this suite is empty.
    pub fn is_empty(&self) -> bool {
        self.unit_tests.len() == 0
    }
}

impl Suite {
    /// Apply a filter to a suite.
    pub fn filter<F, E>(self, project: &Project, filter: F) -> Result<FilteredSuite<F>, E>
    where
        F: for<'a> Filter<State<'a>: FilterState<Error = E>>,
    {
        tracing::warn!(
            "ignoring {} nested tests while filtering",
            self.nested_unit_tests.len()
        );

        let mut filtered = Suite::new();
        let mut matched = Suite::new();

        let mut state = filter.state();

        for (id, test) in self.unit_tests.clone() {
            if state.filter(project, &test)? {
                matched.unit_tests.insert(id, test);
            } else {
                filtered.unit_tests.insert(id, test);
            }
        }

        for (id, test) in self.doc_tests.clone() {
            if state.filter(project, &test)? {
                matched.doc_tests.insert(id, test);
            } else {
                filtered.doc_tests.insert(id, test);
            }
        }

        if let Some(test) = self.template_test.clone() {
            if state.filter(project, &test)? {
                matched.template_test = Some(test);
            } else {
                filtered.template_test = Some(test);
            }
        }

        state.finish(project)?;

        Ok(FilteredSuite {
            raw: self,
            filter,
            matched,
            filtered,
        })
    }
}

impl Default for Suite {
    fn default() -> Self {
        Self::new()
    }
}

impl<'s> IntoIterator for &'s Suite {
    type IntoIter = Tests<'s>;
    type Item = TestRef<'s>;

    fn into_iter(self) -> Self::IntoIter {
        self.tests()
    }
}

/// Returned by [`Suite::tests`].
#[derive(Debug)]
pub struct Tests<'s> {
    unit_iter: btree_map::Values<'s, UnitId, UnitTest>,
    doc_iter: btree_map::Values<'s, DocId, DocTest>,
    template_iter: option::Iter<'s, TemplateTest>,
}

impl<'s> Iterator for Tests<'s> {
    type Item = TestRef<'s>;

    fn next(&mut self) -> Option<Self::Item> {
        self.unit_iter
            .next()
            .map(TestRef::Unit)
            .or_else(|| self.doc_iter.next().map(TestRef::Doc))
            .or_else(|| self.template_iter.next().map(TestRef::Template))
    }
}

/// Returned by [`Suite::unit_tests`].
#[derive(Debug)]
pub struct UnitTests<'s> {
    iter: btree_map::Values<'s, UnitId, UnitTest>,
}

impl<'s> Iterator for UnitTests<'s> {
    type Item = &'s UnitTest;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Returned by [`Suite::doc_tests`].
#[derive(Debug)]
pub struct DocTests<'s> {
    iter: btree_map::Values<'s, DocId, DocTest>,
}

impl<'s> Iterator for DocTests<'s> {
    type Item = &'s DocTest;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// A suite of tests with a filter applied to it.
#[derive(Debug, Clone)]
pub struct FilteredSuite<F> {
    raw: Suite,
    filter: F,
    matched: Suite,
    filtered: Suite,
}

impl<F> FilteredSuite<F> {
    /// The unfiltered inner suite.
    pub fn inner(&self) -> &Suite {
        &self.raw
    }

    /// The filter that was used to filter the tests.
    pub fn filter(&self) -> &F {
        &self.filter
    }

    /// The matched suite, contains only those test which _weren't_ filtered out.
    pub fn matched(&self) -> &Suite {
        &self.matched
    }

    /// The filtered suite, contains only those test which _were_ filtered out.
    pub fn filtered(&self) -> &Suite {
        &self.filtered
    }
}

/// Returned by [`Suite::collect`].
#[derive(Debug, Error)]
pub enum Error {
    /// An error occurred while trying to parse a test [`Id`][id].
    ///
    /// [id]: crate::test::Id
    #[error("an error occurred while collecting a test")]
    Id(#[from] ParseIdError),

    /// An error occurred while trying to collect the template test.
    #[error("an error occurred while collecting the template test")]
    LoadTemplateTest(#[from] TemplateTestLoadError),

    /// An error occurred while trying to collect a unit test.
    #[error("an error occurred while collecting a unit test")]
    LoadUnitTest(#[from] UnitTestLoadError),

    /// An IO error occurred.
    #[error("an io error occurred")]
    Io(#[from] io::Error),
}

/// The result of a test suite run, this contains results for all tests in a
/// suite, including filtered and not-yet-run tests, as well as cached values
/// for the number of filtered, passed and failed tests.
#[derive(Debug, Clone)]
pub struct SuiteResult<'s> {
    id: Uuid,
    total: usize,
    filtered: usize,
    passed: usize,
    failed: usize,
    timestamp: DateTime<Utc>,
    duration: TimeDelta,
    results: BTreeMap<IdRef<'s>, TestResult>,
}

impl<'s> SuiteResult<'s> {
    /// Create a fresh result for a suite, this will have pre-filled results for
    /// all test set to canceled, these results can be overridden while running
    /// the suite.
    pub fn new<F>(suite: &'s FilteredSuite<F>) -> Self {
        Self {
            id: Uuid::new_v4(),
            total: suite.inner().len(),
            filtered: suite.filtered().len(),
            passed: 0,
            failed: 0,
            timestamp: Utc::now(),
            duration: TimeDelta::zero(),
            results: suite
                .matched()
                .tests()
                .map(|test| (test.id(), TestResult::skipped()))
                .chain(
                    suite
                        .filtered()
                        .tests()
                        .map(|test| (test.id(), TestResult::filtered())),
                )
                .collect(),
        }
    }
}

impl<'s> SuiteResult<'s> {
    /// The unique id of this run.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// The total number of tests in the suite, including filtered ones.
    pub fn total(&self) -> usize {
        self.total
    }

    /// The number of tests in the suite which were expected to run, i.e. the
    /// number of tests which were _not_ filtered out.
    pub fn expected(&self) -> usize {
        self.total - self.filtered
    }

    /// The number of tests in the suite which were run, regardless of outcome.
    pub fn run(&self) -> usize {
        self.passed + self.failed
    }

    /// The number of tests in the suite which were filtered out.
    pub fn filtered(&self) -> usize {
        self.filtered
    }

    /// The number of tests in the suite which were _not_ run due to
    /// cancellation.
    pub fn skipped(&self) -> usize {
        self.expected() - self.run()
    }

    /// The number of tests in the suite which passed.
    pub fn passed(&self) -> usize {
        self.passed
    }

    /// The number of tests in the suite which failed.
    pub fn failed(&self) -> usize {
        self.failed
    }

    /// The timestamp at which the suite run started.
    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    /// The duration of the whole suite run.
    pub fn duration(&self) -> TimeDelta {
        self.duration
    }

    /// The individual test results.
    ///
    /// This contains results for all tests in the a suite, not just those added
    /// in [`SuiteResult::set_test_result`].
    pub fn results(&self) -> &BTreeMap<IdRef<'s>, TestResult> {
        &self.results
    }

    /// Whether this suite can be considered a complete pass.
    pub fn is_complete_pass(&self) -> bool {
        self.expected() == self.passed()
    }
}

impl<'s> SuiteResult<'s> {
    /// Sets the timestamp to [`Utc::now`].
    ///
    /// See [`SuiteResult::end`].
    pub fn start(&mut self) {
        self.timestamp = Utc::now();
    }

    /// Sets the duration to the time elapsed since [`SuiteResult::start`] was
    /// called.
    pub fn end(&mut self) {
        self.duration = Utc::now().signed_duration_since(self.timestamp);
    }

    /// Add a test result.
    ///
    /// - This should only add results for each test once, otherwise the test
    ///   will be counted multiple times.
    /// - The results should also only contain failures or passes, cancellations
    ///   and filtered results are ignored, as these are pre-filled when the
    ///   result is constructed.
    pub fn set_test_result(&mut self, id: IdRef<'s>, result: TestResult) {
        debug_assert!(self.results.contains_key(&id));
        debug_assert!(result.is_pass() || result.is_fail());

        if result.is_pass() {
            self.passed += 1;
        } else {
            self.failed += 1;
        }

        self.results.insert(id, result);
    }
}

#[cfg(test)]
mod tests {
    use ecow::eco_vec;
    use tytanic_utils::fs::TempTestEnv;

    use super::*;
    use crate::test::Annotation;
    use crate::test::UnitKind;

    #[test]
    fn test_collect() {
        TempTestEnv::run_no_check(
            |root| {
                root
                    // compile only
                    .setup_file("tests/.hidden/test.typ", "Not loaded")
                    .setup_file("tests/ignored!/test.typ", "Ignored")
                    .setup_file("tests/compile-only/test.typ", "Hello World")
                    // regular ephemeral
                    .setup_file("tests/compare/ephemeral/test.typ", "Hello World")
                    .setup_file("tests/compare/ephemeral/ref.typ", "Hello\nWorld")
                    // ephemeral despite ref directory
                    .setup_file("tests/compare/ephemeral-store/test.typ", "Hello World")
                    .setup_file("tests/compare/ephemeral-store/ref.typ", "Hello\nWorld")
                    .setup_file("tests/compare/ephemeral-store/ref", "Blah Blah")
                    // persistent
                    .setup_file("tests/compare/persistent/test.typ", "Hello World")
                    .setup_file("tests/compare/persistent/ref", "Blah Blah")
                    // not a test
                    .setup_file_empty("tests/not-a-test/test.txt")
                    // ignored test
                    .setup_file("tests/ignored/test.typ", "/// [skip]\nHello World")
            },
            |root| {
                let project = Project::new(root);
                let suite = Suite::collect(&project).unwrap();

                let tests = [
                    ("compile-only", UnitKind::CompileOnly, eco_vec![]),
                    ("compare/ephemeral", UnitKind::Ephemeral, eco_vec![]),
                    ("compare/ephemeral-store", UnitKind::Ephemeral, eco_vec![]),
                    ("compare/persistent", UnitKind::Persistent, eco_vec![]),
                    ("ignored", UnitKind::CompileOnly, eco_vec![Annotation::Skip]),
                ];

                for (key, kind, annotations) in tests {
                    let test = &suite.unit_tests[key];
                    assert_eq!(test.annotations(), &annotations[..]);
                    assert_eq!(test.kind(), kind);
                }
            },
        );
    }

    #[test]
    fn test_collect_nested() {
        TempTestEnv::run_no_check(
            |root| {
                root.setup_file("tests/foo/test.typ", "Hello World")
                    .setup_file("tests/foo/bar/test.typ", "Hello World")
                    .setup_file("tests/qux/test.typ", "Hello World")
                    .setup_file("tests/qux/quux/quz/test.typ", "Hello World")
            },
            |root| {
                let project = Project::new(root);
                let suite = Suite::collect(&project).unwrap();

                assert!(suite.nested_unit_tests.contains_key("foo"));
                assert!(suite.nested_unit_tests.contains_key("qux"));

                assert!(suite.unit_tests.contains_key("foo/bar"));
                assert!(suite.unit_tests.contains_key("qux/quux/quz"));
            },
        );
    }
}
