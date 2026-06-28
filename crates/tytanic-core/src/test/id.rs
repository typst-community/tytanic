use std::cmp::Ordering;
use std::fmt::Display;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::LazyLock;

use ecow::EcoString;
use thiserror::Error;
use unicode_ident::is_xid_continue;
use unicode_ident::is_xid_start;

/// The kind of a test identifier token.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TokenKind {
    /// An unknown token.
    #[default]
    Unknown,

    /// A `@` token.
    At,

    /// A `/` token.
    Slash,

    /// A `#` token.
    Hash,

    /// A `:` token.
    Colon,

    /// A fragment within a path.
    Fragment,
}

/// A lexer for identifier tokens.
#[derive(Debug, Clone)]
pub struct Lexer<'s> {
    rest: &'s str,
    include_unknown: bool,
}

impl<'s> Lexer<'s> {
    /// Creates a new lexer for the given input that will exit early on the
    /// first unknown token.
    pub fn new(input: &'s str) -> Self {
        Self {
            rest: input,
            include_unknown: false,
        }
    }

    /// Creates a new lexer for the given input that will emit unknown tokens.
    pub fn new_with_unknown(input: &'s str) -> Self {
        Self {
            rest: input,
            include_unknown: true,
        }
    }
}

impl<'s> Iterator for Lexer<'s> {
    type Item = (TokenKind, &'s str);

    fn next(&mut self) -> Option<Self::Item> {
        let mut chars = self.rest.char_indices();

        let (kind, len) = match chars.next()?.1 {
            '@' => (TokenKind::At, 1),
            '/' => (TokenKind::Slash, 1),
            '#' => (TokenKind::Hash, 1),
            ':' => (TokenKind::Colon, 1),
            ch if ch == '_' || is_xid_start(ch) => {
                let end = chars
                    .take_while(|&(_, ch)| is_xid_continue(ch) || ch == '-' || ch == '_')
                    .last()
                    .map(|(idx, ch)| idx + ch.len_utf8())
                    .unwrap_or(ch.len_utf8());

                (TokenKind::Fragment, end)
            }
            _ => {
                if !self.include_unknown {
                    self.rest = "";
                    return None;
                }

                // eat all unknown chars
                let end = chars
                    .take_while(|&(_, ch)| {
                        !matches!(ch, '@' | '/' | '#' | ':' | '_') && !is_xid_start(ch)
                    })
                    .last()
                    .map(|(idx, ch)| idx + ch.len_utf8())
                    .unwrap_or(1);

                (TokenKind::Unknown, end)
            }
        };

        let (token, rest) = self.rest.split_at(len);
        self.rest = rest;
        Some((kind, token))
    }
}

/// The kind of a test identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Kind {
    /// A template test identifier.
    Template,

    /// A unit test identifier.
    Unit,

    /// A doc test identifier.
    Doc,
}

impl Kind {
    /// Returns the string representation of this test kind.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Template => "template",
            Self::Unit => "unit",
            Self::Doc => "doc",
        }
    }

    /// Whether this is [`Kind::Template`].
    pub fn is_template(&self) -> bool {
        matches!(self, Self::Template)
    }

    /// Whether this is [`Kind::Unit`].
    pub fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
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

/// Attempts to consume a leading identifier from the given input.
///
/// This will completely ignore unknown tokens, making it suitable for use in an
/// external lexer.
pub fn try_lex_id(input: &str) -> Option<(Id, &str)> {
    let mut lexer = Lexer::new(input);
    let (kind, rest) = try_eat_id(&mut lexer)?;

    // SAFETY: The lexer ensures validity of the identifier.
    Some((unsafe { Id::new_unchecked(kind, input) }, rest))
}

