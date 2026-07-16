//! Test identifier and related types.
//!
//! # Test Identifiers
//!
//! There are three kinds of test identifiers:
//! ```ebnf
//! test_id ::= template_test_id | doc_test_id | unit_test_id;
//!
//! template_test_id ::= '@', keyword;
//! unit_test_id ::= path;
//! doc_test_id ::= path, '#', fragment, [ ':', fragment ];
//!
//! path ::= fragment, { '/', item };
//! fragment ::= (XID_START | '_'), { (XID_CONTINUE | '-' | '_') };
//! keyword ::= ASCII_ALPHA, { ASCII_ALPHA };
//! ```
//!
//! Where `XID_START` and `XID_CONTINUE` are equivalent to `XID_Start` and
//! `XID_Continue` respectively according to [Unicode Standard Annex #31][tr31].
//!
//! **Template test identifiers**
//! Template tests have unique keyword identifiers indicated by a leading `@`
//! followed by a unique keyword. At the moment, template tests are identified
//! as `@template` because there is always just a single template test.
//!
//! **Unit test identifiers** are simple paths, as they refer to unit test
//! directories within the project test root. A unit test defined in
//! `foo/bar/test.typ` would be identified by `foo/bar`.
//!
//! **Doc test identifiers** are similar to unit test identifiers with
//! additional fragments to refer to the item and example block the test is
//! located in. For a function `raw-to-xy` in `core/coords.typ` with an example
//! block named `ex1` the doc test identifier would be
//! `core/coords#raw-to-xy:ex1`
//!
//! # Tests
//! **Template tests**
//! Template tests are special tests which are compiled in the context of a
//! freshly initialized template project.
//!
//! **Unit tests**
//! Unit tests are standalone Typst documents which are compiled and optionally
//! compared to references.
//!
//! **Doc tests are not yet supported.**
//! Doc tests are example snippets in Typst documentation which are executed in
//! the context of a document that imported the package.
//!
//! [tr31]: https://www.unicode.org/reports/tr31/

use std::fmt::Display;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Write;

use chrono::DateTime;
use chrono::TimeDelta;
use chrono::Utc;
use ecow::EcoString;

use ecow::EcoVec;
use ecow::eco_vec;
use thiserror::Error;
use typst::diag::SourceDiagnostic;
use typst::syntax::FileId;
use typst::syntax::RootedPath;
use typst::syntax::Source;
use typst::syntax::VirtualPath;
use typst::syntax::VirtualRoot;

use crate::Project;
use crate::doc;
use crate::doc::Document;
use crate::doc::SaveError;
use crate::doc::compare;
use crate::doc::compile;
use crate::project::vcs;

mod annotation;
mod id;

pub use annotation::Annotation;
pub use annotation::ParseAnnotationError;
pub use id::DocId;
pub use id::Id;
pub use id::IdRef;
pub use id::Kind as IdKind;
pub use id::Lexer as IdLexer;
pub use id::ParseIdError;
pub use id::TEMPLATE_ID;
pub use id::TemplateId;
pub use id::TokenKind as IdTokenKind;
pub use id::UnitId;
pub use id::try_lex_id;

// NOTE(tinger): The order of ignoring and deleting/creating documents is not
// random, this is specifically for VCS like jj with active watchman triggers
// and auto snapshotting.
//
// This is currently untested though.

/// The default test input as source code.
pub const DEFAULT_TEST_INPUT: &str = include_str!("default-test.typ");

/// The default test output as an encoded PNG.
pub const DEFAULT_TEST_OUTPUT: &[u8] = include_bytes!("default-test.png");

/// References for a unit test.
#[derive(Debug, Clone)]
pub enum UnitReference {
    /// An ephemeral reference script used to compile the reference document on
    /// the fly.
    Ephemeral(EcoString),

    /// Persistent references which are stored on disk.
    Persistent {
        /// The reference document.
        doc: Document,

        /// The optimization options to use when storing the document, `None`
        /// disables optimization.
        opt: Option<Box<oxipng::Options>>,
    },
}

impl UnitReference {
    /// The kind of this reference.
    pub fn kind(&self) -> UnitKind {
        match self {
            Self::Ephemeral(_) => UnitKind::Ephemeral,
            Self::Persistent { .. } => UnitKind::Persistent,
        }
    }
}

/// A test.
#[derive(Debug, Clone)]
pub enum Test {
    /// A template test.
    Template(TemplateTest),

    /// A unit test.
    Unit(UnitTest),

    /// A doc test.
    Doc(DocTest),
}

impl Test {
    /// Creates a new template test.
    pub fn new_template<I>(id: I) -> Self
    where
        I: Into<TemplateId>,
    {
        Self::Template(TemplateTest::new(id))
    }

