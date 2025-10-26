//! # `tytanic-utils`
//! A utility crate for Tytanic.
//!
//! This crate makes _*no stability guarantees*_ at the moment and likely
//! won't ever. Some parts of the crate may be moved out and stabilized
//! elsewhere across its lifetime.

pub mod assert;
pub mod fmt;
pub mod fs;
pub mod path;
pub mod result;
#[cfg(any(feature = "typst-manifest-builder", feature = "typst-world-builder"))]
pub mod typst;
#[cfg(feature = "terminal-ui")]
pub mod ui;

/// Re-exports of useful traits and types.
pub mod prelude {
    pub use result::ResultEx;

    use super::*;
}

mod private {
    pub(crate) trait Sealed {}

    impl<T, E> Sealed for Result<T, E> {}
}

/// A macro to forward trait definitions to trait objects and references.
///
/// `where`-bounds are not supported.
///
/// # Examples
/// ```
/// # use std::sync::Arc;
/// # use tytanic_utils::forward_trait;
/// pub trait Foo {
///     fn bar(&self);
///     fn qux(&self);
/// }
///
/// // Implements Foo for `&F`, `Box<F>`, and `Arc<F>` by deferring to the inner
/// // `F` implementation.
/// forward_trait! {
///     impl<F> Foo for [Box<F>, Arc<F>, &F] {
///         fn bar(&self) {
///             F::bar(self)
///         }
///
///         fn qux(&self) {
///             F::qux(self)
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! forward_trait {
    (impl<$pointee:ident> $trait:ident for [$($pointer:ty),+] $funcs:tt) => {
        $(impl<$pointee: ?Sized + $trait> $trait for $pointer $funcs)+
    };
}
