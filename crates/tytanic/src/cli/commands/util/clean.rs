use std::io::Write;

use color_eyre::eyre;
use termcolor::Color;
use tytanic_utils::fmt::Term;

use super::Context;
use crate::cli::commands::FilterOptions;
use crate::cwrite;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "util-clean-args")]
pub struct Args {
    /// Also remove persistent references.
    #[arg(long)]
    pub include_persistent_references: bool,

    #[command(flatten)]
    pub filter: FilterOptions,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let project = ctx.project()?;
    let suite = ctx.collect_tests_with_filter(&project, ctx.filter(&args.filter)?)?;

    let mut temp = 0;
    let mut persistent = 0;
    for test in suite.matched().unit_tests() {
        test.delete_temporary_directories(&project)?;
        if args.include_persistent_references && test.kind().is_persistent() {
            test.delete_reference_document(&project)?;
            persistent += 1;
        }
        temp += 1;
    }

    let mut w = ctx.ui.stderr();
    write!(w, "Removed temporary directories for ")?;
    cwrite!(colored(w, Color::Green), "{temp}")?;
    writeln!(w, " {}", Term::simple("test").with(temp))?;

    if persistent != 0 {
        write!(w, "Removed persistent references for ")?;
        cwrite!(colored(w, Color::Green), "{persistent}")?;
        writeln!(w, " {}", Term::simple("test").with(temp))?;
    }

    Ok(())
}
