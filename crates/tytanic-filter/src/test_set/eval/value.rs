use tytanic_core::test::Test;

use crate::test_set::ast::Num;
use crate::test_set::ast::Str;
use crate::test_set::eval::Error;
use crate::test_set::eval::Func;
use crate::test_set::eval::Set;

/// The value of a test set expression.
#[derive(Debug, Clone)]
pub enum Value {
    /// A test.
    Test(Test),

    /// A test set.
    Set(Set),

    /// A function.
    Func(Func),

    /// An unsigned integer.
    Num(Num),

    /// A string.
    Str(Str),
}

impl Value {
    /// The type of this expression.
    pub fn as_type(&self) -> Type {
        match self {
            Value::Test(_) => Type::Test,
            Value::Set(_) => Type::Set,
            Value::Func(_) => Type::Func,
            Value::Num(_) => Type::Num,
            Value::Str(_) => Type::Str,
        }
    }

    /// Convert this value into a `T` or return an error.
    pub fn expect_type<V>(self) -> Result<V, Error>
    where
        V: TryFromValue,
    {
        V::try_from_value(self)
    }
}

impl From<Set> for Value {
    fn from(value: Set) -> Self {
        Self::Set(value)
    }
}

impl From<Func> for Value {
    fn from(value: Func) -> Self {
        Self::Func(value)
    }
}

impl From<Num> for Value {
    fn from(value: Num) -> Self {
        Self::Num(value)
    }
}

impl From<Str> for Value {
    fn from(value: Str) -> Self {
        Self::Str(value)
    }
}

/// A trait for types which can be unwrapped from a [`Value`].
pub trait TryFromValue: Sized {
    /// Attempts to convert the given value into this type.
    fn try_from_value(value: Value) -> Result<Self, Error>;
}

/// The type of a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    /// A test.
    Test,

    /// A test set.
    Set,

    /// A function.
    Func,

    /// An unsigned integer.
    Num,

    /// A string.
    Str,
}

impl Type {
    /// The name of this type.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Test => "test",
            Self::Set => "test set",
            Self::Func => "function",
            Self::Num => "number",
            Self::Str => "string",
        }
    }
}
