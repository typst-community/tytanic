use std::hash::Hash;

use pest::iterators::Pair;

use crate::test_set::ast::Error;
use crate::test_set::ast::Glob;
use crate::test_set::ast::PairExt;
use crate::test_set::ast::PairsExt;
use crate::test_set::ast::Regex;
use crate::test_set::ast::Rule;
use crate::test_set::ast::Str;
use crate::test_set::eval;
use crate::test_set::eval::Context;
use crate::test_set::eval::Eval;
use crate::test_set::eval::Set;
use crate::test_set::eval::Value;

/// A pattern literal node.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Pat {
    /// A glob pattern literal.
    Glob(Glob),

    /// A regex pattern literal.
    Regex(Regex),

    /// An exact pattern literal.
    Exact(Str),
}

impl std::fmt::Debug for Pat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (prefix, pat) = match self {
            Pat::Glob(glob) => ("glob", glob.as_str()),
            Pat::Regex(regex) => ("regex", regex.as_str()),
            Pat::Exact(pat) => ("exact", pat.as_str()),
        };

        write!(f, "{prefix}:{pat:?}")
    }
}

impl Pat {
    /// Returns true if the id matches this pattern.
    pub fn is_match<S: AsRef<str>>(&self, id: S) -> bool {
        match self {
            Self::Glob(pat) => pat.is_match(id),
            Self::Regex(regex) => regex.is_match(id),
            Self::Exact(pat) => id.as_ref() == pat.as_str(),
        }
    }
}

impl Eval for Pat {
    fn eval(&self, _ctx: &Context) -> Result<Value, eval::Error> {
        Ok(Value::Set(Set::coerce_pat(self.clone())))
    }
}

impl Pat {
    pub(super) fn parse(pair: Pair<'_, Rule>) -> Result<Self, Error> {
        pair.expect_rules(&[Rule::pat_inner])?;
        let mut pairs = pair.into_inner();

        let kind = pairs.expect_pair(&[Rule::pat_kind])?.as_str();
        let _ = pairs.expect_pair(&[Rule::pat_sep])?;
        let inner = pairs.expect_pair(&[Rule::pat_raw_lit, Rule::str_double, Rule::str_single])?;
        pairs.expect_end()?;

        let pat: Str = if inner.as_rule() == Rule::pat_raw_lit {
            Str(inner.as_str().into())
        } else {
            Str::parse(inner)?
        };

        Ok(match kind {
            "g" | "glob" => Self::Glob(Glob::new(&pat)?),
            "r" | "regex" => Self::Regex(Regex::new(&pat)?),
            "e" | "exact" => Self::Exact(pat),
            _ => unreachable!("unhandled kind: {kind:?}"),
        })
    }
}
