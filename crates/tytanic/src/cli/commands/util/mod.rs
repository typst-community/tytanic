use color_eyre::eyre;
use tytanic_core::config::{ProjectConfig, SettingsConfig, TestConfig};

use super::Context;

pub mod about;
pub mod clean;
pub mod completion;
pub mod fonts;
pub mod manpage;
pub mod migrate;
pub mod vcs;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "util-args")]
pub struct Args {
    /// The sub command to run.
    #[command(subcommand)]
    pub cmd: Command,
}

impl Args {
    /// Adds the CLI arguments to the CLI config layer.
    pub fn cli_config_layer(
        &self,
        settings: &mut SettingsConfig,
        project: &mut ProjectConfig,
        test: &mut TestConfig,
    ) {
        match &self.cmd {
            Command::About => {}
            Command::Clean(clean::Args {
                include_persistent_references: _,
                filter: _,
            }) => {}
            Command::Completion(completion::Args { shell: _ }) => {}
            Command::Manpage(manpage::Args { dir: _ }) => {}
            Command::Fonts(fonts::Args {
                variants: _,
                json: _,
            }) => {}
            Command::Migrate(migrate::Args {
                confirm: _,
                name: _,
            }) => {}
            Command::Vcs(args) => args.cli_config_layer(settings, project, test),
        }
    }
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// Print information about this program.
    #[command()]
    About,

    /// Remove test output artifacts.
    #[command()]
    Clean(clean::Args),

    /// Generate completions.
    #[command()]
    Completion(completion::Args),

    /// Generate a man page for Tytanic.
    #[command()]
    Manpage(manpage::Args),

    /// List all available fonts.
    #[command()]
    Fonts(fonts::Args),

    /// Migrate the test structure to the new version.
    #[command()]
    Migrate(migrate::Args),

    /// Vcs related commands.
    #[command()]
    Vcs(vcs::Args),
}

impl Command {
    pub fn run(&self, ctx: &mut Context) -> eyre::Result<()> {
        match self {
            Command::About => about::run(ctx),
            Command::Clean(args) => clean::run(ctx, args),
            Command::Completion(args) => completion::run(ctx, args),
            Command::Manpage(args) => manpage::run(ctx, args),
            Command::Fonts(args) => fonts::run(ctx, args),
            Command::Migrate(args) => migrate::run(ctx, args),
            Command::Vcs(args) => args.cmd.run(ctx),
        }
    }
}
