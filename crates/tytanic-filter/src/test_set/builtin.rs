//! Built-in bindings for the Tytanic testset DSL.
//!
//! See the language [reference] and [guide] for more info.
//!
//! [reference]: https://typst-community.github.io/tytanic/reference/test-sets/index.html
//! [guide]: https://typst-community.github.io/tytanic/guides/test-sets.html

use tytanic_core::test::Test;

use crate::test_set::ast::Id;
use crate::test_set::eval::Context;
use crate::test_set::eval::Error;
use crate::test_set::eval::Func;
use crate::test_set::eval::Set;
use crate::test_set::eval::Value;

/// Creates the default context used by Tytanic, this contains bindings for the
/// constructor functions in [`dsl`].
pub fn context() -> Context {
    type FuncPtr = for<'a, 'b> fn(&'a Context, &'b [Value]) -> Result<Value, Error>;

    let mut ctx = Context::new();

    let functions = [
        ("all", dsl::func_all_ctor as FuncPtr),
        ("none", dsl::func_none_ctor),
        ("skip", dsl::func_skip_ctor),
        ("unit", dsl::func_unit_ctor),
        ("template", dsl::func_template_ctor),
        ("compile-only", dsl::func_compile_only_ctor),
        ("ephemeral", dsl::func_ephemeral_ctor),
        ("persistent", dsl::func_persistent_ctor),
    ];

    for (id, func) in functions {
        ctx.bind(Id(id.into()), Value::Func(Func::new(func)));
    }

    ctx
}

/// Functions and value constructors for the built-ins of the testset DSL.
///
/// All bindings are prefixed with their respective [`Value`] type.
pub mod dsl {
    use super::*;

    /// The constructor function for the test set returned by [`set_all`].
    pub fn func_all_ctor(ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        Func::expect_no_args("all", ctx, args)?;
        Ok(Value::Set(set_all()))
    }

    /// Constructor for the `all()` test set. A test set which contains _all_
    /// tests.
    pub fn set_all() -> Set {
        Set::new(|_, _, _| Ok(true))
    }

    /// The constructor function for the test set returned by [`set_none`].
    pub fn func_none_ctor(ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        Func::expect_no_args("none", ctx, args)?;
        Ok(Value::Set(set_none()))
    }

    /// Constructor for the `none()` test set. A test set which contains _no_
    /// tests.
    pub fn set_none() -> Set {
        Set::new(|_, _, _| Ok(false))
    }

    /// The constructor function for the test set returned by [`set_skip`].
    pub fn func_skip_ctor(ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        Func::expect_no_args("skip", ctx, args)?;
        Ok(Value::Set(set_skip()))
    }

    /// Constructs the `skip()` test set. A test set which contains all tests
    /// marked with the `skip` annotation.
    pub fn set_skip() -> Set {
        Set::new(|_, _, test: &Test| Ok(test.as_unit_test().is_some_and(|unit| unit.is_skip())))
    }

    /// The constructor function for the test set returned by [`set_unit`].
    pub fn func_unit_ctor(ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        Func::expect_no_args("unit", ctx, args)?;
        Ok(Value::Set(set_unit()))
    }

    /// Constructs the `unit()` test set. A test set which contains all unit tests.
    pub fn set_unit() -> Set {
        Set::new(|_, _, test: &Test| Ok(test.as_unit_test().is_some()))
    }

    /// The constructor function for the test set returned by [`set_template`].
    pub fn func_template_ctor(ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        Func::expect_no_args("template", ctx, args)?;
        Ok(Value::Set(set_template()))
    }

    /// Constructs the `template()` test set. A test set which contains all
    /// template tests.
    pub fn set_template() -> Set {
        Set::new(|_, _, test: &Test| Ok(test.as_template_test().is_some()))
    }

    /// The constructor function for the test set returned by
    /// [`set_compile_only`].
    pub fn func_compile_only_ctor(ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        Func::expect_no_args("compile-only", ctx, args)?;
        Ok(Value::Set(set_compile_only()))
    }

    /// Constructs the `compile-only()` test set. A test set which contains all
    /// `compile-only` unit tests.
    pub fn set_compile_only() -> Set {
        Set::new(|_, _, test: &Test| {
            Ok(test
                .as_unit_test()
                .is_some_and(|unit| unit.kind().is_compile_only()))
        })
    }

    /// The constructor function for the test set returned by [`set_ephemeral`].
    pub fn func_ephemeral_ctor(ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        Func::expect_no_args("ephemeral", ctx, args)?;
        Ok(Value::Set(set_ephemeral()))
    }

    /// Constructs the `ephemeral()` test set. A test set which contains all
    /// `ephemeral` unit tests.
    pub fn set_ephemeral() -> Set {
        Set::new(|_, _, test: &Test| {
            Ok(test
                .as_unit_test()
                .is_some_and(|unit| unit.kind().is_ephemeral()))
        })
    }

    /// The constructor function for the test set returned by
    /// [`set_persistent`].
    pub fn func_persistent_ctor(ctx: &Context, args: &[Value]) -> Result<Value, Error> {
        Func::expect_no_args("persistent", ctx, args)?;
        Ok(Value::Set(set_persistent()))
    }

    /// Constructs the `persistent()` test set. A test set which contains all
    /// `persistent` unit tests.
    pub fn set_persistent() -> Set {
        Set::new(|_, _, test: &Test| {
            Ok(test
                .as_unit_test()
                .is_some_and(|unit| unit.kind().is_persistent()))
        })
    }
}