/// Attempts to consume a leading identifier from the given input.
///
/// This will only consume tokens if a valid identifier is found.
fn try_eat_id<'a>(lexer: &mut Lexer<'a>) -> Option<(Kind, &'a str)> {
    fn inner<'a>(lexer: &mut Lexer<'a>) -> Option<Kind> {
        let mut tokens = lexer.by_ref().peekable();

        match tokens.next() {
            Some((TokenKind::At, _)) => {
                tokens.next_if(|&(kind, token)| {
                    kind == TokenKind::Fragment && token.chars().all(|ch| ch.is_ascii_alphabetic())
                })?;

                Some(Kind::Template)
            }
            Some((TokenKind::Fragment, _)) => {
                while tokens
                    .next_if(|&(kind, _)| kind == TokenKind::Slash)
                    .is_some()
                {
                    tokens.next_if(|&(kind, _)| kind == TokenKind::Fragment)?;
                }

                let kind = if tokens
                    .next_if(|&(kind, _)| kind == TokenKind::Hash)
                    .is_some()
                {
                    tokens.next_if(|&(kind, _)| kind == TokenKind::Fragment)?;

                    if tokens
                        .next_if(|&(kind, _)| kind == TokenKind::Colon)
                        .is_some()
                    {
                        tokens.next_if(|&(kind, _)| kind == TokenKind::Fragment)?;
                    }

                    Kind::Doc
                } else {
                    Kind::Unit
                };

                Some(kind)
            }
            _ => None,
        }
    }

    let prev = lexer.clone();
    let res = inner(lexer);

    let ident = &prev.rest[..prev.rest.len() - lexer.rest.len()];

    if res.is_none() {
        *lexer = prev;
    }

    res.map(|kind| (kind, ident))
}

macro_rules! declare_narrow_id_type {
    (
        $(#[$docs:meta])*
        $id:ident: $kind:ident;
        ($valid_bind_kind:pat, $valid_bind_token:pat) => $valid_body:expr
    ) => {
        $(#[$docs])*
        #[derive(Clone, PartialOrd, Ord, PartialEq, Eq, Hash, serde::Serialize)]
        #[repr(transparent)]
        #[serde(transparent)]
        pub struct $id(EcoString);

        impl $id {
            /// Creates a new identifier from the given raw string.
            pub fn new<S>(input: S) -> Result<Self, ParseIdError>
            where
                S: Into<EcoString> + AsRef<str>
            {
                match Id::parse_kind(input.as_ref()) {
                    Some(kind) if kind == Kind::$kind => {},
                    Some(kind) => return Err(ParseIdError::UnexpectedKind {
                        expected: Kind::$kind,
                        given: kind,
                    }),
                    None => return Err(ParseIdError::Invalid(input.into())),
                }

                Ok(Self(input.into()))
            }

            /// Creates a new identifier from the given raw string without
            /// checking if its valid.
            ///
            /// # Safety
            /// The caller must ensure that the given raw string is a valid
            /// identifier.
            pub unsafe fn new_unchecked<S>(input: S) -> Self
            where
                S: Into<EcoString> + AsRef<str>
            {
                debug_assert!(Self::is_valid(input.as_ref()));
                Self(input.into())
            }

            /// Whether the given string would be a valid identifier.
            pub fn is_valid(input: &str) -> bool {
                Id::parse_kind(input).is_some_and(|kind| kind == Kind::$kind)
            }

            /// Wraps this typed identifier in a generic identifier.
            pub fn into_ident(self) -> Id {
                Id::$kind(self)
            }

            /// Returns the inner raw string.
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }

            /// Unwraps the inner string.
            pub fn into_inner(self) -> EcoString {
                self.0
            }
        }

        impl std::str::FromStr for $id {
            type Err = ParseIdError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                $id::new(value)
            }
        }

        impl TryFrom<&str> for $id {
            type Error = ParseIdError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                $id::new(value)
            }
        }

        impl TryFrom<&String> for $id {
            type Error = ParseIdError;

            fn try_from(value: &String) -> Result<Self, Self::Error> {
                $id::new(value)
            }
        }

        impl TryFrom<&EcoString> for $id {
            type Error = ParseIdError;

            fn try_from(value: &EcoString) -> Result<Self, Self::Error> {
                $id::new(value)
            }
        }

        impl AsRef<str> for $id {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl std::borrow::Borrow<str> for $id {
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }

        impl std::fmt::Display for $id {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0, f)
            }
        }

        impl std::fmt::Debug for $id {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Debug::fmt(&self.0, f)
            }
        }
    };
}