    /// Creates a new template test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new_template<S>(id: S) -> Result<Self, ParseIdError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        TemplateTest::try_new(id).map(Self::Template)
    }

    /// Creates a new unit test.
    pub fn new_unit<I>(id: I, kind: UnitKind) -> Self
    where
        I: Into<UnitId>,
    {
        Self::Unit(UnitTest::new(id, kind))
    }

    /// Creates a new unit test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new_unit<S>(id: S, kind: UnitKind) -> Result<Self, ParseIdError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        UnitTest::try_new(id, kind).map(Self::Unit)
    }

    /// Creates a new doc test.
    pub fn new_doc<I>(id: I) -> Self
    where
        I: Into<DocId>,
    {
        Self::Doc(DocTest::new(id))
    }

    /// Creates a new doc test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new_doc<S>(id: S) -> Result<Self, ParseIdError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        DocTest::try_new(id).map(Self::Doc)
    }
}

impl Test {
    /// The identifier of this test.
    pub fn id(&self) -> IdRef<'_> {
        match self {
            Self::Doc(test) => test.id().into(),
            Self::Template(test) => test.id().into(),
            Self::Unit(test) => test.id().into(),
        }
    }
}

impl Test {
    /// The kind of this test.
    pub fn kind(&self) -> Kind {
        match self {
            Self::Template(_) => Kind::Template,
            Self::Unit(unit) => Kind::Unit(unit.kind()),
            Self::Doc(_) => Kind::Doc,
        }
    }

    /// Whether this is a valid [`TemplateTest`].
    pub fn is_template(&self) -> bool {
        self.kind().is_template()
    }

    /// Returns the inner [`TemplateTest`] or `None` if its a different kind.
    pub fn as_template(&self) -> Option<&TemplateTest> {
        match self {
            Self::Template(test) => Some(test),
            _ => None,
        }
    }

    /// Whether this is a valid [`UnitTest`].
    pub fn is_unit(&self) -> bool {
        self.kind().is_unit()
    }

    /// Returns the inner [`UnitTest`] or `None` if its a different kind.
    pub fn as_unit(&self) -> Option<&UnitTest> {
        match self {
            Self::Unit(test) => Some(test),
            _ => None,
        }
    }

    /// Whether this is a valid [`DocTest`].
    pub fn is_doc(&self) -> bool {
        self.kind().is_doc()
    }

    /// Returns the inner [`DocTest`] or `None` if its a different kind.
    pub fn as_doc(&self) -> Option<&DocTest> {
        match self {
            Self::Doc(test) => Some(test),
            _ => None,
        }
    }

    /// Returns a [`TestRef`] borrowing the inner test.
    pub fn as_test_ref(&self) -> TestRef<'_> {
        match self {
            Self::Template(test) => TestRef::Template(test),
            Self::Unit(test) => TestRef::Unit(test),
            Self::Doc(test) => TestRef::Doc(test),
        }
    }
}

impl From<TestRef<'_>> for Test {
    fn from(value: TestRef<'_>) -> Self {
        match value {
            TestRef::Template(test) => Self::Template(test.clone()),
            TestRef::Unit(test) => Self::Unit(test.clone()),
            TestRef::Doc(test) => Self::Doc(test.clone()),
        }
    }
}

/// The reference version of [`Test`].
#[derive(Debug, Clone, Copy)]
pub enum TestRef<'t> {
    /// A template test.
    Template(&'t TemplateTest),

    /// A unit test.
    Unit(&'t UnitTest),

    /// A doc test.
    Doc(&'t DocTest),
}

impl<'t> TestRef<'t> {
    /// The identifier of this test.
    pub fn id(self) -> IdRef<'t> {
        match self {
            Self::Doc(test) => test.id().into(),
            Self::Template(test) => test.id().into(),
            Self::Unit(test) => test.id().into(),
        }
    }
}

impl<'t> TestRef<'t> {
    /// The kind of this test.
    pub fn kind(self) -> Kind {
        match self {
            Self::Template(_) => Kind::Template,
            Self::Unit(unit) => Kind::Unit(unit.kind()),
            Self::Doc(_) => Kind::Doc,
        }
    }

    /// Whether this is a valid [`TemplateTest`].
    pub fn is_template(self) -> bool {
        self.kind().is_template()
    }

    /// Returns the inner [`TemplateTest`] or `None` if its a different kind.
    pub fn as_template(self) -> Option<&'t TemplateTest> {
        match self {
            Self::Template(test) => Some(test),
            _ => None,
        }
    }

    /// Whether this is a valid [`UnitTest`].
    pub fn is_unit(self) -> bool {
        self.kind().is_unit()
    }

    /// Returns the inner [`UnitTest`] or `None` if its a different kind.
    pub fn as_unit(self) -> Option<&'t UnitTest> {
        match self {
            Self::Unit(test) => Some(test),
            _ => None,
        }
    }

    /// Whether this is a valid [`DocTest`].
    pub fn is_doc(self) -> bool {
        self.kind().is_doc()
    }

    /// Returns the inner [`DocTest`] or `None` if its a different kind.
    pub fn as_doc(self) -> Option<&'t DocTest> {
        match self {
            Self::Doc(test) => Some(test),
            _ => None,
        }
    }
}

