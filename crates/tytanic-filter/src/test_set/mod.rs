//! Test set expression filters.
//!
//! The result of a test set expression is a test set, a filter which matches
//! tests by its various attributes, like its identifier, annotations, test
//! kind, and more.
//!
//! Test set expressions are created using a function DSL which is described in
//! its [reference], see its [guide] for a less technical introduction.
//!
//! [reference]: https://typst-community.github.io/tytanic/reference/test-sets/index.html
//! [guide]: https://typst-community.github.io/tytanic/guides/test-sets.html

use ecow::EcoString;
use thiserror::Error;

use tytanic_core::filter::Filter;
use tytanic_core::project::ProjectContext;
use tytanic_core::test::Test;

use crate::test_set::eval::Eval;
use crate::test_set::eval::Value;

pub mod ast;
pub mod builtin;
pub mod eval;

/// A generic test set expression filter, this filter checks whether a test
/// should be filtered out by checking it against the inner test set within its
/// evaluation context.
///
/// This also includes extra parsing logic for the special `all:` modifier
/// prefix, which is not part of the test set grammar, but can be used by the
/// caller to handle instances where multiple tests match but only one is
/// usually expected.
///
/// This is an implementation of [`Filter`].
#[derive(Debug, Clone)]
pub struct ExpressionFilter {
    input: EcoString,
    all: bool,
    ctx: eval::Context,
    set: eval::Set,
}

impl ExpressionFilter {
    /// Parse and evaluate a string into a test set with the given context.
    pub fn new<S: Into<EcoString>>(ctx: eval::Context, input: S) -> Result<Self, Error> {
        let input = input.into();

        let (all, expr) = input
            .strip_prefix("all:")
            .map(|rest| (true, rest))
            .unwrap_or((false, &input));

        let set = ast::parse(expr)?.eval(&ctx).and_then(Value::expect_type)?;

        Ok(Self {
            input,
            all,
            ctx,
            set,
        })
    }
}

impl ExpressionFilter {
    /// The input expression the inner test set was parsed from.
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Whether this test set expression has the special `all:` modifier.
    ///
    /// Handling this is up to the caller and has no impact on the inner test
    /// set.
    pub fn all(&self) -> bool {
        self.all
    }

    /// The context used to evaluate the inner test set.
    pub fn ctx(&self) -> &eval::Context {
        &self.ctx
    }

    /// The inner test set.
    pub fn set(&self) -> &eval::Set {
        &self.set
    }
}

impl ExpressionFilter {
    /// Applies a function to the inner test set, this is useful for
    /// optimization or for adding implicit test sets like wrapping a test set
    /// in `(...) ~ skip()`.
    pub fn map<F>(self, f: F) -> Self
    where
        F: FnOnce(eval::Set) -> eval::Set,
    {
        Self {
            set: f(self.set),
            ..self
        }
    }
}

impl Filter for ExpressionFilter {
    type Error = eval::Error;

    fn filter(&mut self, ctx: &ProjectContext, test: &Test) -> Result<bool, Self::Error> {
        self.set.contains(ctx, &self.ctx, test)
    }
}

/// Returned by [`ExpressionFilter::new`].
#[derive(Debug, Error)]
pub enum Error {
    /// An error occurred during parsing.
    #[error(transparent)]
    Parse(#[from] ast::Error),

    /// An error occurred during evaluation.
    #[error(transparent)]
    Eval(#[from] eval::Error),
}
