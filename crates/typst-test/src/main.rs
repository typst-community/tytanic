use std::io::IsTerminal;
use std::path::Path;
use std::process::ExitCode;
use std::{fs, io};

use clap::{ColorChoice, Parser};
use project::test::Filter;
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use tracing_tree::HierarchicalLayer;

use self::cli::CliResult;
use self::project::Project;
use self::report::Reporter;

mod cli;
mod project;
mod report;
mod util;

fn main() -> ExitCode {
    ExitCode::from(match main_impl() {
        Ok(cli_res) => match cli_res {
            CliResult::Ok => cli::EXIT_OK,
            CliResult::TestFailure => cli::EXIT_TEST_FAILURE,
            CliResult::OperationFailure { message } => {
                eprintln!("{message}");
                cli::EXIT_OPERATION_FAILURE
            }
        },
        Err(err) => {
            eprintln!(
                "typst-test ran into an unexpected error, this is most likely a bug\n\
                please consider reporting this at {}/issues\n\
                Error: {err}",
                std::env!("CARGO_PKG_REPOSITORY")
            );

            cli::EXIT_ERROR
        }
    })
}

fn main_impl() -> anyhow::Result<CliResult> {
    let args = cli::Args::parse();

    if args.verbose >= 1 {
        tracing_subscriber::registry()
            .with(
                HierarchicalLayer::new(4)
                    .with_targets(true)
                    .with_ansi(match args.color {
                        ColorChoice::Auto => io::stderr().is_terminal(),
                        ColorChoice::Always => true,
                        ColorChoice::Never => false,
                    }),
            )
            .with(Targets::new().with_target(
                std::env!("CARGO_CRATE_NAME"),
                match args.verbose {
                    1 => Level::ERROR,
                    2 => Level::WARN,
                    3 => Level::INFO,
                    4 => Level::DEBUG,
                    _ => Level::TRACE,
                },
            ))
            .init();
    }

    let root = if let Some(root) = args.root.clone() {
        let canonical_root = fs::canonicalize(&root)?;
        if !project::is_project_root(&canonical_root)? {
            tracing::warn!("project root doesn't contain manifest");
        }
        root.to_path_buf()
    } else {
        let pwd = std::env::current_dir()?;
        if let Some(root) = project::try_find_project_root(&pwd)? {
            root.to_path_buf()
        } else {
            return Ok(CliResult::operation_failure(
                "Must be inside a typst project or pass the project root using --root",
            ));
        }
    };

    let mut reporter = Reporter::new(util::term::color_stream(args.color, false));
    let manifest = project::try_open_manifest(&root)?;
    let mut project = Project::new(root, Path::new("tests"), manifest);

    let (test_args, compare) = match args.cmd {
        cli::Command::Init { no_example } => return cmd::init(&project, &mut reporter, no_example),
        cli::Command::Uninit => return cmd::uninit(&mut project, &mut reporter),
        cli::Command::Clean => return cmd::clean(&mut project, &mut reporter),
        cli::Command::Add { open, test } => {
            return cmd::add(&mut project, &mut reporter, test, open)
        }
        cli::Command::Edit { test } => return cmd::edit(&mut project, &mut reporter, test),
        cli::Command::Remove { test } => return cmd::remove(&mut project, &mut reporter, test),
        cli::Command::Status => return cmd::status(&mut project, &mut reporter),
        cli::Command::Update {
            test_filter,
            exact,
            no_optimize,
        } => {
            return cmd::update(
                &mut project,
                &mut reporter,
                test_filter.map(|f| Filter::new(f, exact)),
                args.typst,
                args.fail_fast,
                !no_optimize,
            )
        }
        cli::Command::Compile(args) => (args, false),
        cli::Command::Run(args) => (args, true),
    };

    cmd::run(
        &mut project,
        &mut reporter,
        test_args
            .test_filter
            .map(|f| Filter::new(f, test_args.exact)),
        args.typst,
        args.fail_fast,
        compare,
    )
}

