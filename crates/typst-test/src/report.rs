use std::fmt::Debug;
use std::io;

use termcolor::{Color, ColorSpec, WriteColor};

use crate::project::test::{CompareFailure, Test, TestFailure, UpdateFailure};
use crate::project::Project;

pub const MAX_PADDING: usize = 20;
pub const MAX_TEST_LIST: usize = 10;

fn write_bold_colored<W: WriteColor + ?Sized>(
    w: &mut W,
    annot: &str,
    color: Color,
) -> io::Result<()> {
    w.set_color(ColorSpec::new().set_bold(true).set_fg(Some(color)))?;
    write!(w, "{annot}")?;
    w.reset()?;
    Ok(())
}

fn write_hint<W: WriteColor + ?Sized>(w: &mut W, pad: &str, hint: &str) -> io::Result<()> {
    write!(w, "{pad}")?;
    write_bold_colored(w, "hint: ", Color::Cyan)?;

    let mut lines = hint.lines();
    if let Some(first) = lines.next() {
        writeln!(w, "{}", first)?;
    }

    for line in lines {
        writeln!(w, "{pad}      {}", line)?;
    }

    Ok(())
}

fn write_program_buffer<W: WriteColor + ?Sized>(
    w: &mut W,
    pad: &str,
    name: &str,
    buffer: &[u8],
) -> io::Result<()> {
    if buffer.is_empty() {
        return Ok(());
    }

    let mut frame_spec = ColorSpec::new();
    frame_spec.set_bold(true);

    if let Ok(s) = std::str::from_utf8(buffer) {
        w.set_color(&frame_spec)?;
        writeln!(w, "{pad}┏━ {name}")?;
        w.reset()?;
        for line in s.lines() {
            w.set_color(&frame_spec)?;
            write!(w, "{pad}┃")?;
            w.reset()?;
            writeln!(w, "{line}")?;
        }
        w.set_color(&frame_spec)?;
        writeln!(w, "{pad}┗━ {name}")?;
        w.reset()?;
    } else {
        writeln!(w, "{pad}{name} was not valid utf8:")?;
        writeln!(w, "{pad}{buffer:?}")?;
    }

    Ok(())
}

fn write_test<W: WriteColor + ?Sized>(
    w: &mut W,
    padding: Option<usize>,
    name: &str,
    annot: (&str, Color),
    details: impl FnOnce(&str, &mut W) -> io::Result<()>,
) -> io::Result<()> {
    let pad = std::cmp::min(padding.unwrap_or_default(), MAX_PADDING);

    write!(w, "{name:<pad$} ")?;

    write_bold_colored(w, annot.0, annot.1)?;
    writeln!(w)?;
    details(&" ".repeat(pad + 1), w)?;

    Ok(())
}

struct Inner<W: ?Sized> {
    padding: Option<usize>,
    writer: W,
}

pub struct Reporter {
    inner: Box<Inner<dyn WriteColor + Send + Sync + 'static>>,
}

impl Debug for Reporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "..")
    }
}

