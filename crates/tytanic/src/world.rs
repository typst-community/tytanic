// SPDX-License-Identifier: Apache-2.0
// Credits: The Typst Authors

#![allow(dead_code)]

// TODO(tinger): Upstream this to typst-kit.

use std::path::PathBuf;

use chrono::Datelike;
use chrono::Timelike;
use color_eyre::eyre;
use typst::Library;
use typst::LibraryExt;
use typst::World;
use typst::diag::FileResult;
use typst::foundations::Bytes;
use typst::foundations::Datetime;
use typst::foundations::Dict;
use typst::foundations::Duration;
use typst::syntax::FileId;
use typst::text::Font;
use typst::text::FontBook;
use typst::utils::LazyHash;
use typst_kit::datetime::Time;
use typst_kit::diagnostics::DiagnosticWorld;
use typst_kit::downloader::SystemDownloader;
use typst_kit::files::FsRoot;
use typst_kit::fonts;
use typst_kit::fonts::FontStore;
use typst_kit::packages::FsPackages;
use typst_kit::packages::SystemPackages;
use typst_kit::packages::UniversePackages;
use typst_syntax::Source;
use typst_syntax::package::PackageSpec;
use tytanic_core::Project;
use tytanic_core::TemplateTest;
use tytanic_core::UnitTest;
use tytanic_core::library::augmented_default_library;
use tytanic_core::library::augmented_library;
use tytanic_core::world_builder::ComposedWorld;
use tytanic_core::world_builder::ProvideFile;
use tytanic_core::world_builder::ProvideFont;
use tytanic_core::world_builder::file::FilesystemFileProvider;

use crate::cli::commands::CompileOptions;
use crate::cli::commands::FontOptions;
use crate::cli::commands::PackageOptions;
use crate::cli::commands::Switch;

#[tracing::instrument]
fn package_storage(package_opts: &PackageOptions) -> SystemPackages {
    let agent = format!("{}/{}", tytanic_core::TOOL_NAME, env!("CARGO_PKG_VERSION"));

    let downloader = match package_opts.certificate.clone() {
        Some(path) => SystemDownloader::with_cert_path(agent, path),
        None => SystemDownloader::new(agent),
    };
    let package_path = match &package_opts.package_path {
        Some(package_path) => Some(FsPackages::new(package_path)),
        None => FsPackages::system_data(),
    };
    let package_cache_path = match &package_opts.package_cache_path {
        Some(package_cache_path) => Some(FsPackages::new(package_cache_path)),
        None => FsPackages::system_cache(),
    };

    SystemPackages::from_parts(
        package_path,
        package_cache_path,
        UniversePackages::new(downloader),
    )
}

/// A file provider which is rooted at a project's root and provides access to
/// all files in that project as well as access to packages on demand.
#[tracing::instrument(skip(project))]
pub fn project_file_provider(
    project: &Project,
    package_opts: &PackageOptions,
) -> Box<dyn ProvideFile> {
    Box::new(FilesystemFileProvider::new(
        project.root(),
        Some(package_storage(package_opts)),
    )) as _
}

/// Provides access as if in a freshly created template from the given template
/// package project.
///
/// This means that only files within the template root can be accessed, not
/// the whole project. Additionally, imports to packages matching the current
/// template project's version and name in the `preview` namespace are routed
/// to the current package and are subject to the same access rules as a normal
/// package.
///
/// Panics if the project has no manifest.
#[tracing::instrument(skip(project))]
pub fn template_file_provider(
    project: &Project,
    package_opts: &PackageOptions,
) -> Box<dyn ProvideFile> {
    let manifest = project.manifest().unwrap();

    let spec = PackageSpec {
        namespace: "preview".into(),
        name: manifest.package.name.clone(),
        version: manifest.package.version,
    };

    Box::new(FilesystemFileProvider::with_overrides(
        project.template_root().unwrap(),
        [(
            spec,
            FsRoot::new(project.root().as_std_path().to_path_buf()),
        )],
        Some(package_storage(package_opts)),
    ))
}

/// A font provider that provides embedded and system fonts.
#[tracing::instrument]
pub fn font_provider(font_opts: &FontOptions) -> Box<dyn ProvideFont> {
    let mut store = FontStore::new();

    #[cfg(feature = "embedded-fonts")]
    if font_opts.use_embedded_fonts.get_or_default() {
        store.extend(fonts::embedded());
    }

    if font_opts.use_system_fonts.get_or_default() {
        store.extend(fonts::system());
    }

    store.extend(
        font_opts
            .font_paths
            .iter()
            .map(PathBuf::as_path)
            .flat_map(fonts::scan),
    );

    tracing::debug!(fonts = ?store.book().families().count(), "collected font families");
    Box::new(store)
}

