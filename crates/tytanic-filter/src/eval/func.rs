use std::fmt;
use std::fmt::Debug;
use std::sync::Arc;

use ecow::eco_vec;

use super::Context;
use super::Error;
use super::TryFromValue;
use super::Type;
use super::Value;

/// The backing implementation for a [`Func`].
type FuncImpl<T> =
    Arc<dyn Fn(&Context<T>, &[Value<T>]) -> Result<Value<T>, Error> + Send + Sync + 'static>;

/// A function value, this can be called with a set of positional arguments to
/// produce a value. This is most commonly used as a constructor for tests sets.
#[derive(Clone)]
pub struct Func<T>(FuncImpl<T>);

impl<T> Func<T> {
    /// Create a new function with the given implementation.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&Context<T>, &[Value<T>]) -> Result<Value<T>, Error> + Send + Sync + 'static,
    {
        Self(Arc::new(f) as _)
    }

    /// Call the given function with the given context and arguments.
    pub fn call(&self, ctx: &Context<T>, args: &[Value<T>]) -> Result<Value<T>, Error> {
        (self.0)(ctx, args)
    }
}

impl<T> Debug for Func<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Func").field(&..).finish()
    }
}

impl<T> Func<T> {
    /// Ensure there are no args.
    pub fn expect_no_args(id: &str, _ctx: &Context<T>, args: &[Value<T>]) -> Result<(), Error> {
        if args.is_empty() {
            Ok(())
        } else {
            Err(Error::InvalidArgumentCount {
                func: id.into(),
                expected: 0,
                is_min: false,
                found: args.len(),
            })
        }
    }

    /// Extract an exact number of values from the given arguments. Validates the
    /// types of all arguments.
    pub fn expect_args_exact<V: TryFromValue<T> + Debug, const N: usize>(
        func: &str,
        _ctx: &Context<T>,
        args: &[Value<T>],
    ) -> Result<[V; N], Error>
    where
        T: Clone,
    {
        if args.len() < N {
            return Err(Error::InvalidArgumentCount {
                func: func.into(),
                expected: N,
                is_min: false,
                found: args.len(),
            });
        }

        Ok(args
            .iter()
            .take(N)
            .cloned()
            .map(V::try_from_value)
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .expect("we checked both min and max of the args"))
    }

    /// Extract a variadic number of values with a minimum amount given arguments.
    /// Validates the types of all arguments.
    pub fn expect_args_min<V: TryFromValue<T> + Debug, const N: usize>(
        func: &str,
        _ctx: &Context<T>,
        args: &[Value<T>],
    ) -> Result<([V; N], Vec<V>), Error>
    where
        T: Clone,
    {
        if args.len() < N {
            return Err(Error::InvalidArgumentCount {
                func: func.into(),
                expected: N,
                is_min: true,
                found: args.len(),
            });
        }

        let min = args
            .iter()
            .take(N)
            .cloned()
            .map(V::try_from_value)
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .expect("we checked both min and max of the args");

        Ok((
            min,
            args[N..]
                .iter()
                .cloned()
                .map(V::try_from_value)
                .collect::<Result<_, _>>()?,
        ))
    }
}

impl<T> TryFromValue<T> for Func<T> {
    fn try_from_value(value: Value<T>) -> Result<Self, Error> {
        Ok(match value {
            Value::Func(set) => set,
            _ => {
                return Err(Error::TypeMismatch {
                    expected: eco_vec![Type::Func],
                    found: value.as_type(),
                })
            }
        })
    }
}

/// Ensure Func<T> is thread safe if T is.
#[allow(dead_code)]
fn assert_traits() {
    tytanic_utils::assert::send::<Func<()>>();
    tytanic_utils::assert::sync::<Func<()>>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Num;

    const NUM: Num = Num(0);
    const VAL: Value<()> = Value::Num(NUM);

    #[test]
    fn test_expect_args_variadic_min_length() {
        let ctx = Context::new();

        assert_eq!(
            Func::expect_args_min::<Num, 0>("f", &ctx, &[]).unwrap(),
            ([], vec![]),
        );
        assert_eq!(
            Func::expect_args_min("f", &ctx, &[VAL]).unwrap(),
            ([], vec![NUM]),
        );
        assert_eq!(
            Func::expect_args_min("f", &ctx, &[VAL, VAL]).unwrap(),
            ([], vec![NUM, NUM]),
        );

        assert!(Func::expect_args_min::<Num, 1>("f", &ctx, &[]).is_err());
        assert_eq!(
            Func::expect_args_min("f", &ctx, &[VAL]).unwrap(),
            ([NUM], vec![]),
        );
        assert_eq!(
            Func::expect_args_min("f", &ctx, &[VAL, VAL]).unwrap(),
            ([NUM], vec![NUM]),
        );

        assert!(Func::expect_args_min::<Num, 2>("f", &ctx, &[]).is_err());
        assert!(Func::expect_args_min::<Num, 2>("f", &ctx, &[VAL]).is_err(),);
        assert_eq!(
            Func::expect_args_min("f", &ctx, &[VAL, VAL]).unwrap(),
            ([NUM, NUM], vec![]),
        );
    }
}
