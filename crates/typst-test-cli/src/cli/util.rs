use super::{CliResult, Context, Global};

pub mod clean;
pub mod export;
pub mod fonts;

#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// Remove test output artifacts
    Clean,

    /// Comp[ile and export tests and references
    #[command(visible_alias = "e")]
    Export(export::Args),

    /// List all available fonts
    Fonts(fonts::Args),
}

impl Command {
    pub fn run(&self, ctx: Context, global: &Global) -> anyhow::Result<CliResult> {
        match self {
            Command::Clean => clean::run(ctx, global),
            Command::Export(args) => export::run(ctx, global, args),
            Command::Fonts(args) => fonts::run(ctx, global, args),
        }
    }
}
