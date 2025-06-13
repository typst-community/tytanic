use std::fmt::Debug;
use std::sync::Arc;

use ecow::eco_vec;

use super::Context;
use super::Error;
use super::Test;
use super::TryFromValue;
use super::Type;
use super::Value;
use crate::ast::Pat;

/// The backing implementation for a [`Set`].
type SetImpl<T> = Arc<dyn Fn(&Context<T>, &T) -> Result<bool, Error> + Send + Sync + 'static>;

/// A test set, this can be used to check if a test is contained in it and is
/// expected to be the top level value in an [`ExpressionFilter`][filter].
///
/// [filter]: crate::ExpressionFilter
#[derive(Clone)]
pub struct Set<T>(SetImpl<T>);

impl<T> Set<T> {
    /// Create a new set with the given implementation.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&Context<T>, &T) -> Result<bool, Error> + Send + Sync + 'static,
    {
        Self(Arc::new(f) as _)
    }

    /// Whether the given test is contained within this set.
    pub fn contains(&self, ctx: &Context<T>, test: &T) -> Result<bool, Error> {
        (self.0)(ctx, test)
    }
}

impl<T> Debug for Set<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Set").field(&..).finish()
    }
}

impl<T: Test> Set<T> {
    /// Construct a test set which contains all tests matching the given pattern.
    ///
    /// This is the test set created from pattern literals like `r:'foot-(\w-)+'`.
    pub fn coerce_pat(pat: Pat) -> Set<T> {
        Set::new(move |_, test: &T| Ok(pat.is_match(test.id())))
    }
}

impl<T: 'static> Set<T> {
    /// Construct a set which contains all tests _not_ contained in the given
    /// set.
    ///
    /// This is the test set created by `!set`.
    pub fn expr_comp(set: Set<T>) -> Self {
        Self::new(move |ctx, test| Ok(!set.contains(ctx, test)?))
    }

    /// Construct a set which contains all tests which are contained in any of
    /// the given sets.
    ///
    /// This is the test set created by `a | b`.
    pub fn expr_union<I>(a: Set<T>, b: Set<T>, rest: I) -> Self
    where
        I: IntoIterator<Item = Set<T>>,
    {
        let sets: Vec<_> = [a, b].into_iter().chain(rest).collect();

        Self::new(move |ctx, test| {
            for set in &sets {
                if set.contains(ctx, test)? {
                    return Ok(true);
                }
            }

            Ok(false)
        })
    }

    /// Construct a set which contains all tests which are contained in all of
    /// the given sets.
    ///
    /// This is the test set created by `a & b`.
    pub fn expr_inter<I>(a: Set<T>, b: Set<T>, rest: I) -> Self
    where
        I: IntoIterator<Item = Set<T>>,
    {
        let sets: Vec<_> = [a, b].into_iter().chain(rest).collect();

        Self::new(move |ctx, test| {
            for set in &sets {
                if !set.contains(ctx, test)? {
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
    pub fn expr_diff(a: Set<T>, b: Set<T>) -> Self {
        Self::new(move |ctx, test| Ok(a.contains(ctx, test)? && !b.contains(ctx, test)?))
    }

    /// Construct a set which contains all tests which are contained in the
    /// either the first or the second, but not both sets.
    ///
    /// This is the test set created by `a ^ b`.
    pub fn expr_sym_diff(a: Set<T>, b: Set<T>) -> Self {
        Self::new(move |ctx, test| Ok(a.contains(ctx, test)? ^ b.contains(ctx, test)?))
    }
}

impl<T> TryFromValue<T> for Set<T> {
    fn try_from_value(value: Value<T>) -> Result<Self, Error> {
        Ok(match value {
            Value::Set(set) => set,
            _ => {
                return Err(Error::TypeMismatch {
                    expected: eco_vec![Type::Set],
                    found: value.as_type(),
                })
            }
        })
    }
}

/// Ensure Set<T> is thread safe if T is.
#[allow(dead_code)]
fn assert_traits() {
    tytanic_utils::assert::send::<Set<()>>();
    tytanic_utils::assert::sync::<Set<()>>();
}
