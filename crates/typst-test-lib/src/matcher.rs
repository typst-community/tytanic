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
pub trait Matcher {
    /// Returns whether this test's identifier matches.
    fn is_match(&self, test: &Test) -> bool;
}

pub fn build_matcher(expr: Expr) -> eval::Matcher {
    match expr {
        Expr::Unary(UnaryExpr { op, expr }) => match op {
            UnaryOp::Complement => {
                eval::Matcher::Unary(Box::new(UnaryMatcher::Complement(build_matcher(*expr))))
            }
        },
        Expr::Binary(BinaryExpr { op, lhs, rhs }) => match op {
            BinaryOp::SymmetricDifference => eval::Matcher::Binary(Box::new(
                BinaryMatcher::SymmetricDifference(build_matcher(*lhs), build_matcher(*rhs)),
            )),
            BinaryOp::Difference => eval::Matcher::Binary(Box::new(BinaryMatcher::Difference(
                build_matcher(*lhs),
                build_matcher(*rhs),
            ))),
            BinaryOp::Intersection => eval::Matcher::Binary(Box::new(BinaryMatcher::Intersect(
                build_matcher(*lhs),
                build_matcher(*rhs),
            ))),
            BinaryOp::Union => eval::Matcher::Binary(Box::new(BinaryMatcher::Union(
                build_matcher(*lhs),
                build_matcher(*rhs),
            ))),
        },
        Expr::Atom(Atom::Value(Value {
            id: Identifier { value },
        })) => match value.as_str() {
            "all" => eval::Matcher::All(AllMatcher),
            "none" => eval::Matcher::None(NoneMatcher),
            "ignored" => eval::Matcher::Ignored(IgnoredMatcher),
            "compile-only" => eval::Matcher::Kind(KindMatcher::compile_only()),
            "ephemeral" => eval::Matcher::Kind(KindMatcher::ephemeral()),
            "persistent" => eval::Matcher::Kind(KindMatcher::persistent()),
            _ => panic!("unknown matcher: '{value}'"),
        },
        Expr::Atom(Atom::Function(Function {
            id: Identifier { value },
            args: Arguments {
                arg: Argument { matcher },
            },
        })) => match value.as_str() {
            "test" => match matcher {
                NameMatcher::Exact(name) => {
                    eval::Matcher::Identifier(IdentifierMatcher::Exact(name.into()))
                }
                NameMatcher::Contains(name) => {
                    eval::Matcher::Identifier(IdentifierMatcher::Contains(name.into()))
                }
                NameMatcher::Regex(name) => {
                    eval::Matcher::Identifier(IdentifierMatcher::Regex(Regex::new(&name).unwrap()))
                }
                // default to contains for test
                NameMatcher::Plain(name) => {
                    eval::Matcher::Identifier(IdentifierMatcher::Contains(name.into()))
                }
            },
            _ => panic!("unknown matcher: '{value}(...)'"),
        },
    }
}
