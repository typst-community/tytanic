use std::cmp::Ordering;
use std::fmt::Display;
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
pub fn try_lex_ident(input: &str) -> Option<(Ident, &str)> {
    let mut lexer = Lexer::new(input);
    let (kind, rest) = try_eat_ident(&mut lexer)?;

    // SAFETY: The lexer ensures validity of the identifier.
    Some((unsafe { Ident::new_unchecked(kind, input) }, rest))
}

/// Attempts to consume a leading identifier from the given input.
///
/// This will only consume tokens if a valid identifier is found.
fn try_eat_ident<'a>(lexer: &mut Lexer<'a>) -> Option<(Kind, &'a str)> {
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

macro_rules! declare_narrow_ident_type {
    (
        $(#[$docs:meta])*
        $ident:ident: $kind:ident;
        ($valid_bind_kind:pat, $valid_bind_token:pat) => $valid_body:expr
    ) => {
        $(#[$docs])*
        #[derive(Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
        #[repr(transparent)]
        pub struct $ident(EcoString);

        impl $ident {
            /// Creates a new identifier from the given raw string.
            pub fn new<S>(input: S) -> Result<Self, ParseIdentError>
            where
                S: Into<EcoString> + AsRef<str>
            {
                match Ident::parse_kind(input.as_ref()) {
                    Some(kind) if kind == Kind::$kind => {},
                    Some(kind) => return Err(ParseIdentError::UnexpectedKind {
                        expected: Kind::$kind,
                        given: kind,
                    }),
                    None => return Err(ParseIdentError::Invalid(input.into())),
                }

                Ok(Self(input.into()))
            }

            /// Creates a new identifier from the given raw string without
            /// checking if its valid.
            ///
            /// # Safety
            /// The caller must ensure that the given raw string is a valid
            /// identifier.
            pub unsafe fn new_unchecked(input: &str) -> Self {
                debug_assert!(Self::is_valid(input));

                // SAFETY: This is `repr(transparent)`.
                unsafe {
                    std::mem::transmute(input)
                }
            }

            /// Whether the given string would be a valid identifier.
            pub fn is_valid(input: &str) -> bool {
                Ident::parse_kind(input).is_some_and(|kind| kind == Kind::$kind)
            }

            /// Wraps this identifier in a generic identifier.
            pub fn into_ident(self) -> Ident {
                Ident::$kind(self)
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

        impl std::str::FromStr for $ident {
            type Err = ParseIdentError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                $ident::new(value)
            }
        }

        impl TryFrom<&str> for $ident {
            type Error = ParseIdentError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                $ident::new(value)
            }
        }

        impl TryFrom<&String> for $ident {
            type Error = ParseIdentError;

            fn try_from(value: &String) -> Result<Self, Self::Error> {
                $ident::new(value)
            }
        }

        impl TryFrom<&EcoString> for $ident {
            type Error = ParseIdentError;

            fn try_from(value: &EcoString) -> Result<Self, Self::Error> {
                $ident::new(value)
            }
        }

        impl AsRef<str> for $ident {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl std::borrow::Borrow<str> for $ident {
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }

        impl std::fmt::Display for $ident {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0, f)
            }
        }

        impl std::fmt::Debug for $ident {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Debug::fmt(&self.0, f)
            }
        }
    };
}

/// A test identifier.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Ident {
    /// A doc test identifier.
    Template(TemplateIdent),

    /// A unit test identifier.
    Unit(UnitIdent),

    /// A doc test identifier.
    Doc(DocIdent),
}