/// A test identifier.
#[derive(Clone, PartialEq, Eq)]
pub enum Id {
    /// A doc test identifier.
    Template(TemplateId),

    /// A unit test identifier.
    Unit(UnitId),

    /// A doc test identifier.
    Doc(DocId),
}

impl Id {
    /// Creates a new identifier from the given string.
    pub fn new<S>(input: S) -> Result<Self, ParseIdError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        let kind = match Self::parse_kind(input.as_ref()) {
            Some(kind) => kind,
            None => return Err(ParseIdError::Invalid(input.into())),
        };

        Ok(match kind {
            Kind::Template => Self::Template(TemplateId(input.into())),
            Kind::Unit => Self::Unit(UnitId(input.into())),
            Kind::Doc => Self::Doc(DocId(input.into())),
        })
    }

    /// Creates a new identifier from the given kind and string without
    /// checking if its valid or matches its kind.
    ///
    /// # Safety
    /// The caller must ensure that the given string is a valid identifier and
    /// matches the kind.
    pub unsafe fn new_unchecked<S>(kind: Kind, input: S) -> Self
    where
        S: Into<EcoString>,
    {
        let input = input.into();
        debug_assert!(Self::is_valid(&input));

        match kind {
            Kind::Template => Self::Template(TemplateId(input)),
            Kind::Unit => Self::Unit(UnitId(input)),
            Kind::Doc => Self::Doc(DocId(input)),
        }
    }

    /// Attempts to parse the identifier kind from this input.
    fn parse_kind(input: &str) -> Option<Kind> {
        let mut lexer = Lexer::new(input);
        let kind = try_eat_id(&mut lexer);

        if let Some((kind, token)) = kind
            && lexer.rest.is_empty()
            && match kind {
                Kind::Template => token == "@template",
                _ => true,
            }
        {
            Some(kind)
        } else {
            None
        }
    }

    /// Whether the given string would be a valid identifier.
    pub fn is_valid(input: &str) -> bool {
        Self::parse_kind(input).is_some()
    }

    /// Returns the inner dynamically sized slice struct.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Template(ident) => ident.as_str(),
            Self::Unit(ident) => ident.as_str(),
            Self::Doc(ident) => ident.as_str(),
        }
    }

    /// Unwraps the inner kind and string.
    pub fn into_inner(self) -> (Kind, EcoString) {
        match self {
            Self::Template(ident) => (Kind::Template, ident.into_inner()),
            Self::Unit(ident) => (Kind::Unit, ident.into_inner()),
            Self::Doc(ident) => (Kind::Doc, ident.into_inner()),
        }
    }
}

impl Id {
    /// The kind of this test identifier.
    pub fn kind(&self) -> Kind {
        match self {
            Self::Template(_) => Kind::Template,
            Self::Unit(_) => Kind::Unit,
            Self::Doc(_) => Kind::Doc,
        }
    }

    /// Whether this is a [`TemplateId`].
    pub fn is_template(&self) -> bool {
        matches!(self, Self::Template(_))
    }

    /// Returns the inner [`TemplateId`] or `None` if its another kind.
    pub fn as_template(&self) -> Option<&TemplateId> {
        match self {
            Self::Template(ident) => Some(ident),
            _ => None,
        }
    }

    /// Whether this is a [`UnitId`].
    pub fn is_unit(&self) -> bool {
        matches!(self, Self::Unit(_))
    }

    /// Returns the inner [`UnitId`] or `None` if its another kind.
    pub fn as_unit(&self) -> Option<&UnitId> {
        match self {
            Self::Unit(ident) => Some(ident),
            _ => None,
        }
    }

