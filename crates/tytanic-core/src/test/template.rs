use std::fs;
use std::io;

use typst::syntax::FileId;
use typst::syntax::RootedPath;
use typst::syntax::Source;
use typst::syntax::VirtualPath;
use typst::syntax::VirtualRoot;

use super::Id;
use crate::project::Project;

/// A compile-only template test.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Test {
    id: Id,
}

impl Test {
    pub fn load(project: &Project) -> Option<Self> {
        if project.template_entrypoint().is_some() {
            return Some(Self { id: Id::template() });
        }

        None
    }
}

impl Test {
    pub fn id(&self) -> &Id {
        &self.id
    }
}

impl Test {
    /// Loads the test script source of this test.
    #[tracing::instrument(skip(project))]
    pub fn load_source(&self, project: &Project) -> io::Result<Source> {
        let test_script = project
            .template_entrypoint()
            .expect("Existence of template test ensures existence of entrypoint");

        Ok(Source::new(
            FileId::new(RootedPath::new(
                VirtualRoot::Project,
                VirtualPath::virtualize(project.root().as_std_path(), test_script.as_std_path())
                    .expect("Project::root and Test::templat_entrypoint must never emit escaping or invalid paths"),
            )),
            fs::read_to_string(test_script)?,
        ))
    }
}