impl Ident {
    /// Creates a new identifier from the given string.
    pub fn new<S>(input: S) -> Result<Self, ParseIdentError>
    where
        S: Into<EcoString> + AsRef<str>,
    {
        let kind = match Self::parse_kind(input.as_ref()) {
            Some(kind) => kind,
            None => return Err(ParseIdentError::Invalid(input.into())),
        };

        Ok(match kind {
            Kind::Template => Self::Template(TemplateIdent(input.into())),
            Kind::Unit => Self::Unit(UnitIdent(input.into())),
            Kind::Doc => Self::Doc(DocIdent(input.into())),
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
            Kind::Template => Self::Template(TemplateIdent(input)),
            Kind::Unit => Self::Unit(UnitIdent(input)),
            Kind::Doc => Self::Doc(DocIdent(input)),
        }
    }

    /// Attempts to parse the identifier kind from this input.
    fn parse_kind(input: &str) -> Option<Kind> {
        let mut lexer = Lexer::new(input);
        let kind = try_eat_ident(&mut lexer);

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

impl Ident {
    /// The kind of this test identifier.
    pub fn kind(&self) -> Kind {
        match self {
            Self::Template(_) => Kind::Template,
            Self::Unit(_) => Kind::Unit,
            Self::Doc(_) => Kind::Doc,
        }
    }

    /// Whether this is a [`TemplateIdent`].
    pub fn is_template(&self) -> bool {
        matches!(self, Self::Template(_))
    }

    /// Returns the inner [`TemplateIdent`] or `None` if its another kind.
    pub fn as_template(&self) -> Option<&TemplateIdent> {
        match self {
            Self::Template(ident) => Some(ident),
            _ => None,
        }
    }

    /// Whether this is a [`UnitIdent`].
    pub fn is_unit(&self) -> bool {
        matches!(self, Self::Unit(_))
    }

    /// Returns the inner [`UnitIdent`] or `None` if its another kind.
    pub fn as_unit(&self) -> Option<&UnitIdent> {
        match self {
            Self::Unit(ident) => Some(ident),
            _ => None,
        }
    }

    /// Whether this is a [`DocIdent`].
    pub fn is_doc(&self) -> bool {
        matches!(self, Self::Doc(_))
    }

    /// Returns the inner [`DocIdent`] or `None` if its another kind.
    pub fn to_doc(&self) -> Option<&DocIdent> {
        match self {
            Self::Doc(ident) => Some(ident),
            _ => None,
        }
    }
}

impl FromStr for Ident {
    type Err = ParseIdentError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<&str> for Ident {
    type Error = ParseIdentError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&String> for Ident {
    type Error = ParseIdentError;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for Ident {
    type Error = ParseIdentError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&EcoString> for Ident {
    type Error = ParseIdentError;

    fn try_from(value: &EcoString) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<EcoString> for Ident {
    type Error = ParseIdentError;

    fn try_from(value: EcoString) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<TemplateIdent> for Ident {
    fn from(value: TemplateIdent) -> Self {
        Self::Template(value)
    }
}

impl From<UnitIdent> for Ident {
    fn from(value: UnitIdent) -> Self {
        Self::Unit(value)
    }
}

impl From<DocIdent> for Ident {
    fn from(value: DocIdent) -> Self {
        Self::Doc(value)
    }
}

impl std::ops::Deref for Ident {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for Ident {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Ident {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<Ident> for String {
    fn from(value: Ident) -> String {
        value.into_inner().1.into()
    }
}

impl From<Ident> for EcoString {
    fn from(value: Ident) -> EcoString {
        value.into_inner().1
    }
}

impl PartialOrd for Ident {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Ord for Ident {
    fn cmp(&self, other: &Self) -> Ordering {
        Ord::cmp(self.as_str(), other.as_str())
    }
}

impl std::fmt::Display for Ident {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ident::Template(ident) => std::fmt::Display::fmt(ident, f),
            Ident::Unit(ident) => std::fmt::Display::fmt(ident, f),
            Ident::Doc(ident) => std::fmt::Display::fmt(ident, f),
        }
    }
}

impl std::fmt::Debug for Ident {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ident::Template(ident) => std::fmt::Debug::fmt(ident, f),
            Ident::Unit(ident) => std::fmt::Debug::fmt(ident, f),
            Ident::Doc(ident) => std::fmt::Debug::fmt(ident, f),
        }
    }
}

/// A statically available [`Ident`] for the single template test identifier.
pub static TEMPLATE_IDENT: LazyLock<Ident> = LazyLock::new(|| {
    // SAFETY: `@template` is the only valid template identifier.
    unsafe { Ident::new_unchecked(Kind::Template, "@template") }
});

declare_narrow_ident_type! {
    /// A template test identifier.
    ///
    /// At the moment this is always `@template`, but this may change in the
    /// future if/when templates support more than one project initialization
    /// scaffold.
    TemplateIdent: Template;
    (kind, token) => kind == Kind::Template && token == "@template"
}

impl Ident {
    /// The name component of this template test identifier.
    ///
    /// At the moment this is always `template`, see type level description.
    pub fn name(&self) -> &str {
        &self.as_str()[1..]
    }
}

declare_narrow_ident_type! {
    /// A unit test identifier.
    UnitIdent: Unit;
    (kind, _) => kind == Kind::Unit
}

impl UnitIdent {
    /// Creates new unit test identifier from the given path.
    pub fn new_from_path<P>(path: P) -> Result<Self, ParseIdentError>
    where
        P: AsRef<Path>,
    {
        let as_utf8 = path
            .as_ref()
            .to_str()
            .ok_or_else(|| ParseIdentError::NotUtf8(path.as_ref().to_path_buf()))?;

        if std::path::MAIN_SEPARATOR != '/' {
            Self::new(as_utf8.replace(std::path::MAIN_SEPARATOR, "/"))
        } else {
            Self::new(as_utf8)
        }
    }
}

impl UnitIdent {
    /// The path component of this unit test identifier.
    pub fn path(&self) -> &str {
        self.as_str()
    }
}

declare_narrow_ident_type! {
    /// A doc test identifier.
    DocIdent: Doc;
    (kind, _) => kind == Kind::Doc
}

impl DocIdent {
    /// The path component of this doc test identifier.
    pub fn path(&self) -> &str {
        self.as_str()
            .rsplit_once('#')
            .map(|(path, _)| path)
            .expect("doc test ident must have '#'")
    }

    /// The item component of this doc test identifier.
    pub fn item(&self) -> &str {
        let (_, rest) = self
            .as_str()
            .rsplit_once('#')
            .expect("doc test ident must have '#'");

        rest.rsplit_once(':').map(|(item, _)| item).unwrap_or(rest)
    }

    /// The block component of this doc test identifier, if one was given.
    pub fn block(&self) -> Option<&str> {
        self.as_str().rsplit_once(':').map(|(_, block)| block)
    }
}

/// An error returned by the various identifier types when parsing fails.
#[derive(Debug, Error)]
pub enum ParseIdentError {
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

        assert_eq!(try_eat_ident(&mut lexer), Some((Kind::Template, input)));
    }

    #[test]
    fn test_lex_unit_ident_single_fragment() {
        let input = "core";
        let mut lexer = Lexer::new(input);

        assert_eq!(try_eat_ident(&mut lexer), Some((Kind::Unit, input)));
    }

    #[test]
    fn test_lex_unit_ident() {
        let input = "core/coords";
        let mut lexer = Lexer::new(input);

        assert_eq!(try_eat_ident(&mut lexer), Some((Kind::Unit, input)));
    }

    #[test]
    fn test_lex_doc_ident_single_fragment() {
        let input = "core#config";
        let mut lexer = Lexer::new(input);

        assert_eq!(try_eat_ident(&mut lexer), Some((Kind::Doc, input)));
    }

    #[test]
    fn test_lex_doc_ident() {
        let input = "core/coords#raw-to-xy";
        let mut lexer = Lexer::new(input);

        assert_eq!(try_eat_ident(&mut lexer), Some((Kind::Doc, input)));
    }

    #[test]
    fn test_lex_doc_ident_full_single_fragment() {
        let input = "core#config:ex1";
        let mut lexer = Lexer::new(input);

        assert_eq!(try_eat_ident(&mut lexer), Some((Kind::Doc, input)));
    }

    #[test]
    fn test_lex_doc_ident_full() {
        let input = "core/coords#raw-to-xy:ex1";
        let mut lexer = Lexer::new(input);

        assert_eq!(try_eat_ident(&mut lexer), Some((Kind::Doc, input)));
    }

    #[test]
    fn test_lex_doc_ident_full_external_lexer() {
        let input = "core/coords#raw-to-xy:ex1 ~ foo()";
        let mut lexer = Lexer::new(input);

        assert_eq!(
            try_eat_ident(&mut lexer),
            Some((Kind::Doc, "core/coords#raw-to-xy:ex1"))
        );
        assert_eq!(lexer.rest, " ~ foo()");
    }

    #[test]
    fn test_doc_test_components() {
        let ident = DocIdent::new("core/coords#raw-to-xy").unwrap();

        assert_eq!(ident.path(), "core/coords");
        assert_eq!(ident.item(), "raw-to-xy");
        assert_eq!(ident.block(), None);
    }

    #[test]
    fn test_doc_test_full_components() {
        let ident = DocIdent::new("core/coords#raw-to-xy:ex1").unwrap();

        assert_eq!(ident.path(), "core/coords");
        assert_eq!(ident.item(), "raw-to-xy");
        assert_eq!(ident.block(), Some("ex1"));
    }
}