impl<'t> From<&'t Test> for TestRef<'t> {
    fn from(value: &'t Test) -> Self {
        match value {
            Test::Template(t) => Self::Template(t),
            Test::Unit(t) => Self::Unit(t),
            Test::Doc(t) => Self::Doc(t),
        }
    }
}

impl<'t> From<&'t TemplateTest> for TestRef<'t> {
    fn from(value: &'t TemplateTest) -> Self {
        Self::Template(value)
    }
}

impl<'t> From<&'t UnitTest> for TestRef<'t> {
    fn from(value: &'t UnitTest) -> Self {
        Self::Unit(value)
    }
}

impl<'t> From<&'t DocTest> for TestRef<'t> {
    fn from(value: &'t DocTest) -> Self {
        Self::Doc(value)
    }
}

/// A template test.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateTest {
    id: TemplateId,
    annotations: EcoVec<Annotation>,
}

impl TemplateTest {
    /// Creates a new template test.
    pub fn new<I>(id: I) -> Self
    where
        I: Into<TemplateId>,
    {
        Self {
            id: id.into(),
            annotations: eco_vec![],
        }
    }

    /// Creates a new template test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new<S>(id: S) -> Result<Self, ParseIdError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Ok(Self {
            id: TemplateId::new(id)?,
            annotations: eco_vec![],
        })
    }

    /// Attempts to load the template test for the given project.
    #[tracing::instrument(skip(project))]
    pub fn load(project: &Project) -> Result<Self, TemplateTestLoadError> {
        if project.template_entrypoint().is_none() {
            return Err(TemplateTestLoadError::NotFound);
        }

        Ok(Self {
            id: TEMPLATE_ID.clone(),
            annotations: eco_vec![],
        })
    }
}

impl TemplateTest {
    /// The identifier of this template test.
    pub fn id(&self) -> &TemplateId {
        &self.id
    }

    /// This test's annotations.
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }
}

impl TemplateTest {
    /// Loads the template entrypoint source for this test.
    #[tracing::instrument(skip(project))]
    pub fn load_source(&self, project: &Project) -> io::Result<Source> {
        let test_script = project
            .template_entrypoint()
            .expect("existence of template test ensures existence of entrypoint");

        Ok(Source::new(
            FileId::new(RootedPath::new(
                VirtualRoot::Project,
                VirtualPath::virtualize(project.root().as_std_path(), test_script.as_std_path())
                    .expect(
                        "Project::root and Project::template_entrypoint must never emit \
                         escaping or invalid paths",
                    ),
            )),
            fs::read_to_string(test_script)?,
        ))
    }
}

/// Returned by [`TemplateTest::load`].
#[derive(Debug, Error)]
pub enum TemplateTestLoadError {
    /// The project has no template test.
    #[error("the project has no template test")]
    NotFound,

    /// An io error occured.
    #[error("an io error occured")]
    Io(#[from] io::Error),
}

/// A unit test.
#[derive(Clone)]
pub enum UnitTest {
    /// A compile-only unit test.
    CompileOnly(CompileOnlyUnitTest),

    /// An ephemeral unit test.
    Ephemeral(EphemeralUnitTest),

    /// A persistent unit test.
    Persistent(PersistentUnitTest),
}

impl UnitTest {
    /// Creates a new unit test.
    pub fn new<I>(id: I, kind: UnitKind) -> Self
    where
        I: Into<UnitId>,
    {
        match kind {
            UnitKind::CompileOnly => UnitTest::CompileOnly(CompileOnlyUnitTest::new(id)),
            UnitKind::Ephemeral => UnitTest::Ephemeral(EphemeralUnitTest::new(id)),
            UnitKind::Persistent => UnitTest::Persistent(PersistentUnitTest::new(id)),
        }
    }

