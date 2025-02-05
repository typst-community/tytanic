use color_eyre::eyre;
use tytanic_core::doc::render::{self, Origin};
use tytanic_core::test_set::eval;

use super::options::Switch;
use super::{Context, CANCELLED};
use crate::cli::options::{CompileOptions, Direction, ExportOptions, FilterOptions, RunnerOptions};
use crate::cli::TestFailure;
use crate::report::Reporter;
use crate::runner::{Action, Runner, RunnerConfig};

#[derive(clap::Args, Debug, Clone)]
#[group(id = "update-args")]
pub struct Args {
    #[command(flatten)]
    pub compile: CompileOptions,

    #[command(flatten)]
    pub export: ExportOptions,

    #[command(flatten)]
    pub runner: RunnerOptions,

    #[command(flatten)]
    pub filter: FilterOptions,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let project = ctx.project()?;
    let mut set = ctx.test_set(&args.filter)?;
    set.add_intersection(eval::Set::built_in_persistent());
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
            warnings: args.compile.warnings,
            optimize: args.export.optimize_refs.get_or_default(),
            fail_fast: args.runner.fail_fast.get_or_default(),
            pixel_per_pt: render::ppi_to_ppp(args.export.ppi),
            action: Action::Update {
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