impl Reporter {
    pub fn new<W: WriteColor + Send + Sync + 'static>(writer: W) -> Self {
        Self {
            inner: Box::new(Inner {
                padding: None,
                writer,
            }),
        }
    }

    pub fn set_padding(&mut self, max_padding: Option<usize>) {
        self.inner.padding = max_padding;
    }

    pub fn raw(&mut self, f: impl FnOnce(&mut dyn WriteColor) -> io::Result<()>) -> io::Result<()> {
        f(&mut self.inner.writer)
    }

    pub fn test_success(&mut self, test: &Test, annot: &str) -> io::Result<()> {
        write_test(
            &mut self.inner.writer,
            self.inner.padding,
            test.name(),
            (annot, Color::Green),
            |_, _| Ok(()),
        )
    }

    pub fn test_added(&mut self, test: &Test, no_ref: bool) -> io::Result<()> {
        write_test(
            &mut self.inner.writer,
            self.inner.padding,
            test.name(),
            ("added", Color::Green),
            |pad, w| {
                if no_ref {
                    write_hint(
                        w,
                        pad,
                        &format!("Test template used, no default reference generated\nrun `typst-test update --exact {}` to accept test",
                        test.name(),)
                    )?;
                }

                Ok(())
            },
        )
    }

    pub fn test_failure(&mut self, test: &Test, error: TestFailure) -> io::Result<()> {
        write_test(
            &mut self.inner.writer,
            self.inner.padding,
            test.name(),
            ("failed", Color::Red),
            |pad, w| {
                match error {
                    TestFailure::Preparation(e) => writeln!(w, "{pad}{e}")?,
                    TestFailure::Cleanup(e) => writeln!(w, "{pad}{e}")?,
                    TestFailure::Compilation(e) => {
                        writeln!(w, "{pad}Compilation failed ({})", e.output.status)?;
                        write_program_buffer(w, pad, "stdout", &e.output.stdout)?;
                        write_program_buffer(w, pad, "stderr", &e.output.stderr)?;
                    }
                    TestFailure::Comparison(CompareFailure::PageCount { output, reference }) => {
                        writeln!(
                            w,
                            "{pad}Expected {reference} page{}, got {output} page{}",
                            if reference == 1 { "" } else { "s" },
                            if output == 1 { "" } else { "s" },
                        )?;
                    }
                    TestFailure::Comparison(CompareFailure::Page { pages, diff_dir }) => {
                        for (p, _) in pages {
                            writeln!(w, "{pad}Page {p} did not match")?;
                        }

                        if let Some(diff_dir) = diff_dir {
                            write_hint(
                                w,
                                pad,
                                &format!("Diff images have been saved at {diff_dir:?}",),
                            )?;
                        }
                    }
                    TestFailure::Comparison(CompareFailure::MissingOutput) => {
                        writeln!(w, "{pad}No output was generated")?;
                    }
                    TestFailure::Comparison(CompareFailure::MissingReferences) => {
                        writeln!(w, "{pad}No references were found")?;
                        write_hint(
                            w,
                            pad,
                            &format!(
                                "Use `typst-test update --exact {}` to accept the test output",
                                test.name(),
                            ),
                        )?;
                    }
                    TestFailure::Update(UpdateFailure::Optimize { error }) => {
                        writeln!(w, "{pad}Failed to optimize image")?;
                        writeln!(w, "{pad}{error}")?;
                    }
                }

                Ok(())
            },
        )
    }

    pub fn project(&mut self, project: &Project) -> io::Result<()> {
        if let Some(manifest) = project.manifest() {
            self.raw(|w| {
                write!(w, " Project ┌ ")?;
                write_bold_colored(w, &manifest.package.name.to_string(), Color::Cyan)?;
                write!(w, ":")?;
                write_bold_colored(w, &manifest.package.version.to_string(), Color::Cyan)?;
                writeln!(w)
            })?;

            // TODO: list [tool.typst-test] settings
        } else {
            self.raw(|w| {
                write!(w, " Project ┌ ")?;
                write_bold_colored(w, "none", Color::Yellow)?;
                writeln!(w)
            })?;
        }

        self.raw(|w| {
            write!(w, "Template ├ ")?;
            if project.template().is_some() {
                write_bold_colored(w, "found", Color::Green)?;
            } else {
                write_bold_colored(w, "not found", Color::Yellow)?;
                write!(
                    w,
                    " (looked at {:?})",
                    project.tests_root_dir().join("template.typ")
                )?;
            }
            writeln!(w)
        })?;

        let tests = project.tests();
        if tests.is_empty() {
            self.raw(|w| {
                write!(w, "   Tests └ ")?;
                write_bold_colored(w, "none", Color::Cyan)
            })?;
        } else if tests.len() <= MAX_TEST_LIST {
            self.raw(|w| {
                write!(w, "   Tests ├ ")?;
                write_bold_colored(w, &tests.len().to_string(), Color::Cyan)?;
                writeln!(w)?;
                for (idx, name) in project.tests().keys().enumerate() {
                    if idx == tests.len() - 1 {
                        writeln!(w, "         └ {}", name)?;
                    } else {
                        writeln!(w, "         │ {}", name)?;
                    }
                }

                Ok(())
            })?;
        } else {
            self.raw(|w| {
                write!(w, "   Tests └ ")?;
                write_bold_colored(w, &tests.len().to_string(), Color::Cyan)?;
                writeln!(w)
            })?;
        }

        Ok(())
    }
}
