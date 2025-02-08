use color_eyre::eyre;
use tytanic_core::doc::compare::Strategy;
use tytanic_core::doc::render::{self, Origin};

use super::options::Switch;
use super::{Context, CANCELLED};
use crate::cli::options::{
    CompareOptions, CompileOptions, Direction, ExportOptions, FilterOptions, OptionDelegate,
    RunnerOptions,
};
use crate::cli::TestFailure;
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
    let set = ctx.test_set(&args.filter)?;
    let suite = ctx.collect_tests(&project, &set)?;
    let world = ctx.world()?;

    let origin = match args.export.dir {
        Direction::Ltr => Origin::TopLeft,
        Direction::Rtl => Origin::TopRight,
    };

    let runner = Runner::new(
        &project,
        &suite,
        &world,
        RunnerConfig {
            warnings: args.compile.warnings.into_native(),
            optimize: args.export.optimize_refs.get_or_default(),
            fail_fast: args.runner.fail_fast.get_or_default(),
            pixel_per_pt: render::ppi_to_ppp(args.export.ppi),
            action: Action::Run {
                strategy: args
                    .compare
                    .compare
                    .get_or_default()
                    .then_some(Strategy::Simple {
                        max_delta: args.compare.max_delta,
                        max_deviation: args.compare.max_deviations,
                    }),
                export_ephemeral: args.export.export_ephemeral.get_or_default(),
                origin,
            },
            cancellation: &CANCELLED,
        },
    );

    let reporter = Reporter::new(
        ctx.ui,
        &project,
        &world,
        ctx.ui.can_live_report() && ctx.args.output.verbose == 0,
    );
    let result = runner.run(&reporter)?;

    if !result.is_complete_pass() {
        eyre::bail!(TestFailure);
    }

    Ok(())
}
