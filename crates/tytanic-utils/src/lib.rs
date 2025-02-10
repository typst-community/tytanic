//! A utility library for tytanic and its internal crates.
//!
//! This library makes _*no stability guarantees*_ at the moment and likely
//! won't ever.

pub mod fmt;
pub mod fs;
pub mod path;
pub mod result;

/// Re-exports of useful traits and types.
pub mod prelude {
    pub use result::ResultEx;

    use super::*;
}

mod private {
    pub(crate) trait Sealed {}

    impl<T, E> Sealed for Result<T, E> {}
}
