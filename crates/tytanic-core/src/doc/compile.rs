//! Test document compilation and diagnostics handling.

use std::fmt::Debug;

use ecow::{eco_vec, EcoVec};
use thiserror::Error;
use typst::diag::{FileResult, Severity, SourceDiagnostic, Warned};
use typst::foundations::{Bytes, Datetime};
use typst::model::Document;
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};
use tytanic_utils::fmt::Term;

/// How to handle warnings during compilation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Warnings {
    /// Ignore all warnings.
    Ignore,

    /// Emit all warnings.
    #[default]
    Emit,

    /// Promote all warnings to errors.
    Promote,
}

/// An error which may occur during compilation. This struct only exists to
/// implement [`Error`][trait@std::error::Error].
#[derive(Debug, Clone, Error)]
#[error("compilation failed with {} {}", .0.len(), Term::simple("error").with(.0.len()))]
pub struct Error(pub EcoVec<SourceDiagnostic>);

/// Compiles a source with the given global world.
pub fn compile(
    source: Source,
    world: &dyn World,
    warnings: Warnings,
) -> Warned<Result<Document, Error>> {
    struct TestWorldAdapter<'s, 'w> {
        source: &'s Source,
        global: &'w dyn World,
    }

    impl World for TestWorldAdapter<'_, '_> {
        fn library(&self) -> &LazyHash<Library> {
            self.global.library()
        }

        fn book(&self) -> &LazyHash<FontBook> {
            self.global.book()
        }

        fn main(&self) -> FileId {
            self.source.id()
        }

        fn source(&self, id: FileId) -> FileResult<Source> {
            if id == self.source.id() {
                Ok(self.source.clone())
            } else {
                self.global.source(id)
            }
        }

        fn file(&self, id: FileId) -> FileResult<Bytes> {
            self.global.file(id)
        }

        fn font(&self, index: usize) -> Option<Font> {
            self.global.font(index)
        }

        fn today(&self, offset: Option<i64>) -> Option<Datetime> {
            self.global.today(offset)
        }
    }

    let Warned {
        output,
        warnings: mut emitted,
    } = typst::compile(&TestWorldAdapter {
        source: &source,
        global: world,
    });

    match warnings {
        Warnings::Ignore => Warned {
            output: output.map_err(Error),
            warnings: eco_vec![],
        },
        Warnings::Emit => Warned {
            output: output.map_err(Error),
            warnings: emitted,
        },
        Warnings::Promote => {
            emitted = emitted
                .into_iter()
                .map(|mut warning| {
                    warning.severity = Severity::Error;
                    warning.with_hint("this warning was promoted to an error")
                })
                .collect();

            match output {
                Ok(doc) if emitted.is_empty() => Warned {
                    output: Ok(doc),
                    warnings: eco_vec![],
                },
                Ok(_) => Warned {
                    output: Err(Error(emitted)),
                    warnings: eco_vec![],
                },
                Err(errors) => {
                    emitted.extend(errors);
                    Warned {
                        output: Err(Error(emitted)),
                        warnings: eco_vec![],
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::_dev::VirtualWorld;

    const TEST_PASS: &str = "Hello World";
    const TEST_WARN: &str = "#set text(font: \"foo\"); Hello World";
    const TEST_FAIL: &str = "#set text(font: \"foo\"); #panic()";

    #[test]
    fn test_compile_pass_ignore_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_PASS);

        let Warned { output, warnings } = compile(source, &world, Warnings::Ignore);
        assert!(output.is_ok());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_compile_pass_emit_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_PASS);

        let Warned { output, warnings } = compile(source, &world, Warnings::Emit);
        assert!(output.is_ok());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_compile_pass_promote_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_PASS);

        let Warned { output, warnings } = compile(source, &world, Warnings::Promote);
        assert!(output.is_ok());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_compile_warn_ignore_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_WARN);

        let Warned { output, warnings } = compile(source, &world, Warnings::Ignore);
        assert!(output.is_ok());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_compile_warn_emit_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_WARN);

        let Warned { output, warnings } = compile(source, &world, Warnings::Emit);
        assert!(output.is_ok());
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn test_compile_warn_promote_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_WARN);

        let Warned { output, warnings } = compile(source, &world, Warnings::Promote);
        assert_eq!(output.unwrap_err().0.len(), 1);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_compile_fail_ignore_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_FAIL);

        let Warned { output, warnings } = compile(source, &world, Warnings::Ignore);
        assert_eq!(output.unwrap_err().0.len(), 1);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_compile_fail_emit_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_FAIL);

        let Warned { output, warnings } = compile(source, &world, Warnings::Emit);
        assert_eq!(output.unwrap_err().0.len(), 1);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn test_compile_fail_promote_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_FAIL);

        let Warned { output, warnings } = compile(source, &world, Warnings::Promote);
        assert_eq!(output.unwrap_err().0.len(), 2);
        assert!(warnings.is_empty());
    }
}
