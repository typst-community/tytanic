use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;

/// A regex pattern literal node.
///
/// This implements traits such a [`Eq`] without regard for the internal
/// structure, it purely compares by looking at the source pattern.
#[derive(Clone)]
pub struct Regex(pub regex::Regex);

impl Regex {
    /// Creates a new [`Regex`] from the given pattern.
    pub fn new<S: AsRef<str>>(pat: S) -> Result<Self, regex::Error> {
        Ok(Self(regex::Regex::new(pat.as_ref())?))
    }
}

impl Regex {
    /// The inner regex pattern.
    pub fn as_regex(&self) -> &regex::Regex {
        &self.0
    }

    /// The inner string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Unwraps the inner regex pattern.
    pub fn into_inner(self) -> regex::Regex {
        self.0
    }
}

impl Regex {
    /// Returns true if the id matches this pattern.
    pub fn is_match<S: AsRef<str>>(&self, id: S) -> bool {
        self.0.is_match(id.as_ref())
    }
}

impl Debug for Regex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\"", self.0.as_str())
    }
}

impl PartialEq for Regex {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl Eq for Regex {}

impl Hash for Regex {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_str().hash(state);
    }
}

impl Deref for Regex {
    type Target = regex::Regex;

    fn deref(&self) -> &Self::Target {
        self.as_regex()
    }
}

impl AsRef<regex::Regex> for Regex {
    fn as_ref(&self) -> &regex::Regex {
        self.as_regex()
    }
}

impl AsRef<str> for Regex {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<regex::Regex> for Regex {
    fn from(value: regex::Regex) -> Self {
        Self(value)
    }
}

impl From<Regex> for regex::Regex {
    fn from(value: Regex) -> Self {
        value.into_inner()
    }
}
