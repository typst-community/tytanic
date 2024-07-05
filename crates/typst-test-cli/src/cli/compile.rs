use super::{run, CliResult, Context, Global};

#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    #[command(flatten)]
    pub run_args: run::Args,
}

pub fn run(ctx: Context, global: &Global, args: &Args) -> anyhow::Result<CliResult> {
    run::run(ctx, global, &args.run_args, |ctx| {
        ctx.with_compare(false).with_update(false)
    })
}
