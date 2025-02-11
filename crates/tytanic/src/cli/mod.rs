use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::{env, io};

use color_eyre::eyre;
use color_eyre::eyre::WrapErr;
use commands::CompileOptions;
use termcolor::Color;
use thiserror::Error;
use tytanic_core::dsl;
use tytanic_core::project::Project;
use tytanic_core::suite::{Filter, FilterError, FilteredSuite, Suite};
use tytanic_core::test::Id;
use tytanic_filter::{eval, Error as ExpressionFilterError, ExpressionFilter};

use self::commands::{CliArguments, FilterOptions, Switch};
use crate::ui::{self, Ui};
use crate::world::SystemWorld;
use crate::{cwrite, kit};

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

    /// The terminal ui.
    pub ui: &'a Ui,
}

impl<'a> Context<'a> {
    pub fn new(args: &'a CliArguments, ui: &'a Ui) -> Self {
        Self { args, ui }
    }
}

impl Context<'_> {
    pub fn error_root_not_found(&self, root: &Path) -> io::Result<()> {
        writeln!(self.ui.error()?, "Root '{}' not found", root.display())
    }

    pub fn error_no_project(&self) -> io::Result<()> {
        writeln!(self.ui.error()?, "Must be in a typst project")?;

        let mut w = self.ui.hint()?;
        write!(w, "You can pass the project root using ")?;
        cwrite!(colored(w, Color::Cyan), "--root <path>")?;
        writeln!(w)
    }

    pub fn error_test_set(&self, error: ExpressionFilterError) -> io::Result<()> {
        writeln!(
            self.ui.error()?,
            "Couldn't parse or evaluate test set expression:\n{error:?}",
        )
    }

    pub fn error_test_already_exists(&self, id: &Id) -> io::Result<()> {
        let mut w = self.ui.error()?;

        write!(w, "Test ")?;
        ui::write_test_id(&mut w, id)?;
        writeln!(w, " already exists")
    }

    pub fn error_missing_tests(&self, missing: &BTreeSet<Id>) -> io::Result<()> {
        let mut w = self.ui.error()?;

        for id in missing {
            write!(w, "Test ")?;
            ui::write_test_id(&mut w, id)?;
            writeln!(w, " not found")?;
        }

        Ok(())
    }

    pub fn warn_no_tests(&self) -> io::Result<()> {
        writeln!(self.ui.warn()?, "Matched no tests")
    }

    pub fn error_too_many_tests(&self, expr: &str) -> io::Result<()> {
        writeln!(self.ui.error()?, "Matched more than one test")?;

        let mut w = self.ui.hint()?;
        write!(w, "use '")?;
        cwrite!(colored(w, Color::Cyan), "all:")?;
        writeln!(w, "{expr}' to confirm using all tests")
    }

    pub fn warn_nested_tests(&self) -> io::Result<()> {
        writeln!(self.ui.warn()?, "Found nested tests")?;

        writeln!(
            self.ui.hint()?,
            "This is no longer supported, these tests will be ignored"
        )?;
        writeln!(
            self.ui.hint()?,
            "This will become a hard error in a future version"
        )?;

        let mut w = self.ui.hint()?;
        write!(w, "You can run ")?;
        cwrite!(colored(w, Color::Cyan), "tt util migrate")?;
        writeln!(w, " to automatically move the tests")
    }
}

// TODO(tinger): cache these values
impl Context<'_> {
    /// Resolve the current root.
    pub fn root(&self) -> eyre::Result<PathBuf> {
        Ok(match &self.args.root {
            Some(root) => {
                if !root.try_exists()? {
                    self.error_root_not_found(root)?;
                    eyre::bail!(OperationFailure);
                }

                root.canonicalize()?
            }
            None => env::current_dir().wrap_err("reading PWD")?,
        })
    }

    /// Discover the current and ensure it is initialized.
    pub fn project(&self) -> eyre::Result<Project> {
        let root = self.root()?;

        let Some(project) = Project::discover(root, self.args.root.is_some())? else {
            self.error_no_project()?;
            eyre::bail!(OperationFailure);
        };

        Ok(project)
    }

    /// Create a new filter from given arguments.
    pub fn filter(&self, filter: &FilterOptions) -> eyre::Result<Filter> {
        if !filter.tests.is_empty() {
            Ok(Filter::Explicit(filter.tests.iter().cloned().collect()))
        } else {
            let ctx = dsl::context();
            let mut set = match ExpressionFilter::new(ctx, &filter.expression) {
                Ok(set) => set,
                Err(err) => {
                    self.error_test_set(err)?;
                    eyre::bail!(OperationFailure);
                }
            };

            if filter.skip.get_or_default() {
                set = set.map(|set| eval::Set::expr_diff(set, dsl::built_in::skip()));
            }

            Ok(Filter::TestSet(set))
        }
    }

    /// Collect and filter tests for the given project.
    pub fn collect_tests_with_filter(
        &self,
        project: &Project,
        filter: Filter,
    ) -> eyre::Result<FilteredSuite> {
        let suite = self.collect_tests(project)?;

        match suite.filter(filter) {
            Ok(suite) => Ok(suite),
            Err(err) => match err {
                FilterError::TestSet(err) => eyre::bail!(err),
                FilterError::Missing(missing) => {
                    self.error_missing_tests(&missing)?;
                    eyre::bail!(OperationFailure);
                }
            },
        }
    }

    /// Collect all tests for the given project.
    pub fn collect_tests(&self, project: &Project) -> eyre::Result<Suite> {
        let suite = Suite::collect(project.paths())?;

        if !suite.nested().is_empty() {
            self.warn_nested_tests()?;
        }

        Ok(suite)
    }

    /// Create a SystemWorld from the given args.
    pub fn world(&self, compile_options: &CompileOptions) -> eyre::Result<SystemWorld> {
        kit::world(
            self.root()?,
            &self.args.font,
            &self.args.package,
            compile_options,
        )
    }
}

impl Context<'_> {
    /// Run the parsed command and report errors as ui messages.
    pub fn run(&mut self) -> eyre::Result<()> {
        // TODO(tinger): catch internal errors here and transform them into
        // error messages
        self.args.cmd.run(self)
    }
}
