use std::io::Write;

use color_eyre::eyre;
use termcolor::Color;
use tytanic_core::Id;
use tytanic_core::test::Test;
use tytanic_filter::test_set::builtin::dsl;
use tytanic_filter::test_set::eval;
use tytanic_utils::fmt::Term;

use super::Context;
use super::FilterOptions;
use crate::cli::OperationFailure;
use crate::cwrite;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "delete-args")]
pub struct Args {
    #[command(flatten)]
    pub filter: FilterOptions,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let project = ctx.project()?;

    let mut filter = ctx.filter(&args.filter)?;

    if let Some(exact) = filter.exact()
        && exact.expected().contains(&Id::template())
    {
        writeln!(ctx.ui.error()?, "Cannot delete template test")?;
        eyre::bail!(OperationFailure);
    }

    filter.map_test_set(|set| eval::Set::expr_diff(set, dsl::set_template()));

    let suite = ctx.collect_tests_with_filter(&project, filter)?;

    // If we have more than 1 + the exact tests, then they were matched by the
    // test set. In this case we must ensure that we require the `all:` prefix.
    let too_many = 1 + suite
        .filter()
        .exact()
        .map(|exact| exact.expected().len())
        .unwrap_or_default();

    if suite.matched().len() > too_many
        && let Some(set) = suite.filter().test_set()
        && set.all()
    {
        ctx.error_too_many_tests(set.input())?;
        eyre::bail!(OperationFailure);
    }

    for test in suite.matched() {
        if let Test::Unit(test) = test {
            test.delete(&project)?;
        }
    }

    let len = suite.matched().len();

    let mut w = ctx.ui.stderr();
    write!(w, "Deleted ")?;
    cwrite!(bold_colored(w, Color::Green), "{len}")?;
    writeln!(w, " {}", Term::simple("test").with(len))?;

    Ok(())
}
