use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::{fs, io};

use clap::{ColorChoice, Parser};
use project::test::Test;
use project::ScaffoldMode;
use rayon::prelude::*;
use report::Reporter;
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use tracing_tree::HierarchicalLayer;

use self::project::test::context::Context;
use self::project::Project;

mod cli;
mod project;
mod report;
mod util;

fn run(
    reporter: Reporter,
    project: &Project,
    typst: PathBuf,
    fail_fast: bool,
    compare: bool,
) -> anyhow::Result<()> {
    if project.tests().is_empty() {
        reporter.raw(|w| writeln!(w, "No tests detected for {}", project.name()))?;
        return Ok(());
    }

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
                Err(err) => reporter.test_failure(test.name(), err)?,
            }

            Ok(())
        })?;

    ctx.cleanup()?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
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

    let root = if let Some(root) = args.root {
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
            anyhow::bail!("must be inside a typst project or pass the project root using --root");
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
        cli::Command::Init { no_example } => {
            let mode = if no_example {
                ScaffoldMode::NoExample
            } else {
                ScaffoldMode::WithExample
            };

            if project.init(mode)? {
                println!("initialized tests for {}", project.name());
            } else {
                println!(
                    "could not initialize tests for {}, {:?} already exists",
                    project.name(),
                    project.tests_root_dir()
                );
            }
            return Ok(());
        }
        cli::Command::Uninit => {
            project.uninit()?;
            println!("removed tests for {}", project.name());
            return Ok(());
        }
        cli::Command::Clean => {
            project.clean_artifacts()?;
            println!("removed test artifacts for {}", project.name());
            return Ok(());
        }
        cli::Command::Add { open, test } => {
            project.load_template()?;
            let test = Test::new(test);
            project.add_test(&test)?;
            reporter.test_success(test.name(), "added")?;

            if open {
                // BUG: this may fail silently if the path doesn't exist
                open::that_detached(project.test_file(&test))?;
            }

            return Ok(());
        }
        cli::Command::Edit { test } => {
            let test = project.find_test(&test)?;
            open::that_detached(project.test_file(&test))?;
            return Ok(());
        }
        cli::Command::Remove { test } => {
            let test = project.find_test(&test)?;
            project.remove_test(test.name())?;
            reporter.test_success(test.name(), "removed")?;
            return Ok(());
        }
        cli::Command::Status => {
            project.load_tests()?;
            project.load_template()?;

            if let Some(manifest) = project.manifest() {
                println!(
                    "Package: {}:{}",
                    manifest.package.name, manifest.package.version
                );

                // TODO: list [tool.typst-test] settings
            }

            println!(
                "Template: {}",
                if project.template().is_some() {
                    "found"
                } else {
                    "not found"
                }
            );

            if project.tests().is_empty() {
                println!("Tests: none");
            } else {
                println!("Tests:");
                for test in project.tests() {
                    println!("  {}", test.name());
                }
            }

            return Ok(());
        }
        cli::Command::Update { test_filter, exact } => {
            project.load_tests()?;
            filter_tests(project.tests_mut(), test_filter, exact);
            reporter.set_padding(project.tests().iter().map(|t| t.name().len()).max());

            run(reporter.clone(), &project, args.typst, true, false)?;

            let tests = project.tests();
            project.update_tests(tests.par_iter())?;
            return Ok(());
        }
        cli::Command::Compile(args) => (args, false),
        cli::Command::Run(args) => (args, true),
    };

    project.load_tests()?;
    filter_tests(project.tests_mut(), test_args.test_filter, test_args.exact);
    reporter.set_padding(project.tests().iter().map(|t| t.name().len()).max());

    run(
        reporter.clone(),
        &project,
        args.typst,
        test_args.fail_fast,
        compare,
    )
}