mod cmd {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    use rayon::prelude::*;

    use crate::cli::CliResult;
    use crate::project::test::context::Context;
    use crate::project::test::{Filter, Test};
    use crate::project::{Project, ScaffoldMode};
    use crate::report::Reporter;

    macro_rules! bail_gracefully {
        (if_uninit; $project:expr, $reporter:expr) => {
            if !$project.is_init()? {
                return Ok(CliResult::operation_failure(format!(
                    "Project '{}' was not initialized",
                    $project.name(),
                )));
            }
        };
        (if_test_not_found; $test:expr => $name:ident; $project:expr, $reporter:expr) => {
            let Some($name) = $project.get_test(&$test) else {
                return Ok(CliResult::operation_failure(format!(
                    "Test '{}' could not be found",
                    $test,
                )));
            };
        };
        (if_no_tests; $project:expr, $reporter:expr) => {
            if $project.tests().is_empty() {
                return Ok(CliResult::operation_failure(format!(
                    "Project '{}' did not contain any tests",
                    $project.name(),
                )));
            }
        };
        (if_no_match; $filter:expr; $project:expr, $reporter:expr) => {
            if let Some(filter) = &$filter {
                match filter {
                    Filter::Exact(f) => {
                        $project.tests_mut().retain(|n, _| n == f);
                    }
                    Filter::Contains(f) => {
                        $project.tests_mut().retain(|n, _| n.contains(f));
                    }
                }

                if $project.tests().is_empty() {
                    return Ok(CliResult::operation_failure(format!(
                        "Filter '{}' did not match any tests",
                        filter.value(),
                    )));
                }
            }
        };
    }

    pub fn init(
        project: &Project,
        reporter: &mut Reporter,
        no_example: bool,
    ) -> anyhow::Result<CliResult> {
        if project.is_init()? {
            return Ok(CliResult::operation_failure(format!(
                "Project '{}' was already initialized",
                project.name(),
            )));
        }

        let mode = if no_example {
            ScaffoldMode::NoExample
        } else {
            ScaffoldMode::WithExample
        };

        project.init(mode)?;
        reporter.raw(|w| writeln!(w, "Initialized project '{}'", project.name()))?;

        Ok(CliResult::Ok)
    }

