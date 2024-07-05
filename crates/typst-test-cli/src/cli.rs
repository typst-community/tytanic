use std::fmt::Display;
use std::path::PathBuf;

use clap::{ArgAction, ColorChoice};
use typst_test_lib::matcher;
use typst_test_lib::matcher::eval::Matcher;

use crate::fonts::FontSearcher;
use crate::project::Project;
use crate::report::Reporter;

pub mod add;
pub mod compare;
pub mod compile;
pub mod edit;
pub mod init;
pub mod list;
pub mod remove;
pub mod run;
pub mod status;
pub mod uninit;
pub mod update;
pub mod util;

pub struct Context<'a> {
    pub project: &'a mut Project,
    pub reporter: &'a mut Reporter,
}

#[repr(u8)]
pub enum CliResult {
    /// Typst-test ran succesfully.
    Ok = EXIT_OK,

    /// At least one test failed.
    TestFailure = EXIT_TEST_FAILURE,

    /// The requested operation failed gracefully.
    OperationFailure {
        message: Box<dyn Display + Send + 'static>,
        hint: Option<Box<dyn Display + Send + 'static>>,
    } = EXIT_OPERATION_FAILURE,
}

impl CliResult {
    pub fn operation_failure<M>(message: M) -> Self
    where
        M: Display + Send + 'static,
    {
        Self::OperationFailure {
            message: Box::new(message) as _,
            hint: None,
        }
    }

    pub fn hinted_operation_failure<M, H>(message: M, hint: H) -> Self
    where
        M: Display + Send + 'static,
        H: Display + Send + 'static,
    {
        Self::OperationFailure {
            message: Box::new(message) as _,
            hint: Some(Box::new(hint) as _),
        }
    }
}

/// Typst-test ran succesfully.
pub const EXIT_OK: u8 = 0;

/// At least one test failed.
pub const EXIT_TEST_FAILURE: u8 = 1;

/// The requested operation failed gracefully.
pub const EXIT_OPERATION_FAILURE: u8 = 2;

/// An unexpected error occured.
pub const EXIT_ERROR: u8 = 3;

macro_rules! ansi {
    ($s:expr; b) => {
        concat!("\x1B[1m", $s, "\x1B[0m")
    };
    ($s:expr; u) => {
        concat!("\x1B[4m", $s, "\x1B[0m")
    };
    ($s:expr;) => {
        $s
    };
    ($s:expr; $first:ident $( + $rest:tt)*) => {
        ansi!(ansi!($s; $($rest)*); $first)
    };
}

// NOTE: we use clap style formatting here and keep it simple to avoid a proc macro dependency for
// a single use of static ansi formatting
#[rustfmt::skip]
static AFTER_LONG_ABOUT: &str = concat!(
    ansi!("Exit Codes:\n"; u + b),
    "  ", ansi!("0"; b), "  Success\n",
    "  ", ansi!("1"; b), "  At least one test failed\n",
    "  ", ansi!("2"; b), "  The requested operation failed\n",
    "  ", ansi!("3"; b), "  An unexpected error occured",
);

#[derive(clap::Parser, Debug, Clone)]
pub struct Global {
    /// The project root directory
    #[arg(long, short, global = true)]
    pub root: Option<PathBuf>,

    /// A matcher expression for which tests to include in the given operation
    #[arg(long, short, global = true)]
    pub expression: Option<String>,

    #[command(flatten, next_help_heading = "Font Options")]
    pub fonts: FontArgs,

    #[command(flatten, next_help_heading = "Package Options")]
    pub package: PackageArgs,

    #[command(flatten, next_help_heading = "Output Options")]
    pub output: OutputArgs,
}

