# Architecture
Tytanic is split into three libraries and one binrary crate.
The three library crates are split for various different reasons.

> [!note]
> Note that, because Tytanic is on version 0.x.y, any version may introduce breaking changes, as per the SemVer specification.
> But the project tries to keep such breaking changes at a minimum.

# Binary crate: `tytanic`
The `tytanic` crate is the default crate of the cargo workspace and compiles to the `tt` binary.
It contains the various CLI specific implementations, like argument parsing and terminal UI formatting.
At the moment, it also also contains the test runner logic.
This will eventually be moved into the core library crate or it's own runner crate.

# Library crate: `tytanic-core`
This crate contains most of the core types and implementations useful for managing Tytanic tests.
The core crate will eventually contain all necessary features for loading, running and managing tests, such that a non-CLI downstream consumer can use it.
It may be further split up as features are added.
This crate does not yet make SemVer guarantees, but breaking changes are kept at a reasonable minimum.

# Library crate: `tytanic-filter`
The filter crate contains the parsing and evaluation of test set expressions, generic over their test type.
This was primarily moved out of the core crate to re-use it outside of Tytanic.
This crate does not yet make SemVer guarnatees, but breaking changes are kept at a reasonable minimum.

# Library crate: `tytanic-utils`
The utils crate contains helpers which were used in both the core library and the binary.
Keeping them as a `#[doc(hidden)]` module inside the core crate simply made using them inside the binary too cumbersome.
This crate makes no SemVer guarantees at all, and should be treated like a private module.
Most of the types and functions are for convenience in handling filesystem access, but also contains formatting and unit testing helpers.
