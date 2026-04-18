use std::path::Path;

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../Cargo.toml");
    let content = std::fs::read_to_string(&manifest).expect("workspace Cargo.toml");
    let table: toml::Table = content.parse().expect("valid TOML");

    let version = table["workspace"]["dependencies"]["typst"]
        .as_str()
        .expect("typst version string");

    println!("cargo::rerun-if-changed=../../Cargo.toml");
    println!("cargo::rustc-env=TYPST_VERSION={version}");
}
