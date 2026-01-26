use std::sync::Arc;

use pest::iterators::Pair;
use pest::pratt_parser::PrattParser;

use crate::test_set::ast::Atom;
use crate::test_set::ast::Error;
use crate::test_set::ast::Func;
use crate::test_set::ast::Id;
use crate::test_set::ast::Num;
use crate::test_set::ast::Pat;
use crate::test_set::ast::Rule;
use crate::test_set::ast::Str;
use crate::test_set::eval;
use crate::test_set::eval::Context;
use crate::test_set::eval::Eval;
use crate::test_set::eval::Set;
use crate::test_set::eval::Value;

/// An unary prefix operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrefixOp {
    /// The negation operator. Matches the symbols `not` and `!`.
    Not,
}

/// A binary infix operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InfixOp {
    /// The union/or operator. Matches the symbols `or` and `|`.
    Union,

    /// The intersection/and operator. Matches the symbols `and` and `&`.
    Inter,

    /// The difference operator. Matches the symbols `diff` and `~`.
    Diff,

    /// The symmetric difference/xor operator. Matches the symbols `xor` and
    /// `^`.
    SymDiff,
}

/// An expression node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    /// An expression atom.
    Atom(Atom),

    /// A function call expression.
    Func(Func),

    /// A prefix expression.
    Prefix {
        /// The unary prefix operator.
        op: PrefixOp,

        /// The inner expression.
        expr: Arc<Expr>,
    },

    /// An infix expression.
    Infix {
        /// The binary infix operator.
        op: InfixOp,

        /// The left-hand side of this binary expression.
        lhs: Arc<Expr>,

        /// The right-hand side of this binary expression.
        rhs: Arc<Expr>,
    },
}

// TODO(tinger): Flatten intersection and union chains.
impl Eval for Expr {
    fn eval(&self, ctx: &Context) -> Result<Value, eval::Error> {
        match self {
            Self::Atom(atom) => atom.eval(ctx),
            Self::Func(func) => func.eval(ctx),
            Self::Prefix { op, expr } => {
                // Unary prefix operator is only valid for test sets.
                let set: Set = expr.eval(ctx)?.expect_type()?;

                Ok(Value::Set(match op {
                    PrefixOp::Not => Set::expr_comp(set),
                }))
            }
            Self::Infix { op, lhs, rhs } => {
                // Binary infix operator is only valid for test sets.
                let lhs: Set = lhs.eval(ctx)?.expect_type()?;
                let rhs: Set = rhs.eval(ctx)?.expect_type()?;

                Ok(Value::Set(match op {
                    InfixOp::Union => Set::expr_union(lhs, rhs, []),
                    InfixOp::Inter => Set::expr_inter(lhs, rhs, []),
                    InfixOp::Diff => Set::expr_diff(lhs, rhs),
                    InfixOp::SymDiff => Set::expr_sym_diff(lhs, rhs),
                }))
            }
        }
    }
}

impl Expr {
    pub(super) fn parse(pair: Pair<'_, Rule>, pratt: &PrattParser<Rule>) -> Result<Expr, Error> {
        pratt
            .map_primary(|primary| {
                Ok(match primary.as_rule() {
                    Rule::id => Expr::Atom(Atom::Id(Id::parse(primary)?)),
                    Rule::pat_inner => Expr::Atom(Atom::Pat(Pat::parse(primary)?)),
                    Rule::str_single | Rule::str_double => {
                        Expr::Atom(Atom::Str(Str::parse(primary)?))
                    }
                    Rule::num_inner => Expr::Atom(Atom::Num(Num::parse(primary)?)),
                    Rule::func => Expr::Func(Func::parse(primary, pratt)?),
                    Rule::expr => Self::parse(primary, pratt)?,
                    x => unreachable!("unhandled primary expression {x:?}"),
                })
            })
            .map_prefix(|op, expr| match op.as_rule().to_prefix() {
                Some(op) => Ok(Expr::Prefix {
                    op,
                    expr: Arc::new(expr?),
                }),
                None => unreachable!("unhandled prefix operator {:?}", op.as_rule()),
            })
            .map_infix(|lhs, op, rhs| match op.as_rule().to_infix() {
                Some(op) => Ok(Expr::Infix {
                    op,
                    lhs: Arc::new(lhs?),
                    rhs: Arc::new(rhs?),
                }),
                None => unreachable!("unhandled infix operator {:?}", op.as_rule()),
            })
            .parse(pair.into_inner())
    }
}
