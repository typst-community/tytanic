//! Diagnostic and result formatting inline with that of the Typst CLI.

use std::io;

use codespan_reporting::diagnostic::Diagnostic;
use codespan_reporting::diagnostic::Label;
use codespan_reporting::files::Error;
use codespan_reporting::files::Files;
use codespan_reporting::term::Config;
use codespan_reporting::term::emit;
use ecow::eco_format;
use termcolor::WriteColor;
use typst::diag::FileError;
use typst::diag::Severity;
use typst::diag::SourceDiagnostic;
use typst_syntax::FileId;
use typst_syntax::Lines;
use typst_syntax::Span;
use tytanic_utils::fmt::Term;
use tytanic_utils::typst::world::ProvideFile;

use crate::analysis::PageError;
use crate::result::ComparisonFailure;

/// Writes a comparison failure into the color writer.
pub fn write_comparison_failure(
    writer: &mut dyn WriteColor,
    failure: &ComparisonFailure,
) -> io::Result<()> {
    let primary = failure.error().primary_page_count();
    let reference = failure.error().reference_page_count();

    if primary != reference {
        writeln!(
            writer,
            "Expected {reference} {}, got {primary} {}",
            Term::simple("page").with(reference),
            Term::simple("page").with(primary),
        )?;
    }

    for (p, e) in failure.error().page_errors() {
        let p = p + 1;
        match e {
            PageError::Dimensions { output, reference } => {
                writeln!(writer, "Page {p} had different dimensions")?;
                writeln!(writer, "  Output: {output}")?;
                writeln!(writer, "  Reference: {reference}")?;
            }
            PageError::Deviations { deviations } => {
                writeln!(
                    writer,
                    "Page {p} had {deviations} {}",
                    Term::simple("deviation").with(*deviations),
                )?;
            }
        }
    }

    Ok(())
}

/// Writes the given diagnostics into a color writer the same way the Typst CLI
/// does.
pub fn write_diagnostics(
    writer: &mut dyn WriteColor,
    config: &Config,
    files: &dyn ProvideFile,
    diagnostics: &[SourceDiagnostic],
) -> Result<(), Error> {
    fn resolve_span_to_label(shim: Shim<'_>, span: Span) -> Option<Label<FileId>> {
        Some(Label::primary(
            span.id()?,
            span.range()
                .or_else(|| shim.0.provide_source(span.id()?).ok()?.range(span))?,
        ))
    }

    let files = Shim(files);

    for diagnostic in diagnostics.iter() {
        let diag = match diagnostic.severity {
            Severity::Error => Diagnostic::error(),
            Severity::Warning => Diagnostic::warning(),
        }
        .with_message(diagnostic.message.clone())
        .with_notes(
            diagnostic
                .hints
                .iter()
                .map(|e| (eco_format!("hint: {e}")).into())
                .collect(),
        )
        .with_labels(
            resolve_span_to_label(files, diagnostic.span)
                .into_iter()
                .collect(),
        );

        emit(writer, config, &files, &diag)?;

        // Stacktrace-like helper diagnostics.
        for point in &diagnostic.trace {
            let message = point.v.to_string();
            let help = Diagnostic::help().with_message(message).with_labels(
                resolve_span_to_label(files, point.span)
                    .into_iter()
                    .collect(),
            );

            emit(writer, config, &files, &help)?;
        }
    }

    Ok(())
}

/// A shim around [`ProvideFile`] that implements [`Files`].
#[derive(Clone, Copy)]
struct Shim<'a>(&'a dyn ProvideFile);

impl Shim<'_> {
    fn lookup(&self, id: FileId) -> Result<Lines<String>, Error> {
        Ok(match self.0.provide_source(id) {
            Ok(source) => source.lines().clone(),
            Err(FileError::NotSource) => {
                let bytes = self.0.provide_bytes(id).unwrap();
                Lines::try_from(&bytes).expect("file is not valid utf-8")
            }
            Err(_) => {
                unreachable!()
            }
        })
    }
}

impl<'a> Files<'a> for Shim<'_> {
    type FileId = FileId;
    type Name = String;
    type Source = Lines<String>;

    fn name(&'a self, id: FileId) -> Result<Self::Name, Error> {
        let vpath = id.vpath();

        Ok(if let Some(package) = id.package() {
            format!("{package}{}", vpath.as_rooted_path().display())
        } else {
            vpath.as_rooted_path().to_string_lossy().into()
        })
    }

    fn source(&'a self, id: FileId) -> Result<Self::Source, Error> {
        self.lookup(id)
    }

    fn line_index(&'a self, id: FileId, given: usize) -> Result<usize, Error> {
        let source = self.lookup(id)?;
        source
            .byte_to_line(given)
            .ok_or_else(|| Error::IndexTooLarge {
                given,
                max: source.len_bytes(),
            })
    }

    fn line_range(&'a self, id: FileId, given: usize) -> Result<std::ops::Range<usize>, Error> {
        let source = self.lookup(id)?;
        source
            .line_to_range(given)
            .ok_or_else(|| Error::LineTooLarge {
                given,
                max: source.len_lines(),
            })
    }

    fn column_number(&'a self, id: FileId, _: usize, given: usize) -> Result<usize, Error> {
        let source = self.lookup(id)?;
        source.byte_to_column(given).ok_or_else(|| {
            let max = source.len_bytes();
            if given <= max {
                Error::InvalidCharBoundary { given }
            } else {
                Error::IndexTooLarge { given, max }
            }
        })
    }
}
