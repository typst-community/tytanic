use std::io::Write;

use color_eyre::eyre;
use termcolor::Color;
use tytanic_core::test::Test;
use tytanic_core::test::UnitKind;

use super::Context;
use super::FilterOptions;
use crate::cwrite;
use crate::json::TestJson;
use crate::ui;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "list-args")]
pub struct Args {
    /// Print a JSON describing the project to stdout.
    #[arg(long)]
    pub json: bool,

    #[command(flatten)]
    pub filter: FilterOptions,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let project = ctx.project()?;
    let suite = ctx.collect_tests_with_filter(&project, ctx.filter(&args.filter)?)?;

    if args.json {
        serde_json::to_writer_pretty(
            ctx.ui.stdout(),
            &suite
                .matched_len()
                .tests()
                .map(|test| TestJson::new(&project, test))
                .collect::<Vec<_>>(),
        )?;

        return Ok(());
    }

    let mut w = ctx.ui.stderr();

    // NOTE(tinger): Max padding of 50 should be enough for most cases.
    let pad = Ord::min(
        suite
            .matched_len()
            .tests()
            .map(|test| test.id().len())
            .max()
            .unwrap_or(usize::MAX),
        50,
    );

    for test in suite.matched_len().tests() {
        ui::write_test_ident(&mut w, test.id())?;
        if let Some(pad) = pad.checked_sub(test.id().len()) {
            write!(w, "{: >pad$} ", "")?;
        }

        match test {
            Test::Unit(test) => {
                let color = match test.kind() {
                    UnitKind::Ephemeral => Color::Green,
                    UnitKind::Persistent => Color::Green,
                    UnitKind::CompileOnly => Color::Yellow,
                };
                // pad by 12 for `compile-only`
                cwrite!(bold_colored(w, color), "{: <12}", test.kind().as_str())?;

                if test.is_skip() {
                    write!(w, " ")?;
                    cwrite!(bold_colored(w, Color::Cyan), "skip")?;
                }
            }
            Test::Template(_) => {
                cwrite!(bold_colored(w, Color::Magenta), "{: <12}", "template")?;
            }
            Test::Doc(doc_test) => {
                cwrite!(bold_colored(w, Color::Blue), "{: <12}", "doc")?;
            }
        }

        writeln!(w)?;
    }

    Ok(())
}
