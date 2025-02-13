//! Test document compilation and diagnostics handling.

use std::fmt::Debug;
use std::sync::LazyLock;
use std::sync::OnceLock;

use ecow::{eco_format, eco_vec, EcoVec};
use thiserror::Error;
use typst::diag::{FileResult, Severity, SourceDiagnostic, Warned};
use typst::foundations::{Bytes, Datetime};
use typst::layout::PagedDocument;
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, Source, Span};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};
use tytanic_utils::fmt::Term;

use crate::library::augmented_default_library;

static AUGMENTED_LIBRARY: LazyLock<LazyHash<Library>> =
    LazyLock::new(|| LazyHash::new(augmented_default_library()));

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
///
/// This function compiles a test source by wrapping the provided [`World`]
/// implementation in a short lived wrapper which exposes `source` as the
/// compilation entrypoint. An optional package spec can be provided for
/// re-routing package imports to the local root, this is primarily useful for
/// template tests which access unreleased versions of a package.
pub fn compile(
    source: Source,
    world: &dyn World,
    augment: bool,
    reroute_to_self: Option<&PackageSpec>,
    warnings: Warnings,
) -> Warned<Result<PagedDocument, Error>> {
    struct TestWorldAdapter<'s, 'w, 'p> {
        source: &'s Source,
        global: &'w dyn World,
        augment: bool,
        package: Option<&'p PackageSpec>,
        accessed_old: OnceLock<(PackageSpec, PackageSpec)>,
    }

    impl TestWorldAdapter<'_, '_, '_> {
        fn transform_id(&self, id: FileId) -> FileId {
            let Some(this) = self.package else {
                return id;
            };

            match id.package() {
                Some(pacakge) if pacakge == this => FileId::new(None, id.vpath().clone()),
                Some(package) => {
                    if package.namespace == this.namespace && package.name == this.name {
                        _ = self.accessed_old.set((package.clone(), this.clone()));
                    }

                    id
                }
                None => id,
            }
        }
    }

    impl World for TestWorldAdapter<'_, '_, '_> {
        fn library(&self) -> &LazyHash<Library> {
            if self.augment {
                &AUGMENTED_LIBRARY
            } else {
                self.global.library()
            }
        }

        fn book(&self) -> &LazyHash<FontBook> {
            self.global.book()
        }

        fn main(&self) -> FileId {
            self.source.id()
        }

        fn source(&self, id: FileId) -> FileResult<Source> {
            let id = self.transform_id(id);

            if id == self.source.id() {
                Ok(self.source.clone())
            } else {
                self.global.source(id)
            }
        }

        fn file(&self, id: FileId) -> FileResult<Bytes> {
            let id = self.transform_id(id);

            self.global.file(id)
        }

        fn font(&self, index: usize) -> Option<Font> {
            self.global.font(index)
        }

        fn today(&self, offset: Option<i64>) -> Option<Datetime> {
            self.global.today(offset)
        }
    }

    let test_world = TestWorldAdapter {
        source: &source,
        global: world,
        augment,
        package: reroute_to_self,
        accessed_old: OnceLock::new(),
    };

    let Warned {
        output,
        warnings: mut emitted,
    } = typst::compile(&test_world);

    if let Some((old, new)) = test_world.accessed_old.into_inner() {
        emitted.push(SourceDiagnostic {
            severity: Severity::Warning,
            span: Span::detached(),
            message: eco_format!("Accessed {old} in tests for package {new}"),
            trace: eco_vec![],
            hints: eco_vec![eco_format!(
                "Did you forget to update the package import in your template?"
            )],
        });
    }

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

        let Warned { output, warnings } = compile(source, &world, false, None, Warnings::Ignore);
        assert!(output.is_ok());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_compile_pass_emit_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_PASS);

        let Warned { output, warnings } = compile(source, &world, false, None, Warnings::Emit);
        assert!(output.is_ok());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_compile_pass_promote_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_PASS);

        let Warned { output, warnings } = compile(source, &world, false, None, Warnings::Promote);
        assert!(output.is_ok());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_compile_warn_ignore_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_WARN);

        let Warned { output, warnings } = compile(source, &world, false, None, Warnings::Ignore);
        assert!(output.is_ok());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_compile_warn_emit_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_WARN);

        let Warned { output, warnings } = compile(source, &world, false, None, Warnings::Emit);
        assert!(output.is_ok());
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn test_compile_warn_promote_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_WARN);

        let Warned { output, warnings } = compile(source, &world, false, None, Warnings::Promote);
        assert_eq!(output.unwrap_err().0.len(), 1);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_compile_fail_ignore_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_FAIL);

        let Warned { output, warnings } = compile(source, &world, false, None, Warnings::Ignore);
        assert_eq!(output.unwrap_err().0.len(), 1);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_compile_fail_emit_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_FAIL);

        let Warned { output, warnings } = compile(source, &world, false, None, Warnings::Emit);
        assert_eq!(output.unwrap_err().0.len(), 1);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn test_compile_fail_promote_warnings() {
        let world = VirtualWorld::default();
        let source = Source::detached(TEST_FAIL);

        let Warned { output, warnings } = compile(source, &world, false, None, Warnings::Promote);
        assert_eq!(output.unwrap_err().0.len(), 2);
        assert!(warnings.is_empty());
    }
}
