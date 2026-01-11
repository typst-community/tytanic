# Development Setup
This guide covers setting up a development environment for contributing to Tytanic.

## Prerequisites
This section only lists the prerequisites, the next section goes into detail on how to set these up for common systems.

### Required
Depending on what you want to work on you need to install some tools and libraries.

To work on Tytanic itself you'll need to install a Rust stable and a nightly toolchain.
The stable toolchain must be least the version specified in `Cargo.toml` and the nightly toolchain can be any version higher than that.
At the time of writing the MSRV (minimum supported Rust version) is Rust 1.89, but you may need a newer version by the time you read this.
The nightly toolchain is only used for formatting features which aren't stable yet.

> [!note]
> The nix devShell doesn't currently provide a nightly toolchain, you can install it outside of the devShell to get nightly formatting.
> Most formatting for which nightly is required is doable by hand though such one line per use and use grouping.

If you want to contribute to the documentation you need to install [mdBook].

### Optional
While these are optional, it is recommended to also install the following additional tools:
- [just] for running common tasks defined in the `Justfile`.
- [cargo-insta] for easier handling and better review of snapshot tests.
- [cargo-nextest] for faster test execution.

All of these can be installed with `cargo install` or [cargo-binstall].

## Setup
### Using Nix (Linux/macOS)
Tytanic provides a flake with a devShell, you can enter the repository, activate the devShell and start working.
It is recommended to start your editor from the devShell to ensure it uses the right version of rust-analyzer.

```bash
# Enter the repository.
cd tytanic

# Activate the devshell.
nix develop

# Start building tytanic.
cargo build
```

> [!note]
> The devShell does not currently provide the optional prerequisites.

### Plain Rust (Windows/Linux/macOS)
If this is your first time using Rust, take a look at the [Rust Book][rust-book], it has a comprehensive guide on setting up a Rust development environment.
Note that if you use Linux, some distros provide [rustup] as a package, so the curl script shown in the book may not be necessary.

#### Linux/macOS
Install the [OpenSSL] library and [pkg-config] using a package manager, you may need to install a dev package to ensure the headers are included.

## Common Tasks
The `Justfile` provides recipes for common operations, you can list them by running `just` without any arguments.
The just recipes assume you are have cargo-insta and cargo-nextest installed.

```shell
just           # List all available tasks
just run       # Build and run (release mode)
just test      # Run tests with snapshot updates
just check     # Run lints (fmt, clippy, mdbook)
just ci        # Run full CI locally
just book      # Build and serve documentation
just install   # Install tytanic locally
```

### Running Tests
To run the test suite use either `cargo test` or `cargo nextest`, the following examples use cargo-nextest and cargo-insta.

```shell
# Run all tests with colorful snapshot diffs.
cargo insta test --test-runner nextest --workspace
```

You can run `just test` to do this.

### Checking Code Quality
Before submitting a PR, ensure your code is well formatted and passes all checks.

```shell
# Check formatting (might require running outside the devShell on nix).
cargo +nightly fmt --all --check

# Check all lints.
cargo clippy --workspace --all-targets --all-features

# Check documentation builds
cargo doc --workspace --no-deps
```

You can run `just ci` to run a portion of CI locally.

### Building Documentation
To build the documentation simply run mdBook and check the output.

```shell
# Run this from the repo root.
mdbook serve docs/book --open
```

You can run `just book` to do this anywhere.

## IDE
No special configuration should be needed, but you may configure [rust-analyzer] to use clippy for its check command right away.

Here's how that would look like for [VS Code] (saved as `$repo/.vscode/settings.json`):
```json
{
  "rust-analyzer.check.command": "clippy",
}
```

## Project Layout
The project is laid out as follows:
- All Rust crates are placed in `crates`.
  Familiarize yourself with the crate structure by taking a look at the [architecture.md] documentation.
- Additional assets are placed in `assets`.
- Documentation is placed in `docs`.
  `docs` contains both contributor documentation (like this document) as well as user documentation as an mdBook in `docs/book`.

## Troubleshooting
### OpenSSL Build Errors
If you see errors about OpenSSL during build it means you likely don't have the right version (or any) of OpenSSL.
Cargo may report the following error:
```
error: failed to run custom build command for `openssl-sys`
```

Another error could be that it requires `pkg-config` to discover OpenSSL, just like with OpenSSL itself this probably means you didn't install pkg-config.

If you have trouble installing it, consider using the `vendor-openssl` feature:
```shell
cargo build --features vendor-openssl
```

### Rust Version Too Old
If cargo says that your rust version is too old, update the toolchain using rustup:
```shell
rustup update
```

### Snapshot Tests failing
If tests fail due to snapshot differences after changing the output of Tytanic make sure to update the snapshots or the code accordingly:
```shell
# Review and accept/reject changes.
cargo insta review
```

## Contributing
Assuming you're setting up your dev environment to contribute, take a look at [CONTRIBUTING.md].
If you don't know what you want to work on yet, take a look at issues labeled with [good first issue].

[CONTRIBUTING.md]: ./CONTRIBUTING.md
[OpenSSL]: https://repology.org/project/openssl/versions
[VS Code]: https://code.visualstudio.com/download
[architecture.md]: ./architecture.md
[cargo-binstall]: https://github.com/cargo-bins/cargo-binstall
[cargo-insta]: https://insta.rs/
[cargo-nextest]: https://nexte.st/
[good first issue]: https://github.com/typst-community/tytanic/issues?q=is%3Aissue%20state%3Aopen%20label%3A%22good%20first%20issue%22
[just]: https://github.com/casey/just
[mdBook]: https://rust-lang.github.io/mdBook/
[pkg-config]: https://repology.org/project/pkg-config/versions
[rust-analyzer]: https://rust-analyzer.github.io/
[rust-book]: https://doc.rust-lang.org/stable/book/ch01-01-installation.html
[rustup]: https://rustup.rs/
