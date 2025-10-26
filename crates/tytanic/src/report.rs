//! Live reporting of test progress.

use std::io;
use std::io::Write;

use chrono::DateTime;
use chrono::TimeDelta;
use chrono::Utc;
use codespan_reporting::term::Config;
use termcolor::Color;
use termcolor::ColorChoice;
use tytanic_core::analysis::PageError;
use tytanic_core::diag;
use tytanic_core::diag::write_diagnostics;
use tytanic_core::project::ProjectContext;
use tytanic_core::result::ComparisonResult;
use tytanic_core::result::CompilationResult;
use tytanic_core::result::Kind;
use tytanic_core::result::SuiteTrace;
use tytanic_core::result::TestTrace;
use tytanic_core::suite::Suite;
use tytanic_core::test::Test;
use tytanic_runner::provide::WorldProvider;
use tytanic_runner::report;
use tytanic_runner::report::Error;
use tytanic_runner::report::Reporter;
use tytanic_utils::cwrite;
use tytanic_utils::fmt::Term;
use tytanic_utils::ui;
use tytanic_utils::ui::CWrite;
use tytanic_utils::ui::Indented;
use uuid::Uuid;

use crate::ui::Ui;
use crate::ui::write_test_ident;

/// The padding to use for annotations while test run reporting.
const RUN_ANNOT_PADDING: usize = 10;

/// Writes a padded duration in human readable form.
fn write_duration(w: &mut dyn Write, duration: TimeDelta) -> io::Result<()> {
    let s = duration.num_seconds();
    let ms = duration.subsec_millis();

    if s > 0 {
        write!(w, "{s: >2}s")?;
    } else {
        write!(w, "   ")?;
    }

    write!(w, " {ms: >3}ms")?;

    Ok(())
}

/// Returns the color to use for a test's duration.
fn duration_color(duration: TimeDelta) -> Color {
    match duration.num_seconds() {
        0 if duration.is_zero() => Color::Rgb(128, 128, 128),
        0 => Color::Green,
        1..=5 => Color::Yellow,
        _ => Color::Red,
    }
}

/// A reporter for test output and test run status reporting.
#[derive(Debug)]
pub struct StderrReporter<'ui, 'p> {
    ui: &'ui Ui,
    provider: &'p WorldProvider,

    live: bool,
}

impl<'ui, 'p> StderrReporter<'ui, 'p> {
    pub fn new(ui: &'ui Ui, provider: &'p WorldProvider, live: bool) -> Self {
        Self { ui, provider, live }
    }
}

impl StderrReporter<'_, '_> {
    fn report_diagnostics(&self, ctx: &ProjectContext, test: &Test, result: &CompilationResult) {
        let mut out = Indented::new(termcolor::StandardStream::stderr(ColorChoice::Auto), 16);
        let files = self.provider.file_provider(ctx, test).unwrap();

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

impl StderrReporter<'_, '_> {
    /// Clears the last line, i.e the status output.
    pub fn clear_status(&self) -> io::Result<()> {
        if !self.live {
            return Ok(());
        }

        write!(self.ui.stderr(), "\x1B[0F\x1B[0J")
    }

    /// Reports the current status of an ongoing run.
    pub fn report_status(&self, suite: &Suite, trace: &SuiteTrace) -> io::Result<()> {
        if !self.live {
            return Ok(());
        }

        let run = trace.passed() + trace.failed();
        let duration = Utc::now().signed_duration_since(trace.start());

        let mut w = ui::annotated(self.ui.stderr(), "", Color::Black, RUN_ANNOT_PADDING)?;

        write!(w, "[")?;
        {
            let mut w = ui::colored(
                &mut w,
                duration_color(duration.checked_div(run as i32).unwrap_or_default()),
            )?;
            write_duration(&mut w, duration)?;
            w.finish()?;
        }
        write!(w, "] ")?;

        cwrite!(bold(w), "{}", run)?;
        write!(w, "/")?;
        cwrite!(bold(w), "{}", suite.len())?;
        write!(w, " tests run: ")?;

        if trace.passed() == suite.len() {
            cwrite!(bold(w), "all {}", trace.passed())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Green), "passed")?;
        } else if trace.failed() == suite.len() {
            cwrite!(bold(w), "all {}", trace.failed())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Red), "failed")?;
        } else {
            cwrite!(bold(w), "{}", trace.passed())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Green), "passed")?;

            write!(w, ", ")?;
            cwrite!(bold(w), "{}", trace.failed())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Red), "failed")?;
        }

        if trace.filtered() != 0 {
            write!(w, ", ")?;
            cwrite!(bold(w), "{}", trace.filtered())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Yellow), "filtered")?;
        }

        writeln!(w)?;

        Ok(())
    }
}

