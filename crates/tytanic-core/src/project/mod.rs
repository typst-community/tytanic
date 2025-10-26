//! Projects represent the context in which a test suite run is executed.
//!
//! A project is usually identified by its `typst.toml` manifest file, most
//! often in the case of packages.
//!
//! For the purposes of storage access and artifact generation the VCS used by
//! the consumer is also of importance. A user may provide configuration though
//! files or CLI options which should alter the behavior of the test runner.
//!
//! A [`ProjectContext`] bundles this contextual information, such that it can
//! be easily passed around between components of Tytanic.

use std::io;
use std::path::Path;
use std::path::PathBuf;

use store::Kind;
use store::MigrateError;
use thiserror::Error;
use typst_syntax::package::PackageManifest;

use crate::config::ConfigSource;
use crate::config::LayeredConfig;
use crate::config::PartialConfigSource;
use crate::config::ProjectConfig;
use crate::config::ReadError;
use crate::config::ValidationError;
use crate::config::validate_project_path;
use crate::project::store::Store;
use crate::project::vcs::Vcs;
use crate::test::UnitIdent;

pub mod store;
pub mod vcs;

/// The name of the manifest file which is used to discover the project root
/// automatically.
pub const MANIFEST_FILE: &str = "typst.toml";

/// The project context provides information about a config.
#[derive(Debug)]
pub struct ProjectContext {
    root: PathBuf,
    vcs: Option<Vcs>,
    manifest: Option<Box<PackageManifest>>,
    config: Box<LayeredConfig>,
    store: Store,
}

impl ProjectContext {
    /// Creates a project context at from the given start directory.
    ///
    /// This will climb up the directory tree until it finds all necessary root
    /// directories.
    ///
    /// Returns `None` if no project root is found.
    ///
    /// # Errors
    /// May return an error if the PWD or its ancestors are inaccessible.
    ///
    /// # Examples
    /// ```no_run
    /// # use std::path::Path;
    /// # use tytanic_core::config::LayeredConfig;
    /// # use tytanic_core::project::ProjectContext;
    /// # use tytanic_core::project::vcs::Kind;
    /// # use tytanic_core::project::vcs::Vcs;
    /// // From within a sub diretory of /home/user/src/monorepo/my-package
    /// let cwd = std::env::current_dir()?;
    /// let ctx = ProjectContext::discover_project_and_vcs(
    ///     cwd,
    ///     Box::new(LayeredConfig::new()),
    /// )?.unwrap();
    ///
    /// assert_eq!(ctx.root(), Path::new("/home/user/src/monorepo/my-package"));
    /// assert_eq!(ctx.vcs().map(Vcs::kind), Some(Kind::Git));
    /// assert_eq!(
    ///     ctx.vcs().and_then(|vcs| vcs.root()),
    ///     Some(Path::new("/home/user/src/monorepo")),
    /// );
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn discover_project_and_vcs<P>(
        start: P,
        config: Box<LayeredConfig>,
    ) -> Result<Option<Self>, LoadError>
    where
        P: AsRef<Path>,
    {
        fn inner(
            start: &Path,
            config: Box<LayeredConfig>,
        ) -> Result<Option<ProjectContext>, LoadError> {
            let mut project_root = None;
            let mut vcs = None;

            for ancestor in start.ancestors() {
                if project_root.is_none() && ancestor.join(MANIFEST_FILE).try_exists()? {
                    project_root = Some(ancestor.to_path_buf());
                }

                if vcs.is_none() && let Some(kind) = Vcs::try_infer_kind(ancestor)? {
                    vcs = Some(Vcs::new(ancestor, kind));
                }

                if project_root.is_some() && vcs.is_some() {
                    break;
                }
            }

            let Some(project_root) = project_root else {
                return Ok(None);
            };

            ProjectContext::load(project_root, vcs, config).map(Some)
        }

        inner(start.as_ref(), config)
    }

