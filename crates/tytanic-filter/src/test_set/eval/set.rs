use std::fmt::Debug;
use std::sync::Arc;

use ecow::eco_vec;
use tytanic_core::project::ProjectContext;
use tytanic_core::test::Test;

use crate::test_set::ast::Pat;
use crate::test_set::eval::Context;
use crate::test_set::eval::Error;
use crate::test_set::eval::TryFromValue;
use crate::test_set::eval::Type;
use crate::test_set::eval::Value;

/// The backing implementation for a [`Set`].
type SetImpl =
    Arc<dyn Fn(&ProjectContext, &Context, &Test) -> Result<bool, Error> + Send + Sync + 'static>;

/// A test set, this can be used to check if a test is contained in it and is
/// expected to be the top level value in an [`ExpressionFilter`][filter].
///
/// [filter]: crate::ExpressionFilter
#[derive(Clone)]
pub struct Set(SetImpl);

impl Set {
    /// Create a new set with the given implementation.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&ProjectContext, &Context, &Test) -> Result<bool, Error> + Send + Sync + 'static,
    {
        Self(Arc::new(f) as _)
    }

    /// Whether the given test is contained within this set.
    pub fn contains(
        &self,
        project_ctx: &ProjectContext,
        eval_ctx: &Context,
        test: &Test,
    ) -> Result<bool, Error> {
        (self.0)(project_ctx, eval_ctx, test)
    }
}

impl Debug for Set {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Set").field(&..).finish()
    }
}

impl Set {
    /// Construct a test set which contains all tests matching the given pattern.
    ///
    /// This is the test set created from pattern literals like `r:'foot-(\w-)+'`.
    pub fn coerce_pat(pat: Pat) -> Set {
        Set::new(move |_, _, test: &Test| Ok(pat.is_match(test.ident())))
    }
}

impl Set {
    /// Construct a set which contains all tests _not_ contained in the given
    /// set.
    ///
    /// This is the test set created by `!set`.
    pub fn expr_comp(set: Set) -> Self {
        Self::new(move |project_ctx, eval_ctx, test| {
            Ok(!set.contains(project_ctx, eval_ctx, test)?)
        })
    }

    /// Construct a set which contains all tests which are contained in any of
    /// the given sets.
    ///
    /// This is the test set created by `a | b`.
    pub fn expr_union<I>(a: Set, b: Set, rest: I) -> Self
    where
        I: IntoIterator<Item = Set>,
    {
        let sets: Vec<_> = [a, b].into_iter().chain(rest).collect();

        Self::new(move |project_ctx, eval_ctx, test| {
            for set in &sets {
                if set.contains(project_ctx, eval_ctx, test)? {
                    return Ok(true);
                }
            }

            Ok(false)
        })
    }

    /// Construct a set which contains all tests which are contained in all the
    /// given sets.
    ///
    /// This is the test set created by `a & b`.
    pub fn expr_inter<I>(a: Set, b: Set, rest: I) -> Self
    where
        I: IntoIterator<Item = Set>,
    {
        let sets: Vec<_> = [a, b].into_iter().chain(rest).collect();

        Self::new(move |project_ctx, eval_ctx, test| {
            for set in &sets {
                if !set.contains(project_ctx, eval_ctx, test)? {
                    return Ok(false);
                }
            }

            Ok(true)
        })
    }

    /// Construct a set which contains all tests which are contained in the
    /// first but not the second set.
    ///
    /// This is the test set created by `a ~ b` and is equivalent to `a & !b`.
    pub fn expr_diff(a: Set, b: Set) -> Self {
        Self::new(move |project_ctx, eval_ctx, test| {
            Ok(a.contains(project_ctx, eval_ctx, test)?
                && !b.contains(project_ctx, eval_ctx, test)?)
        })
    }

    /// Construct a set which contains all tests which are contained in the
    /// either the first or the second, but not both sets.
    ///
    /// This is the test set created by `a ^ b`.
    pub fn expr_sym_diff(a: Set, b: Set) -> Self {
        Self::new(move |project_ctx, eval_ctx, test| {
            Ok(a.contains(project_ctx, eval_ctx, test)?
                ^ b.contains(project_ctx, eval_ctx, test)?)
        })
    }
}

impl TryFromValue for Set {
    fn try_from_value(value: Value) -> Result<Self, Error> {
        Ok(match value {
            Value::Set(set) => set,
            _ => {
                return Err(Error::TypeMismatch {
                    expected: eco_vec![Type::Set],
                    found: value.as_type(),
                });
            }
        })
    }
}

/// Ensure Set is thread safe if T is.
#[allow(dead_code)]
fn assert_traits() {
    tytanic_utils::assert::send::<Set>();
    tytanic_utils::assert::sync::<Set>();
}
