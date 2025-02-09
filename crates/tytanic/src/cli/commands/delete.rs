use std::io::Write;

use color_eyre::eyre;
use termcolor::Color;
use tytanic_core::suite::Filter;
use tytanic_utils::fmt::Term;

use super::{Context, FilterOptions};
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
    let suite = ctx.collect_tests_with_filter(&project, ctx.filter(&args.filter)?)?;

    let len = suite.matched().len();

    match len {
        0 => {
            ctx.warn_no_tests()?;
            return Ok(());
        }
        1 => {}
        _ => {
            if let Filter::TestSet(set) = suite.filter() {
                if !set.all() {
                    ctx.error_too_many_tests(&args.filter.expression)?;
                    eyre::bail!(OperationFailure);
                }
            }
        }
    }

    for test in suite.matched().values() {
        test.delete(project.paths())?;
    }

    let mut w = ctx.ui.stderr();

    write!(w, "Deleted ")?;
    cwrite!(bold_colored(w, Color::Green), "{len}")?;
    writeln!(w, " {}", Term::simple("test").with(len))?;

    Ok(())
}