    /// Whether this is a [`DocId`].
    pub fn is_doc(&self) -> bool {
        matches!(self, Self::Doc(_))
    }

    /// Returns the inner [`DocId`] or `None` if its another kind.
    pub fn to_doc(&self) -> Option<&DocId> {
        match self {
            Self::Doc(ident) => Some(ident),
            _ => None,
        }
    }
    /// Returns an [`IdRef`] borrowing the inner id.
    pub fn as_id_ref(&self) -> IdRef<'_> {
        match self {
            Id::Template(id) => IdRef::Template(id),
            Id::Unit(id) => IdRef::Unit(id),
            Id::Doc(id) => IdRef::Doc(id),
        }
    }
}

impl FromStr for Id {
    type Err = ParseIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<&str> for Id {
    type Error = ParseIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&String> for Id {
    type Error = ParseIdError;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for Id {
    type Error = ParseIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&EcoString> for Id {
    type Error = ParseIdError;

    fn try_from(value: &EcoString) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<EcoString> for Id {
    type Error = ParseIdError;

    fn try_from(value: EcoString) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<TemplateId> for Id {
    fn from(value: TemplateId) -> Self {
        Self::Template(value)
    }
}

impl From<UnitId> for Id {
    fn from(value: UnitId) -> Self {
        Self::Unit(value)
    }
}

impl From<DocId> for Id {
    fn from(value: DocId) -> Self {
        Self::Doc(value)
    }
}

impl std::ops::Deref for Id {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for Id {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Id {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Hash for Id {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl From<Id> for String {
    fn from(value: Id) -> String {
        value.into_inner().1.into()
    }
}

impl From<Id> for EcoString {
    fn from(value: Id) -> EcoString {
        value.into_inner().1
    }
}

impl PartialOrd for Id {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Ord for Id {
    fn cmp(&self, other: &Self) -> Ordering {
        Ord::cmp(self.as_str(), other.as_str())
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Template(ident) => std::fmt::Display::fmt(ident, f),
            Self::Unit(ident) => std::fmt::Display::fmt(ident, f),
            Self::Doc(ident) => std::fmt::Display::fmt(ident, f),
        }
    }
}

impl std::fmt::Debug for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Template(ident) => std::fmt::Debug::fmt(ident, f),
            Self::Unit(ident) => std::fmt::Debug::fmt(ident, f),
            Self::Doc(ident) => std::fmt::Debug::fmt(ident, f),
        }
    }
}

impl From<IdRef<'_>> for Id {
    fn from(value: IdRef<'_>) -> Self {
        match value {
            IdRef::Template(test) => Self::Template(test.clone()),
            IdRef::Unit(test) => Self::Unit(test.clone()),
            IdRef::Doc(test) => Self::Doc(test.clone()),
        }
    }
}

/// The reference version of [`Id`].
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdRef<'id> {
    /// A doc test identifier.
    Template(&'id TemplateId),

    /// A unit test identifier.
    Unit(&'id UnitId),

    /// A doc test identifier.
    Doc(&'id DocId),
}

impl<'id> IdRef<'id> {
    /// Returns the inner dynamically sized slice struct.
    pub fn as_str(self) -> &'id str {
        match self {
            Self::Template(ident) => ident.as_str(),
            Self::Unit(ident) => ident.as_str(),
            Self::Doc(ident) => ident.as_str(),
        }
    }
}

impl<'id> IdRef<'id> {
    /// The kind of this test identifier.
    pub fn kind(self) -> Kind {
        match self {
            Self::Template(_) => Kind::Template,
            Self::Unit(_) => Kind::Unit,
            Self::Doc(_) => Kind::Doc,
        }
    }

    /// Whether this is a [`TemplateId`].
    pub fn is_template(self) -> bool {
        matches!(self, Self::Template(_))
    }

    /// Returns the inner [`TemplateId`] or `None` if its another kind.
    pub fn as_template(self) -> Option<&'id TemplateId> {
        match self {
            Self::Template(ident) => Some(ident),
            _ => None,
        }
    }

    /// Whether this is a [`UnitId`].
    pub fn is_unit(self) -> bool {
        matches!(self, Self::Unit(_))
    }

    /// Returns the inner [`UnitId`] or `None` if its another kind.
    pub fn as_unit(self) -> Option<&'id UnitId> {
        match self {
            Self::Unit(ident) => Some(ident),
            _ => None,
        }
    }

    /// Whether this is a [`DocId`].
    pub fn is_doc(self) -> bool {
        matches!(self, Self::Doc(_))
    }

    /// Returns the inner [`DocId`] or `None` if its another kind.
    pub fn to_doc(self) -> Option<&'id DocId> {
        match self {
            Self::Doc(ident) => Some(ident),
            _ => None,
        }
    }
}

impl<'id> From<&'id Id> for IdRef<'id> {
    fn from(value: &'id Id) -> Self {
        match value {
            Id::Template(id) => Self::Template(id),
            Id::Unit(id) => Self::Unit(id),
            Id::Doc(id) => Self::Doc(id),
        }
    }
}

impl<'id> From<&'id TemplateId> for IdRef<'id> {
    fn from(value: &'id TemplateId) -> Self {
        Self::Template(value)
    }
}

impl<'id> From<&'id UnitId> for IdRef<'id> {
    fn from(value: &'id UnitId) -> Self {
        Self::Unit(value)
    }
}

impl<'id> From<&'id DocId> for IdRef<'id> {
    fn from(value: &'id DocId) -> Self {
        Self::Doc(value)
    }
}

