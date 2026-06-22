use std::io::Write;
use std::ops::Not;

use color_eyre::eyre;
use termcolor::Color;
use typst::diag::Warned;
use typst::utils::Scalar;
use typst_kit::diagnostics;
use typst_kit::diagnostics::DiagnosticFormat;
use typst_render::RenderOptions;
use typst_syntax::FileId;
use typst_syntax::RootedPath;
use typst_syntax::Source;
use typst_syntax::VirtualPath;
use typst_syntax::VirtualRoot;
use tytanic_core::doc::Document;
use tytanic_core::doc::render::ppi_to_ppp;
use tytanic_core::test::Id;
use tytanic_core::test::UnitTest;
use tytanic_core::test::unit::DEFAULT_TEST_INPUT;
use tytanic_core::test::unit::Kind;
use tytanic_core::test::unit::Reference;

use super::CompileOptions;
use super::Context;
use super::ExportOptions;
use super::KindOption;
use super::OptionDelegate;
use super::Switch;
use super::TemplateSwitch;
use crate::DEFAULT_OPTIMIZE_OPTIONS;
use crate::cli::OperationFailure;
use crate::cli::commands::DiagnosticFormat as CliDiagnosticFormat;
use crate::cwriteln;
use crate::ui;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "new-args")]
pub struct Args {
    /// The type of test to create.
    #[arg(long = "type", short, group = "type", default_value = "persistent")]
    pub kind: KindOption,

    /// Shorthand for `--type=persistent`.
    #[arg(long, short = 'P', group = "type")]
    pub persistent: bool,

    /// Shorthand for `--type=ephemeral`.
    #[arg(long, short = 'E', group = "type")]
    pub ephemeral: bool,

    /// Shorthand for `--type=compile-only`.
    #[arg(long, short = 'C', group = "type")]
    pub compile_only: bool,

    #[command(flatten)]
    pub template: TemplateSwitch,

    #[command(flatten)]
    pub compile: CompileOptions,

    #[command(flatten)]
    pub export: ExportOptions,

    /// The name of the new test.
    #[arg(value_name = "NAME")]
    pub test: Id,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    if args.test == Id::template() {
        writeln!(ctx.ui.error()?, "Cannot create template test")?;
        eyre::bail!(OperationFailure);
    }

    let project = ctx.project()?;
    let suite = ctx.collect_tests(&project)?;

    if suite.contains(&args.test) {
        let mut w = ctx.ui.error()?;

        write!(w, "Test ")?;
        ui::write_test_id(&mut w, &args.test)?;
        writeln!(w, " already exists")?;
        eyre::bail!(OperationFailure);
    }

    let id = args.test.clone();

    let kind = if args.persistent {
        Kind::Persistent
    } else if args.ephemeral {
        Kind::Ephemeral
    } else if args.compile_only {
        Kind::CompileOnly
    } else {
        args.kind.into_native()
    };

    let source = project
        .unit_test_template()
        .filter(|_| args.template.get_or_default())
        .unwrap_or(DEFAULT_TEST_INPUT);

    let reference = match kind {
        Kind::CompileOnly => None,
        Kind::Ephemeral => Some(Reference::Ephemeral(source.into())),
        Kind::Persistent => {
            let providers =
                ctx.providers(&project, &ctx.args.package, &ctx.args.font, &args.compile)?;

            let path = project.unit_test_template_file();

            let id = FileId::new(RootedPath::new(
                VirtualRoot::Project,
                match VirtualPath::virtualize(project.root().as_std_path(), path.as_std_path()) {
                    Ok(path) => path,
                    Err(err) => eyre::bail!("failed to virtualize test path: {err:?}"),
                },
            ));
            let world = providers.system_world(Source::new(id, source.into()));

            let Warned { output, warnings } = Document::compile(
                &world,
                &RenderOptions {
                    pixel_per_pt: Scalar::new(ppi_to_ppp(
                        args.export.ppi.unwrap_or(project.config().defaults.ppi),
                    )),
                    render_bleed: false,
                },
                args.compile.warnings.into_native(),
            );

            let format = match args.compile.diagnostic_format {
                CliDiagnosticFormat::Human => DiagnosticFormat::Human,
                CliDiagnosticFormat::Short => DiagnosticFormat::Short,
            };

            let doc = match output {
                Ok(doc) => {
                    diagnostics::emit(&mut ctx.ui.stderr(), &world, &warnings, format)?;
                    doc
                }
                Err(err) => {
                    let mut w = ctx.ui.stderr();
                    diagnostics::emit(&mut w, &world, &warnings, format)?;
                    diagnostics::emit(&mut w, &world, &err.0, format)?;

                    eyre::bail!(OperationFailure);
                }
            };

            Some(Reference::Persistent {
                doc,
                opt: args
                    .export
                    .optimize_refs
                    .get_or_default()
                    .not()
                    .then(|| Box::new(DEFAULT_OPTIMIZE_OPTIONS.clone())),
            })
        }
    };

    UnitTest::create(&project, id, source, reference)?;

    let mut w = ctx.ui.stderr();

    write!(w, "Added ")?;
    cwriteln!(colored(w, Color::Cyan), "{}", args.test)?;

    Ok(())
}
