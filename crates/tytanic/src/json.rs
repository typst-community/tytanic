//! Common report PODs for stable JSON representation of internal entities.

use camino::Utf8PathBuf;
use serde::Serialize;
use typst_syntax::package::PackageManifest;
use typst_syntax::package::PackageVersion;
use tytanic_core::TemplateTest;
use tytanic_core::UnitTest;
use tytanic_core::project::Project;
use tytanic_core::suite::Suite;
use tytanic_core::test::DocId;
use tytanic_core::test::DocTest;
use tytanic_core::test::TemplateId;
use tytanic_core::test::TestRef;
use tytanic_core::test::UnitId;

#[derive(Debug, Serialize)]
pub struct ProjectJson<'m, 's> {
    pub package: Option<PackageJson<'m>>,
    pub vcs: Option<String>,
    pub unit_tests: Vec<UnitTestJson<'s>>,
    pub doc_tests: Vec<DocTestJson<'s>>,
    pub template_test: Option<TemplateTestJson<'s>>,
}

impl<'m, 's> ProjectJson<'m, 's> {
    pub fn new(project: &Project, manifest: Option<&'m PackageManifest>, suite: &'s Suite) -> Self {
        Self {
            package: manifest.map(|m| PackageJson {
                name: &m.package.name,
                version: &m.package.version,
            }),
            vcs: project.vcs().map(|vcs| vcs.kind().to_string()),
            unit_tests: suite
                .unit_tests()
                .map(|test| UnitTestJson::new(project, test))
                .collect(),
            doc_tests: suite
                .doc_tests()
                .map(|test| DocTestJson::new(project, test))
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
pub enum TestJson<'s> {
    #[serde(rename = "unit")]
    Unit(UnitTestJson<'s>),

    #[serde(rename = "template")]
    Template(TemplateTestJson<'s>),

    #[serde(rename = "doc")]
    Doc(DocTestJson<'s>),
}

impl<'s> TestJson<'s> {
    pub fn new(project: &Project, test: TestRef<'s>) -> Self {
        match test {
            TestRef::Unit(test) => Self::Unit(UnitTestJson::new(project, test)),
            TestRef::Template(test) => Self::Template(TemplateTestJson::new(project, test)),
            TestRef::Doc(test) => Self::Doc(DocTestJson::new(project, test)),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct UnitTestJson<'s> {
    pub id: &'s UnitId,
    pub kind: &'static str,
    pub is_skip: bool,
    pub path: Utf8PathBuf,
}

impl<'s> UnitTestJson<'s> {
    pub fn new(project: &Project, test: &'s UnitTest) -> Self {
        Self {
            id: test.id(),
            kind: test.kind().as_str(),
            is_skip: test.is_skip(),
            path: project.unit_test_dir(test.id()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TemplateTestJson<'s> {
    pub id: &'s TemplateId,
    pub path: Utf8PathBuf,
}

impl<'s> TemplateTestJson<'s> {
    pub fn new(project: &Project, test: &'s TemplateTest) -> Self {
        Self {
            id: test.id(),
            path: project.template_root().unwrap(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DocTestJson<'s> {
    pub id: &'s DocId,
    pub path: Utf8PathBuf,
}

impl<'s> DocTestJson<'s> {
    pub fn new(project: &Project, test: &'s DocTest) -> Self {
        Self {
            id: test.id(),
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
