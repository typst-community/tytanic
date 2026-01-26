use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;

/// A glob pattern literal node.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Glob(pub glob::Pattern);

impl Glob {
    /// Creates a new [`Glob`] from the given pattern.
    pub fn new<S: AsRef<str>>(pat: S) -> Result<Self, glob::PatternError> {
        Ok(Self(glob::Pattern::new(pat.as_ref())?))
    }
}

impl Glob {
    /// The inner glob pattern.
    pub fn as_glob(&self) -> &glob::Pattern {
        &self.0
    }

    /// The inner string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Unwraps the inner glob pattern.
    pub fn into_inner(self) -> glob::Pattern {
        self.0
    }
}

impl Glob {
    /// Returns true if the id matches this pattern.
    pub fn is_match<S: AsRef<str>>(&self, id: S) -> bool {
        self.0.matches(id.as_ref())
    }
}

impl Debug for Glob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\"", self.0.as_str())
    }
}

impl Deref for Glob {
    type Target = glob::Pattern;

    fn deref(&self) -> &Self::Target {
        self.as_glob()
    }
}

impl AsRef<glob::Pattern> for Glob {
    fn as_ref(&self) -> &glob::Pattern {
        self.as_glob()
    }
}

impl AsRef<str> for Glob {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<glob::Pattern> for Glob {
    fn from(value: glob::Pattern) -> Self {
        Self(value)
    }
}

impl From<Glob> for glob::Pattern {
    fn from(value: Glob) -> Self {
        value.into_inner()
    }
}