impl std::ops::Deref for IdRef<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for IdRef<'_> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for IdRef<'_> {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<IdRef<'_>> for String {
    fn from(value: IdRef<'_>) -> String {
        value.as_str().into()
    }
}

impl From<IdRef<'_>> for EcoString {
    fn from(value: IdRef<'_>) -> EcoString {
        value.as_str().into()
    }
}

impl PartialOrd for IdRef<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Ord for IdRef<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        Ord::cmp(self.as_str(), other.as_str())
    }
}

impl std::fmt::Display for IdRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Template(ident) => std::fmt::Display::fmt(ident, f),
            Self::Unit(ident) => std::fmt::Display::fmt(ident, f),
            Self::Doc(ident) => std::fmt::Display::fmt(ident, f),
        }
    }
}

impl std::fmt::Debug for IdRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Template(ident) => std::fmt::Debug::fmt(ident, f),
            Self::Unit(ident) => std::fmt::Debug::fmt(ident, f),
            Self::Doc(ident) => std::fmt::Debug::fmt(ident, f),
        }
    }
}

/// A statically available [`Id`] for the single template test identifier.
pub static TEMPLATE_ID: LazyLock<TemplateId> = LazyLock::new(|| {
    // SAFETY: `@template` is the only valid template identifier.
    unsafe { TemplateId::new_unchecked("@template") }
});

declare_narrow_id_type! {
    /// A template test identifier.
    ///
    /// At the moment this is always `@template`, but this may change in the
    /// future if/when templates support more than one project initialization
    /// scaffold.
    TemplateId: Template;
    (kind, token) => kind == Kind::Template && token == "@template"
}

impl Id {
    /// The name component of this template test identifier.
    ///
    /// At the moment this is always `template`, see type level description.
    pub fn name(&self) -> &str {
        &self.as_str()[1..]
    }
}

declare_narrow_id_type! {
    /// A unit test identifier.
    UnitId: Unit;
    (kind, _) => kind == Kind::Unit
}

