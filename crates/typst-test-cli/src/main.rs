#![feature(once_cell_try)]

use std::io::Write;
use std::process::ExitCode;

use clap::{ColorChoice, Parser};
use termcolor::Color;
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use tracing_tree::HierarchicalLayer;

use crate::cli::{Context, OutputFormat};
use crate::report::Reporter;

mod cli;
mod download;
mod fonts;
mod package;
mod project;
mod report;
mod test;
mod util;
mod world;

const IS_OUTPUT_STDERR: bool = false;

fn main() -> ExitCode {
    let mut args = cli::Args::parse();

    // BUG: this interferes with the live printing
    if args.global.output.verbose >= 1 {
        tracing_subscriber::registry()
            .with(
                HierarchicalLayer::new(4)
                    .with_targets(true)
                    .with_ansi(util::term::color(
                        args.global.output.color,
                        IS_OUTPUT_STDERR,
                    )),
            )
            .with(Targets::new().with_target(
                std::env!("CARGO_CRATE_NAME"),
                match args.global.output.verbose {
                    1 => Level::ERROR,
                    2 => Level::WARN,
                    3 => Level::INFO,
                    4 => Level::DEBUG,
                    _ => Level::TRACE,
                },
            ))
            .init();

        // don't do any fancy line clearing if we're logging
        args.global.output.format = OutputFormat::Plain;
    }

    if !args.global.output.format.is_pretty() {
        args.global.output.color = ColorChoice::Never;
    }

    // TODO: simpler output when using plain
    let reporter = Reporter::new(
        util::term::color_stream(args.global.output.color, IS_OUTPUT_STDERR),
        args.global.output.format,
    );

    let mut ctx = Context::new(&args, reporter);

    match ctx.run() {
        Ok(()) => {}
        Err(_) if ctx.is_operation_failure() => {}
        Err(err) => {
            ctx.unexpected_error(|r| {
                writeln!(
                    r,
                    "typst-test ran into an unexpected error, this is most likely a bug"
                )
                .unwrap();
                writeln!(
                    r,
                    "Please consider reporting this at {}/issues/new",
                    std::env!("CARGO_PKG_REPOSITORY")
                )
                .unwrap();

                if !std::env::var("RUST_BACKTRACE").is_ok_and(|var| var == "full") {
                    r.hint(
                        "consider running with the environment variable RUST_BACKTRACE set to 'full' when reporting issues",
                    )
                    .unwrap();
                }

                writeln!(r).unwrap();

                r.write_annotated("Error:", Color::Red, None, |r| writeln!(r, "{err:?}"))
                    .unwrap();

                Ok(())
            })
            .unwrap();
        }
    };

    ctx.exit()
}