    /// Creates a new unit test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new<S>(id: S, kind: UnitKind) -> Result<Self, ParseIdError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Ok(match kind {
            UnitKind::CompileOnly => UnitTest::CompileOnly(CompileOnlyUnitTest::try_new(id)?),
            UnitKind::Ephemeral => UnitTest::Ephemeral(EphemeralUnitTest::try_new(id)?),
            UnitKind::Persistent => UnitTest::Persistent(PersistentUnitTest::try_new(id)?),
        })
    }

    /// Creates a new compile-only unit test.
    pub fn new_compile_only<I>(id: I) -> Self
    where
        I: Into<UnitId>,
    {
        Self::new(id, UnitKind::CompileOnly)
    }
    /// Creates a new compile-only unit test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new_compile_only<S>(id: S) -> Result<Self, ParseIdError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Self::try_new(id, UnitKind::CompileOnly)
    }

    /// Creates a new ephemeral unit test.
    pub fn new_ephemeral<I>(id: I) -> Self
    where
        I: Into<UnitId>,
    {
        Self::new(id, UnitKind::Ephemeral)
    }
    /// Creates a new ephemeral unit test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new_ephemeral<S>(id: S) -> Result<Self, ParseIdError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Self::try_new(id, UnitKind::Ephemeral)
    }

    /// Creates a new persistent unit test.
    pub fn new_persistent<I>(id: I) -> Self
    where
        I: Into<UnitId>,
    {
        Self::new(id, UnitKind::Persistent)
    }

    /// Creates a new persistent unit test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new_persistent<S>(id: S) -> Result<Self, ParseIdError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Self::try_new(id, UnitKind::Persistent)
    }

    /// Attempts to load the unit test for the given project.
    #[tracing::instrument(skip(project))]
    pub fn load<I>(project: &Project, id: I) -> Result<Self, UnitTestLoadError>
    where
        I: Into<UnitId> + std::fmt::Debug,
    {
        let id = id.into();
        let test_script = project.unit_test_script(&id);

        if !test_script.try_exists()? {
            return Err(UnitTestLoadError::NotFound(id));
        }

        let kind = if project.unit_test_ref_script(&id).try_exists()? {
            UnitKind::Ephemeral
        } else if project.unit_test_ref_dir(&id).try_exists()? {
            UnitKind::Persistent
        } else {
            UnitKind::CompileOnly
        };

        let annotations = Annotation::collect(&fs::read_to_string(test_script)?)?;

        Ok(match kind {
            UnitKind::CompileOnly => Self::CompileOnly(CompileOnlyUnitTest { id, annotations }),
            UnitKind::Ephemeral => Self::Ephemeral(EphemeralUnitTest { id, annotations }),
            UnitKind::Persistent => Self::Persistent(PersistentUnitTest { id, annotations }),
        })
    }

    /// Creates a new test on disk, the kind is inferred from the passed
    /// reference and annotations are parsed from the test script.
    ///
    /// # Panics
    /// Panics if the given id is the template test id.
    #[tracing::instrument(skip(project, source, reference))]
    pub fn create<I>(
        project: &Project,
        id: I,
        source: &str,
        reference: Option<UnitReference>,
    ) -> Result<Self, CreateError>
    where
        I: Into<UnitId> + std::fmt::Debug,
    {
        let id = id.into();
        let test_dir = project.unit_test_dir(&id);
        tytanic_utils::fs::create_dir(test_dir, true)?;

        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(project.unit_test_script(&id))?;

        file.write_all(source.as_bytes())?;

        let annotations = Annotation::collect(source)?;

        let this = match reference.as_ref().map(UnitReference::kind) {
            None | Some(UnitKind::CompileOnly) => {
                Self::CompileOnly(CompileOnlyUnitTest { id, annotations })
            }
            Some(UnitKind::Ephemeral) => Self::Ephemeral(EphemeralUnitTest { id, annotations }),
            Some(UnitKind::Persistent) => Self::Persistent(PersistentUnitTest { id, annotations }),
        };

        match reference {
            Some(UnitReference::Ephemeral(reference)) => {
                this.create_reference_script(project, reference.as_str())?;
            }
            Some(UnitReference::Persistent {
                doc: reference,
                opt: options,
            }) => {
                this.create_reference_document(project, &reference, options.as_deref())?;
            }
            None => {}
        }

        Ok(this)
    }
}

impl UnitTest {
    /// The identifier of this test.
    pub fn id(&self) -> &UnitId {
        match self {
            Self::CompileOnly(test) => test.id(),
            Self::Ephemeral(test) => test.id(),
            Self::Persistent(test) => test.id(),
        }
    }

    /// The unit test kind.
    pub fn kind(&self) -> UnitKind {
        match self {
            Self::CompileOnly(_) => UnitKind::CompileOnly,
            Self::Ephemeral(_) => UnitKind::Ephemeral,
            Self::Persistent(_) => UnitKind::Persistent,
        }
    }

    /// This test's annotations.
    pub fn annotations(&self) -> &[Annotation] {
        match self {
            Self::CompileOnly(test) => test.annotations(),
            Self::Ephemeral(test) => test.annotations(),
            Self::Persistent(test) => test.annotations(),
        }
    }

    /// Whether this test has a skip annotation.
    pub fn is_skip(&self) -> bool {
        self.annotations().contains(&Annotation::Skip)
    }
}

impl UnitTest {
    /// Creates the temporary directories of this test.
    #[tracing::instrument(skip(project))]
    pub fn create_temporary_directories(&self, project: &Project) -> io::Result<()> {
        let id = self.id();

        if self.kind().is_ephemeral() {
            tytanic_utils::fs::remove_dir(project.unit_test_ref_dir(id), true)?;
            tytanic_utils::fs::create_dir(project.unit_test_ref_dir(id), true)?;
        }

        tytanic_utils::fs::create_dir(project.unit_test_out_dir(id), true)?;

        if !self.kind().is_compile_only() {
            tytanic_utils::fs::create_dir(project.unit_test_diff_dir(id), true)?;
        }

        Ok(())
    }

    /// Creates the test script of this test, this will truncate the file if it
    /// already exists.
    #[tracing::instrument(skip(project, source))]
    pub fn create_script(&self, project: &Project, source: &str) -> io::Result<()> {
        std::fs::write(project.unit_test_script(self.id()), source)?;
        Ok(())
    }

