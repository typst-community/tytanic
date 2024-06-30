use std::fmt::Debug;
use std::sync::Arc;

use ecow::EcoString;
use regex::Regex;

use crate::store::test::Test;
use crate::test::ReferenceKind;

/// A matcher which matches all tests.
#[derive(Debug, Clone)]
pub struct AllMatcher;

impl super::Matcher for AllMatcher {
    fn is_match(&self, _test: &Test) -> bool {
        true
    }
}

/// A matcher which matches no tests.
#[derive(Debug, Clone)]
pub struct NoneMatcher;

impl super::Matcher for NoneMatcher {
    fn is_match(&self, _test: &Test) -> bool {
        false
    }
}

/// A matcher which matches all ignored tests.
#[derive(Debug, Clone)]
pub struct IgnoredMatcher;

impl super::Matcher for IgnoredMatcher {
    fn is_match(&self, test: &Test) -> bool {
        test.is_ignored()
    }
}

/// A matcher which matches all ignored tests.
#[derive(Debug, Clone)]
pub struct KindMatcher {
    pub kind: Option<ReferenceKind>,
}
impl KindMatcher {
    /// A kind matcher whcih matches on compile only tests.
    pub fn compile_only() -> Self {
        Self { kind: None }
    }

    /// A kind matcher whcih matches on ephemeral tests.
    pub fn ephemeral() -> Self {
        Self {
            kind: Some(ReferenceKind::Ephemeral),
        }
    }

    /// A kind matcher whcih matches on persistent tests.
    pub fn persistent() -> Self {
        Self {
            kind: Some(ReferenceKind::Persistent),
        }
    }
}

impl super::Matcher for KindMatcher {
    fn is_match(&self, test: &Test) -> bool {
        test.ref_kind() == self.kind.as_ref()
    }
}

/// A matcher which matches tests by their identifiers.
#[derive(Debug, Clone)]
pub enum IdentifierMatcher {
    /// Matches all tests which match the [`Regex`].
    Regex(Regex),

    /// Matches all tests which have exactly this name.
    Exact(EcoString),

    /// Matches all tests which contain the given term in their name.
    Contains(EcoString),
}

impl super::Matcher for IdentifierMatcher {
    fn is_match(&self, test: &Test) -> bool {
        let id = test.id().as_str();
        match self {
            IdentifierMatcher::Regex(regex) => regex.is_match(id),
            IdentifierMatcher::Exact(term) => id == term,
            IdentifierMatcher::Contains(term) => id.contains(term.as_str()),
        }
    }
}

/// A unary operator matcher.
#[derive(Debug, Clone)]
pub enum UnaryMatcher {
    /// Matches all tests which don't match the inner matcher.
    Complement(Matcher),
}

impl super::Matcher for UnaryMatcher {
    fn is_match(&self, test: &Test) -> bool {
        match self {
            UnaryMatcher::Complement(matcher) => !matcher.is_match(test),
        }
    }
}

/// A binary operator matcher.
#[derive(Debug, Clone)]
pub enum BinaryMatcher {
    /// Matches the union of the inner matchers, those tests that match either
    /// matcher.
    Union(Matcher, Matcher),

    /// Matches the set difference of the inner matchers, those tests that match
    /// the left but not the right matcher.
    Difference(Matcher, Matcher),

    /// Matches the symmetric difference of the inner matchers, those tests that
    /// match only one matcher, but not both.
    SymmetricDifference(Matcher, Matcher),

    /// Matches the intersection of the inner matchers, those tests
    /// that match both.
    Intersect(Matcher, Matcher),
}

impl super::Matcher for BinaryMatcher {
    fn is_match(&self, test: &Test) -> bool {
        match self {
            BinaryMatcher::Union(m1, m2) => m1.is_match(test) || m2.is_match(test),
            BinaryMatcher::Difference(m1, m2) => m1.is_match(test) && !m2.is_match(test),
            BinaryMatcher::SymmetricDifference(m1, m2) => m1.is_match(test) ^ m2.is_match(test),
            BinaryMatcher::Intersect(m1, m2) => m1.is_match(test) && m2.is_match(test),
        }
    }
}

/// A matcher, built up of simpler matchers.
#[derive(Debug, Clone)]
pub enum Matcher {
    /// A matcher matching a single inner matcher.
    Unary(Box<UnaryMatcher>),

    /// A matcher matching with two inner matches.
    Binary(Box<BinaryMatcher>),

    /// An matcher matching on test identifiers.
    Identifier(IdentifierMatcher),

    /// A matcher matching on test kinds.
    Kind(KindMatcher),

    /// A custom matcher.
    Custom(CustomMatcher),

    /// The all matcher.
    All(AllMatcher),

    /// The none matcher.
    None(NoneMatcher),

    /// A matcher matching ignored tests.
    Ignored(IgnoredMatcher),
}

