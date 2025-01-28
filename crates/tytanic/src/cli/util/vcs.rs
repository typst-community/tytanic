use std::io::Write;

use color_eyre::eyre;
use termcolor::Color;
use tytanic_core::stdx::fmt::Term;

use crate::cli::OperationFailure;
use crate::ui;

use super::Context;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "util-vcs-args")]
pub struct Args {
    /// The sub command to run
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// Rewrite all ignore files
    #[command()]
    Ignore,
}

impl Command {
    pub fn run(&self, ctx: &mut Context) -> eyre::Result<()> {
        match self {
            Command::Ignore => {
                let project = ctx.project()?;
                let paths = project.paths();
                let Some(vcs) = project.vcs() else {
                    ctx.ui.warning("no VCS detected")?;
                    eyre::bail!(OperationFailure);
                };

                let suite = ctx.collect_all_tests(&project)?;

                let len = suite.matched().len();

                for test in suite.matched().values() {
                    vcs.ignore(paths, test)?;
                }

                let mut w = ctx.ui.stderr();
                write!(w, "Rewritten ignore files for ")?;
                ui::write_colored(&mut w, Color::Green, |w| write!(w, "{len}"))?;
                writeln!(w, " {}", Term::simple("test").with(len))?;

                Ok(())
            }
        }
    }
}
