use std::path::PathBuf;

use clap::CommandFactory;
use color_eyre::eyre;

use crate::cli::Context;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "util-manpage-args")]
pub struct Args {
    /// The directory to write the man pages to
    #[arg(default_value = ".")]
    pub dir: PathBuf,
}

pub fn run(_ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let mut cmd = crate::CliArguments::command();
    cmd.set_bin_name(std::env!("CARGO_BIN_NAME"));

    tytanic_utils::fs::create_dir(&args.dir, true)?;

    clap_mangen::generate_to(cmd, &args.dir)?;

    Ok(())
}
