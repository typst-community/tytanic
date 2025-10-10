//! Common report PODs for stable JSON representation of internal entities.

use std::path::PathBuf;

use serde::Serialize;
use typst_syntax::package::PackageManifest;
use typst_syntax::package::PackageVersion;
use tytanic_core::TemplateTest;
use tytanic_core::UnitTest;
use tytanic_core::project::Project;
use tytanic_core::suite::Suite;
use tytanic_core::test::Test;

#[derive(Debug, Serialize)]
pub struct ProjectJson<'m, 's> {
    pub package: Option<PackageJson<'m>>,
    pub vcs: Option<String>,
    pub tests: Vec<UnitTestJson<'s>>,
    pub template_test: Option<TemplateTestJson<'s>>,
}

impl<'m, 's> ProjectJson<'m, 's> {
    pub fn new(project: &Project, manifest: Option<&'m PackageManifest>, suite: &'s Suite) -> Self {
        Self {
            package: manifest.map(|m| PackageJson {
                name: &m.package.name,
                version: &m.package.version,
            }),
            vcs: project.vcs().map(|vcs| vcs.to_string()),
            tests: suite
                .unit_tests()
                .map(|test| UnitTestJson::new(project, test))
                .collect(),
            template_test: suite
                .template_test()
                .map(|test| TemplateTestJson::new(project, test)),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PackageJson<'p> {
    pub name: &'p str,
    pub version: &'p PackageVersion,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "test")]
pub enum TestJson<'t> {
    #[serde(rename = "unit")]
    Unit(UnitTestJson<'t>),

    #[serde(rename = "template")]
    Template(TemplateTestJson<'t>),
}

impl<'t> TestJson<'t> {
    pub fn new(project: &Project, test: &'t Test) -> Self {
        match test {
            Test::Unit(test) => Self::Unit(UnitTestJson::new(project, test)),
            Test::Template(test) => Self::Template(TemplateTestJson::new(project, test)),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct UnitTestJson<'t> {
    pub id: &'t str,
    pub kind: &'static str,
    pub is_skip: bool,
    pub path: PathBuf,
}

impl<'t> UnitTestJson<'t> {
    pub fn new(project: &Project, test: &'t UnitTest) -> Self {
        Self {
            id: test.id().as_str(),
            kind: test.kind().as_str(),
            is_skip: test.is_skip(),
            path: project.unit_test_dir(test.id()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TemplateTestJson<'t> {
    pub id: &'t str,
    pub path: PathBuf,
}

impl<'t> TemplateTestJson<'t> {
    pub fn new(project: &Project, test: &'t TemplateTest) -> Self {
        Self {
            id: test.id().as_str(),
            path: project.template_root().unwrap(),
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