    /// Creates a project config at the given project root and config.
    ///
    /// This will climb up the directory tree until it finds the VCS root
    /// directory.
    ///
    /// The project layer of the config will be set to the values parsed from
    /// the project manifest.
    ///
    /// # Errors
    /// May return an error if the project root or its ancestors are
    /// inaccessible.
    ///
    /// # Examples
    /// ```no_run
    /// # use std::path::Path;
    /// # use tytanic_core::config::LayeredConfig;
    /// # use tytanic_core::project::ProjectContext;
    /// # use tytanic_core::project::vcs::Kind;
    /// # use tytanic_core::project::vcs::Vcs;
    /// let ctx = ProjectContext::discover_vcs(
    ///     "/home/user/src/monorepo/my-package",
    ///     Box::new(LayeredConfig::new()),
    /// )?.unwrap();
    ///
    /// assert_eq!(ctx.root(), Path::new("/home/user/src/monorepo/my-package"));
    /// assert_eq!(ctx.vcs().map(Vcs::kind), Some(Kind::Git));
    /// assert_eq!(
    ///     ctx.vcs().and_then(|vcs| vcs.root()),
    ///     Some(Path::new("/home/user/src/monorepo")),
    /// );
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn discover_vcs<P>(project_root: P, config: Box<LayeredConfig>) -> Result<Self, LoadError>
    where
        P: AsRef<Path>,
    {
        fn inner(
            project_root: &Path,
            config: Box<LayeredConfig>,
        ) -> Result<ProjectContext, LoadError> {
            let mut vcs = None;

            for ancestor in project_root.ancestors() {
                if vcs.is_none() {
                    if let Some(kind) = Vcs::try_infer_kind(ancestor)? {
                        vcs = Some(Vcs::new(ancestor, kind));
                    }
                }

                if vcs.is_some() {
                    break;
                }
            }

            ProjectContext::load(project_root, vcs, config)
        }

        inner(project_root.as_ref(), config)
    }

    /// Loads a project context at the given root.
    ///
    /// The project layer of the config will be set to the values parsed from
    /// the project manifest.
    ///
    /// # Errors
    /// May return an error if the project root or its ancestors are
    /// inaccessible.
    ///
    /// # Examples
    /// ```no_run
    /// # use std::path::Path;
    /// # use tytanic_core::config::LayeredConfig;
    /// # use tytanic_core::project::ProjectContext;
    /// # use tytanic_core::project::vcs::Kind;
    /// # use tytanic_core::project::vcs::Vcs;
    /// let ctx = ProjectContext::load(
    ///     "/home/user/src/monorepo/my-package",
    ///     Vcs::new("/home/user/src/monorepo", Kind::Git),
    ///     Box::new(LayeredConfig::new()),
    /// )?.unwrap();
    ///
    /// assert_eq!(ctx.root(), Path::new("/home/user/src/monorepo/my-package"));
    /// assert_eq!(ctx.vcs().map(Vcs::kind), Some(Kind::Git));
    /// assert_eq!(
    ///     ctx.vcs().and_then(|vcs| vcs.root()),
    ///     Some(Path::new("/home/user/src/monorepo")),
    /// );
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn load<P>(
        project_root: P,
        vcs: Option<Vcs>,
        config: Box<LayeredConfig>,
    ) -> Result<Self, LoadError>
    where
        P: AsRef<Path>,
    {
        fn inner(
            project_root: &Path,
            vcs: Option<Vcs>,
            mut config: Box<LayeredConfig>,
        ) -> Result<ProjectContext, LoadError> {
            let manifest = std::fs::read_to_string(project_root.join(MANIFEST_FILE))?;
            let manifest = toml::from_str(&manifest)
                .map_err(ReadError::Parsing)
                .map_err(LoadError::Manifest)?;

            ProjectContext::validate_manifest(project_root, &manifest)
                .map_err(ReadError::Validation)
                .map_err(LoadError::Manifest)?;

            let project_config = ProjectConfig::from_manifest(&manifest)
                .map_err(ReadError::Parsing)
                .map_err(LoadError::Manifest)?;

            if let Some(project_config) = &project_config {
                ProjectContext::validate_manifest_config(project_root, project_config)
                    .map_err(ReadError::Validation)
                    .map_err(LoadError::Manifest)?;
            }

            // TODO: do we allow settings in the project?
            config.with_project_layer(None, project_config);

            let store = Store::with_config(project_root, &config)?;

            Ok(ProjectContext {
                root: project_root.to_path_buf(),
                vcs,
                manifest: Some(Box::new(manifest)),
                config,
                store,
            })
        }

        inner(project_root.as_ref(), vcs, config)
    }

    /// Creates a new project context.
    pub fn new<P>(
        root: P,
        vcs: Option<Vcs>,
        manifest: Option<Box<PackageManifest>>,
        config: Box<LayeredConfig>,
        store_kind: Kind,
    ) -> Self
    where
        P: Into<PathBuf> + AsRef<Path>,
    {
        debug_assert!(root.as_ref().is_absolute(), "project root must be absolute");

        let store_root = root
            .as_ref()
            .join(config.get_project_config_member(ProjectConfig::STORE_ROOT, ()));

        let template_root = config
            .get_project_config_member(ProjectConfig::TEMPLATE_PATH, ())
            .map(|template_path| root.as_ref().join(template_path));

        let store = Store::new(root.as_ref(), store_root, template_root, store_kind);

        Self {
            root: root.into(),
            vcs,
            manifest,
            config,
            store,
        }
    }

