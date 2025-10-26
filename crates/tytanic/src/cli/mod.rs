use std::env;
use std::io;
use std::io::Write;
use std::sync::atomic::AtomicBool;

use color_eyre::eyre;
use color_eyre::eyre::WrapErr;
use termcolor::Color;
use thiserror::Error;
use tytanic_core::config::LayeredConfig;
use tytanic_core::config::ProjectConfig;
use tytanic_core::config::ReadError;
use tytanic_core::config::SettingsConfig;
use tytanic_core::config::TestConfig;
use tytanic_core::project::LoadError;
use tytanic_core::project::ProjectContext;
use tytanic_core::project::store::PersistentReferencesError;
use tytanic_core::project::vcs::Kind as VcsKind;
use tytanic_core::project::vcs::Vcs;
use tytanic_core::suite::SearchOptions;
use tytanic_core::suite::Suite;
use tytanic_core::test;
use tytanic_core::test::ParseIdentError;
use tytanic_filter::CombinedFilter;
use tytanic_filter::exact::ExactFilter;
use tytanic_filter::test_set::ExpressionFilter;
use tytanic_filter::test_set::builtin;
use tytanic_filter::test_set::builtin::dsl;
use tytanic_filter::test_set::eval;
use tytanic_utils::cwrite;

use crate::cli::commands::CliArguments;
use crate::cli::commands::FilterOptions;
use crate::cli::commands::Switch;
use crate::ui::Ui;
use crate::ui::write_test_ident;

pub mod commands;

/// Whether we received a signal we can gracefully exit from.
pub static CANCELLED: AtomicBool = AtomicBool::new(false);

/// Tytanic exited successfully.
pub const EXIT_OK: u8 = 0;

/// At least one test failed.
pub const EXIT_TEST_FAILURE: u8 = 1;

/// The requested operation failed gracefully.
pub const EXIT_OPERATION_FAILURE: u8 = 2;

/// An unexpected error occurred.
pub const EXIT_ERROR: u8 = 3;

/// A graceful error.
#[derive(Debug, Error)]
#[error("an operation failed")]
pub struct OperationFailure;

/// A test failure.
#[derive(Debug, Error)]
#[error("one or more test failed")]
pub struct TestFailure;

pub struct Context<'a> {
    /// The parsed top-level arguments.
    pub args: &'a CliArguments,

    /// The terminal UI.
    pub ui: &'a Ui,
}

impl<'a> Context<'a> {
    pub fn new(args: &'a CliArguments, ui: &'a Ui) -> Self {
        Self { args, ui }
    }
}

impl Context<'_> {
    /// Emit an error that the given expression evaluated to more than the
    /// allowed number of tests for some operation.
    pub fn error_too_many_tests(&self, expr: &str) -> io::Result<()> {
        writeln!(self.ui.error()?, "Matched more than one test")?;

        let mut w = self.ui.hint()?;
        write!(w, "use '")?;
        cwrite!(colored(w, Color::Cyan), "all:")?;
        writeln!(w, "{expr}' to confirm using all tests")
    }
}

// TODO(tinger): Cache these values.
impl Context<'_> {
    /// Discover the current and ensure it is initialized.
    #[tracing::instrument(skip_all)]
    pub fn project(&self) -> eyre::Result<ProjectContext> {
        let mut config = Box::new(LayeredConfig::new());
        config.with_user_layer(SettingsConfig::collect_user().map_err(LoadError::Config)?);

        let mut settings = SettingsConfig::default();
        let mut project = ProjectConfig::default();
        let mut test = TestConfig::default();

        self.args
            .cli_config_layer(&mut settings, &mut project, &mut test);

        config.with_cli_layer(Some(settings), Some(project), Some(test));

        let ctx = match &self.args.root {
            Some(root) => Some({
                if !root.try_exists()? {
                    writeln!(self.ui.error()?, "Root '{}' not found", root.display())?;
                    eyre::bail!(OperationFailure);
                }

                match self.args.vcs {
                    commands::Vcs::Auto => ProjectContext::discover_vcs(root, config)?,
                    commands::Vcs::Git => {
                        ProjectContext::load(root, Some(Vcs::new_rootless(VcsKind::Git)), config)?
                    }
                    commands::Vcs::Jujutsu => ProjectContext::load(
                        root,
                        Some(Vcs::new_rootless(VcsKind::Jujutsu)),
                        config,
                    )?,
                    commands::Vcs::Sapling => ProjectContext::load(
                        root,
                        Some(Vcs::new_rootless(VcsKind::Sapling)),
                        config,
                    )?,
                    commands::Vcs::Hg | commands::Vcs::Mercurial => ProjectContext::load(
                        root,
                        Some(Vcs::new_rootless(VcsKind::Mercurial)),
                        config,
                    )?,
                }
            }),
            None => {
                let cwd = env::current_dir().wrap_err("reading PWD")?;
                ProjectContext::discover_project_and_vcs(cwd, config)?
            }
        };

        let Some(ctx) = ctx else {
            writeln!(self.ui.error()?, "Must be in a typst project")?;

            let mut w = self.ui.hint()?;
            write!(w, "You can pass the project root using ")?;
            cwrite!(colored(w, Color::Cyan), "--root <path>")?;
            writeln!(w)?;
            eyre::bail!(OperationFailure);
        };

        Ok(ctx)
    }

    /// Create a new filter from given arguments.
    #[tracing::instrument(skip_all)]
    pub fn filter(&self, filter: &FilterOptions) -> eyre::Result<CombinedFilter> {
        let mut combined = CombinedFilter::default();

        if !filter.tests.is_empty() {
            combined.with_exact(ExactFilter::new(filter.tests.iter().map(Clone::clone)));
        } else {
            let ctx = builtin::context();
            let mut test_set = ExpressionFilter::new(ctx, &filter.expression)?;

            if filter.skip.get_or_default() {
                test_set = test_set.map(|set| eval::Set::expr_diff(set, dsl::set_skip()));
            }

            combined.with_test_set(test_set);
        }

        Ok(combined)
    }

    /// Collect and filter tests for the given project.
    #[tracing::instrument(skip_all)]
    pub fn collect_tests_with_filter(
        &self,
        project_ctx: &ProjectContext,
        filter: CombinedFilter,
        options: &SearchOptions,
    ) -> eyre::Result<Suite> {
        let mut suite = self.collect_tests(project_ctx, options)?;

        if suite.is_empty() {
            writeln!(self.ui.warn()?, "Suite is empty")?;
        }

        suite.apply_filter(project_ctx, filter)?;

        if suite.matched_len() == 0 {
            writeln!(self.ui.warn()?, "Test set matched no tests")?;
        }

        Ok(suite)
    }

    /// Collect all tests for the given project.
    #[tracing::instrument(skip_all)]
    pub fn collect_tests(
        &self,
        project_ctx: &ProjectContext,
        options: &SearchOptions,
    ) -> eyre::Result<Suite> {
        let suite = Suite::collect(project_ctx, options)?;
        Ok(suite)
    }
}