    /// Creates reference script of this test, this will truncate the file if it
    /// already exists.
    #[tracing::instrument(skip(project, source))]
    pub fn create_reference_script(&self, project: &Project, source: &str) -> io::Result<()> {
        std::fs::write(project.unit_test_ref_script(self.id()), source)?;
        Ok(())
    }

    /// Creates the persistent reference document of this test.
    #[tracing::instrument(skip(project, reference, optimize_options))]
    pub fn create_reference_document(
        &self,
        project: &Project,
        reference: &Document,
        optimize_options: Option<&oxipng::Options>,
    ) -> Result<(), SaveError> {
        // NOTE(tinger): if there are already more pages than we want to create,
        // the surplus pages would persist and make every comparison fail due to
        // a page count mismatch, so we clear them to be sure.
        self.delete_reference_document(project)?;

        let ref_dir = project.unit_test_ref_dir(self.id());
        tytanic_utils::fs::create_dir(&ref_dir, true)?;
        reference.save(&ref_dir, optimize_options)?;

        Ok(())
    }

    /// Deletes all directories and scripts of this test.
    #[tracing::instrument(skip(project))]
    pub fn delete(&self, project: &Project) -> io::Result<()> {
        let id = self.id();
        self.delete_reference_document(project)?;
        self.delete_reference_script(project)?;
        self.delete_temporary_directories(project)?;

        tytanic_utils::fs::remove_file(project.unit_test_script(id))?;
        tytanic_utils::fs::remove_dir(project.unit_test_dir(id), true)?;

        Ok(())
    }

    /// Deletes the temporary directories of this test.
    #[tracing::instrument(skip(project))]
    pub fn delete_temporary_directories(&self, project: &Project) -> io::Result<()> {
        let id = self.id();

        if self.kind().is_ephemeral() {
            tytanic_utils::fs::remove_dir(project.unit_test_ref_dir(id), true)?;
        }

        tytanic_utils::fs::remove_dir(project.unit_test_out_dir(id), true)?;
        tytanic_utils::fs::remove_dir(project.unit_test_diff_dir(id), true)?;
        Ok(())
    }

    /// Deletes the test script of this test.
    #[tracing::instrument(skip(project))]
    pub fn delete_script(&self, project: &Project) -> io::Result<()> {
        tytanic_utils::fs::remove_file(project.unit_test_script(self.id()))?;
        Ok(())
    }

    /// Deletes reference script of this test.
    #[tracing::instrument(skip(project))]
    pub fn delete_reference_script(&self, project: &Project) -> io::Result<()> {
        tytanic_utils::fs::remove_file(project.unit_test_ref_script(self.id()))?;
        Ok(())
    }

    /// Deletes persistent reference document of this test.
    #[tracing::instrument(skip(project))]
    pub fn delete_reference_document(&self, project: &Project) -> io::Result<()> {
        tytanic_utils::fs::remove_dir(project.unit_test_ref_dir(self.id()), true)?;
        Ok(())
    }
}

impl UnitTest {
    /// Removes any previous references, if they exist and creates a reference
    /// script by copying the test script. The variant is changed to
    /// [`UnitTest::Ephemeral`].
    #[tracing::instrument(skip(project))]
    pub fn make_ephemeral(&mut self, project: &Project) -> io::Result<()> {
        let id = self.id().clone();

        // Ensure deletion is recorded before ignore file is updated.
        self.delete_reference_script(project)?;
        self.delete_reference_document(project)?;

        // Copy references after ignore file is updated.
        std::fs::copy(
            project.unit_test_script(&id),
            project.unit_test_ref_script(&id),
        )?;

        let annotations = self.annotations_owned();
        *self = Self::Ephemeral(EphemeralUnitTest { id, annotations });

        Ok(())
    }

    /// Removes any previous references, if they exist and creates persistent
    /// references from the given pages. The variant is changed to
    /// [`UnitTest::Persistent`].
    #[tracing::instrument(skip(project, reference, optimize_options))]
    pub fn make_persistent(
        &mut self,
        project: &Project,
        reference: &Document,
        optimize_options: Option<&oxipng::Options>,
    ) -> Result<(), SaveError> {
        let id = self.id().clone();

        // Ensure deletion/creation is recorded before ignore file is updated.
        self.delete_reference_script(project)?;

        let annotations = self.annotations_owned();
        *self = Self::Persistent(PersistentUnitTest { id, annotations });

        self.create_reference_document(project, reference, optimize_options)?;

        Ok(())
    }

    /// Removes any previous references, if they exist. The variant is changed
    /// to [`UnitTest::CompileOnly`].
    #[tracing::instrument(skip(project))]
    pub fn make_compile_only(&mut self, project: &Project) -> io::Result<()> {
        let id = self.id().clone();

        // Ensure deletion is recorded before ignore file is updated.
        self.delete_reference_document(project)?;
        self.delete_reference_script(project)?;

        let annotations = self.annotations_owned();
        *self = Self::CompileOnly(CompileOnlyUnitTest { id, annotations });

        Ok(())
    }

