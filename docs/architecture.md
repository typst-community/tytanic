# Architecture
Tytanic is split into multiple libraries and one binary crate.
The library crates are split for various different reasons.

> [!note]
> Note that, because Tytanic is on version 0.x.y, any version may introduce breaking changes, as per the SemVer specification.
> But the project tries to keep such breaking changes at a minimum.

# Binary crate: `tytanic`
The `tytanic` crate is the default crate of the cargo workspace and compiles to the `tt` binary.
It contains the various CLI specific implementations, like argument parsing and terminal UI formatting and glue code connecting the various library crates.

# Library crate: `tytanic-core`
This crate contains most of the core types and basic implementations for using the other crates.
This crate does not yet make SemVer guarantees, but breaking changes are kept at a reasonable minimum.

# Library crate: `tytanic-filter`
The filter crate contains the parsing and evaluation of test set expressions, generic over their test type.
This was primarily moved out of the core crate to re-use it outside of Tytanic and was generic over the test type.
However, it is now tied to Tytanic entirely.
This crate does not yet make SemVer guarantees, but breaking changes are kept at a reasonable minimum.

# Library crate: `tytanic-runner`
This contains a default runner implementation used in the Tytanic CLI as well as traits and default implementations for this runner to work in other projects too.
This crate does not yet make SemVer guarantees, but breaking changes are kept at a reasonable minimum.

# Library crate: `tytanic-library`
The library crate contains the augmented standard library available to unit tests and is largely independent of the rest of the crates.
This crate does not yet make SemVer guarantees, but breaking changes are kept at a reasonable minimum.

# Library crate: `tytanic-utils`
The utils crate contains helpers which were used in both the core library and the binary.
Keeping them as a `#[doc(hidden)]` module inside the core crate simply made using them inside the binary too cumbersome.
This crate makes no SemVer guarantees at all, and should be treated like a private module.
Most of the types and functions are for convenience in handling filesystem access, but also contains formatting and unit testing helpers.
