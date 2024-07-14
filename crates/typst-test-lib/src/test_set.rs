use std::collections::BTreeMap;
use std::fmt::{Debug, Display};
use std::str::FromStr;
use std::sync::Arc;

use ecow::EcoString;
use eval::{
    AllMatcher, BinaryMatcher, IdentifierMatcher, IgnoredMatcher, KindMatcher, NoneMatcher,
    UnaryMatcher,
};
use id::Identifier;
use once_cell::sync::Lazy;
use parsing::{
    Argument, Arguments, Atom, BinaryExpr, BinaryOp, Expr, Function, NameMatcher, Rule, UnaryExpr,
    UnaryOp, Value,
};
use pest::error::Error;
use regex::Regex;
use thiserror::Error;

use crate::store::test::Test;

pub mod eval;
pub mod id;
pub mod parsing;

/// A dynamic test set.
pub type TestSet = Arc<dyn Matcher + Send + Sync>;

/// A function which can construct a matcher for the given arguments.
pub type MatcherFactory =
    Box<dyn Fn(Arguments) -> Result<TestSet, BuildTestSetError> + Send + Sync>;

/// An error that occurs when a test set could not be constructed.
#[derive(Debug, Error)]
pub enum BuildTestSetError {
    /// The requested test set could not be found.
    UnknownTestSet { id: EcoString, func: bool },
}

impl Display for BuildTestSetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildTestSetError::UnknownTestSet { id, func } => {
                write!(f, "unknown test set: {id}")?;
                if *func {
                    write!(f, "(...)")?;
                }
            }
        }

        Ok(())
    }
}

/// A test set which matches tests, returning true for all tests contained in
/// this set.
pub trait Matcher: Debug + Send + Sync {
    /// Returns whether this test's identifier matches.
    fn is_match(&self, test: &Test) -> bool;
}

impl Matcher for Arc<dyn Matcher + Send + Sync> {
    fn is_match(&self, test: &Test) -> bool {
        Matcher::is_match(&**self, test)
    }
}

impl Matcher for Box<dyn Matcher + Send + Sync> {
    fn is_match(&self, test: &Test) -> bool {
        Matcher::is_match(&**self, test)
    }
}

impl<M: Matcher + Send + Sync> Matcher for &M {
    fn is_match(&self, test: &Test) -> bool {
        Matcher::is_match(*self, test)
    }
}

/// A full test set expression.
#[derive(Debug, Clone)]
pub struct TestSetExpr {
    root: Expr,
}

impl FromStr for TestSetExpr {
    type Err = Error<Rule>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parsing::parse_test_set_expr(s).map(|root| Self { root })
    }
}

impl TestSetExpr {
    /// Build the test set exression into a matcher.
    pub fn build(self, test_sets: &TestSets) -> Result<TestSet, BuildTestSetError> {
        build_matcher(self.root, test_sets)
    }
}

/// A map of test set values and functions used when building a test set expression into a [`TestSet`].
pub struct TestSets {
    values: BTreeMap<Identifier, TestSet>,
    funcs: BTreeMap<Identifier, MatcherFactory>,
}

impl TestSets {
    /// Try to get a test set value.
    pub fn get_value(&self, id: &str) -> Result<TestSet, BuildTestSetError> {
        self.values
            .get(id)
            .cloned()
            .ok_or_else(|| BuildTestSetError::UnknownTestSet {
                id: id.into(),
                func: false,
            })
    }

    /// Try to construct a test set function.
    pub fn get_func(&self, id: &str, args: Arguments) -> Result<TestSet, BuildTestSetError> {
        (self
            .funcs
            .get(id)
            .ok_or_else(|| BuildTestSetError::UnknownTestSet {
                id: id.into(),
                func: true,
            })?)(args)
    }
}

impl Default for TestSets {
    fn default() -> Self {
        Self {
            values: [
                ("all", Arc::new(AllMatcher) as TestSet),
                ("none", Arc::new(NoneMatcher)),
                ("ignored", Arc::new(IgnoredMatcher)),
                ("compile-only", Arc::new(KindMatcher::compile_only())),
                ("ephemeral", Arc::new(KindMatcher::ephemeral())),
                ("persistent", Arc::new(KindMatcher::persistent())),
                ("default", default_test_set()),
            ]
            .into_iter()
            .map(|(id, m)| (Identifier { id: id.into() }, m))
            .collect(),
            funcs: [(
                "id",
                Box::new(
                    |Arguments {
                         arg: Argument { matcher },
                     }| {
                        Ok(match matcher {
                            NameMatcher::Exact(name) => {
                                Arc::new(IdentifierMatcher::Exact(name.into())) as TestSet
                            }
                            NameMatcher::Contains(name) => {
                                Arc::new(IdentifierMatcher::Contains(name.into()))
                            }
                            NameMatcher::Regex(name) => {
                                Arc::new(IdentifierMatcher::Regex(Regex::new(&name).unwrap()))
                            }
                            // default to contains for id
                            NameMatcher::Plain(name) => {
                                Arc::new(IdentifierMatcher::Contains(name.into()))
                            }
                        })
                    },
                ) as MatcherFactory,
            )]
            .into_iter()
            .map(|(id, m)| (Identifier { id: id.into() }, m))
            .collect(),
        }
    }
}

/// A map of builtin test sets.
///
/// Includes the following values:
/// - `none`
/// - `all`
/// - `ignored`
/// - `compile-only`
/// - `ephemeral`
/// - `persistent`
/// - `default`
///
/// Includes the following function factories:
/// - `id`
pub static BUILTIN_TESTSETS: Lazy<TestSets> = Lazy::new(TestSets::default);

/// Build a matcher from the given [`Expr`] using the given test sets.
pub fn build_matcher(expr: Expr, test_sets: &TestSets) -> Result<TestSet, BuildTestSetError> {
    Ok(match expr {
        Expr::Unary(UnaryExpr { op, expr }) => match op {
            UnaryOp::Complement => {
                Arc::new(UnaryMatcher::Complement(build_matcher(*expr, test_sets)?))
            }
        },
        Expr::Binary(BinaryExpr { op, lhs, rhs }) => match op {
            BinaryOp::SymmetricDifference => Arc::new(BinaryMatcher::SymmetricDifference(
                build_matcher(*lhs, test_sets)?,
                build_matcher(*rhs, test_sets)?,
            )),
            BinaryOp::Difference => Arc::new(BinaryMatcher::Difference(
                build_matcher(*lhs, test_sets)?,
                build_matcher(*rhs, test_sets)?,
            )),
            BinaryOp::Intersection => Arc::new(BinaryMatcher::Intersect(
                build_matcher(*lhs, test_sets)?,
                build_matcher(*rhs, test_sets)?,
            )),
            BinaryOp::Union => Arc::new(BinaryMatcher::Union(
                build_matcher(*lhs, test_sets)?,
                build_matcher(*rhs, test_sets)?,
            )),
        },
        Expr::Atom(Atom::Value(Value { id })) => test_sets.get_value(&id.value)?,
        Expr::Atom(Atom::Function(Function { id, args })) => test_sets.get_func(&id.value, args)?,
    })
}

/// Create the default test set.
pub fn default_test_set() -> TestSet {
    eval::default_test_set()
}
