# Architecture
Tytanic is split into three libraries and one binrary.
The three library crates are split for various different reasons.

> [!note]
> Note that, because tytanic is on version 0.x.y, any version may introduce breaking changes, as per the SemVer specification.
> But the project tries to keep such breaking changes at a minimum.

# Binary crate: `tytanic`
The `tytanic` crate is the default crate of the cargo workspace and compiles to the `tt` binary.
It contains the various CLI specific implementations like argument parsing and terminal UI formatting.
It currently also contains the test runner logic, this will eventually be moved to the core library crate in some capacity.

# Library crate: `tytanic-core`
This crate contains most of the core types and implementations useful for managing tytanic tests.
The core crate will eventually contain all necessary features for loading, running and managing tests.
It may be further split up as features are added.
This crate does not yet make semver guarnatees, breaking changes are kept at a reasonable minimum.

# Library crate: `tytanic-filter`
The filter crate contains the parsing and evaluation of test set expressions, generic over their test type.
This was primarily moved out of the core crate to re-use it outside of tytanic.
This crate does not yet make semver guarnatees, breaking changes are kept at a reasonable minimum.

# Library crate: `tytanic-utils`
The utils crate contains helpers which were used in both the core library and the binary.
Keeping them as a `#[doc(hidden)]` module inside the core crate simply made using them inside the binary too cumbersome.
This crate makes no semver guarantees at all, and can be treated like a private module.
Most of the types and functions are for convenience in handling filesystem access, but also contains formatting and unit testing helpers.
