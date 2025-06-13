use super::Error;
use super::Func;
use super::Set;
use crate::ast::Num;
use crate::ast::Str;

/// The value of a test set expression.
#[derive(Debug, Clone)]
pub enum Value<T> {
    /// A test.
    Test(T),

    /// A test set.
    Set(Set<T>),

    /// A function.
    Func(Func<T>),

    /// An unsigned integer.
    Num(Num),

    /// A string.
    Str(Str),
}

impl<T> Value<T> {
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
    pub fn expect_type<V: TryFromValue<T>>(self) -> Result<V, Error>
    where
        T: Clone,
    {
        V::try_from_value(self)
    }
}

impl<T> From<Set<T>> for Value<T> {
    fn from(value: Set<T>) -> Self {
        Self::Set(value)
    }
}

impl<T> From<Func<T>> for Value<T> {
    fn from(value: Func<T>) -> Self {
        Self::Func(value)
    }
}

impl<T> From<Num> for Value<T> {
    fn from(value: Num) -> Self {
        Self::Num(value)
    }
}

impl<T> From<Str> for Value<T> {
    fn from(value: Str) -> Self {
        Self::Str(value)
    }
}

/// A trait for types which can be unwrapped from a [`Value`].
pub trait TryFromValue<T>: Sized {
    fn try_from_value(value: Value<T>) -> Result<Self, Error>;
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
