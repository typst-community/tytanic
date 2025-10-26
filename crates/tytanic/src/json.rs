//! Common report PODs for stable JSON representation of internal entities.

use std::path::PathBuf;

use serde::Serialize;
use typst_syntax::package::PackageManifest;
use typst_syntax::package::PackageVersion;
use tytanic_core::project::ProjectContext;
use tytanic_core::suite::Suite;
use tytanic_core::test::TemplateTest;
use tytanic_core::test::Test;
use tytanic_core::test::UnitTest;

#[derive(Debug, Serialize)]
pub struct ProjectJson<'m, 's> {
    pub package: Option<PackageJson<'m>>,
    pub vcs: Option<String>,
    pub template_test: Option<TemplateTestJson<'s>>,
    pub unit_tests: Vec<UnitTestJson<'s>>,
}

impl<'m, 's> ProjectJson<'m, 's> {
    pub fn new(
        project: &ProjectContext,
        manifest: Option<&'m PackageManifest>,
        suite: &'s Suite,
    ) -> Self {
        Self {
            package: manifest.map(|m| PackageJson {
                name: &m.package.name,
                version: &m.package.version,
            }),
            vcs: project.vcs().map(|vcs| vcs.to_string()),
            template_test: suite
                .template_test()
                .map(|test| TemplateTestJson::new(project, test)),
            unit_tests: suite
                .unit_tests()
                .map(|test| UnitTestJson::new(project, test))
                .collect(),
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
    #[serde(rename = "template")]
    Template(TemplateTestJson<'t>),

    #[serde(rename = "unit")]
    Unit(UnitTestJson<'t>),
}

impl<'t> TestJson<'t> {
    pub fn new(project: &ProjectContext, test: &'t Test) -> Self {
        match test {
            Test::Template(test) => Self::Template(TemplateTestJson::new(project, test)),
            Test::Unit(test) => Self::Unit(UnitTestJson::new(project, test)),
            Test::Doc(_) => todo!(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TemplateTestJson<'t> {
    pub id: &'t str,
    pub path: PathBuf,
}

impl<'t> TemplateTestJson<'t> {
    pub fn new(project: &ProjectContext, test: &'t TemplateTest) -> Self {
        Self {
            id: test.ident().as_str(),
            path: project.manifest().unwrap(),
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
    pub fn new(project: &ProjectContext, test: &'t UnitTest) -> Self {
        Self {
            id: test.ident().as_str(),
            kind: test.kind().as_str(),
            is_skip: test.is_skip(),
            path: project.store().unit_test_src_path(test.ident()),
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
