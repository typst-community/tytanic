//! # `tytanic-typst-library`
//! Standard library augmentation, i.e. additional functions and values for the
//! Typst standard library.
//!
//! This crate simply provides additional bindings in the Typst standard library
//! to make testing easier, especially package failure paths. The following
//! bindings are added:
//! - `test` (module)
//!   - `catch` (function, in prelude)
//!   - `assert-panic` (function, in prelude)
//!
//! The modifier `in prelude` means that a nested binding is also available from
//! the global scope. This may change in the future for compatibility reasons.
//!
//! # Functions
//! ## `catch`
//! Provides a mechanism to catch panics inside test scripts. Returns the panic
//! message or `none` if no panic was encountered.
//! ```typst
//! #let msg = catch(() => {
//!   panic()
//! })
//! ```
//!
//! ## `assert-panic`
//! Provides an assertion that tests if a given closure panicked, panicking if
//! it did not. Takes an optional `message` similar to other `assert` functions.
//! ```typst
//! #assert-panic(() => {}, message: "Did not panic")
//! ```

use typst::Library;
use typst::LibraryBuilder;
use typst::LibraryExt;
use typst::comemo::Tracked;
use typst::diag::SourceResult;
use typst::diag::bail;
use typst::ecow::EcoString;
use typst::engine::Engine;
use typst::foundations::Context;
use typst::foundations::Func;
use typst::foundations::Module;
use typst::foundations::Repr;
use typst::foundations::Scope;
use typst::foundations::Str;
use typst::foundations::Value;
use typst::foundations::func;

/// Defines prelude items for the given scope, this is a subset of
/// [`define_test_module`].
pub fn define_prelude(scope: &mut Scope) {
    scope.define_func::<catch>();
    scope.define_func::<assert_panic>();
}

/// Defines test module items for the given scope.
pub fn define_test_module(scope: &mut Scope) {
    define_prelude(scope)
}

/// Creates a new test module with the items defined by [`define_test_module`].
pub fn test_module() -> Module {
    let mut scope = Scope::new();
    define_test_module(&mut scope);
    Module::new("test", scope)
}

/// Creates a new augmented default standard library. See [`augmented_library`].
pub fn augmented_default_library() -> Library {
    augmented_library(|x| x)
}

/// Creates a new augmented standard library, applying the given closure to the
/// builder.
///
/// The augmented standard library contains a new test module and a few items in
/// the prelude for easier testing.
pub fn augmented_library(builder: impl FnOnce(LibraryBuilder) -> LibraryBuilder) -> Library {
    let mut lib = builder(Library::builder()).build();
    let scope = lib.global.scope_mut();

    scope.define("test", test_module());
    define_prelude(scope);

    lib
}

#[func]
fn catch(engine: &mut Engine, context: Tracked<Context>, func: Func) -> Value {
    func.call::<[Value; 0]>(engine, context, [])
        .map(|_| Value::None)
        .unwrap_or_else(|errors| {
            Value::Str(Str::from(
                errors
                    .first()
                    .expect("should contain at least one diagnostic")
                    .message
                    .clone(),
            ))
        })
}

#[func]
fn assert_panic(
    engine: &mut Engine,
    context: Tracked<Context>,
    func: Func,
    #[named] message: Option<EcoString>,
) -> SourceResult<()> {
    let result = func.call::<[Value; 0]>(engine, context, []);
    let span = func.span();
    if let Ok(val) = result {
        match message {
            Some(message) => bail!(span, "{}", message),
            None => match val {
                Value::None => bail!(span, "Expected panic, closure returned successfully"),
                _ => bail!(
                    span,
                    "Expected panic, closure returned successfully with {}",
                    val.repr(),
                ),
            },
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use typst::World;
    use typst::diag::FileError;
    use typst::diag::FileResult;
    use typst::foundations::Bytes;
    use typst::foundations::Datetime;
    use typst::layout::PagedDocument;
    use typst::syntax::FileId;
    use typst::syntax::Source;
    use typst::text::Font;
    use typst::text::FontBook;
    use typst::utils::LazyHash;

    use super::*;

    static AUGMENTED_LIBRARY: LazyLock<LazyHash<Library>> =
        LazyLock::new(|| LazyHash::new(augmented_default_library()));

    static FONTS: LazyLock<Vec<Font>> = LazyLock::new(|| {
        typst_assets::fonts()
            .enumerate()
            .map(|(index, bytes)| {
                let data = Bytes::new(bytes);
                Font::new(data, index.try_into().unwrap()).unwrap()
            })
            .collect()
    });

    static FONT_BOOK: LazyLock<LazyHash<FontBook>> =
        LazyLock::new(|| LazyHash::new(FontBook::from_fonts(&*FONTS)));

    struct TestWorld(Source);

    impl World for TestWorld {
        fn library(&self) -> &LazyHash<Library> {
            &AUGMENTED_LIBRARY
        }

        fn book(&self) -> &LazyHash<FontBook> {
            &FONT_BOOK
        }

        fn main(&self) -> FileId {
            self.0.id()
        }

        fn source(&self, id: FileId) -> FileResult<Source> {
            if id == self.0.id() {
                Ok(self.0.clone())
            } else {
                Err(FileError::NotFound(
                    id.vpath().as_rooted_path().to_path_buf(),
                ))
            }
        }

        fn file(&self, id: FileId) -> FileResult<Bytes> {
            if id == self.0.id() {
                Ok(Bytes::new(self.0.text().as_bytes().to_vec()))
            } else {
                Err(FileError::NotFound(
                    id.vpath().as_rooted_path().to_path_buf(),
                ))
            }
        }

        fn font(&self, index: usize) -> Option<Font> {
            FONTS.get(index).cloned()
        }

        fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
            unimplemented!("not used")
        }
    }

    #[test]
    fn test_catch() {
        let world = TestWorld(Source::detached(
            r#"
            #let error = catch(() => {
                panic()
            })
            #assert.eq(error, "panicked")
        "#,
        ));

        typst::compile::<PagedDocument>(&world).output.unwrap();
    }

    #[test]
    fn test_assert_panic() {
        let world = TestWorld(Source::detached(
            r#"
            #assert-panic(() => {
                panic()
            })
        "#,
        ));

        typst::compile::<PagedDocument>(&world).output.unwrap();
    }
}
