//! Live reporting of test progress.

use std::io::{self, Write};
use std::time::Duration;

use color_eyre::eyre;
use termcolor::{Color, WriteColor};
use typst::diag::SourceDiagnostic;
use tytanic_core::doc::compare::{self, PageError};
use tytanic_core::project::Project;
use tytanic_core::stdx::fmt::Term;
use tytanic_core::test::{SuiteResult, Test, TestResult, TestResultKind};

use crate::cwrite;
use crate::ui::{self, Ui};
use crate::world::SystemWorld;

/// The padding to use for annotations while test run reporting.
const RUN_ANNOT_PADDING: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum When {
    Never,
    // TODO(tinger): make this configurable, along side the richness of
    // diagnostics
    #[allow(dead_code)]
    Failure,
    Always,
}

/// A reporter for test output and test run status reporting.
pub struct Reporter<'ui, 'p> {
    ui: &'ui Ui,
    project: &'p Project,
    world: &'p SystemWorld,

    live: bool,
    warnings: When,
    errors: bool,
}

impl<'ui, 'p> Reporter<'ui, 'p> {
    pub fn new(ui: &'ui Ui, project: &'p Project, world: &'p SystemWorld, live: bool) -> Self {
        Self {
            ui,
            project,
            world,
            live,
            warnings: When::Always,
            errors: true,
        }
    }
}

