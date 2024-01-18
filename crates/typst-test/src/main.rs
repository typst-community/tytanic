use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{fs, io};

use clap::{ColorChoice, Parser};
use rayon::prelude::*;
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use tracing_tree::HierarchicalLayer;

use self::cli::CliResult;
use self::project::test::context::Context;
use self::project::test::Test;
use self::project::{Project, ScaffoldMode};
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
                if let Some(message) = message {
                    eprintln!("{message}");
                }
                cli::EXIT_OPERATION_FAILURE
            }
        },
        Err(err) => {
            eprintln!(
                "typst-test ran into an unexpected error, this is most likely a bug\n\
            please consider reporting this at https://github.com/tingerrr/typst-test/issues\n\
            Error: {err}"
            );

            cli::EXIT_ERROR
        }
    })
}

fn main_impl() -> anyhow::Result<CliResult> {
    let args = match cli::Args::try_parse() {
        Ok(args) => args,
        Err(err) => {
            return Ok(CliResult::operation_failure(err.render()));
        }
    };

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
                "must be inside a typst project or pass the project root using --root",
            ));
        }
    };

    let reporter = Reporter::new(util::term::color_stream(args.color, false));

    let manifest = project::try_open_manifest(&root)?;
    let mut project = Project::new(root, Path::new("tests"), manifest, reporter.clone());

    let filter_tests = |tests: &mut HashSet<Test>, filter, exact| match (filter, exact) {
        (Some(f), true) => {
            tests.retain(|t| t.name() == f);
        }
        (Some(f), false) => {
            tests.retain(|t| t.name().contains(&f));
        }
        (None, true) => {
            tracing::warn!("no filter given, --exact is meaning less");
        }
        (None, false) => {}
    };

    let (test_args, compare) = match args.cmd {
        cli::Command::Init { no_example } => return init(&project, reporter, no_example),
        cli::Command::Uninit => return uninit(&project, reporter),
        cli::Command::Clean => return clean(&project, reporter),
        cli::Command::Add { open, test } => return add(&mut project, reporter, test, open),
        cli::Command::Edit { test } => return edit(&project, reporter, &test),
        cli::Command::Remove { test } => return remove(&project, reporter, &test),
        cli::Command::Status => return status(&mut project, reporter),
        cli::Command::Update { test_filter, exact } => {
            return update(
                &mut project,
                reporter,
                args.typst,
                filter_tests,
                test_filter,
                exact,
            )
        }
        cli::Command::Compile(args) => (args, false),
        cli::Command::Run(args) => (args, true),
    };

    project.load_tests()?;
    filter_tests(project.tests_mut(), test_args.test_filter, test_args.exact);
    reporter.set_padding(project.tests().iter().map(|t| t.name().len()).max());

    run(&project, reporter, args.typst, test_args.fail_fast, compare)
}

macro_rules! bail_gracefully {
    (if_uninit; $project:expr, $reporter:expr) => {
        if !$project.is_init()? {
            return Ok(CliResult::operation_failure(format!(
                "Project '{}' was not initialized",
                $project.name(),
            )));
        }
    };
}

fn init(project: &Project, reporter: Reporter, no_example: bool) -> anyhow::Result<CliResult> {
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
    reporter.raw(|w| writeln!(w, "initialized tests for {}", project.name()))?;

    Ok(CliResult::Ok)
}

fn uninit(project: &Project, reporter: Reporter) -> anyhow::Result<CliResult> {
    bail_gracefully!(if_uninit; project, reporter);

    project.uninit()?;
    reporter.raw(|w| writeln!(w, "removed tests for {}", project.name()))?;

    Ok(CliResult::Ok)
}

fn clean(project: &Project, reporter: Reporter) -> anyhow::Result<CliResult> {
    bail_gracefully!(if_uninit; project, reporter);

    project.clean_artifacts()?;
    reporter.raw(|w| writeln!(w, "removed test artifacts for {}", project.name()))?;

    Ok(CliResult::Ok)
}

