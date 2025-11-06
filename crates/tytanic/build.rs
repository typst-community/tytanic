use std::{borrow::Cow, process::Command};

fn main() {
    println!("cargo:rustc-env=TYTANIC_VERSION={}", tytanic_version());
    println!(
        "cargo:rustc-env=TYTANIC_COMMIT_SHA={}",
        tytanic_commit_sha()
    );
    println!(
        "cargo:rustc-env=TYTANIC_TYPST_VERSION={}",
        tytanic_typst_version()
    );
}

/// Retrieves the tytanic version.
///
/// First checks if the "TYTANIC_VERSION" environment variable is set
/// and returns its value if available.
/// Otherwise, falls back to the package version defined in "CARGO_PKG_VERSION".
fn tytanic_version() -> &'static str {
    if let Some(version) = option_env!("TYTANIC_VERSION") {
        return version;
    }

    env!("CARGO_PKG_VERSION")
}

/// Retrieves the commit sha of the current commit.
///
/// First checks if the "TYTANIC_COMMIT_SHA" environment variable is set
/// and returns its value if available.
/// Otherwise, queries git to get the current commit SHA, or returns "unknown hash" on failure.
fn tytanic_commit_sha() -> Cow<'static, str> {
    if let Some(sha) = option_env!("TYTANIC_COMMIT_SHA") {
        return Cow::Borrowed(sha);
    }

    let git_sha = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout.get(..8)?.into()).ok());

    if let Some(sha) = git_sha {
        return Cow::Owned(sha);
    }

    Cow::Borrowed("unknown hash")
}

/// Retrieves the typst version used in current config.
///
/// First checks if the "TYTANIC_TYPST_VERSION" environment variable is set
/// and returns its value if available.
/// Otherwise, queries cargo to get the current version, or returns "unknown commit" on failure.
fn tytanic_typst_version() -> Cow<'static, str> {
    if let Some(version) = option_env!("TYTANIC_TYPST_VERSION") {
        return Cow::Borrowed(version);
    }

    let cargo_version = Command::new("cargo")
        .args(["tree", "-p", "typst", "--depth", "0"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|output| output.split('v').nth(1).map(str::to_string));

    if let Some(version) = cargo_version {
        return Cow::Owned(version);
    }

    Cow::Borrowed("unknown version")
}