    /// Validates a parsed manifest within a project root.
    pub fn validate_manifest(
        root: &Path,
        manifest: &PackageManifest,
    ) -> Result<(), ValidationError> {
        let PackageManifest {
            package,
            template,
            tool: _,
            unknown_fields: _,
        } = manifest;

        validate_project_path(
            ConfigSource::ConfigFile {
                path: root.join(MANIFEST_FILE),
                key: "package.entrypoint".into(),
            },
            root,
            Path::new(package.entrypoint.as_str()),
        )?;

        let Some(template) = template else {
            return Ok(());
        };

        validate_project_path(
            ConfigSource::ConfigFile {
                path: root.join(MANIFEST_FILE),
                key: "template.path".into(),
            },
            root,
            Path::new(template.path.as_str()),
        )?;
        validate_project_path(
            ConfigSource::ConfigFile {
                path: root.join(MANIFEST_FILE),
                key: "template.entrypoint".into(),
            },
            root,
            &Path::new(template.path.as_str()).join(template.entrypoint.as_str()),
        )?;

        Ok(())
    }

    /// Validates a parsed manifest config within a project root.
    pub fn validate_manifest_config(
        root: &Path,
        config: &ProjectConfig,
    ) -> Result<(), ValidationError> {
        config.validate(
            root,
            PartialConfigSource::ConfigFile {
                path: root.join(MANIFEST_FILE),
                manifest: true,
            },
        )
    }
}

impl ProjectContext {
    /// The absolute project root path.
    ///
    /// This path is used as the root for unit test compilations.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The VCS type and root if both are set.
    pub fn vcs(&self) -> Option<&Vcs> {
        self.vcs.as_ref()
    }

    /// The package manifest, if the project is a package.
    pub fn manifest(&self) -> Option<&PackageManifest> {
        self.manifest.as_deref()
    }

    /// The active configurations.
    pub fn config(&self) -> &LayeredConfig {
        &self.config
    }

    /// The store of this project.
    pub fn store(&self) -> &Store {
        &self.store
    }
}

impl ProjectContext {
    /// Migrate the store from legacy to V1 or do nothing if there already is a
    /// V1 store.
    pub fn migrate_store<I>(&mut self, tests: I) -> Result<(), MigrateError>
    where
        I: IntoIterator<Item = UnitIdent>,
    {
        self.store.migrate(tests)?;
        Ok(())
    }
}

/// Returned by [`ProjectContext::detect_project_and_vcs`].
#[derive(Debug, Error)]
pub enum LoadError {
    /// Failed to open package manifest.
    #[error("failed to read package manifest")]
    Manifest(ReadError),

    /// Failed to read a non-manifest config.
    #[error("failed to read manifest config")]
    Config(ReadError),

    /// An IO error occurred.
    #[error("an IO error occurred")]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use crate::project::store::Kind as StoreKind;
    use crate::project::vcs::Kind as VcsKind;
    use tytanic_utils::fs::TempTestEnv;
    use tytanic_utils::typst::manifest::PackageManifestBuilder;
    use tytanic_utils::typst::manifest::TemplateInfoBuilder;

    use super::*;

    #[test]
    fn test_detect_project_and_vcs() {
        TempTestEnv::run_no_check(
            |root| {
                root.setup_dir("repo/.git")
                    .setup_dir("repo/pkg/src")
                    .setup_file(
                        "repo/pkg/typst.toml",
                        include_str!("../../../../assets/test-package/typst.toml"),
                    )
                    .setup_file_empty("repo/pkg/template/main.typ")
                    .setup_file_empty("repo/pkg/lib.typ")
            },
            |root| {
                let ctx = ProjectContext::discover_project_and_vcs(
                    root.join("repo/pkg/src"),
                    Box::new(LayeredConfig::new()),
                )
                .unwrap()
                .unwrap();

                assert_eq!(ctx.root(), root.join("repo/pkg"));
                assert_eq!(ctx.vcs(), Some(&Vcs::new(root.join("repo"), VcsKind::Git)));
            },
        );
    }

