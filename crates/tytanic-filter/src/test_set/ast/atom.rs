use crate::test_set::ast::Id;
use crate::test_set::ast::Num;
use crate::test_set::ast::Pat;
use crate::test_set::ast::Str;
use crate::test_set::eval::Context;
use crate::test_set::eval::Error;
use crate::test_set::eval::Eval;
use crate::test_set::eval::Value;

/// A leaf node within a test set expression such as an identifier or literal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Atom {
    /// A variable.
    Id(Id),

    /// A number literal.
    Num(Num),

    /// A string literal.
    Str(Str),

    /// A pattern literal.
    Pat(Pat),
}

impl Eval for Atom {
    fn eval(&self, ctx: &Context) -> Result<Value, Error> {
        Ok(match self {
            Self::Id(id) => id.eval(ctx)?,
            Self::Num(n) => Value::Num(*n),
            Self::Str(s) => Value::Str(s.clone()),
            Self::Pat(pat) => pat.eval(ctx)?,
        })
    }
}
