//! Test set evaluation.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::fmt::Display;

use ecow::EcoVec;
use thiserror::Error;
use tytanic_utils::fmt::Separators;
use tytanic_utils::fmt::Term;

use crate::test_set::ast::Id;

mod func;
mod set;
mod value;

pub use crate::test_set::eval::func::Func;
pub use crate::test_set::eval::set::Set;
pub use crate::test_set::eval::value::TryFromValue;
pub use crate::test_set::eval::value::Type;
pub use crate::test_set::eval::value::Value;

/// A trait for expressions to be evaluated and matched.
pub trait Eval {
    /// Evaluates this expression to a value.
    fn eval(&self, ctx: &Context) -> Result<Value, Error>;
}

/// An evaluation context used to retrieve bindings in test set expressions.
#[derive(Debug, Clone)]
pub struct Context {
    /// The bindings available for evaluation.
    bindings: BTreeMap<Id, Value>,
}

impl Context {
    /// Create a new evaluation context with no bindings.
    pub fn new() -> Self {
        Self {
            bindings: BTreeMap::new(),
        }
    }
}

impl Context {
    /// Inserts a new binding, possibly overriding an old one, returns the old
    /// binding if there was one.
    pub fn bind<V>(&mut self, id: Id, value: V) -> Option<Value>
    where
        V: Into<Value>,
    {
        tracing::trace!(id = %id.as_str(), "binding value into eval context");
        self.bindings.insert(id, value.into())
    }

    /// Resolves a binding with the given identifier.
    pub fn resolve<I>(&self, id: I) -> Result<Value, Error>
    where
        I: AsRef<str>,
    {
        tracing::trace!(id = %id.as_ref(), "resolving value from eval context");
        let id = id.as_ref();
        self.bindings
            .get(id)
            .cloned()
            .ok_or_else(|| Error::UnknownBinding { id: id.into() })
    }

    /// Find similar bindings to the given identifier.
    pub fn find_similar(&self, id: &str) -> Vec<Id> {
        self.bindings
            .keys()
            .filter(|cand| strsim::jaro(id, cand.as_str()) > 0.7)
            .cloned()
            .collect()
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

/// An error that occurs when a test set expression is evaluated.
#[derive(Debug, Error)]
pub enum Error {
    /// The requested binding could not be found.
    UnknownBinding {
        /// The given identifier.
        id: String,
    },

    /// A function received an incorrect argument count.
    InvalidArgumentCount {
        /// The identifier of the function.
        func: String,

        /// The minimum or exact expected number of arguments, interpretation
        /// depends on `is_min`.
        expected: usize,

        /// Whether the expected number is the minimum and allows more arguments.
        is_min: bool,

        /// The number of arguments passed.
        found: usize,
    },

    /// An invalid type was used in an expression.
    TypeMismatch {
        /// The expected types.
        expected: EcoVec<Type>,

        /// The given type.
        found: Type,
    },

    /// A custom error type.
    Custom(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UnknownBinding { id } => write!(f, "unknown binding: {id}"),
            Error::InvalidArgumentCount {
                func,
                expected,
                is_min,
                found,
            } => {
                let (found, ex) = (*found, *expected);

                if ex == 0 {
                    write!(
                        f,
                        "function {func} expects no {}, got {}",
                        Term::simple("argument").with(ex),
                        found,
                    )?;
                } else if *is_min {
                    write!(
                        f,
                        "function {func} expects at least {ex} {}, got {}",
                        Term::simple("argument").with(ex),
                        found,
                    )?;
                } else {
                    write!(
                        f,
                        "function {func} expects exactly {ex} {}, got {}",
                        Term::simple("argument").with(ex),
                        found,
                    )?;
                }

                Ok(())
            }
            Error::TypeMismatch { expected, found } => write!(
                f,
                "expected {}, found <{}>",
                Separators::comma_or().with(expected.iter().map(|t| format!("<{}>", t.name()))),
                found.name(),
            ),
            Error::Custom(err) => write!(f, "{err}"),
        }
    }
}

/// Ensure Context is thread safe if T is.
#[allow(dead_code)]
fn assert_traits() {
    tytanic_utils::assert::send::<Context>();
    tytanic_utils::assert::sync::<Context>();
}