impl UnitId {
    /// Creates new unit test identifier from the given path.
    pub fn new_from_path<P>(path: P) -> Result<Self, ParseIdError>
    where
        P: AsRef<Path>,
    {
        let as_utf8 = path
            .as_ref()
            .to_str()
            .ok_or_else(|| ParseIdError::NotUtf8(path.as_ref().to_path_buf()))?;

        if std::path::MAIN_SEPARATOR != '/' {
            Self::new(as_utf8.replace(std::path::MAIN_SEPARATOR, "/"))
        } else {
            Self::new(as_utf8)
        }
    }
}

impl UnitId {
    /// The path component of this unit test identifier.
    ///
    ///
    /// # Examples
    /// ```
    /// # use tytanic_core::test::UnitId;
    /// assert_eq!(UnitId::new("foo/bar")?.path(), "foo/bar");
    /// assert_eq!(UnitId::new("foo")?.path(), "foo");
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn path(&self) -> &str {
        self.as_str()
    }

    /// The leading path components of this unit test identifier, if there is
    /// at least one separator.
    ///
    /// # Examples
    /// ```
    /// # use tytanic_core::test::UnitId;
    /// assert_eq!(UnitId::new("foo/bar")?.module(), Some("foo"));
    /// assert_eq!(UnitId::new("foo")?.module(), None);
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn module(&self) -> Option<&str> {
        self.as_str().rsplit_once('/').map(|(m, _)| m)
    }

    /// The last path component of this unit test identifier.
    ///
    ///
    /// # Examples
    /// ```
    /// # use tytanic_core::test::UnitId;
    /// assert_eq!(UnitId::new("foo/bar")?.stem(), "bar");
    /// assert_eq!(UnitId::new("foo")?.stem(), "foo");
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn stem(&self) -> &str {
        self.as_str()
            .rsplit_once('/')
            .map(|(_, s)| s)
            .unwrap_or(self.as_str())
    }
}

declare_narrow_id_type! {
    /// A doc test identifier.
    DocId: Doc;
    (kind, _) => kind == Kind::Doc
}

impl DocId {
    /// The path component of this doc test identifier.
    ///
    /// # Examples
    /// ```
    /// # use tytanic_core::test::DocId;
    /// assert_eq!(DocId::new("foo/bar#qux:zir")?.path(), "foo/bar");
    /// assert_eq!(DocId::new("foo#bar:qux")?.path(), "foo");
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn path(&self) -> &str {
        self.as_str()
            .rsplit_once('#')
            .map(|(path, _)| path)
            .expect("doc test ident must have '#'")
    }

    /// The leading path components of this doc test identifier, if there is at
    /// least one separator.
    ///
    /// # Examples
    /// ```
    /// # use tytanic_core::test::DocId;
    /// assert_eq!(DocId::new("foo/bar#qux:zir")?.path_module(), Some("foo"));
    /// assert_eq!(DocId::new("foo#bar:qux")?.path_module(), None);
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn path_module(&self) -> Option<&str> {
        let path = self.path();
        path.rsplit_once('/').map(|(m, _)| m)
    }

    /// The last path component of this doc test identifier.
    ///
    /// # Examples
    /// ```
    /// # use tytanic_core::test::DocId;
    /// assert_eq!(DocId::new("foo/bar#qux:zir")?.path_stem(), "bar");
    /// assert_eq!(DocId::new("foo#bar:qux")?.path_stem(), "foo");
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn path_stem(&self) -> &str {
        let path = self.path();
        path.rsplit_once('/').map(|(_, s)| s).unwrap_or(path)
    }

    /// The item component of this doc test identifier.
    ///
    ///
    /// # Examples
    /// ```
    /// # use tytanic_core::test::DocId;
    /// assert_eq!(DocId::new("foo/bar#qux:zir")?.item(), "qux");
    /// assert_eq!(DocId::new("foo/bar#qux")?.item(), "qux");
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn item(&self) -> &str {
        let (_, rest) = self
            .as_str()
            .rsplit_once('#')
            .expect("doc test ident must have '#'");

        rest.rsplit_once(':').map(|(item, _)| item).unwrap_or(rest)
    }