impl Context<'_> {
    /// Run the parsed command and report errors as UI messages.
    #[tracing::instrument(skip_all)]
    pub fn run(&mut self) -> eyre::Result<()> {
        let Err(error) = self.args.cmd.run(self) else {
            return Ok(());
        };

        // TODO: remove this in favor of actual error types

        for error in error.chain() {
            // TODO(tinger): Attach test id.
            if let Some(PersistentReferencesError::MissingReferences { indices: pages }) =
                error.downcast_ref()
            {
                if pages.is_empty() {
                    writeln!(self.ui.error()?, "References had zero pages")?;
                } else {
                    writeln!(
                        self.ui.error()?,
                        "References had missing pages, these pages were found: {pages:?}"
                    )?;
                }

                eyre::bail!(OperationFailure);
            }

            // TODO(tinger): Attach test id.
            if let Some(error) = error.downcast_ref::<ParseIdentError>() {
                match error {
                    ParseIdentError::UnexpectedKind { expected, given } => {
                        writeln!(
                            self.ui.error()?,
                            "expected a {expected} identifier, got a {given} identifier"
                        )?;
                    }
                    ParseIdentError::NotUtf8(path) => {
                        writeln!(
                            self.ui.error()?,
                            "the path {path:?} could not be turned into a valid identifier"
                        )?;
                    }
                    ParseIdentError::Invalid(str) => {
                        writeln!(self.ui.error()?, "{str:?} is not a valid identifier")?;
                        writeln!(
                            self.ui.hint()?,
                            "identifiers must not contain other characters than non-alphanumeric, hyphens and underscores"
                        )?;
                    }
                    ParseIdentError::Empty => {
                        writeln!(self.ui.error()?, "A test identifier must not be empty")?;
                    }
                }

                eyre::bail!(OperationFailure);
            }

            // TODO(tinger): Attach test id.
            if let Some(error) = error.downcast_ref::<test::ParseAnnotationError>() {
                writeln!(self.ui.error()?, "Couldn't parse annotations:\n{error}")?;
                eyre::bail!(OperationFailure);
            }

            if let Some(error) = error.downcast_ref::<LoadError>() {
                match error {
                    LoadError::Manifest(ReadError::Parsing(error)) => {
                        writeln!(self.ui.error()?, "Failed to parse manifest:\n{error}")?;
                        eyre::bail!(OperationFailure);
                    }
                    LoadError::Manifest(ReadError::Validation(error)) => {
                        let mut w = self.ui.error()?;
                        writeln!(w, "Failed to validate manifest:")?;
                        writeln!(w, "{error}")?;
                        eyre::bail!(OperationFailure);
                    }
                    LoadError::Config(ReadError::Parsing(error)) => {
                        writeln!(self.ui.error()?, "Failed to parse config:\n{error}")?;
                        eyre::bail!(OperationFailure);
                    }
                    LoadError::Config(ReadError::Validation(error)) => {
                        writeln!(self.ui.error()?, "Failed to validate config:\n{error}")?;
                        eyre::bail!(OperationFailure);
                    }
                    _ => {}
                }
            }

            if let Some(error) = error.downcast_ref::<tytanic_filter::test_set::Error>() {
                match error {
                    tytanic_filter::test_set::Error::Parse(error) => {
                        writeln!(self.ui.error()?, "Couldn't parse test set:\n{error}")?;
                    }
                    tytanic_filter::test_set::Error::Eval(error) => {
                        writeln!(self.ui.error()?, "Couldn't evaluate test set:\n{error}")?;
                    }
                }

                eyre::bail!(OperationFailure);
            }

            if let Some(error) = error.downcast_ref::<tytanic_filter::Error>() {
                match error {
                    tytanic_filter::Error::TestSet(error) => {
                        writeln!(self.ui.error()?, "Couldn't evaluate test set:\n{error}")?;
                    }
                    tytanic_filter::Error::Exact(missing) => {
                        let mut w = self.ui.error()?;

                        for ident in &missing.missing {
                            write!(w, "Test ")?;
                            write_test_ident(&mut w, ident)?;
                            writeln!(w, " not found")?;
                        }
                    }
                }

                eyre::bail!(OperationFailure);
            }
        }

        eyre::bail!(error);
    }
}
