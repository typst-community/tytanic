use std::io::Write;
use std::{fs, io};

use color_eyre::eyre;
use termcolor::Color;
use tytanic_utils::fmt::Term;
use tytanic_utils::result::ResultEx;

use super::Context;
use crate::cwrite;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "util-vcs-args")]
pub struct Args {
    /// The sub command to run.
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// Remove all previously generated tracked `.gitignore` files.
    #[command()]
    Clear,
}

impl Command {
    pub fn run(&self, ctx: &mut Context) -> eyre::Result<()> {
        match self {
            Command::Clear => {
                let project = ctx.project()?;
                let suite = ctx.collect_tests(&project)?;

                let mut len = 0;
                for test in suite.unit_tests() {
                    let dir = project.unit_test_dir(test.id());
                    fs::remove_file(dir.join(".gitignore"))
                        .ignore(|e| e.kind() == io::ErrorKind::NotFound)?;
                    len += 1;
                }

                let mut w = ctx.ui.stderr();
                write!(w, "Removed old `.gitignore` files for ")?;
                cwrite!(colored(w, Color::Green), "{len}")?;
                writeln!(w, " {}", Term::simple("test").with(len))?;

                Ok(())
            }
        }
    }
}
