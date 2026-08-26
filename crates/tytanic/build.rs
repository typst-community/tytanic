use std::borrow::Cow;
use std::process::Command;

fn main() {
    println!("cargo:rustc-env=TYTANIC_VERSION={}", tytanic_version());
    if let Some(tytanic_commit_sha) = tytanic_commit_sha() {
        println!("cargo:rustc-env=TYTANIC_COMMIT_SHA={}", tytanic_commit_sha);
    }
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
/// Otherwise, queries git to get the current commit SHA, or returns None on failure.
fn tytanic_commit_sha() -> Option<Cow<'static, str>> {
    if let Some(sha) = option_env!("TYTANIC_COMMIT_SHA") {
        return Some(Cow::Borrowed(sha));
    }

    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(Cow::Owned)
}
