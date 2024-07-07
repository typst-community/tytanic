use std::fmt::Debug;
use std::sync::Arc;

use eval::{
    AllMatcher, BinaryMatcher, IdentifierMatcher, IgnoredMatcher, KindMatcher, NoneMatcher,
    UnaryMatcher,
};
use parsing::{
    Argument, Arguments, Atom, BinaryExpr, BinaryOp, Expr, Function, Identifier, NameMatcher,
    UnaryExpr, UnaryOp, Value,
};
use regex::Regex;

use crate::store::test::Test;

pub mod eval;
pub mod parsing;

/// A type which matches tests, returning true for all tests which match.
pub trait Matcher: Debug {
    /// Returns whether this test's identifier matches.
    fn is_match(&self, test: &Test) -> bool;
}

impl Matcher for Arc<dyn Matcher> {
    fn is_match(&self, test: &Test) -> bool {
        Matcher::is_match(&**self, test)
    }
}

impl Matcher for Box<dyn Matcher> {
    fn is_match(&self, test: &Test) -> bool {
        Matcher::is_match(&**self, test)
    }
}

impl<M: Matcher> Matcher for &M {
    fn is_match(&self, test: &Test) -> bool {
        Matcher::is_match(*self, test)
    }
}

pub fn build_matcher(expr: Expr) -> Arc<dyn Matcher> {
    match expr {
        Expr::Unary(UnaryExpr { op, expr }) => match op {
            UnaryOp::Complement => Arc::new(UnaryMatcher::Complement(build_matcher(*expr))),
        },
        Expr::Binary(BinaryExpr { op, lhs, rhs }) => match op {
            BinaryOp::SymmetricDifference => Arc::new(BinaryMatcher::SymmetricDifference(
                build_matcher(*lhs),
                build_matcher(*rhs),
            )),
            BinaryOp::Difference => Arc::new(BinaryMatcher::Difference(
                build_matcher(*lhs),
                build_matcher(*rhs),
            )),
            BinaryOp::Intersection => Arc::new(BinaryMatcher::Intersect(
                build_matcher(*lhs),
                build_matcher(*rhs),
            )),
            BinaryOp::Union => Arc::new(BinaryMatcher::Union(
                build_matcher(*lhs),
                build_matcher(*rhs),
            )),
        },
        Expr::Atom(Atom::Value(Value {
            id: Identifier { value },
        })) => match value.as_str() {
            "all" => Arc::new(AllMatcher),
            "none" => Arc::new(NoneMatcher),
            "ignored" => Arc::new(IgnoredMatcher),
            "compile-only" => Arc::new(KindMatcher::compile_only()),
            "ephemeral" => Arc::new(KindMatcher::ephemeral()),
            "persistent" => Arc::new(KindMatcher::persistent()),
            _ => panic!("unknown matcher: '{value}'"),
        },
        Expr::Atom(Atom::Function(Function {
            id: Identifier { value },
            args: Arguments {
                arg: Argument { matcher },
            },
        })) => match value.as_str() {
            "test" => match matcher {
                NameMatcher::Exact(name) => Arc::new(IdentifierMatcher::Exact(name.into())),
                NameMatcher::Contains(name) => Arc::new(IdentifierMatcher::Contains(name.into())),
                NameMatcher::Regex(name) => {
                    Arc::new(IdentifierMatcher::Regex(Regex::new(&name).unwrap()))
                }
                // default to contains for test
                NameMatcher::Plain(name) => Arc::new(IdentifierMatcher::Contains(name.into())),
            },
            _ => panic!("unknown matcher: '{value}(...)'"),
        },
    }
}