    pub fn uninit(project: &mut Project, reporter: &mut Reporter) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project, reporter);

        project.discover_tests()?;
        let count = project.tests().len();

        project.uninit()?;
        reporter.raw(|w| {
            writeln!(
                w,
                "Removed {} test{}",
                count,
                if count == 1 { "" } else { "s" }
            )
        })?;

        Ok(CliResult::Ok)
    }

    pub fn clean(project: &mut Project, reporter: &mut Reporter) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project, reporter);

        project.discover_tests()?;

        project.clean_artifacts()?;
        reporter.raw(|w| writeln!(w, "Removed test artifacts"))?;

        Ok(CliResult::Ok)
    }

    pub fn add(
        project: &mut Project,
        reporter: &mut Reporter,
        test: String,
        open: bool,
    ) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project, reporter);

        project.discover_tests()?;
        project.load_template()?;

        let test = Test::new(test);
        if project.get_test(test.name()).is_some() {
            return Ok(CliResult::operation_failure(format!(
                "Test '{}' already exists",
                test.name()
            )));
        };

        reporter.set_padding(Some(test.name().len()));

        let no_ref = !project.create_test(&test)?;
        reporter.test_added(project, &test, no_ref)?;

        if open {
            // BUG: this may fail silently if the path doesn't exist
            open::that_detached(test.test_file(project))?;
        }

        Ok(CliResult::Ok)
    }

    pub fn remove(
        project: &mut Project,
        reporter: &mut Reporter,
        test: String,
    ) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project, reporter);

        project.discover_tests()?;
        bail_gracefully!(if_test_not_found; test => test; project, reporter);

        project.remove_test(test.name())?;
        reporter.test_success(project, test, "removed")?;

        Ok(CliResult::Ok)
    }

    pub fn edit(
        project: &mut Project,
        _reporter: &mut Reporter,
        test: String,
    ) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project, reporter);

        project.discover_tests()?;
        bail_gracefully!(if_test_not_found; test => test; project, reporter);

        open::that_detached(test.test_file(project))?;

        Ok(CliResult::Ok)
    }

    pub fn update(
        project: &mut Project,
        reporter: &mut Reporter,
        test_filter: Option<Filter>,
        typst: PathBuf,
        fail_fast: bool,
        optimize: bool,
    ) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project, reporter);

        project.discover_tests()?;
        run_tests(
            project,
            reporter,
            test_filter,
            |project| {
                let mut ctx = Context::new(project, typst);
                ctx.with_fail_fast(fail_fast)
                    .with_update(true)
                    .with_optimize(optimize);
                ctx
            },
            "updated",
        )
    }

    pub fn status(project: &mut Project, reporter: &mut Reporter) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project, reporter);

        project.discover_tests()?;
        project.load_template()?;

        if let Some(manifest) = project.manifest() {
            reporter.raw(|w| {
                writeln!(
                    w,
                    "Package: {}:{}",
                    manifest.package.name, manifest.package.version
                )
            })?;

            // TODO: list [tool.typst-test] settings
        }

        reporter.raw(|w| {
            writeln!(
                w,
                "Template: {}",
                if project.template().is_some() {
                    "found"
                } else {
                    "not found"
                }
            )
        })?;

        if project.tests().is_empty() {
            reporter.raw(|w| writeln!(w, "Tests: none"))?;
        } else {
            reporter.raw(|w| writeln!(w, "Tests:"))?;
            for name in project.tests().keys() {
                reporter.raw(|w| writeln!(w, "  {}", name))?;
            }
        }

        Ok(CliResult::Ok)
    }

    pub fn run(
        project: &mut Project,
        reporter: &mut Reporter,
        test_filter: Option<Filter>,
        typst: PathBuf,
        fail_fast: bool,
        compare: bool,
    ) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_uninit; project, reporter);

        project.discover_tests()?;
        run_tests(
            project,
            reporter,
            test_filter,
            |project| {
                let mut ctx = Context::new(project, typst);
                ctx.with_fail_fast(fail_fast).with_compare(compare);
                ctx
            },
            "ok",
        )
    }

    fn run_tests(
        project: &mut Project,
        reporter: &mut Reporter,
        test_filter: Option<Filter>,
        prepare_ctx: impl FnOnce(&Project) -> Context,
        done_annot: &str,
    ) -> anyhow::Result<CliResult> {
        bail_gracefully!(if_no_tests; project, reporter);
        bail_gracefully!(if_no_match; test_filter; project, reporter);

        reporter.set_padding(project.tests().iter().map(|(name, _)| name.len()).max());

        let ctx = prepare_ctx(project);
        ctx.prepare()?;

        let reporter = Mutex::new(reporter);
        let all_ok = AtomicBool::new(true);
        let res = project.tests().par_iter().try_for_each(
            |(_, test)| -> Result<(), Option<anyhow::Error>> {
                match ctx.test(test).run() {
                    Ok(Ok(_)) => {
                        reporter
                            .lock()
                            .unwrap()
                            .test_success(project, test, done_annot)
                            .map_err(|e| Some(e.into()))?;
                        Ok(())
                    }
                    Ok(Err(err)) => {
                        all_ok.store(false, Ordering::Relaxed);
                        reporter
                            .lock()
                            .unwrap()
                            .test_failure(project, test, err)
                            .map_err(|e| Some(e.into()))?;
                        if ctx.fail_fast() {
                            Err(None)
                        } else {
                            Ok(())
                        }
                    }
                    Err(err) => Err(Some(err.into())),
                }
            },
        );

        if let Err(Some(err)) = res {
            return Err(err);
        }

        ctx.cleanup()?;

        Ok(if all_ok.into_inner() {
            CliResult::Ok
        } else {
            CliResult::TestFailure
        })
    }
}
