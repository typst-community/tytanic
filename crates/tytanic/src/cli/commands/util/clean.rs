use std::io::Write;

use color_eyre::eyre;
use termcolor::Color;
use tytanic_utils::fmt::Term;

use super::Context;
use crate::cwrite;

pub fn run(ctx: &mut Context) -> eyre::Result<()> {
    let project = ctx.project()?;
    let suite = ctx.collect_tests(&project)?;

    let len = suite.tests().len();

    for test in suite.tests().values() {
        test.delete_temporary_directories(project.paths())?;
    }

    let mut w = ctx.ui.stderr();
    write!(w, "Removed temporary directories for ")?;
    cwrite!(colored(w, Color::Green), "{len}")?;
    writeln!(w, " {}", Term::simple("test").with(len))?;

    Ok(())
}