    fn annotations_owned(&self) -> EcoVec<Annotation> {
        match self {
            Self::CompileOnly(test) => test.annotations.clone(),
            Self::Ephemeral(test) => test.annotations.clone(),
            Self::Persistent(test) => test.annotations.clone(),
        }
    }
}

impl UnitTest {
    /// Loads the test script source of this test.
    #[tracing::instrument(skip(project))]
    pub fn load_source(&self, project: &Project) -> io::Result<Source> {
        let test_script = project.unit_test_script(self.id());

        Ok(Source::new(
            FileId::new(RootedPath::new(
                VirtualRoot::Project,
                VirtualPath::virtualize(project.root().as_std_path(), test_script.as_std_path())
                    .expect("Project and Test must never emit escaping or invalid paths"),
            )),
            std::fs::read_to_string(test_script)?,
        ))
    }

    /// Loads the reference test script source of this test, if this test is
    /// ephemeral.
    #[tracing::instrument(skip(project))]
    pub fn load_reference_source(&self, project: &Project) -> io::Result<Option<Source>> {
        if !self.kind().is_ephemeral() {
            return Ok(None);
        }

        let ref_script = project.unit_test_ref_script(self.id());
        Ok(Some(Source::new(
            FileId::new(RootedPath::new(
                VirtualRoot::Project,
                VirtualPath::virtualize(project.root().as_std_path(), ref_script.as_std_path())
                    .expect("Project and Test must never emit escaping or invalid paths"),
            )),
            std::fs::read_to_string(ref_script)?,
        )))
    }

    /// Loads the test document of this test.
    #[tracing::instrument(skip(project))]
    pub fn load_document(&self, project: &Project) -> Result<Document, doc::LoadError> {
        Document::load(project.unit_test_out_dir(self.id()))
    }

    /// Loads the persistent reference document of this test.
    #[tracing::instrument(skip(project))]
    pub fn load_reference_document(&self, project: &Project) -> Result<Document, doc::LoadError> {
        Document::load(project.unit_test_ref_dir(self.id()))
    }
}

impl UnitTest {
    /// Returns the inner compile-only test or `None` if this is another
    /// variant.
    pub fn as_compile_only(&self) -> Option<&CompileOnlyUnitTest> {
        match self {
            Self::CompileOnly(test) => Some(test),
            _ => None,
        }
    }

    /// Whether the inner test is a compile-only test.
    pub fn is_compile_only(&self) -> bool {
        self.as_compile_only().is_some()
    }

    /// Returns the inner ephemeral test or `None` if this is another variant.
    pub fn as_ephemeral(&self) -> Option<&EphemeralUnitTest> {
        match self {
            Self::Ephemeral(test) => Some(test),
            _ => None,
        }
    }

    /// Whether the inner test is a ephemeral test.
    pub fn is_ephemeral(&self) -> bool {
        self.as_ephemeral().is_some()
    }

    /// Returns the inner persistent test or `None` if this is another variant.
    pub fn as_persistent(&self) -> Option<&PersistentUnitTest> {
        match self {
            Self::Persistent(test) => Some(test),
            _ => None,
        }
    }

    /// Whether the inner test is a persistent test.
    pub fn is_persistent(&self) -> bool {
        self.as_persistent().is_some()
    }
}

impl std::fmt::Debug for UnitTest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CompileOnly(test) => std::fmt::Debug::fmt(&test, f),
            Self::Ephemeral(test) => std::fmt::Debug::fmt(&test, f),
            Self::Persistent(test) => std::fmt::Debug::fmt(&test, f),
        }
    }
}

/// The kind of a unit test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnitKind {
    /// A test without references.
    CompileOnly,

    /// A test with on-the-fly references.
    Ephemeral,

    /// A test with on-disk references.
    Persistent,
}

impl UnitKind {
    /// A string representing the unit test kind.
    pub fn as_str(&self) -> &'static str {
        match self {
            UnitKind::CompileOnly => "compile-only",
            UnitKind::Ephemeral => "ephemeral",
            UnitKind::Persistent => "persistent",
        }
    }

    /// Whether this is [`UnitKind::CompileOnly`].
    pub fn is_compile_only(&self) -> bool {
        matches!(self, Self::CompileOnly)
    }

    /// Whether this is [`UnitKind::Ephemeral`].
    pub fn is_ephemeral(&self) -> bool {
        matches!(self, Self::Ephemeral)
    }

    /// Whether this is [`UnitKind::Persistent`].
    pub fn is_persistent(&self) -> bool {
        matches!(self, Self::Persistent)
    }
}

/// Returned by [`UnitTest::load`].
#[derive(Debug, Error)]
pub enum UnitTestLoadError {
    /// The test id was not found.
    #[error("could not find unit test {0}")]
    NotFound(UnitId),

