//! Test identifier and related types.
//!
//! # Test Identifiers
//!
//! There are three kinds of test identifiers:
//! ```ebnf
//! test_ident ::= template_test_ident | doc_test_ident | unit_test_ident;
//!
//! template_test_ident ::= '@', keyword;
//! unit_test_ident ::= path;
//! doc_test_ident ::= path, '#', fragment, [ ':', fragment ];
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

use ecow::EcoString;

use crate::config::TestConfig;

mod annot;
mod ident;

pub use annot::Annotation;
pub use annot::ParseAnnotationError;
pub use ident::DocIdent;
pub use ident::Ident;
pub use ident::Kind as IdentKind;
pub use ident::Lexer as IdentLexer;
pub use ident::ParseIdentError;
pub use ident::TEMPLATE_IDENT;
pub use ident::TemplateIdent;
pub use ident::TokenKind as IdentTokenKind;
pub use ident::UnitIdent;
pub use ident::try_lex_ident;

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
    pub fn new_template<I>(ident: I) -> Self
    where
        I: Into<TemplateIdent>,
    {
        Self::Template(TemplateTest::new(ident))
    }

    /// Creates a new template test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new_template<S>(ident: S) -> Result<Self, ParseIdentError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        TemplateTest::try_new(ident).map(Self::Template)
    }

    /// Creates a new unit test.
    pub fn new_unit<I>(ident: I, kind: UnitKind) -> Self
    where
        I: Into<UnitIdent>,
    {
        Self::Unit(UnitTest::new(ident, kind))
    }

    /// Creates a new unit test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new_unit<S>(ident: S, kind: UnitKind) -> Result<Self, ParseIdentError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        UnitTest::try_new(ident, kind).map(Self::Unit)
    }

    /// Creates a new doc test.
    pub fn new_doc<I>(ident: I) -> Self
    where
        I: Into<DocIdent>,
    {
        Self::Doc(DocTest::new(ident))
    }

    /// Creates a new doc test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new_doc<S>(ident: S) -> Result<Self, ParseIdentError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        DocTest::try_new(ident).map(Self::Doc)
    }
}

impl Test {
    /// The identifier of this test.
    pub fn ident(&self) -> Ident {
        match self {
            Test::Doc(test) => test.ident().into(),
            Test::Template(test) => test.ident().into(),
            Test::Unit(test) => test.ident().into(),
        }
    }

    /// The config of this test.
    pub fn config(&self) -> Option<&TestConfig> {
        match self {
            Test::Doc(test) => test.config(),
            Test::Template(test) => test.config(),
            Test::Unit(test) => test.config(),
        }
    }
}

impl Test {
    /// The kind of this test.
    pub fn kind(&self) -> Kind {
        match self {
            Test::Template(_) => Kind::Template,
            Test::Unit(unit) => Kind::Unit(unit.kind()),
            Test::Doc(_) => Kind::Doc,
        }
    }

    /// Whether this is a valid [`TemplateTest`].
    pub fn is_template(&self) -> bool {
        self.kind().is_template()
    }

    /// Returns the inner [`TemplateTest`] or `None` if its a different kind.
    pub fn as_template(&self) -> Option<&TemplateTest> {
        match self {
            Test::Template(test) => Some(test),
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
            Test::Unit(test) => Some(test),
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
            Test::Doc(test) => Some(test),
            _ => None,
        }
    }
}

/// A template test.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateTest {
    ident: TemplateIdent,
    config: Option<TestConfig>,
}

impl TemplateTest {
    /// Creates a new template test.
    pub fn new<I>(ident: I) -> Self
    where
        I: Into<TemplateIdent>,
    {
        Self {
            ident: ident.into(),
            config: None,
        }
    }

    /// Creates a new template test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new<S>(ident: S) -> Result<Self, ParseIdentError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Ok(Self {
            ident: TemplateIdent::new(ident)?,
            config: None,
        })
    }
}

impl TemplateTest {
    /// The identifier of this template test.
    pub fn ident(&self) -> TemplateIdent {
        self.ident.clone()
    }

    /// The config of this template test.
    pub fn config(&self) -> Option<&TestConfig> {
        self.config.as_ref()
    }
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
    pub fn new<I>(ident: I, kind: UnitKind) -> Self
    where
        I: Into<UnitIdent>,
    {
        match kind {
            UnitKind::CompileOnly => UnitTest::CompileOnly(CompileOnlyUnitTest::new(ident)),
            UnitKind::Ephemeral => UnitTest::Ephemeral(EphemeralUnitTest::new(ident)),
            UnitKind::Persistent => UnitTest::Persistent(PersistentUnitTest::new(ident)),
        }
    }