impl Reporter<'_, '_> {
    /// Reports the start of a test run.
    pub fn report_start(&self, result: &SuiteResult) -> io::Result<()> {
        let mut w = ui::annotated(
            self.ui.stderr(),
            "Starting",
            Color::Green,
            RUN_ANNOT_PADDING,
        )?;

        cwrite!(bold(w), "{}", result.total())?;
        write!(w, " tests")?;

        if result.filtered() != 0 {
            write!(w, ", ")?;
            cwrite!(bold(w), "{}", result.filtered())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Yellow), "filtered")?;
        }

        write!(w, " (run ID: ")?;
        cwrite!(bold(w), "{}", result.id())?;
        writeln!(w, ")")?;

        Ok(())
    }

    /// Reports the end of a test run.
    pub fn report_end(&self, result: &SuiteResult) -> io::Result<()> {
        let mut w = self.ui.stderr();

        let color = if result.failed() == 0 {
            Color::Green
        } else if result.passed() == 0 {
            Color::Red
        } else {
            Color::Yellow
        };

        writeln!(w, "{:─>RUN_ANNOT_PADDING$}", "")?;

        let mut w = ui::annotated(w, "Summary", color, RUN_ANNOT_PADDING)?;

        write!(w, "[")?;
        write_duration(
            &mut ui::colored(
                &mut w,
                duration_color(
                    result
                        .duration()
                        .checked_div(result.run() as u32)
                        .unwrap_or_default(),
                ),
            )?,
            result.duration(),
        )?;
        write!(w, "] ")?;

        cwrite!(bold(w), "{}", result.run())?;
        write!(w, "/")?;
        cwrite!(bold(w), "{}", result.expected())?;
        write!(w, " tests run: ")?;

        if result.passed() == result.total() {
            cwrite!(bold(w), "all {}", result.passed())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Green), "passed")?;
        } else if result.failed() == result.total() {
            cwrite!(bold(w), "all {}", result.failed())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Red), "failed")?;
        } else {
            cwrite!(bold(w), "{}", result.passed())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Green), "passed")?;

            write!(w, ", ")?;
            cwrite!(bold(w), "{}", result.failed())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Red), "failed")?;
        }

        if result.filtered() != 0 {
            write!(w, ", ")?;
            cwrite!(bold(w), "{}", result.filtered())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Yellow), "filtered")?;
        }

        if result.skipped() != 0 {
            write!(w, ", ")?;
            cwrite!(bold(w), "{}", result.skipped())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Yellow), "skipped")?;
        }

        writeln!(w)?;

        // TODO(tinger): report failures, mean and avg time

        Ok(())
    }

    /// Clears the last line, i.e the status output.
    pub fn clear_status(&self) -> io::Result<()> {
        if !self.live {
            return Ok(());
        }

        write!(self.ui.stderr(), "\x1B[0F\x1B[0J")
    }

    /// Reports the current status of an ongoing test run.
    pub fn report_status(&self, result: &SuiteResult) -> io::Result<()> {
        if !self.live {
            return Ok(());
        }

        let duration = result.timestamp().elapsed();

        let mut w = ui::annotated(self.ui.stderr(), "", Color::Black, RUN_ANNOT_PADDING)?;

        write!(w, "[")?;
        write_duration(
            &mut ui::colored(
                &mut w,
                duration_color(
                    duration
                        .checked_div(result.run() as u32)
                        .unwrap_or_default(),
                ),
            )?,
            result.duration(),
        )?;
        write!(w, "] ")?;

        cwrite!(bold(w), "{}", result.run())?;
        write!(w, "/")?;
        cwrite!(bold(w), "{}", result.expected())?;
        write!(w, " tests run: ")?;

        if result.passed() == result.total() {
            cwrite!(bold(w), "all {}", result.passed())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Green), "passed")?;
        } else if result.failed() == result.total() {
            cwrite!(bold(w), "all {}", result.failed())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Red), "failed")?;
        } else {
            cwrite!(bold(w), "{}", result.passed())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Green), "passed")?;

            write!(w, ", ")?;
            cwrite!(bold(w), "{}", result.failed())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Red), "failed")?;
        }

        if result.filtered() != 0 {
            write!(w, ", ")?;
            cwrite!(bold(w), "{}", result.filtered())?;
            write!(w, " ")?;
            cwrite!(colored(w, Color::Yellow), "filtered")?;
        }

        writeln!(w)?;

        Ok(())
    }

    /// Report that a test has passed.
    pub fn report_test_pass(
        &self,
        test: &Test,
        duration: Duration,
        warnings: &[SourceDiagnostic],
    ) -> eyre::Result<()> {
        let mut w = ui::annotated(self.ui.stderr(), "pass", Color::Green, RUN_ANNOT_PADDING)?;

        write!(w, "[")?;
        write_duration(
            &mut ui::colored(&mut w, duration_color(duration))?,
            duration,
        )?;
        write!(w, "] ")?;
        ui::write_test_id(&mut w, test.id())?;
        writeln!(w)?;

        ui::write_diagnostics(
            &mut w,
            self.ui.diagnostic_config(),
            self.world,
            if self.warnings == When::Always {
                warnings
            } else {
                &[]
            },
            &[],
        )?;

        Ok(())
    }

    /// Report that a test has failed and show its output and failure reason.
    pub fn report_test_fail(
        &self,
        test: &Test,
        result: &TestResult,
        diff_hint: bool,
    ) -> eyre::Result<()> {
        let mut w = ui::annotated(self.ui.stderr(), "fail", Color::Red, RUN_ANNOT_PADDING)?;

        write!(w, "[")?;
        write_duration(
            &mut ui::colored(&mut w, duration_color(result.duration()))?,
            result.duration(),
        )?;
        write!(w, "] ")?;
        ui::write_test_id(&mut w, test.id())?;
        writeln!(w)?;

        match result.kind() {
            Some(TestResultKind::FailedCompilation { error, reference }) => {
                writeln!(
                    w,
                    "Compilation of {} failed",
                    if *reference { "reference" } else { "test" },
                )?;

                ui::write_diagnostics(
                    &mut w,
                    self.ui.diagnostic_config(),
                    self.world,
                    if self.warnings != When::Never {
                        result.warnings()
                    } else {
                        &[]
                    },
                    if self.errors { &error.0 } else { &[] },
                )?;
            }
            Some(TestResultKind::FailedComparison(compare::Error {
                output,
                reference,
                pages,
            })) => {
                if output != reference {
                    writeln!(
                        w,
                        "Expected {reference} {}, got {output} {}",
                        Term::simple("page").with(*reference),
                        Term::simple("page").with(*output),
                    )?;
                }

                for (p, e) in pages {
                    let p = p + 1;
                    match e {
                        PageError::Dimensions { output, reference } => {
                            writeln!(w, "Page {p} had different dimensions")?;
                            w.write_with(2, |w| {
                                writeln!(w, "Output: {}", output)?;
                                writeln!(w, "Reference: {}", reference)
                            })?;
                        }
                        PageError::SimpleDeviations { deviations } => {
                            writeln!(
                                w,
                                "Page {p} had {deviations} {}",
                                Term::simple("deviation").with(*deviations),
                            )?;
                        }
                    }
                }

                if diff_hint {
                    writeln!(
                        ui::hint(w)?,
                        "Diff images have been saved at '{}'",
                        self.project.paths().test_diff_dir(test.id()).display()
                    )?;
                }
            }
            _ => unreachable!(),
        }

        Ok(())
    }
}

/// Writes a padded duration in human readable form
fn write_duration(w: &mut dyn WriteColor, duration: Duration) -> io::Result<()> {
    let s = duration.as_secs();
    let ms = duration.subsec_millis();
    let us = duration.subsec_micros().saturating_sub(ms * 1000);

    write!(w, "{s: >2}s")?;
    write!(w, " {ms: >3}ms")?;
    write!(w, " {us: >3}µs")?;

    Ok(())
}

/// Returns the color to use for a test's duration.
fn duration_color(duration: Duration) -> Color {
    match duration.as_secs() {
        0 if duration.is_zero() => Color::Rgb(128, 128, 128),
        0 => Color::Green,
        1..=5 => Color::Yellow,
        _ => Color::Red,
    }
}
