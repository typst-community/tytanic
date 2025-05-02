use clap::CommandFactory;
use clap_complete::Shell;
use color_eyre::eyre;

use crate::cli::Context;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "util-completion-args")]
pub struct Args {
    /// The shell to complete the arguments for.
    #[arg()]
    shell: Shell,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let mut cmd = crate::CliArguments::command();

    clap_complete::generate(
        args.shell,
        &mut cmd,
        std::env!("CARGO_BIN_NAME"),
        &mut ctx.ui.stdout(),
    );

    Ok(())
}
