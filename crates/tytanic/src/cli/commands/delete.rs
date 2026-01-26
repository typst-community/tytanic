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
    let too_many = filter
        .exact()
        .map(|exact| exact.expected().len())
        .unwrap_or(1);
    let not_all = filter.test_set().map(|set| set.all()).unwrap_or_default();

    if let Some(exact) = filter.exact()
        && exact.expected().contains(&Id::template())
    {
        writeln!(ctx.ui.error()?, "Cannot delete template test")?;
        eyre::bail!(OperationFailure);
    }

    filter.map_test_set(|set| eval::Set::expr_diff(set, dsl::set_template()));

    let suite = ctx.collect_tests_with_filter(&project, filter)?;

    // TODO(tinger): How should this be handled, if there is a set that matches
    // more than one test and also an exact filter set we probably don't emit a
    // helpful message here.
    if suite.matched().len() > too_many && not_all {
        if let Some(expression) = &args.filter.expression {
            ctx.error_too_many_tests(expression)?;
        } else {
            todo!();
        }
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
