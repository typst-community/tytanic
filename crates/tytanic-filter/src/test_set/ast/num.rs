use std::fmt::Debug;

use ecow::eco_vec;
use pest::iterators::Pair;

use crate::test_set::ast::Error;
use crate::test_set::ast::PairExt;
use crate::test_set::ast::Rule;
use crate::test_set::eval;
use crate::test_set::eval::Context;
use crate::test_set::eval::Eval;
use crate::test_set::eval::TryFromValue;
use crate::test_set::eval::Type;
use crate::test_set::eval::Value;

/// A number literal node.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Num(pub usize);

impl Debug for Num {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<usize> for Num {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<Num> for usize {
    fn from(value: Num) -> Self {
        value.0
    }
}

impl Eval for Num {
    fn eval(&self, _ctx: &Context) -> Result<Value, eval::Error> {
        Ok(Value::Num(*self))
    }
}

impl TryFromValue for Num {
    fn try_from_value(value: Value) -> Result<Self, eval::Error> {
        Ok(match value {
            Value::Num(set) => set,
            _ => {
                return Err(eval::Error::TypeMismatch {
                    expected: eco_vec![Type::Num],
                    found: value.as_type(),
                });
            }
        })
    }
}

impl Num {
    pub(super) fn parse(pair: Pair<'_, Rule>) -> Result<Self, Error> {
        pair.expect_rules(&[Rule::num_inner])?;
        let mut s = pair.as_str().as_bytes();
        let mut num = 0;

        while let Some((&d, rest)) = s.split_first() {
            debug_assert!(
                matches!(d, b'0'..=b'9' | b'_'),
                "parser should ensure this is only digits and underscores",
            );

            s = rest;

            if d == b'_' {
                continue;
            }

            // Decimal equivalent of shift left and or LSB.
            num *= 10;
            num += (d - b'0') as usize;
        }

        Ok(Self(num))
    }
}
