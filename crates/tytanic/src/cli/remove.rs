use std::io::Write;

use color_eyre::eyre;
use termcolor::Color;
use tytanic_core::stdx::fmt::Term;

use super::{Context, FilterArgs};
use crate::cwrite;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "remove-args")]
pub struct Args {
    /// Whether to the skip confirmation prompt
    #[arg(long, short)]
    pub force: bool,

    #[command(flatten)]
    pub filter: FilterArgs,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let project = ctx.project()?;
    let set = ctx.test_set(&args.filter)?;
    let suite = ctx.collect_tests(&project, &set)?;

    let len = suite.matched().len();

    let confirmed = args.force
        || ctx.ui.prompt_yes_no(
            format!(
                "confirm deletion of {len} {}",
                Term::simple("test").with(len)
            ),
            false,
        )?;

    if !confirmed {
        ctx.error_aborted()?;
    }

    for test in suite.matched().values() {
        test.delete(project.paths())?;
    }

    let mut w = ctx.ui.stderr();

    write!(w, "Removed ")?;
    cwrite!(bold_colored(w, Color::Green), "{len}")?;
    writeln!(w, " {}", Term::simple("test").with(len))?;

    Ok(())
}
