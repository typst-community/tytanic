#![allow(missing_docs)]

use std::path::Path;
use std::sync::LazyLock;

use chrono::DateTime;
use chrono::Utc;
use codespan_reporting::term::Config;
use termcolor::ColorChoice;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_tree::HierarchicalLayer;
use tytanic_core::config::LayeredConfig;
use tytanic_core::config::ProjectConfig;
use tytanic_core::config::SettingsConfig;
use tytanic_core::config::TestConfig;
use tytanic_core::config::Warnings;
use tytanic_core::diag;
use tytanic_core::discover::unit::SearchOptions;
use tytanic_core::discover::unit::search;
use tytanic_core::project::ProjectContext;
use tytanic_core::result::ComparisonResult;
use tytanic_core::result::CompilationResult;
use tytanic_core::result::SuiteTrace;
use tytanic_core::result::TestTrace;
use tytanic_core::suite::Suite;
use tytanic_core::test::Test;
use tytanic_core::world::default::WorldProvider;
use tytanic_filter::ExpressionFilter;
use tytanic_filter::builtin;
use tytanic_runner::Runner;
use tytanic_runner::default::DefaultRunner;
use tytanic_runner::default::RunnerConfig;
use tytanic_runner::default::export::CachingExporter;
use tytanic_runner::default::render::CachingRenderer;
use tytanic_runner::default::report;
use tytanic_runner::default::report::Reporter;
use tytanic_utils::ui::Indented;
use uuid::Uuid;

static DEFAULT_OPTIMIZE_OPTIONS: LazyLock<oxipng::Options> =
    LazyLock::new(oxipng::Options::max_compression);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let filter = Targets::new().with_targets([
    //     ("tytanic_core", LevelFilter::TRACE),
    //     ("tytanic_discover", LevelFilter::WARN),
    //     ("tytanic_filter", LevelFilter::WARN),
    //     ("tytanic_runner", LevelFilter::TRACE),
    //     ("tytanic_test_cli", LevelFilter::TRACE),
    //     ("tytanic_typst_library", LevelFilter::WARN),
    // ]);

    // tracing_subscriber::registry()
    //     .with(
    //         HierarchicalLayer::new(4)
    //             .with_targets(true)
    //             .with_ansi(true)
    //             .with_deferred_spans(true),
    //     )
    //     .with(filter)
    //     .init();

    let root = Path::new("/home/tinger/Source/github.com/tingerrr/hydra");

    let mut config = LayeredConfig::new();
    config.with_cli_layer(
        Some(SettingsConfig {
            fail_fast: Some(false),
            ..Default::default()
        }),
        Some(ProjectConfig {
            unit_tests_root: None,
            artifact_store_root: None,
            font_paths: None,
            optimize_refs: None,
            defaults: Some(TestConfig {
                warnings: Some(Warnings::Promote),
                ..Default::default()
            }),
        }),
        None,
    );

    let ctx = ProjectContext::discover_project_and_vcs(root, Box::new(config))?;

    let Some(ctx) = ctx else {
        return Ok(());
    };

    let opt = SearchOptions::default();
    let (tests, _errors) = search(ctx.store(), &opt);

    let testset = ExpressionFilter::new(builtin::context(), "all()")?;
    let suite = Suite::from_tests_filter(tests.into_iter().map(Test::Unit), &ctx, &testset)?;

    let export_temporary_artifacts = ctx
        .config()
        .get_settings_member(SettingsConfig::EXPORT_EPHEMERAL, ());

    let optimize_reference_document = ctx
        .config()
        .get_project_config_member(ProjectConfig::OPTIMIZE_REFS, ());

    let provider = WorldProvider::new();
    let reporter = StderrReporter(&provider);
    // let reporter = ();
    let renderer = CachingRenderer::new();
    let exporter = CachingExporter::new(
        export_temporary_artifacts,
        if optimize_reference_document {
            Some(Box::new(DEFAULT_OPTIMIZE_OPTIONS.clone()))
        } else {
            None
        },
    );

    let runner = DefaultRunner::new(
        &provider,
        &reporter,
        &renderer,
        &exporter,
        RunnerConfig { fail_fast: false },
    );

    let run_id = Uuid::new_v4();

    // runner.run_suite(&ctx, &suite, run_id, true).unwrap();
    runner.run_suite(&ctx, &suite, run_id, false).unwrap();

    Ok(())
}

#[derive(Debug)]
struct StderrReporter<'a>(&'a WorldProvider);