    /// An error occurred while parsing a test annotation.
    #[error("an error occurred while parsing a test annotation")]
    Annotation(#[from] ParseAnnotationError),

    /// An io error occured.
    #[error("an io error occured")]
    Io(#[from] io::Error),
}

/// Returned by [`UnitTest::create`].
#[derive(Debug, Error)]
pub enum CreateError {
    /// An error occurred while parsing a test annotation.
    #[error("an error occurred while parsing a test annotation")]
    Annotation(#[from] ParseAnnotationError),

    /// An error occurred while saving test files.
    #[error("an error occurred while saving test files")]
    Save(#[from] doc::SaveError),

    /// An error occurred while updating the VCS ignore file.
    #[error("an error occurred while updating the VCS ignore file")]
    Vcs(#[from] vcs::IgnoreDirectoryError),

    /// An IO error occurred.
    #[error("an io error occurred")]
    Io(#[from] io::Error),
}

/// A compile-only unit test.
///
/// A unit test without references.
#[derive(Debug, Clone)]
pub struct CompileOnlyUnitTest {
    id: UnitId,
    annotations: EcoVec<Annotation>,
}

impl CompileOnlyUnitTest {
    /// Creates a new compile-only test.
    pub fn new<I>(id: I) -> Self
    where
        I: Into<UnitId>,
    {
        Self {
            id: id.into(),
            annotations: eco_vec![],
        }
    }

    /// Creates a new compile-only test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new<S>(id: S) -> Result<Self, ParseIdError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Ok(Self {
            id: UnitId::new(id)?,
            annotations: eco_vec![],
        })
    }
}

impl CompileOnlyUnitTest {
    /// The identifier of this unit test.
    pub fn id(&self) -> &UnitId {
        &self.id
    }

    /// This test's annotations.
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }
}

/// An ephemeral unit test.
///
/// A unit test with on-the-fly references.
#[derive(Debug, Clone)]
pub struct EphemeralUnitTest {
    id: UnitId,
    annotations: EcoVec<Annotation>,
}

impl EphemeralUnitTest {
    /// Creates a new ephemeral test.
    pub fn new<I>(id: I) -> Self
    where
        I: Into<UnitId>,
    {
        Self {
            id: id.into(),
            annotations: eco_vec![],
        }
    }

    /// Creates a new ephemeral test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new<S>(id: S) -> Result<Self, ParseIdError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Ok(Self {
            id: UnitId::new(id)?,
            annotations: eco_vec![],
        })
    }
}

impl EphemeralUnitTest {
    /// The identifier of this unit test.
    pub fn id(&self) -> &UnitId {
        &self.id
    }

    /// This test's annotations.
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }
}

/// A persistent unit test.
///
/// A unit test with on-disk references.
#[derive(Debug, Clone)]
pub struct PersistentUnitTest {
    id: UnitId,
    annotations: EcoVec<Annotation>,
}

impl PersistentUnitTest {
    /// Creates a new persistent test.
    pub fn new<I>(id: I) -> Self
    where
        I: Into<UnitId>,
    {
        Self {
            id: id.into(),
            annotations: eco_vec![],
        }
    }

    /// Creates a new persistent test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new<S>(id: S) -> Result<Self, ParseIdError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Ok(Self {
            id: UnitId::new(id)?,
            annotations: eco_vec![],
        })
    }
}

impl PersistentUnitTest {
    /// The identifier of this unit test.
    pub fn id(&self) -> &UnitId {
        &self.id
    }

    /// This test's annotations.
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }
}

/// A doc test.
#[derive(Debug, Clone, PartialEq)]
pub struct DocTest {
    id: DocId,
}

impl DocTest {
    /// Creates a new doc test.
    pub fn new<I>(id: I) -> Self
    where
        I: Into<DocId>,
    {
        Self { id: id.into() }
    }

    /// Creates a new doc test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new<S>(id: S) -> Result<Self, ParseIdError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Ok(Self {
            id: DocId::new(id)?,
        })
    }
}

impl DocTest {
    /// The identifier of this test.
    pub fn id(&self) -> &DocId {
        &self.id
    }
}

/// The kind of a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Kind {
    /// A template test.
    Template,

    /// A unit test.
    Unit(UnitKind),

    /// A doc test.
    Doc,
}

impl Kind {
    /// A string representing the test kind.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Template => "template",
            Self::Unit(_) => "unit",
            Self::Doc => "doc",
        }
    }

    /// Whether this is [`Kind::Template`].
    pub fn is_template(&self) -> bool {
        matches!(self, Self::Template)
    }

    /// Whether this is [`Kind::Unit`].
    pub fn is_unit(&self) -> bool {
        matches!(self, Self::Unit(_))
    }

    /// Whether this is [`Kind::Doc`].
    pub fn is_doc(&self) -> bool {
        matches!(self, Self::Doc)
    }
}

impl Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(f)
    }
}

/// The stage of a single test run.
#[derive(Debug, Clone, Default)]
pub enum Stage {
    /// The test was canceled or not started in the first place.
    #[default]
    Skipped,