impl super::Matcher for Matcher {
    fn is_match(&self, test: &Test) -> bool {
        match self {
            Matcher::Unary(inner) => inner.is_match(test),
            Matcher::Binary(inner) => inner.is_match(test),
            Matcher::Identifier(inner) => inner.is_match(test),
            Matcher::Kind(inner) => inner.is_match(test),
            Matcher::Custom(inner) => inner.is_match(test),
            Matcher::All(inner) => inner.is_match(test),
            Matcher::None(inner) => inner.is_match(test),
            Matcher::Ignored(inner) => inner.is_match(test),
        }
    }
}

impl Default for Matcher {
    fn default() -> Self {
        Self::Unary(Box::new(UnaryMatcher::Complement(Matcher::Ignored(
            IgnoredMatcher,
        ))))
    }
}

/// A custom user supplied matcher.
#[derive(Clone)]
pub struct CustomMatcher {
    pub custom: Arc<dyn Fn(&Test) -> bool>,
}

impl Debug for CustomMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomMatcher")
            .field("custom", &..)
            .finish()
    }
}

impl CustomMatcher {
    pub fn custom(&mut self, matcher: Arc<dyn Fn(&Test) -> bool>) -> &mut Self {
        self.custom = matcher;
        self
    }
}

impl super::Matcher for CustomMatcher {
    fn is_match(&self, test: &Test) -> bool {
        (self.custom)(test)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::Matcher as _;
    use crate::test::id::Identifier;
    use crate::test::ReferenceKind;

    macro_rules! assert_matcher {
        ($m:expr, $matches:expr $(,)?) => {
            assert_eq!(
                [
                    ("mod/test-1", Some(ReferenceKind::Ephemeral), false),
                    ("mod/test-2", Some(ReferenceKind::Persistent), false),
                    ("mod/other/test-1", None, false),
                    ("mod/other/test-2", Some(ReferenceKind::Ephemeral), false),
                    ("top-level", None, false),
                    ("ignored", Some(ReferenceKind::Persistent), true),
                ]
                .map(|(id, r, i)| Test::new_test(Identifier::new(id).unwrap(), r, i,))
                .iter()
                .map(|t| $m.is_match(t))
                .collect::<Vec<_>>(),
                $matches,
            );
        };
    }

    #[test]
    fn test_default() {
        let m = Matcher::default();
        assert_matcher!(m, [true, true, true, true, true, false]);
    }

    #[test]
    fn test_name_regex() {
        let m = IdentifierMatcher::Regex(Regex::new(r#"mod/.+/test"#).unwrap());
        assert_matcher!(m, [false, false, true, true, false, false]);
    }

    #[test]
    fn test_name_contains() {
        let m = IdentifierMatcher::Contains("-".into());
        assert_matcher!(m, [true, true, true, true, true, false]);
    }

    #[test]
    fn test_name_exact() {
        let m = IdentifierMatcher::Exact("mod/test-1".into());
        assert_matcher!(m, [true, false, false, false, false, false]);
    }

    #[test]
    fn test_kind() {
        let m = KindMatcher::compile_only();
        assert_matcher!(m, [false, false, true, false, true, false]);

        let m = KindMatcher::ephemeral();
        assert_matcher!(m, [true, false, false, true, false, false]);

        let m = KindMatcher::persistent();
        assert_matcher!(m, [false, true, false, false, false, true]);
    }

    #[test]
    fn test_ignored() {
        let m = IgnoredMatcher;
        assert_matcher!(m, [false, false, false, false, false, true]);
    }

    #[test]
    fn test_all() {
        let m = AllMatcher;
        assert_matcher!(m, [true, true, true, true, true, true]);
    }

    #[test]
    fn test_none() {
        let m = NoneMatcher;
        assert_matcher!(m, [false, false, false, false, false, false]);
    }

    #[test]
    fn test_complement() {
        let m = UnaryMatcher::Complement(Matcher::Ignored(IgnoredMatcher));
        assert_matcher!(m, [true, true, true, true, true, false]);
    }

    #[test]
    fn test_binary() {
        let m = BinaryMatcher::Union(
            Matcher::Identifier(IdentifierMatcher::Regex(Regex::new(r#"test-\d"#).unwrap())),
            Matcher::Kind(KindMatcher::compile_only()),
        );
        assert_matcher!(m, [true, true, true, true, true, false]);

        let m = BinaryMatcher::Intersect(
            Matcher::Identifier(IdentifierMatcher::Regex(Regex::new(r#"test-\d"#).unwrap())),
            Matcher::Kind(KindMatcher::compile_only()),
        );
        assert_matcher!(m, [false, false, true, false, false, false]);

        let m = BinaryMatcher::Difference(
            Matcher::Identifier(IdentifierMatcher::Regex(Regex::new(r#"test-\d"#).unwrap())),
            Matcher::Kind(KindMatcher::compile_only()),
        );
        assert_matcher!(m, [true, true, false, true, false, false]);

        let m = BinaryMatcher::SymmetricDifference(
            Matcher::Identifier(IdentifierMatcher::Regex(Regex::new(r#"test-\d"#).unwrap())),
            Matcher::Kind(KindMatcher::compile_only()),
        );
        assert_matcher!(m, [true, true, false, true, true, false]);
    }
}