impl StderrReporter<'_> {
    fn report_diagnostics(&self, ctx: &ProjectContext, test: &Test, result: &CompilationResult) {
        let mut out = Indented::new(termcolor::StandardStream::stderr(ColorChoice::Auto), 16);
        let files = self.0.file_provider(ctx, test).unwrap();

        match result {
            CompilationResult::Passed(output) => {
                diag::write_diagnostics(&mut out, &Config::default(), &files, output.warnings())
                    .unwrap()
            }
            CompilationResult::Failed(failure) => {
                diag::write_diagnostics(&mut out, &Config::default(), &files, failure.warnings())
                    .unwrap();
                diag::write_diagnostics(&mut out, &Config::default(), &files, failure.errors())
                    .unwrap();
            }
        }
    }

    fn report_comparison(&self, _ctx: &ProjectContext, _test: &Test, result: &ComparisonResult) {
        let mut out = Indented::new(termcolor::StandardStream::stderr(ColorChoice::Auto), 16);

        if let ComparisonResult::Failed(failure) = result {
            diag::write_comparison_failure(&mut out, failure).unwrap()
        }
    }
}

impl Reporter for StderrReporter<'_> {
    fn report_suite_started(
        &self,
        _ctx: &ProjectContext,
        suite: &Suite,
        run_id: Uuid,
        start: DateTime<Utc>,
    ) -> Result<(), report::Error> {
        eprintln!(
            "run started (id: {run_id}, start: {}, tests: {}, filtered: {})",
            start.format("%+"),
            suite.matched_len(),
            suite.filtered_len(),
        );

        Ok(())
    }

    fn report_suite_finished(
        &self,
        _ctx: &ProjectContext,
        _suite: &Suite,
        trace: &SuiteTrace,
    ) -> Result<(), report::Error> {
        eprintln!(
            "run finished (id: {}, end: {}, passed: {}, failed: {}, skipped: {}, filtered: {})",
            trace.run_id(),
            trace.end().format("%+"),
            trace.passed(),
            trace.failed(),
            trace.skipped(),
            trace.filtered(),
        );

        Ok(())
    }

    fn report_test_started(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        start: DateTime<Utc>,
    ) -> Result<(), report::Error> {
        let _ctx = ctx;
        let _test = test;
        let _start = start;

        Ok(())
    }

    fn report_test_stage_prepare(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        result: bool,
    ) -> Result<(), report::Error> {
        let _ctx = ctx;
        let _test = test;
        let _result = result;

        Ok(())
    }

    fn report_test_stage_reference_compilation(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        result: &CompilationResult,
    ) -> Result<(), report::Error> {
        let _ctx = ctx;
        let _test = test;
        let _result = result;

        Ok(())
    }

    fn report_test_stage_primary_compilation(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        result: &CompilationResult,
    ) -> Result<(), report::Error> {
        let _ctx = ctx;
        let _test = test;
        let _result = result;

        Ok(())
    }

    fn report_test_stage_comparison(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        result: &ComparisonResult,
    ) -> Result<(), report::Error> {
        let _ctx = ctx;
        let _test = test;
        let _result = result;

        Ok(())
    }

    fn report_test_stage_update(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        result: bool,
    ) -> Result<(), report::Error> {
        let _ctx = ctx;
        let _test = test;
        let _result = result;

        Ok(())
    }

    fn report_test_stage_cleanup(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        result: bool,
    ) -> Result<(), report::Error> {
        let _ctx = ctx;
        let _test = test;
        let _result = result;

        Ok(())
    }

    fn report_test_finished(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        trace: &TestTrace,
    ) -> Result<(), report::Error> {
        eprintln!(
            "  {}{: >7}\u{1b}[0m {} \u{1b}[34m{}\u{1b}[0m",
            if trace.kind().is_passed() {
                "\u{1b}[32m"
            } else {
                "\u{1b}[31m"
            },
            if trace.kind().is_passed() {
                if trace.update().is_some_and(|u| u) {
                    "update"
                } else {
                    "success"
                }
            } else {
                "fail"
            },
            {
                let duration = trace.duration();
                format!("{: >3}ms", duration.num_milliseconds())
            },
            test.ident(),
        );

        if let Some(res) = trace.reference_compilation() {
            self.report_diagnostics(ctx, test, res);
        }

        if let Some(res) = trace.primary_compilation() {
            self.report_diagnostics(ctx, test, res);
        }

        if let Some(res) = trace.comparison() {
            self.report_comparison(ctx, test, res);
        }

        Ok(())
    }
}