    /// The test was filtered out by a [`Filter`].
    ///
    /// [`Filter`]: crate::filter::Filter
    Filtered,

    /// The test failed compilation.
    FailedCompilation {
        /// The inner error.
        error: compile::Error,

        /// Whether this was a compilation failure of the reference.
        reference: bool,
    },

    /// The test passed compilation, but failed comparison.
    FailedComparison(compare::Error),

    /// The test passed compilation, but did not run comparison.
    PassedCompilation,

    /// The test passed compilation and comparison.
    PassedComparison,

    /// The test passed compilation and updated its references.
    Updated {
        /// Whether the references were optimized.
        optimized: bool,
    },
}

/// The result of a single test run.
#[derive(Debug, Clone)]
pub struct TestResult {
    stage: Stage,
    warnings: EcoVec<SourceDiagnostic>,
    timestamp: DateTime<Utc>,
    duration: TimeDelta,
}

impl TestResult {
    /// Create a result for a test for a skipped test. This will set the
    /// starting time to now, the duration to zero and the result to `None`.
    ///
    /// This can be used for constructing test results in advance to ensure an
    /// aborted test run contains a skip result for all yet-to-be-run tests.
    pub fn skipped() -> Self {
        Self {
            stage: Stage::Skipped,
            warnings: eco_vec![],
            timestamp: Utc::now(),
            duration: TimeDelta::zero(),
        }
    }

    /// Create a result for a test for a filtered test. This will set the
    /// starting time to now, the duration to zero and the result to filtered.
    pub fn filtered() -> Self {
        Self {
            stage: Stage::Filtered,
            warnings: eco_vec![],
            timestamp: Utc::now(),
            duration: TimeDelta::zero(),
        }
    }
}

impl TestResult {
    /// The stage of this rest result, if it was started.
    pub fn stage(&self) -> &Stage {
        &self.stage
    }

    /// The warnings of the test emitted by the compiler.
    pub fn warnings(&self) -> &[SourceDiagnostic] {
        &self.warnings
    }

    /// The timestamp at which the suite run started.
    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    /// The duration of the test, this a zero if this test wasn't started.
    pub fn duration(&self) -> TimeDelta {
        self.duration
    }

    /// Whether the test was not started.
    pub fn is_skipped(&self) -> bool {
        matches!(&self.stage, Stage::Skipped)
    }

    /// Whether the test was filtered out.
    pub fn is_filtered(&self) -> bool {
        matches!(&self.stage, Stage::Filtered)
    }

    /// Whether the test passed compilation and/or comparison/update.
    pub fn is_pass(&self) -> bool {
        matches!(
            &self.stage,
            Stage::PassedCompilation | Stage::PassedComparison | Stage::Updated { .. }
        )
    }

    /// Whether the test failed compilation or comparison.
    pub fn is_fail(&self) -> bool {
        matches!(
            &self.stage,
            Stage::FailedCompilation { .. } | Stage::FailedComparison(..),
        )
    }

    /// The errors emitted by the compiler if compilation failed.
    pub fn errors(&self) -> Option<&[SourceDiagnostic]> {
        match &self.stage {
            Stage::FailedCompilation { error, .. } => Some(&error.0),
            _ => None,
        }
    }
}

impl TestResult {
    /// Sets the timestamp to [`Utc::now`].
    ///
    /// See [`TestResult::end`].
    pub fn start(&mut self) {
        self.timestamp = Utc::now();
    }

    /// Sets the duration to the time elapsed since [`TestResult::start`] was
    /// called.
    pub fn end(&mut self) {
        self.duration = Utc::now().signed_duration_since(self.timestamp);
    }

    /// Sets the kind for this test to a compilation pass.
    pub fn set_passed_compilation(&mut self) {
        self.stage = Stage::PassedCompilation;
    }

    /// Sets the kind for this test to a reference compilation failure.
    pub fn set_failed_reference_compilation(&mut self, error: compile::Error) {
        self.stage = Stage::FailedCompilation {
            error,
            reference: true,
        };
    }

    /// Sets the kind for this test to a test compilation failure.
    pub fn set_failed_test_compilation(&mut self, error: compile::Error) {
        self.stage = Stage::FailedCompilation {
            error,
            reference: false,
        };
    }

    /// Sets the kind for this test to a test comparison pass.
    pub fn set_passed_comparison(&mut self) {
        self.stage = Stage::PassedComparison;
    }

    /// Sets the kind for this test to a comparison failure.
    pub fn set_failed_comparison(&mut self, error: compare::Error) {
        self.stage = Stage::FailedComparison(error);
    }

    /// Sets the kind for this test to a test update.
    pub fn set_updated(&mut self, optimized: bool) {
        self.stage = Stage::Updated { optimized };
    }

    /// Sets the warnings for this test.
    pub fn set_warnings<I>(&mut self, warnings: I)
    where
        I: Into<EcoVec<SourceDiagnostic>>,
    {
        self.warnings = warnings.into();
    }
}

impl Default for TestResult {
    fn default() -> Self {
        Self::skipped()
    }
}
