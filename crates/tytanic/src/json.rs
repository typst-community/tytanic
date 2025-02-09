//! Common report PODs for stable JSON representation of internal entities.

use serde::Serialize;
use typst_syntax::package::{PackageManifest, PackageVersion};
use tytanic_core::project::Project;
use tytanic_core::suite::Suite;
use tytanic_core::test::Test;

#[derive(Debug, Serialize)]
pub struct ProjectJson<'m, 's> {
    pub package: Option<PackageJson<'m>>,
    pub vcs: Option<String>,
    pub tests: Vec<TestJson<'s>>,
    pub is_template: bool,
}

impl<'m, 's> ProjectJson<'m, 's> {
    pub fn new(project: &Project, manifest: Option<&'m PackageManifest>, suite: &'s Suite) -> Self {
        Self {
            package: manifest.map(|m| PackageJson {
                name: &m.package.name,
                version: &m.package.version,
            }),
            vcs: project.vcs().map(|vcs| vcs.to_string()),
            tests: suite.tests().values().map(TestJson::new).collect(),
            is_template: manifest.and_then(|m| m.template.as_ref()).is_some(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PackageJson<'p> {
    pub name: &'p str,
    pub version: &'p PackageVersion,
}

#[derive(Debug, Serialize)]
pub struct TestJson<'t> {
    pub id: &'t str,
    pub kind: &'static str,
}

impl<'t> TestJson<'t> {
    pub fn new(test: &'t Test) -> Self {
        Self {
            id: test.id().as_str(),
            kind: test.kind().as_str(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct FontVariantJson {
    pub weight: u16,
    pub style: &'static str,
    pub stretch: f64,
}

#[derive(Debug, Serialize)]
pub struct FontJson<'f> {
    pub name: &'f str,
    pub variants: Vec<FontVariantJson>,
}

#[derive(Serialize)]
pub struct FailedJson {
    pub compilation: usize,
    pub comparison: usize,
    pub otherwise: usize,
}

#[derive(Serialize)]
pub struct DurationJson {
    pub seconds: u64,
    pub nanoseconds: u32,
}