    /// The block component of this doc test identifier, if one was given.
    ///
    /// ```
    /// # use tytanic_core::test::DocId;
    /// assert_eq!(DocId::new("foo/bar#qux:zir")?.block(), Some("zir"));
    /// assert_eq!(DocId::new("foo/bar#qux")?.block(), None);
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    /// The `"zir"` in `foo/bar#qux:zir` and `None` in `foo#bar`.
    pub fn block(&self) -> Option<&str> {
        self.as_str().rsplit_once(':').map(|(_, block)| block)
    }
}

/// An error returned by the various identifier types when parsing fails.
#[derive(Debug, Error)]
pub enum ParseIdError {
    /// The input was not valid UTF-8.
    #[error("expected {expected:?}, got {given:?}")]
    UnexpectedKind {
        /// The expected kind of identifier.
        expected: Kind,

        /// The kind of identifier that was given.
        given: Kind,
    },

    /// The input was not valid UTF-8.
    #[error("the input was not valid UTF-8: {0:?}")]
    NotUtf8(PathBuf),

    /// The input was not a valid identifier.
    #[error("the input was not a valid identifier: {0:?}")]
    Invalid(EcoString),

    /// The identifier was empty.
    #[error("the identifier was empty")]
    Empty,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lex_template_ident() {
        let input = "@template";
        let mut lexer = Lexer::new(input);

        assert_eq!(try_eat_id(&mut lexer), Some((Kind::Template, input)));
    }

    #[test]
    fn test_lex_unit_ident_single_fragment() {
        let input = "core";
        let mut lexer = Lexer::new(input);

        assert_eq!(try_eat_id(&mut lexer), Some((Kind::Unit, input)));
    }

    #[test]
    fn test_lex_unit_ident() {
        let input = "core/coords";
        let mut lexer = Lexer::new(input);

        assert_eq!(try_eat_id(&mut lexer), Some((Kind::Unit, input)));
    }

    #[test]
    fn test_lex_doc_ident_single_fragment() {
        let input = "core#config";
        let mut lexer = Lexer::new(input);

        assert_eq!(try_eat_id(&mut lexer), Some((Kind::Doc, input)));
    }

    #[test]
    fn test_lex_doc_ident() {
        let input = "core/coords#raw-to-xy";
        let mut lexer = Lexer::new(input);

        assert_eq!(try_eat_id(&mut lexer), Some((Kind::Doc, input)));
    }

    #[test]
    fn test_lex_doc_ident_full_single_fragment() {
        let input = "core#config:ex1";
        let mut lexer = Lexer::new(input);

        assert_eq!(try_eat_id(&mut lexer), Some((Kind::Doc, input)));
    }

    #[test]
    fn test_lex_doc_ident_full() {
        let input = "core/coords#raw-to-xy:ex1";
        let mut lexer = Lexer::new(input);

        assert_eq!(try_eat_id(&mut lexer), Some((Kind::Doc, input)));
    }

    #[test]
    fn test_lex_doc_ident_full_external_lexer() {
        let input = "core/coords#raw-to-xy:ex1 ~ foo()";
        let mut lexer = Lexer::new(input);

        assert_eq!(
            try_eat_id(&mut lexer),
            Some((Kind::Doc, "core/coords#raw-to-xy:ex1"))
        );
        assert_eq!(lexer.rest, " ~ foo()");
    }

    #[test]
    fn test_doc_test_components() {
        let ident = DocId::new("core/coords#raw-to-xy").unwrap();

        assert_eq!(ident.path(), "core/coords");
        assert_eq!(ident.item(), "raw-to-xy");
        assert_eq!(ident.block(), None);
    }

    #[test]
    fn test_doc_test_full_components() {
        let ident = DocId::new("core/coords#raw-to-xy:ex1").unwrap();

        assert_eq!(ident.path(), "core/coords");
        assert_eq!(ident.item(), "raw-to-xy");
        assert_eq!(ident.block(), Some("ex1"));
    }
}