fn add(
    project: &mut Project,
    reporter: Reporter,
    test: String,
    open: bool,
) -> anyhow::Result<CliResult> {
    bail_gracefully!(if_uninit; project, reporter);

    project.load_template()?;

    let test = Test::new(test);
    if project.get_test(test.name())?.is_some() {
        return Ok(CliResult::operation_failure(format!(
            "test '{}' already exsits",
            test.name()
        )));
    }

    project.add_test(&test)?;
    reporter.test_success(test.name(), "added")?;

    if open {
        // BUG: this may fail silently if the path doesn't exist
        open::that_detached(project.test_file(&test))?;
    }

    Ok(CliResult::Ok)
}

fn remove(project: &Project, reporter: Reporter, test: &str) -> anyhow::Result<CliResult> {
    bail_gracefully!(if_uninit; project, reporter);

    let test = project.find_test(test)?;
    project.remove_test(test.name())?;
    reporter.test_success(test.name(), "removed")?;

    Ok(CliResult::Ok)
}

fn edit(project: &Project, _reporter: Reporter, test: &str) -> anyhow::Result<CliResult> {
    bail_gracefully!(if_uninit; project, reporter);

    let test = project.find_test(test)?;
    open::that_detached(project.test_file(&test))?;

    Ok(CliResult::Ok)
}

fn update(
    project: &mut Project,
    reporter: Reporter,
    typst: PathBuf,
    filter_tests: impl Fn(&mut HashSet<Test>, Option<String>, bool),
    test_filter: Option<String>,
    exact: bool,
) -> anyhow::Result<CliResult> {
    bail_gracefully!(if_uninit; project, reporter);

    if project.tests().is_empty() {
        reporter.raw(|w| writeln!(w, "Project '{}' did not contain any tests", project.name()))?;
        return Ok(CliResult::Ok);
    }

    project.load_tests()?;
    filter_tests(project.tests_mut(), test_filter, exact);
    reporter.set_padding(project.tests().iter().map(|t| t.name().len()).max());

    let all_ok = run_tests(project, reporter, typst, true, false)?;
    if !all_ok {
        return Ok(CliResult::operation_failure(
            "At least one test failed, aborting update...",
        ));
    }

    let tests = project.tests();
    project.update_tests(tests.par_iter())?;

    Ok(CliResult::Ok)
}

fn status(project: &mut Project, reporter: Reporter) -> anyhow::Result<CliResult> {
    bail_gracefully!(if_uninit; project, reporter);

    project.load_tests()?;
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
        for test in project.tests() {
            reporter.raw(|w| writeln!(w, "  {}", test.name()))?;
        }
    }

    Ok(CliResult::Ok)
}

fn run(
    project: &Project,
    reporter: Reporter,
    typst: PathBuf,
    fail_fast: bool,
    compare: bool,
) -> anyhow::Result<CliResult> {
    bail_gracefully!(if_uninit; project, reporter);

    if project.tests().is_empty() {
        reporter.raw(|w| writeln!(w, "Project '{}' did not contain any tests", project.name()))?;
        return Ok(CliResult::Ok);
    }

    let all_ok = run_tests(project, reporter, typst, fail_fast, compare)?;
    Ok(if all_ok {
        CliResult::Ok
    } else {
        CliResult::TestFailure
    })
}

fn run_tests(
    project: &Project,
    reporter: Reporter,
    typst: PathBuf,
    fail_fast: bool,
    compare: bool,
) -> anyhow::Result<bool> {
    let all_ok = AtomicBool::new(true);

    // TODO: fail_fast currently doesn't really do anything other than returning early, other tests
    //       still run, this makes sense as we're not stopping the other threads just yet
    let ctx = Context::new(project, typst, fail_fast);
    ctx.prepare()?;
    project
        .tests()
        .par_iter()
        .try_for_each(|test| -> anyhow::Result<()> {
            match ctx.test(test).run(compare)? {
                Ok(_) => reporter.test_success(test.name(), "ok")?,
                Err(err) => {
                    all_ok.store(false, Ordering::Relaxed);
                    reporter.test_failure(test.name(), err)?
                }
            }

            Ok(())
        })?;
    ctx.cleanup()?;

    Ok(all_ok.into_inner())
}