    /// Creates a new unit test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new<S>(ident: S, kind: UnitKind) -> Result<Self, ParseIdentError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Ok(match kind {
            UnitKind::CompileOnly => UnitTest::CompileOnly(CompileOnlyUnitTest::try_new(ident)?),
            UnitKind::Ephemeral => UnitTest::Ephemeral(EphemeralUnitTest::try_new(ident)?),
            UnitKind::Persistent => UnitTest::Persistent(PersistentUnitTest::try_new(ident)?),
        })
    }

    /// Creates a new compile-only unit test.
    pub fn new_compile_only<I>(ident: I) -> Self
    where
        I: Into<UnitIdent>,
    {
        Self::new(ident, UnitKind::CompileOnly)
    }
    /// Creates a new compile-only unit test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new_compile_only<S>(ident: S) -> Result<Self, ParseIdentError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Self::try_new(ident, UnitKind::CompileOnly)
    }

    /// Creates a new ephemeral unit test.
    pub fn new_ephemeral<I>(ident: I) -> Self
    where
        I: Into<UnitIdent>,
    {
        Self::new(ident, UnitKind::Ephemeral)
    }
    /// Creates a new ephemeral unit test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new_ephemeral<S>(ident: S) -> Result<Self, ParseIdentError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Self::try_new(ident, UnitKind::Ephemeral)
    }

    /// Creates a new persistent unit test.
    pub fn new_persistent<I>(ident: I) -> Self
    where
        I: Into<UnitIdent>,
    {
        Self::new(ident, UnitKind::Persistent)
    }
    /// Creates a new persistent unit test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new_persistent<S>(ident: S) -> Result<Self, ParseIdentError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Self::try_new(ident, UnitKind::Persistent)
    }
}

impl UnitTest {
    /// The identifier of this test.
    pub fn ident(&self) -> UnitIdent {
        match self {
            Self::CompileOnly(test) => test.ident(),
            Self::Ephemeral(test) => test.ident(),
            Self::Persistent(test) => test.ident(),
        }
    }

    /// The config of this unit test.
    pub fn config(&self) -> Option<&TestConfig> {
        match self {
            Self::CompileOnly(test) => test.config(),
            Self::Ephemeral(test) => test.config(),
            Self::Persistent(test) => test.config(),
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

/// An ephemeral unit test.
///
/// A unit test without references.
#[derive(Debug, Clone)]
pub struct CompileOnlyUnitTest {
    ident: UnitIdent,
    config: Option<TestConfig>,
}

impl CompileOnlyUnitTest {
    /// Creates a new compile-only test.
    pub fn new<I>(ident: I) -> Self
    where
        I: Into<UnitIdent>,
    {
        Self {
            ident: ident.into(),
            config: None,
        }
    }

    /// Creates a new compile-only test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new<S>(ident: S) -> Result<Self, ParseIdentError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Ok(Self {
            ident: UnitIdent::new(ident)?,
            config: None,
        })
    }
}

impl CompileOnlyUnitTest {
    /// The identifier of this unit test.
    pub fn ident(&self) -> UnitIdent {
        self.ident.clone()
    }

    /// The config of this unit test.
    pub fn config(&self) -> Option<&TestConfig> {
        self.config.as_ref()
    }
}

/// An ephemeral unit test.
///
/// A unit test with on-the-fly references.
#[derive(Debug, Clone)]
pub struct EphemeralUnitTest {
    ident: UnitIdent,
    config: Option<TestConfig>,
}

impl EphemeralUnitTest {
    /// Creates a new ephemeral test.
    pub fn new<I>(ident: I) -> Self
    where
        I: Into<UnitIdent>,
    {
        Self {
            ident: ident.into(),
            config: None,
        }
    }

    /// Creates a new ephemeral test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new<S>(ident: S) -> Result<Self, ParseIdentError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Ok(Self {
            ident: UnitIdent::new(ident)?,
            config: None,
        })
    }
}

impl EphemeralUnitTest {
    /// The identifier of this unit test.
    pub fn ident(&self) -> UnitIdent {
        self.ident.clone()
    }

    /// The config of this unit test.
    pub fn config(&self) -> Option<&TestConfig> {
        self.config.as_ref()
    }
}

/// A persistent unit test.
///
/// A unit test with on-disk references.
#[derive(Debug, Clone)]
pub struct PersistentUnitTest {
    ident: UnitIdent,
    config: Option<TestConfig>,
}

impl PersistentUnitTest {
    /// Creates a new persistent test.
    pub fn new<I>(ident: I) -> Self
    where
        I: Into<UnitIdent>,
    {
        Self {
            ident: ident.into(),
            config: None,
        }
    }

    /// Creates a new persistent test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new<S>(ident: S) -> Result<Self, ParseIdentError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Ok(Self {
            ident: UnitIdent::new(ident)?,
            config: None,
        })
    }
}

impl PersistentUnitTest {
    /// The identifier of this unit test.
    pub fn ident(&self) -> UnitIdent {
        self.ident.clone()
    }

    /// The config of this unit test.
    pub fn config(&self) -> Option<&TestConfig> {
        self.config.as_ref()
    }
}

/// A doc test.
#[derive(Debug, Clone, PartialEq)]
pub struct DocTest {
    ident: DocIdent,
    config: Option<TestConfig>,
}

impl DocTest {
    /// Creates a new doc test.
    pub fn new<I>(ident: I) -> Self
    where
        I: Into<DocIdent>,
    {
        Self {
            ident: ident.into(),
            config: None,
        }
    }

    /// Creates a new doc test from an untyped identifier.
    ///
    /// # Errors
    /// Returns an error if the identifier is invalid.
    pub fn try_new<S>(ident: S) -> Result<Self, ParseIdentError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        Ok(Self {
            ident: DocIdent::new(ident)?,
            config: None,
        })
    }
}

impl DocTest {
    /// The identifier of this test.
    pub fn ident(&self) -> DocIdent {
        self.ident.clone()
    }

    /// The config of this test.
    pub fn config(&self) -> Option<&TestConfig> {
        self.config.as_ref()
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
