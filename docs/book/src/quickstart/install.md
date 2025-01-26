# Installation
To install `tytanic` on your PC, you must, for the time being, compile it from source.
Once `tytanic` reaches 0.1.0, this restriction will be lifted and each release will provide precompiled binaries for major operating systems (Windows, Linux and macOS).

## Installation From Source
To install `tytanic` from source, you must have a Rust toolchain (Rust **v1.80.0+**) and cargo installed.

To install the latest stable release version run:
```bash
cargo install --locked tytanic
```

If you want to install the latest nightly version, you can run:
```bash
cargo install --locked --git https://github.com/tingerrr/tytanic
```
This version has the newest features, but may have unfixed bugs or rough edges.

## Required Libraries
### OpenSSL
OpenSSL (**v1.0.1** to **v3.x.x**) or LibreSSL (**v2.5** to **v3.7.x**) are required to allow `tytanic` to download packages from the [Typst Universe](https://typst.app/universe) package registry.

When installing from source the `vendor-openssl` feature can be used on operating systems other than Windows and macOS to  vendor and statically link to OpenSSL, avoiding the need for it on the operating system.
