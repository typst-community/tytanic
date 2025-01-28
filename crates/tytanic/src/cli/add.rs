use std::io::Write;
use std::ops::Not;

use color_eyre::eyre;
use termcolor::Color;
use typst::diag::Warned;
use typst_syntax::{FileId, Source, VirtualPath};
use tytanic_core::doc::render::ppi_to_ppp;
use tytanic_core::doc::Document;
use tytanic_core::test::{Id, Reference, Test};

use super::{CompileArgs, Context, ExportArgs};
use crate::cli::OperationFailure;
use crate::{ui, DEFAULT_OPTIMIZE_OPTIONS};

#[derive(clap::Args, Debug, Clone)]
#[group(id = "add-args")]
pub struct Args {
    /// Whether this test creates it's references on the fly
    ///
    /// An ephemeral test consists of two scripts which are compared
    /// against each other. The reference script must be called `ref.typ`.
    #[arg(long, short)]
    pub ephemeral: bool,

    /// Whether this test has no references at all
    #[arg(long, short, conflicts_with = "ephemeral")]
    pub compile_only: bool,

    /// Ignore the test template for this test
    #[arg(long, conflicts_with_all = ["ephemeral", "compile_only"])]
    pub no_template: bool,

    #[command(flatten)]
    pub compile: CompileArgs,

    #[command(flatten)]
    pub export: ExportArgs,

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

    if let Some(template) = suite.template().filter(|_| !args.no_template) {
        if args.ephemeral {
            Test::create(
                paths,
                vcs,
                id,
                template,
                Some(Reference::Ephemeral(template.into())),
            )?;
        } else if args.compile_only {
            Test::create(paths, vcs, id, template, None)?;
        } else {
            let world = ctx.world(&args.compile)?;

            // TODO(tinger): properly report diagnostics
            let Warned {
                output,
                warnings: _,
            } = Document::compile(
                Source::new(FileId::new_fake(VirtualPath::new("")), template.to_owned()),
                &world,
                ppi_to_ppp(args.export.render.pixel_per_inch),
            );
            let doc = output?;

            Test::create(
                paths,
                vcs,
                id,
                template,
                Some(Reference::Persistent(
                    doc,
                    args.export
                        .no_optimize_references
                        .not()
                        .then(|| Box::new(DEFAULT_OPTIMIZE_OPTIONS.clone())),
                )),
            )?;
        };
    } else {
        Test::create_default(paths, vcs, id)?;
    }

    let mut w = ctx.ui.stderr();

    write!(w, "Added ")?;
    ui::write_colored(&mut w, Color::Cyan, |w| writeln!(w, "{}", args.test))?;

    Ok(())
}