/// A datetime provider that provides a fixed date.
#[tracing::instrument]
pub fn datetime_provider(compile_opts: &CompileOptions) -> eyre::Result<Box<Time>> {
    Ok(Box::new(
        Time::fixed(
            Datetime::from_ymd_hms(
                compile_opts.timestamp.year(),
                compile_opts
                    .timestamp
                    .month()
                    .try_into()
                    .expect("DateLike::month must return values in 1..=12"),
                compile_opts
                    .timestamp
                    .day()
                    .try_into()
                    .expect("DateLike::day must return values in 1..=31"),
                compile_opts
                    .timestamp
                    .hour()
                    .try_into()
                    .expect("DateLike::day must return values in 1..=24"),
                compile_opts
                    .timestamp
                    .minute()
                    .try_into()
                    .expect("DateLike::day must return values in 1..=60"),
                compile_opts
                    .timestamp
                    .second()
                    .try_into()
                    .expect("DateLike::day must return values in 1..=60"),
            )
            .ok_or_else(|| {
                eyre::eyre!(
                    "failed to convert timestamp into Typst datetime: {}",
                    compile_opts.timestamp
                )
            })?,
        )
        .map_err(|err| eyre::eyre!("failed to create fixed compilation timestamp: {err}"))?,
    ))
}

/// Provides the augmented library.
#[tracing::instrument]
pub fn augmented_library_provider() -> Box<LazyHash<Library>> {
    Box::new(LazyHash::new(augmented_default_library()))
}

/// Provides the augmented library with additional inputs.
///
/// Inputs are exposed to the world/test via `sys.inputs`.
///
/// See also [`augmented_library_provider`].
#[tracing::instrument]
pub fn augmented_library_provider_with_inputs(inputs: Dict) -> Box<LazyHash<Library>> {
    Box::new(LazyHash::new(augmented_library(|builder| {
        builder.with_inputs(inputs)
    })))
}

/// Provides the default library.
#[tracing::instrument]
pub fn default_library_provider() -> Box<LazyHash<Library>> {
    Box::new(LazyHash::new(Library::default()))
}

/// A set of providers used to construct worlds.
pub struct Providers {
    augmented_library: Box<LazyHash<Library>>,
    default_library: Box<LazyHash<Library>>,
    project_files: Box<dyn ProvideFile>,
    template_files: Option<Box<dyn ProvideFile>>,
    fonts: Box<dyn ProvideFont>,
    datetime: Box<Time>,
}

impl Providers {
    pub fn new(
        project: &Project,
        package_opts: &PackageOptions,
        font_opts: &FontOptions,
        compile_opts: &CompileOptions,
    ) -> eyre::Result<Providers> {
        Ok(Self {
            augmented_library: augmented_library_provider(),
            default_library: default_library_provider(),
            project_files: project_file_provider(project, package_opts),
            template_files: project.manifest().and_then(|m| {
                m.template
                    .is_some()
                    .then(|| template_file_provider(project, package_opts))
            }),
            fonts: font_provider(font_opts),
            datetime: datetime_provider(compile_opts)?,
        })
    }
}

impl Providers {
    /// Constructs a world for unit test creation.
    pub fn system_world(&self, source: Source) -> NewTestWorld<'_> {
        NewTestWorld(
            ComposedWorld::builder()
                .library_provider(&self.augmented_library)
                .file_provider(&*self.project_files)
                .font_provider(&*self.fonts)
                .datetime_provider(&self.datetime)
                .build(source.id()),
            source,
        )
    }

    /// Constructs a world for unit tests.
    ///
    /// The `alternative_library` argument can be assembled by test code to e.g. provide additional
    /// system inputs.
    pub fn unit_world<'w>(
        &'w self,
        project: &Project,
        test: &'w UnitTest,
        is_ref: bool,
        alternative_library: Option<&'w LazyHash<Library>>,
    ) -> ComposedWorld<'w> {
        let id = if is_ref {
            project.unit_test_ref_script_id(test.id(), project)
        } else {
            project.unit_test_script_id(test.id(), project)
        };

        let library = if let Some(library) = alternative_library {
            library
        } else {
            &*self.augmented_library
        };

        ComposedWorld::builder()
            .library_provider(library)
            .file_provider(&*self.project_files)
            .font_provider(&*self.fonts)
            .datetime_provider(&self.datetime)
            .build(id)
    }

    /// Constructs a world for template tests.
    ///
    /// Panics if the project has no manifest.
    pub fn template_world<'w>(
        &'w self,
        project: &Project,
        _test: &'w TemplateTest,
    ) -> ComposedWorld<'w> {
        let id = project
            .template_entrypoint_id()
            .expect("Providers::template_world must not be called without template test");

        ComposedWorld::builder()
            .library_provider(&self.default_library)
            .file_provider(&**self.template_files.as_ref().unwrap())
            .font_provider(&*self.fonts)
            .datetime_provider(&self.datetime)
            .build(id)
    }
}

pub struct NewTestWorld<'w>(ComposedWorld<'w>, Source);

impl World for NewTestWorld<'_> {
    fn library(&self) -> &LazyHash<Library> {
        self.0.library()
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.0.book()
    }

    fn main(&self) -> FileId {
        self.1.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.1.id() {
            Ok(self.1.clone())
        } else {
            self.0.source(id)
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.1.id() {
            Ok(Bytes::new(self.1.text().to_owned()))
        } else {
            self.0.file(id)
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.0.font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.0.today(offset)
    }
}

impl DiagnosticWorld for NewTestWorld<'_> {
    fn name(&self, id: FileId) -> String {
        format!("{id:?}")
    }
}
