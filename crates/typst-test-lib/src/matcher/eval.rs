use std::fmt::Debug;
use std::sync::Arc;

use ecow::EcoString;
use regex::Regex;

use crate::matcher::Matcher;
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
    Complement(Arc<dyn Matcher>),
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
    Union(Arc<dyn Matcher>, Arc<dyn Matcher>),

    /// Matches the set difference of the inner matchers, those tests that match
    /// the left but not the right matcher.
    Difference(Arc<dyn Matcher>, Arc<dyn Matcher>),

    /// Matches the symmetric difference of the inner matchers, those tests that
    /// match only one matcher, but not both.
    SymmetricDifference(Arc<dyn Matcher>, Arc<dyn Matcher>),

    /// Matches the intersection of the inner matchers, those tests
    /// that match both.
    Intersect(Arc<dyn Matcher>, Arc<dyn Matcher>),
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

pub fn default_matcher() -> Arc<dyn Matcher> {
    Arc::new(UnaryMatcher::Complement(Arc::new(IgnoredMatcher)))
}

/// A matcher for running an arbitray function on tests.
#[derive(Clone)]
pub struct FnMatcher {
    pub custom: Arc<dyn Fn(&Test) -> bool>,
}

impl Debug for FnMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomMatcher")
            .field("custom", &..)
            .finish()
    }
}

impl FnMatcher {
    pub fn custom(&mut self, matcher: Arc<dyn Fn(&Test) -> bool>) -> &mut Self {
        self.custom = matcher;
        self
    }
}

impl super::Matcher for FnMatcher {
    fn is_match(&self, test: &Test) -> bool {
        (self.custom)(test)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let m = default_matcher();
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
        let m = UnaryMatcher::Complement(Arc::new(IgnoredMatcher));
        assert_matcher!(m, [true, true, true, true, true, false]);
    }

    #[test]
    fn test_binary() {
        let m = BinaryMatcher::Union(
            Arc::new(IdentifierMatcher::Regex(Regex::new(r#"test-\d"#).unwrap())),
            Arc::new(KindMatcher::compile_only()),
        );
        assert_matcher!(m, [true, true, true, true, true, false]);

        let m = BinaryMatcher::Intersect(
            Arc::new(IdentifierMatcher::Regex(Regex::new(r#"test-\d"#).unwrap())),
            Arc::new(KindMatcher::compile_only()),
        );
        assert_matcher!(m, [false, false, true, false, false, false]);

        let m = BinaryMatcher::Difference(
            Arc::new(IdentifierMatcher::Regex(Regex::new(r#"test-\d"#).unwrap())),
            Arc::new(KindMatcher::compile_only()),
        );
        assert_matcher!(m, [true, true, false, true, false, false]);

        let m = BinaryMatcher::SymmetricDifference(
            Arc::new(IdentifierMatcher::Regex(Regex::new(r#"test-\d"#).unwrap())),
            Arc::new(KindMatcher::compile_only()),
        );
        assert_matcher!(m, [true, true, false, true, true, false]);
    }
}