impl Reporter for StderrReporter<'_, '_> {
    fn report_suite_started(
        &self,
        _ctx: &ProjectContext,
        suite: &Suite,
        run_id: Uuid,
        _start: DateTime<Utc>,
    ) -> Result<(), report::Error> {
        let mut w = ui::annotated(
            self.ui.stderr(),
            "Starting",
            Color::Green,
            RUN_ANNOT_PADDING,
        )?;

        cwrite!(bold(w), "{}", suite.matched_len())?;
        write!(w, " tests")?;

        if suite.filtered_len() != 0 {
            write!(w, ", ")?;
            cwrite!(bold(w), "{}", suite.filtered_len())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Yellow), "filtered")?;
        }

        write!(w, " (run ID: ")?;
        cwrite!(bold(w), "{run_id}")?;
        writeln!(w, ")")?;

        Ok(())
    }

    fn report_suite_finished(
        &self,
        _ctx: &ProjectContext,
        suite: &Suite,
        trace: &SuiteTrace,
    ) -> Result<(), report::Error> {
        let mut w = self.ui.stderr();

        let run = trace.passed() + trace.failed();
        let color = if trace.failed() == 0 {
            Color::Green
        } else if trace.passed() == 0 {
            Color::Red
        } else {
            Color::Yellow
        };

        writeln!(w, "{:─>RUN_ANNOT_PADDING$}", "")?;

        let mut w = ui::annotated(w, "Summary", color, RUN_ANNOT_PADDING)?;

        write!(w, "[")?;
        {
            let mut w = ui::colored(
                &mut w,
                duration_color(trace.duration().checked_div(run as i32).unwrap_or_default()),
            )?;
            write_duration(&mut w, trace.duration())?;
            w.finish()?;
        }
        write!(w, "] ")?;

        cwrite!(bold(w), "{run}")?;
        write!(w, "/")?;
        cwrite!(bold(w), "{}", suite.matched_len())?;
        write!(w, " tests run: ")?;

        if trace.passed() == trace.len() {
            cwrite!(bold(w), "all {}", trace.passed())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Green), "passed")?;
        } else if trace.failed() == trace.len() {
            cwrite!(bold(w), "all {}", trace.failed())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Red), "failed")?;
        } else {
            cwrite!(bold(w), "{}", trace.passed())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Green), "passed")?;

            write!(w, ", ")?;
            cwrite!(bold(w), "{}", trace.failed())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Red), "failed")?;
        }

        if trace.filtered() != 0 {
            write!(w, ", ")?;
            cwrite!(bold(w), "{}", trace.filtered())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Yellow), "filtered")?;
        }

        if trace.skipped() != 0 {
            write!(w, ", ")?;
            cwrite!(bold(w), "{}", trace.skipped())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Yellow), "skipped")?;
        }

        writeln!(w)?;

        // TODO(tinger): Report failures, mean, and average time.

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
        let (annot, color) = match trace.kind() {
            Kind::Unfinished => ("skip", Color::Yellow),
            Kind::Failed => ("fail", Color::Red),
            Kind::Passed => (
                if trace.update().is_some_and(|u| u) {
                    "update"
                } else {
                    "pass"
                },
                Color::Green,
            ),
        };

        let mut w = ui::annotated(self.ui.stderr(), annot, color, RUN_ANNOT_PADDING)?;

        write!(w, "[")?;
        {
            let mut w = ui::colored(&mut w, duration_color(trace.duration()))?;
            write_duration(&mut w, trace.duration())?;
            w.finish()?;
        }
        write!(w, "] ")?;
        write_test_ident(&mut w, test.ident())?;
        writeln!(w)?;

        if let Some(ComparisonResult::Failed(failure)) = trace.comparison() {
            let error = failure.error();

            let primary = error.primary_page_count();
            let reference = error.reference_page_count();

            if reference != primary {
                writeln!(
                    w,
                    "Expected {reference} {}, got {primary } {}",
                    Term::simple("page").with(reference),
                    Term::simple("page").with(primary),
                )?;

                for (p, e) in error.page_errors() {
                    let p = p + 1;
                    match e {
                        PageError::Dimensions { output, reference } => {
                            writeln!(w, "Page {p} had different dimensions")?;
                            w.write_with(2, |w| {
                                writeln!(w, "Output: {output}")?;
                                writeln!(w, "Reference: {reference}")
                            })?;
                        }
                        PageError::Deviations { deviations } => {
                            writeln!(
                                w,
                                "Page {p} had {deviations} {}",
                                Term::simple("deviation").with(*deviations),
                            )?;
                        }
                    }
                }
            }
        } else {
            // TODO(tinger): Deduplicate some of the diagnostics.

            if let Some(CompilationResult::Failed(failure)) = trace.primary_compilation() {
                let files = self.provider.file_provider(ctx, test).ok_or_else(|| {
                    Error::Other(
                        format!("Failed to retreive file provider for {}", test.ident()).into(),
                    )
                })?;

                write_diagnostics(
                    &mut w,
                    self.ui.diagnostic_config(),
                    &files,
                    failure.warnings(),
                )
                .map_err(|error| Error::Other(error.into()))?;

                write_diagnostics(
                    &mut w,
                    self.ui.diagnostic_config(),
                    &files,
                    failure.errors(),
                )
                .map_err(|error| Error::Other(error.into()))?;

                writeln!(w, "Compilation of primary document failed")?;
            }

            if let Some(CompilationResult::Failed(failure)) = trace.reference_compilation() {
                let files = self.provider.file_provider(ctx, test).ok_or_else(|| {
                    Error::Other(
                        format!("Failed to retreive file provider for {}", test.ident()).into(),
                    )
                })?;

                write_diagnostics(
                    &mut w,
                    self.ui.diagnostic_config(),
                    &files,
                    failure.warnings(),
                )
                .map_err(|error| Error::Other(error.into()))?;

                write_diagnostics(
                    &mut w,
                    self.ui.diagnostic_config(),
                    &files,
                    failure.errors(),
                )
                .map_err(|error| Error::Other(error.into()))?;

                writeln!(w, "Compilation of reference document failed")?;
            }
        }

        Ok(())
    }
}