    #[test]
    fn test_detect_legacy_store() {
        TempTestEnv::run_no_check(
            |root| {
                root.setup_dir("tests")
                    .setup_file(
                        "typst.toml",
                        include_str!("../../../../assets/test-package/typst.toml"),
                    )
                    .setup_file_empty("template/main.typ")
                    .setup_file_empty("lib.typ")
            },
            |root| {
                let ctx = ProjectContext::discover_project_and_vcs(
                    root.join("repo/pkg/src"),
                    Box::new(LayeredConfig::new()),
                )
                .unwrap()
                .unwrap();

                assert_eq!(ctx.store().kind(), StoreKind::Legacy);
                assert_eq!(ctx.store().src_root(), root.join("tests"));
            },
        );
    }

    #[test]
    fn test_detect_v1_store() {
        TempTestEnv::run_no_check(
            |root| {
                root.setup_dir("tests")
                    .setup_file(
                        "typst.toml",
                        include_str!("../../../../assets/test-package/typst.toml"),
                    )
                    .setup_file_empty("template/main.typ")
                    .setup_file_empty("lib.typ")
            },
            |root| {
                let ctx = ProjectContext::discover_project_and_vcs(
                    root.join("repo/pkg/src"),
                    Box::new(LayeredConfig::new()),
                )
                .unwrap()
                .unwrap();

                assert_eq!(ctx.store().kind(), StoreKind::Legacy);
                assert_eq!(ctx.store().src_root(), root.join("tests"));
            },
        );
    }

    #[test]
    fn test_config_validation_default() {
        TempTestEnv::run_no_check(
            |root| root.setup_dir("tests"),
            |root| {
                let config = ProjectConfig::default();
                ProjectContext::validate_manifest_config(root, &config).unwrap();
            },
        );
    }

    #[test]
    fn test_manifest_validation_trivial_existing_paths() {
        TempTestEnv::run_no_check(
            |root| {
                root.setup_dir("qux")
                    .setup_file_empty("src/lib.typ")
                    .setup_file_empty("template/main.typ")
            },
            |root| {
                let manifest = PackageManifestBuilder::new()
                    .template(
                        TemplateInfoBuilder::new()
                            .path("template")
                            .entrypoint("main.typ")
                            .build(),
                    )
                    .build();

                let config = ProjectConfig {
                    store_root: Some("qux".into()),
                    ..Default::default()
                };

                ProjectContext::validate_manifest(root, &manifest).unwrap();
                ProjectContext::validate_manifest_config(root, &config).unwrap();
            },
        );
    }

    #[test]
    fn test_manifest_validation_non_trivial_paths() {
        TempTestEnv::run_no_check(
            |root| root.setup_file_empty("src/lib.typ"),
            |root| {
                let manifest = PackageManifestBuilder::new()
                    .template(
                        TemplateInfoBuilder::new()
                            .path("..")
                            .entrypoint(".")
                            .build(),
                    )
                    .build();

                let config = ProjectConfig {
                    store_root: Some("/.".into()),
                    ..Default::default()
                };

                assert_eq!(
                    ProjectContext::validate_manifest(root, &manifest)
                        .unwrap_err()
                        .source(),
                    &ConfigSource::ConfigFile {
                        path: root.join(MANIFEST_FILE),
                        key: "template.path".into()
                    },
                );
                assert_eq!(
                    ProjectContext::validate_manifest_config(root, &config)
                        .unwrap_err()
                        .source(),
                    &ConfigSource::ConfigFile {
                        path: root.join(MANIFEST_FILE),
                        key: "tool.tytanic.tests".into()
                    },
                );
            },
        );
    }

    #[test]
    fn test_manifest_validation_non_existent_paths() {
        TempTestEnv::run_no_check(
            |root| root.setup_file_empty("src/lib.typ"),
            |root| {
                let manifest = PackageManifestBuilder::new()
                    .template(
                        TemplateInfoBuilder::new()
                            .path("template")
                            .entrypoint("main.typ")
                            .build(),
                    )
                    .build();

                let config = ProjectConfig {
                    store_root: Some("tests".into()),
                    ..Default::default()
                };

                assert_eq!(
                    ProjectContext::validate_manifest(root, &manifest)
                        .unwrap_err()
                        .source(),
                    &ConfigSource::ConfigFile {
                        path: root.join(MANIFEST_FILE),
                        key: "template.path".into()
                    },
                );
                assert_eq!(
                    ProjectContext::validate_manifest_config(root, &config)
                        .unwrap_err()
                        .source(),
                    &ConfigSource::ConfigFile {
                        path: root.join(MANIFEST_FILE),
                        key: "tool.tytanic.tests".into()
                    },
                );
            },
        );
    }
}
