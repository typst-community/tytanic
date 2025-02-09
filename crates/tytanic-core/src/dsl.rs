//! Implementations for the test set DSL. See the language [reference] and
//! [guide] for more info.
//!
//! [reference]: https://tingerrr.github.io/tytanic/reference/test-sets/index.html
//! [guide]: https://tingerrr.github.io/tytanic/guides/test-sets.html

use tytanic_filter::ast::Id;
use tytanic_filter::eval::{self, Context, Error, Func, Set, Value};

use crate::test::Test;

impl eval::Test for Test {
    fn id(&self) -> &str {
        self.id().as_str()
    }
}

/// Creates the default context used by tytanic, this contains bindings for the
/// constructor functions in [`built_in`].
pub fn context() -> Context<Test> {
    type FuncPtr =
        for<'a, 'b> fn(&'a Context<Test>, &'b [Value<Test>]) -> Result<Value<Test>, Error>;

    let mut ctx = Context::new();

    let functions = [
        ("all", built_in::all_ctor as FuncPtr),
        ("none", built_in::none_ctor),
        ("skip", built_in::skip_ctor),
        ("compile_only", built_in::compile_only_ctor),
        ("ephemeral", built_in::ephemeral_ctor),
        ("persistent", built_in::persistent_ctor),
    ];

    for (id, func) in functions {
        ctx.bind(Id(id.into()), Value::Func(Func::new(func)));
    }

    ctx
}

/// Function definitions for the tytanic test set DSL default evaluation
/// context.
pub mod built_in {
    use tytanic_filter::eval::{Context, Error, Func, Value};

    use super::*;

    /// The constructor function for the test set returned by [`all`].
    pub fn all_ctor(ctx: &Context<Test>, args: &[Value<Test>]) -> Result<Value<Test>, Error> {
        Func::expect_no_args("all", ctx, args)?;
        Ok(Value::Set(all()))
    }

    /// Constructor for the `all()` test set. A test set which contains _all_
    /// tests.
    pub fn all() -> Set<Test> {
        Set::new(|_, _| Ok(true))
    }

    /// The constructor function for the test set returned by [`none`].
    pub fn none_ctor(ctx: &Context<Test>, args: &[Value<Test>]) -> Result<Value<Test>, Error> {
        Func::expect_no_args("none", ctx, args)?;
        Ok(Value::Set(none()))
    }

    /// Constructor for the `none()` test set. A test set which contains _no_
    /// tests.
    pub fn none() -> Set<Test> {
        Set::new(|_, _| Ok(false))
    }

    /// The constructor function for the test set returned by [`skip`].
    pub fn skip_ctor(ctx: &Context<Test>, args: &[Value<Test>]) -> Result<Value<Test>, Error> {
        Func::expect_no_args("skip", ctx, args)?;
        Ok(Value::Set(skip()))
    }

    /// Constructs the `skip()` test set. A test set which contains all tests marked
    /// with the `skip` annotation.
    pub fn skip() -> Set<Test> {
        Set::new(|_, test: &Test| Ok(test.is_skip()))
    }

    /// The constructor function for the test set returned by [`compile_only`].
    pub fn compile_only_ctor(
        ctx: &Context<Test>,
        args: &[Value<Test>],
    ) -> Result<Value<Test>, Error> {
        Func::expect_no_args("compile-only", ctx, args)?;
        Ok(Value::Set(compile_only()))
    }

    /// Constructs the `compile-only()` test set. A test set which contains all
    /// `compile-only` tests.
    pub fn compile_only() -> Set<Test> {
        Set::new(|_, test: &Test| Ok(test.kind().is_compile_only()))
    }

    /// The constructor function for the test set returned by [`ephemeral`].
    pub fn ephemeral_ctor(ctx: &Context<Test>, args: &[Value<Test>]) -> Result<Value<Test>, Error> {
        Func::expect_no_args("ephemeral", ctx, args)?;
        Ok(Value::Set(ephemeral()))
    }

    /// Constructs the `ephemeral()` test set. A test set which contains all
    /// `ephemeral` tests.
    pub fn ephemeral() -> Set<Test> {
        Set::new(|_, test: &Test| Ok(test.kind().is_ephemeral()))
    }

    /// The constructor function for the test set returned by [`persistent`].
    pub fn persistent_ctor(
        ctx: &Context<Test>,
        args: &[Value<Test>],
    ) -> Result<Value<Test>, Error> {
        Func::expect_no_args("persistent", ctx, args)?;
        Ok(Value::Set(persistent()))
    }

    /// Constructs the `persistent()` test set. A test set which contains all
    /// `persistent` tests.
    pub fn persistent() -> Set<Test> {
        Set::new(|_, test: &Test| Ok(test.kind().is_persistent()))
    }
}
