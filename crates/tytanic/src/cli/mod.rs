use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::{env, io};

use color_eyre::eyre;
use color_eyre::eyre::WrapErr;
use options::{CliArguments, FilterOptions, Switch};
use termcolor::Color;
use thiserror::Error;
use tytanic_core::config::{Config, ConfigLayer};
use tytanic_core::project::Project;
use tytanic_core::test::{Id, Suite};
use tytanic_core::test_set::{self, eval, Error as TestSetError, TestSet};

use crate::ui::{self, Ui};
use crate::world::SystemWorld;
use crate::{cwrite, kit};

pub mod add;
pub mod list;
pub mod options;
pub mod remove;
pub mod run;
pub mod status;
pub mod update;
pub mod util;

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
    pub fn error_aborted(&self) -> io::Result<()> {
        writeln!(self.ui.error()?, "Operation aborted")
    }

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

    pub fn error_test_set_failure(&self, error: TestSetError) -> io::Result<()> {
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

    pub fn error_no_tests(&self) -> io::Result<()> {
        writeln!(self.ui.error()?, "Matched no tests")
    }

    pub fn error_too_many_tests(&self, expr: &str) -> io::Result<()> {
        writeln!(self.ui.error()?, "Matched more than one test")?;

        let mut w = self.ui.hint()?;
        write!(w, "use '")?;
        cwrite!(colored(w, Color::Cyan), "all:")?;
        writeln!(w, "{expr}' to confirm using all tests")
    }

    pub fn error_nested_tests(&self) -> io::Result<()> {
        writeln!(self.ui.error()?, "Found nested tests")?;

        let mut w = self.ui.hint()?;
        writeln!(w, "This is no longer supported")?;
        write!(w, "You can run ")?;
        cwrite!(colored(w, Color::Cyan), "tt util migrate")?;
        writeln!(w, " to automatically fix the tests")
    }

    pub fn run(&mut self) -> eyre::Result<()> {
        self.args.cmd.run(self)
    }
}

// TODO(tinger): cache these values
impl Context<'_> {
    /// Resolve the current root.
    pub fn root(&self) -> eyre::Result<PathBuf> {
        Ok(match &self.args.typst.root {
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

    /// Resolve the user and override config layers.
    pub fn config(&self) -> eyre::Result<Config> {
        // TODO(tinger): cli/envar overrides go here

        let mut config = Config::new(None);
        config.user = ConfigLayer::collect_user()?;

        Ok(config)
    }

    /// Discover the current and ensure it is initialized.
    pub fn project(&self) -> eyre::Result<Project> {
        let root = self.root()?;

        let Some(project) = Project::discover(root, self.args.typst.root.is_some())? else {
            self.error_no_project()?;
            eyre::bail!(OperationFailure);
        };

        Ok(project)
    }

    /// Create a new test set from the arguments with the given context.
    pub fn test_set(&self, filter: &FilterOptions) -> eyre::Result<TestSet> {
        if !filter.tests.is_empty() {
            let mut tests = filter
                .tests
                .iter()
                .map(|test| eval::Set::built_in_pattern(test_set::Pat::Exact(test.into())));

            let a = tests.next().expect("`tests` is not empty");

            let set = match tests.next() {
                Some(b) => eval::Set::built_in_union(a, b, tests),
                None => a,
            };

            Ok(TestSet::new(eval::Context::empty(), set))
        } else {
            let ctx = eval::Context::with_built_ins();
            let mut set = match TestSet::parse_and_evaluate(ctx, &filter.expression) {
                Ok(set) => set,
                Err(err) => {
                    self.error_test_set_failure(err)?;
                    eyre::bail!(OperationFailure);
                }
            };

            if filter.skip.get_or_default() {
                set.add_implicit_skip();
            }

            Ok(set)
        }
    }

    /// Collect and filter tests for the given project.
    pub fn collect_tests(&self, project: &Project, set: &TestSet) -> eyre::Result<Suite> {
        if !util::migrate::collect_old_structure(project.paths(), "self")?.is_empty() {
            self.error_nested_tests()?;
            eyre::bail!(OperationFailure);
        }

        let suite = Suite::collect(project.paths(), set)?;

        Ok(suite)
    }

    /// Collect all tests for the given project.
    pub fn collect_all_tests(&self, project: &Project) -> eyre::Result<Suite> {
        let suite = Suite::collect(
            project.paths(),
            &TestSet::new(eval::Context::empty(), eval::Set::built_in_all()),
        )?;
        Ok(suite)
    }

    /// Create a SystemWorld from the given args.
    pub fn world(&self) -> eyre::Result<SystemWorld> {
        kit::world(self.root()?, &self.args.typst)
    }
}
