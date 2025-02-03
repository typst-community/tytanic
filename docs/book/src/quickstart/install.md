# Installation
## Versions
You can either install a stable version or a nightly version, a stable version uses a version tag like `v0.1.1`, whereas nightly versions are simply whatever is currently pointed to by the `main` branch on the GitHub repository.

Nightly has the newest features, but may have unfixed bugs or rough edges, use this with caution and backup your tests.

Once installed you will have a `tt` binary available, make sure to have a look at [Dependencies](#dependencies) if running `tytanic` spits out some error about dynamic libraries.

## Methods
### Download from GitHub
You can download pre-built binaries of all stable versions from the [release page][releases] of the GitHub repository, these are automatically built for Linux, macOS and Windows.
Nightly versions are not pre-built.

After you downloaded the correct archive for your operating system and architecture you have to extract them and place the `tt` binary somewhere in your `$PATH`.

### Using cargo-binstall
The most straight forward way to install `tytanic` is to use `cargo-binstall`, this saves you the hassle of compiling from source:
```shell
cargo binstall tytanic
```

This methods requires `cargo-binstall` to be installed.

<div class="warning">

Installing via `cargo-binstall` will not work for versions `v0.1.1` or earlier.

You can use one of the other installation methods for those versions.

</div>

### Installation From Source
To install `tytanic` from source, you must have a Rust toolchain (Rust **v1.80.0+**) and `cargo` installed, you can get these using [`rustup`][rustup].

#### Stable
```shell
cargo install --locked tytanic@v0.1.1
```

#### Nightly
```shell
cargo install --locked --git https://github.com/tingerrr/tytanic
```

This method usually doesn't require manually placing the `tt` binary in your `$PATH` because the cargo binary directory should already be in there.

### Nix Flake
#### Stable
```shell
nix run github:tingerrr/tytanic/v0.1.1
```

#### Nightly
```shell
nix run github:tingerrr/tytanic
```

This method does't require any extraction or `$PATH` modifications.

## Dependencies
The following dependencies are required for running `tytanic`, though they are widely used and should in most cases already be installed if you used `typst` before.
`tytanic` tries to provide feature flags for vendoring dependencies where possible.

### OpenSSL
OpenSSL (**v1.0.1** to **v3.x.x**) or LibreSSL (**v2.5** to **v3.7.x**) are required to allow `tytanic` to download packages from the [Typst Universe][universe] package registry.

When installing from source the `vendor-openssl` feature can be used on unix-like operating systems to vendor OpenSSL.
This avoids the need for it on the operating system.

[releases]: https://github.com/tingerrr/tytanic/releases/
[rustup]: https://www.rust-lang.org/tools/install
[universe]: https://typst.app/universe