impl Global {
    pub fn matcher(&self) -> anyhow::Result<Matcher> {
        Ok(self
            .expression
            .as_deref()
            .map(matcher::parsing::parse_matcher_expr)
            .transpose()?
            .flatten()
            .map(matcher::build_matcher)
            .unwrap_or_default())
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct FontArgs {
    /// Do not read system fonts
    #[arg(long, global = true)]
    pub ignore_system_fonts: bool,

    /// Add a directory to read fonts from (can be repeated)
    #[arg(long = "font-path", value_name = "FONT_PATH", global = true, action = ArgAction::Append)]
    pub font_paths: Vec<PathBuf>,
}

impl FontArgs {
    pub fn searcher(&self) -> FontSearcher {
        let mut searcher = FontSearcher::new();
        searcher.search(
            self.font_paths.iter().map(PathBuf::as_path),
            !self.ignore_system_fonts,
        );

        searcher
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct PackageArgs {
    // TODO: add package dir args
}

#[derive(clap::Args, Debug, Clone)]
pub struct OutputArgs {
    /// The output format to use
    ///
    /// Using anything but pretty implies --color=never
    #[arg(
        long,
        short,
        global = true,
        visible_alias = "fmt",
        default_value = "pretty"
    )]
    pub format: OutputFormat,

    /// When to use colorful output
    ///
    /// If set to auto, color will only be enabled if a capable terminal is
    /// detected.
    #[clap(
        long,
        short,
        global = true,
        value_name = "WHEN",
        require_equals = true,
        num_args = 0..=1,
        default_value = "auto",
        default_missing_value = "always",
    )]
    pub color: ColorChoice,

    /// Produce more logging output [-v ... -vvvvv]
    ///
    /// Logs are written to stderr, the increasing number of verbose flags
    /// corresponds to the log levels ERROR, WARN, INFO, DEBUG, TRACE.
    #[arg(long, short, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

// TODO: add json
#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum OutputFormat {
    /// Pretty human-readible color output
    Pretty,

    /// Plain output for script processing
    Plain,
}

impl OutputFormat {
    pub fn is_pretty(&self) -> bool {
        matches!(self, Self::Pretty)
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct MutationArgs {
    /// Allow operating on more than one test if multiple tests match
    #[arg(long, short)]
    pub all: bool,
}

/// Execute, compare and update visual regression tests for typst
#[derive(clap::Parser, Debug, Clone)]
#[clap(after_long_help = AFTER_LONG_ABOUT)]
pub struct Args {
    #[command(flatten)]
    pub global: Global,

    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// Initialize the current project with a test directory
    Init(init::Args),

    /// Remove the test directory from the current project
    Uninit,

    /// Show information about the current project
    #[command(visible_alias = "st")]
    Status,

    /// List the tests in the current project
    #[command(visible_alias = "ls")]
    List,

    /// Compile and compare tests
    #[command(name = "run", visible_alias = "r")]
    Compare(compare::Args),

    /// Compile tests
    #[command(visible_alias = "c")]
    Compile(compile::Args),

    /// Compile and update tests
    #[command(visible_alias = "u")]
    Update(update::Args),

    /// Add a new test
    ///
    /// The default test simply contains `Hello World`, if a
    /// test template file is given, it is used instead.
    #[command(visible_alias = "a")]
    Add(add::Args),

    /// Edit existing tests
    #[command()]
    Edit(edit::Args),

    /// Remove tests
    #[command(visible_alias = "rm")]
    Remove(remove::Args),

    /// Utility commands
    #[command()]
    Util(util::Args),
}

macro_rules! bail_if_uninit {
    ($ctx:expr) => {
        if !$ctx.project.is_init()? {
            return Ok(CliResult::operation_failure(format!(
                "Project '{}' was not initialized",
                $ctx.project.name(),
            )));
        }
    };
}

macro_rules! bail_if_invalid_matcher_expr {
    ($global:expr => $ident:ident) => {
        let $ident = match $global.matcher() {
            Ok(matcher) => matcher,
            Err(err) => {
                return Ok(CliResult::operation_failure(format!(
                    "Could not parse matcher expression: {err}",
                )));
            }
        };
    };
}

pub(crate) use {bail_if_invalid_matcher_expr, bail_if_uninit};

impl Command {
    pub fn run(&self, ctx: Context, global: &Global) -> anyhow::Result<CliResult> {
        match self {
            Command::Init(args) => init::run(ctx, global, args),
            Command::Uninit => uninit::run(ctx, global),
            Command::Add(args) => add::run(ctx, global, args),
            Command::Edit(args) => edit::run(ctx, global, args),
            Command::Remove(args) => remove::run(ctx, global, args),
            Command::Status => status::run(ctx, global),
            Command::List => list::run(ctx, global),
            Command::Update(args) => update::run(ctx, global, args),
            Command::Compare(args) => compare::run(ctx, global, args),
            Command::Compile(args) => compile::run(ctx, global, args),
            Command::Util(args) => args.cmd.run(ctx, global),
        }
    }
}
