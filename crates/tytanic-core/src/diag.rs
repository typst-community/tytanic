//! Unified diagnostic printing for XML reports and CLI tools.

use std::path::Path;

use codespan_reporting::diagnostic::Diagnostic;
use codespan_reporting::diagnostic::Label;
use codespan_reporting::files::Error;
use codespan_reporting::files::Files;
use codespan_reporting::term::emit;
use codespan_reporting::term::Config;
use ecow::eco_format;
use termcolor::WriteColor;
use typst::diag::Severity;
use typst::diag::SourceDiagnostic;
use typst::syntax::FileId;
use typst::syntax::Source;
use typst::syntax::Span;
use typst::World;
use typst::WorldExt;

/// A trait to configure emission of Typst diagnostics.
pub trait DiagnosticContext {
    /// Returns the config used for formatting diagnostics.
    fn config(&self) -> &Config;

    /// Returns the world for looking up sources.
    fn world(&self) -> &dyn World;

    /// Returns the root to use for resolving paths.
    fn root(&self) -> &Path;
}

/// Writes Typst diagnostics to a writer with colors if possible.
///
/// The given root path is used to resolve paths and should point to the root
/// used by the world.
///
/// # Panics
/// Panics if the diagnostics have spans pointing to files not found by the
/// given world.
pub fn write_diagnostics(
    w: &mut dyn WriteColor,
    ctx: &dyn DiagnosticContext,
    warnings: &[SourceDiagnostic],
    errors: &[SourceDiagnostic],
) -> Result<(), Error> {
    fn resolve_label(world: &dyn World, span: Span) -> Option<Label<FileId>> {
        Some(Label::primary(span.id()?, world.range(span)?))
    }

    let ctx = ContextShim(ctx);

    for diagnostic in warnings.iter().chain(errors) {
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
            resolve_label(ctx.0.world(), diagnostic.span)
                .into_iter()
                .collect(),
        );

        emit(w, ctx.0.config(), &ctx, &diag)?;

        // Stacktrace-like helper diagnostics.
        for point in &diagnostic.trace {
            let message = point.v.to_string();
            let help = Diagnostic::help().with_message(message).with_labels(
                resolve_label(ctx.0.world(), point.span)
                    .into_iter()
                    .collect(),
            );

            emit(w, ctx.0.config(), &ctx, &help)?;
        }
    }

    Ok(())
}

struct ContextShim<'ctx>(&'ctx dyn DiagnosticContext);

impl ContextShim<'_> {
    fn lookup(&self, id: FileId) -> Source {
        self.0
            .world()
            .source(id)
            .expect("world must have source for which it emitted diagnostic")
    }
}

impl<'a> Files<'a> for ContextShim<'_> {
    type FileId = FileId;
    type Name = String;
    type Source = Source;

    fn name(&'a self, id: FileId) -> Result<Self::Name, Error> {
        let vpath = id.vpath();
        Ok(if let Some(package) = id.package() {
            format!("{package}{}", vpath.as_rooted_path().display())
        } else {
            vpath
                .resolve(self.0.root())
                .unwrap_or_else(|| vpath.as_rootless_path().to_path_buf())
                .to_string_lossy()
                .into()
        })
    }

    fn source(&'a self, id: FileId) -> Result<Self::Source, Error> {
        Ok(self.lookup(id))
    }

    fn line_index(&'a self, id: FileId, given: usize) -> Result<usize, Error> {
        let source = self.lookup(id);
        source
            .byte_to_line(given)
            .ok_or_else(|| Error::IndexTooLarge {
                given,
                max: source.len_bytes(),
            })
    }

    fn line_range(&'a self, id: FileId, given: usize) -> Result<std::ops::Range<usize>, Error> {
        let source = self.lookup(id);
        source
            .line_to_range(given)
            .ok_or_else(|| Error::LineTooLarge {
                given,
                max: source.len_lines(),
            })
    }

    fn column_number(&'a self, id: FileId, _: usize, given: usize) -> Result<usize, Error> {
        let source = self.lookup(id);
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
