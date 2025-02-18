use color_eyre::eyre;
use tytanic_core::doc::compare::Strategy;
use tytanic_core::doc::render::{self, Origin};

use super::{
    CompareOptions, CompileOptions, Context, Direction, ExportOptions, FilterOptions,
    OptionDelegate, RunnerOptions, Switch,
};
use crate::cli::{TestFailure, CANCELLED};
use crate::report::Reporter;
use crate::runner::{Action, Runner, RunnerConfig};

#[derive(clap::Args, Debug, Clone)]
#[group(id = "run-args")]
pub struct Args {
    #[command(flatten)]
    pub compile: CompileOptions,

    #[command(flatten)]
    pub compare: CompareOptions,

    #[command(flatten)]
    pub export: ExportOptions,

    #[command(flatten)]
    pub runner: RunnerOptions,

    #[command(flatten)]
    pub filter: FilterOptions,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let project = ctx.project()?;
    let suite = ctx.collect_tests_with_filter(&project, ctx.filter(&args.filter)?)?;
    let world = ctx.world(&args.compile)?;

    let origin = match args
        .export
        .dir
        .map(OptionDelegate::into_native)
        .unwrap_or(project.config().defaults.direction)
    {
        Direction::Ltr => Origin::TopLeft,
        Direction::Rtl => Origin::TopRight,
    };

    let pixel_per_pt = render::ppi_to_ppp(args.export.ppi.unwrap_or(project.config().defaults.ppi));

    let max_delta = args
        .compare
        .max_delta
        .unwrap_or(project.config().defaults.max_delta);

    let max_deviation = args
        .compare
        .max_deviations
        .unwrap_or(project.config().defaults.max_deviations);

    let runner = Runner::new(
        &project,
        &suite,
        &world,
        RunnerConfig {
            warnings: args.compile.warnings.into_native(),
            optimize: args.export.optimize_refs.get_or_default(),
            fail_fast: args.runner.fail_fast.get_or_default(),
            pixel_per_pt,
            action: Action::Run {
                strategy: args
                    .compare
                    .compare
                    .get_or_default()
                    .then_some(Strategy::Simple {
                        max_delta,
                        max_deviation,
                    }),
                export_ephemeral: args.export.export_ephemeral.get_or_default(),
                origin,
            },
            cancellation: &CANCELLED,
        },
    );

    let reporter = Reporter::new(
        ctx.ui,
        &world,
        ctx.ui.can_live_report() && ctx.args.output.verbose == 0,
    );
    let result = runner.run(&reporter)?;

    if !result.is_complete_pass() {
        eyre::bail!(TestFailure);
    }

    Ok(())
}
