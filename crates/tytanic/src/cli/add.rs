use std::io::Write;
use std::ops::Not;

use color_eyre::eyre;
use termcolor::Color;
use typst::diag::Warned;
use typst_syntax::{FileId, Source, VirtualPath};
use tytanic_core::doc::render::ppi_to_ppp;
use tytanic_core::doc::Document;
use tytanic_core::test::{self, Id, Reference, Test};

use super::options::{Switch, Warnings};
use super::Context;
use crate::cli::options::{CompileOptions, ExportOptions};
use crate::cli::OperationFailure;
use crate::{cwriteln, ui, DEFAULT_OPTIMIZE_OPTIONS};

#[derive(clap::Args, Debug, Clone)]
#[group(id = "add-args")]
pub struct Args {
    /// Whether to create an ephemeral test
    #[arg(long, short)]
    pub ephemeral: bool,

    /// Whether to create a compile only test
    #[arg(long, short, conflicts_with = "ephemeral")]
    pub compile_only: bool,

    /// Ignore the test template for this test
    #[arg(long)]
    pub no_template: bool,

    #[command(flatten)]
    pub compile: CompileOptions,

    #[command(flatten)]
    pub export: ExportOptions,

    /// The name of the test to add
    pub test: Id,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let project = ctx.project()?;
    let suite = ctx.collect_all_tests(&project)?;

    if suite.matched().contains_key(&args.test) {
        ctx.error_test_already_exists(&args.test)?;
        eyre::bail!(OperationFailure);
    }

    let paths = project.paths();
    let vcs = project.vcs();
    let id = args.test.clone();

    'create: {
        let source = suite
            .template()
            .filter(|_| !args.no_template)
            .unwrap_or(test::DEFAULT_TEST_INPUT);

        let reference = if args.ephemeral {
            Some(Reference::Ephemeral(source.into()))
        } else if args.compile_only {
            None
        } else {
            if args.no_template {
                // NOTE(tinger): this is an optimized case where we write the
                // already optimized bytes directly to disk, skipping redunant
                // png compression optimization and compilation from source
                Test::create_default(paths, vcs, id)?;
                break 'create;
            }

            let world = ctx.world()?;
            let path = project.paths().template();

            let path = path
                .strip_prefix(project.paths().project_root())
                .expect("template is in project root");

            let Warned {
                output,
                mut warnings,
            } = Document::compile(
                Source::new(FileId::new(None, VirtualPath::new(path)), source.into()),
                &world,
                ppi_to_ppp(args.export.ppi),
                args.compile.warnings == Warnings::Promote,
            );

            if args.compile.warnings == Warnings::Ignore {
                warnings.clear();
            }

            let doc = match output {
                Ok(doc) => {
                    ui::write_diagnostics(
                        &mut ctx.ui.stderr(),
                        ctx.ui.diagnostic_config(),
                        &world,
                        &warnings,
                        &[],
                    )?;
                    doc
                }
                Err(err) => {
                    ui::write_diagnostics(
                        &mut ctx.ui.stderr(),
                        ctx.ui.diagnostic_config(),
                        &world,
                        &warnings,
                        &err.0,
                    )?;
                    eyre::bail!(OperationFailure);
                }
            };

            Some(Reference::Persistent(
                doc,
                args.export
                    .optimize_refs
                    .get_or_default()
                    .not()
                    .then(|| Box::new(DEFAULT_OPTIMIZE_OPTIONS.clone())),
            ))
        };

        Test::create(paths, vcs, id, source, reference)?;
    }

    let mut w = ctx.ui.stderr();

    write!(w, "Added ")?;
    cwriteln!(colored(w, Color::Cyan), "{}", args.test)?;

    Ok(())
}
