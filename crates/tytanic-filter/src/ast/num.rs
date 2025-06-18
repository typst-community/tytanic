use std::fmt::Debug;

use ecow::eco_vec;
use pest::iterators::Pair;

use super::Error;
use super::PairExt;
use super::Rule;
use crate::eval;
use crate::eval::Context;
use crate::eval::Eval;
use crate::eval::Test;
use crate::eval::TryFromValue;
use crate::eval::Type;
use crate::eval::Value;

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

impl<T: Test> Eval<T> for Num {
    fn eval(&self, _ctx: &Context<T>) -> Result<Value<T>, eval::Error> {
        Ok(Value::Num(*self))
    }
}

impl<T> TryFromValue<T> for Num {
    fn try_from_value(value: Value<T>) -> Result<Self, eval::Error> {
        Ok(match value {
            Value::Num(set) => set,
            _ => {
                return Err(eval::Error::TypeMismatch {
                    expected: eco_vec![Type::Num],
                    found: value.as_type(),
                })
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
