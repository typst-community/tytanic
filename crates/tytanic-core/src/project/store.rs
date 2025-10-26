//! File system access and artifact management.
//!
//! During test runs various artifacts may be written to or read from disk, such
//! as persistent reference documents, temporary test output for debugging, log
//! files or test run results.
//!
//! Particularity temporary output and reference documents were previously
//! stored alongside the test sources. These can be accessed through
//! [`Store::Legacy`].
//!
//! Newly created projects will use the [`Store::V1`] variant, which stores
//! almost all artifacts in a single artifact store root directory.
//!
//! The `Legacy` store offers a migration function to move artifacts to the new
//! store directory:
//! ```no_run
//! # use tytanic_core::project::store::Kind;
//! # use tytanic_core::project::store::Store;
//! let root = "/home/user/src/my-package";
//!
//! let mut store = Store::new(root, format!("{root}/tests"), Kind::V1);
//!
//! store = Store::V1(match store {
//!     Store::V1(v1) => v1,
//!     Store::Legacy(legacy) => legacy.migrate(
//!         format!("{root}/store"),
//!         None,
//!         [/* unit tests to migrate here */],
//!     )?,
//! });
//! # Ok::<_, Box<dyn std::error::Error>>(())
//! ```
//!
//! # Legacy Store
//! The legacy store stores artifacts alongside their unit tests. This has
//! the downside of not allowing other tests to add artifacts easily without
//! polluting a repository. When the legacy store is detected it should be
//! migrated to the new store. It has the following structure:
//! ```txt
//! <test_root>
//! ├─ foo-compile-only
//! │  ├─ diff
//! │  │  └─ ...
//! │  ├─ out
//! │  │  └─ ...
//! │  └─ test.typ
//! ├─ foo-ephemeral
//! │  ├─ diff
//! │  ├─ ref
//! │  │  └─ ...
//! │  ├─ out
//! │  │  └─ ...
//! │  ├─ ref.typ
//! │  └─ test.typ
//! └─ foo-persistent
//!    ├─ diff
//!    │  └─ ...
//!    ├─ ref
//!    │  └─ ...
//!    ├─ out
//!    │  └─ ...
//!    └─ test.typ
//! ```
//! The `out`, `ref` and `diff` directories contain the temporary output,
//! references (temporary or persistent), or difference output respectively.
//!
//! # New Store
//! The new store is a dedicated directory in which artifacts are stored. It has
//! the following structure:
//! ```txt
//! <store_root>
//! ├─ src
//! │  ├─ foo-compile-only
//! │  │  └─ test.typ
//! │  ├─ foo-ephemeral
//! │  │  ├─ ref.typ
//! │  │  └─ test.typ
//! │  └─ foo-persistent
//! │     └─ test.typ
//! ├─ ref
//! │  ├─ template
//! │  ├─ doc
//! │  └─ unit
//! │     └─ persistent
//! │        └─ ...
//! └─ tmp
//!    ├─ <ignore_file>
//!    ├─ <run_id>
//!    │  ├─ run.xml
//!    │  ├─ template
//!    │  ├─ doc
//!    │  └─ unit
//!    │     ├─ foo-compile-only
//!    │     │  └─ ...
//!    │     ├─ foo-ephemeral
//!    │     │  └─ ...
//!    │     └─ foo-persistent
//!    │        └─ ...
//!    └─ <run_id>
//!       └─ ...
//! ```
//!
//! The top-level `ref` directory contains persistent references with the doc
//! and unit tests being stored in sub directories according to their identifier
//! components. Template references are placed directly in the template
//! directory.
//!
//! The top-level `tmp` directory contains a VCS-specific ignore file and a set
//! of directories identified by their `run id` for which outputs have been
//! generated. These run directories may also contain other output, such as
//! JUnit XML files for the run.

use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use ignore::WalkBuilder;
use ignore::WalkState;
use thiserror::Error;
use tytanic_utils::result::PathError;
use tytanic_utils::result::ResultEx;
use uuid::Uuid;

use crate::config::LayeredConfig;
use crate::config::ProjectConfig;
use crate::project::vcs::IgnoreDirectoryError;
use crate::test::DocIdent;
use crate::test::Ident;
use crate::test::ParseIdentError;
use crate::test::TemplateIdent;
use crate::test::TemplateTest;
use crate::test::UnitIdent;
use crate::test::UnitKind;
use crate::test::UnitTest;

// TODO: Add file that stores the version for future migrations in the new
// store.

mod legacy;
mod v1;

/// The kind of temporary artifact of a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactKind {
    /// The output of a test script.
    Primary,

    /// The output of an ephemeral reference script.
    Reference,

    /// The difference of the primary and reference output.
    Difference,
}

impl ArtifactKind {
    /// Returns the sub directory name for this kind.
    fn sub_dir(&self) -> &'static str {
        match self {
            ArtifactKind::Primary => "out",
            ArtifactKind::Reference => "ref",
            ArtifactKind::Difference => "diff",
        }
    }
}

/// The store kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// The new store.
    V1,

    /// The legacy store.
    Legacy,
}

/// The store is an abstraction over filesystem access in a project.
///
/// See the [module-level documentation][self] for more info.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Store {
    project_root: PathBuf,
    // - legacy: the unit test root
    // - v1: the store root containing the `src`, `ref` and `tmp` roots
    store_root: PathBuf,
    template_root: Option<PathBuf>,
    kind: Kind,
}

impl Store {
    /// Creates a new legacy store from the given roots.
    pub fn new<P, Q>(
        project_root: P,
        store_root: Q,
        template_root: Option<PathBuf>,
        kind: Kind,
    ) -> Store
    where
        P: Into<PathBuf> + AsRef<Path>,
        Q: Into<PathBuf> + AsRef<Path>,
    {
        debug_assert!(
            project_root.as_ref().is_absolute(),
            "project root must be absolute"
        );
        debug_assert!(
            store_root.as_ref().is_absolute(),
            "store root must be absolute"
        );
        if let Some(template_root) = &template_root {
            debug_assert!(
                template_root.is_absolute(),
                "template root must be absolute"
            );
        }

        Self {
            project_root: project_root.into(),
            store_root: store_root.into(),
            template_root: template_root.into(),
            kind,
        }
    }

    /// Attempts to discover the store kind from the given project root and
    /// config.
    ///
    /// # Examples
    /// ```no_run
    /// # use tytanic_core::config::LayeredConfig;
    /// # use tytanic_core::config::ProjectConfig;
    /// # use tytanic_core::project::store::Store;
    /// let mut config = LayeredConfig::new();
    /// config.with_project_layer(None, Some(ProjectConfig {
    ///     artifact_store_root: Some("tests".into()),
    ///     ..Default::default()
    /// }));
    ///
    /// let store = Store::with_config("~/src/project", &config)?;
    /// Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_config<P>(project_root: P, config: &LayeredConfig) -> io::Result<Self>
    where
        P: Into<PathBuf> + AsRef<Path>,
    {
        let store_root = project_root
            .as_ref()
            .join(config.get_project_config_member(ProjectConfig::STORE_ROOT, ()));

        let template_root = config
            .get_project_config_member(ProjectConfig::TEMPLATE_PATH, ())
            .map(|template_path| project_root.as_ref().join(template_path));

        Self::with_inferred_kind(project_root, store_root, template_root)
    }

    /// Attempts to discover the store kind from the given project and store
    /// root.
    ///
    /// # Examples
    /// ```no_run
    /// # use tytanic_core::project::store::Store;
    /// let store = Store::with_inferred_kind(
    ///     "~/src/project",
    ///     "~/src/project/tests",
    ///     None,
    /// )?;
    /// Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_inferred_kind<P, Q>(
        project_root: P,
        store_root: Q,
        template_root: Option<PathBuf>,
    ) -> io::Result<Self>
    where
        P: Into<PathBuf> + AsRef<Path>,
        Q: Into<PathBuf> + AsRef<Path>,
    {
        debug_assert!(
            project_root.as_ref().is_absolute(),
            "project root must be absolute"
        );
        debug_assert!(
            store_root.as_ref().is_absolute(),
            "store root must be absolute"
        );

        let kind = Self::try_infer_kind(store_root.as_ref())?;

        Ok(Self {
            project_root: project_root.into(),
            store_root: store_root.into(),
            template_root: template_root.into(),
            kind,
        })
    }

    /// Attempts to infer the kind of a store from its directory structure.
    ///
    /// # Examples
    /// ```no_run
    /// # use tytanic_core::project::store::Kind;
    /// # use tytanic_core::project::store::Store;
    /// let kind = Store::try_infer_kind("~/src/project/tests")?;
    /// assert_eq!(kind, Kind::Legacy);
    /// Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn try_infer_kind<P>(store_root: P) -> io::Result<Kind>
    where
        P: AsRef<Path>,
    {
        // The top level entries in the test root are test identifiers for which
        // this name would be invalid, so we infer that this must be the v1
        // store.
        Ok(if store_root.as_ref().join("ref").try_exists()? {
            Kind::V1
        } else {
            Kind::Legacy
        })
    }
}

impl Store {
    /// The kind of this store.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The absolute unit test root directory.
    ///
    /// This directory contains the unit test source files.
    pub fn src_root(&self) -> PathBuf {
        match self.kind {
            Kind::V1 => self.store_root.join("src"),
            Kind::Legacy => self.store_root.to_path_buf(),
        }
    }

    /// The absolute `tmp` root directory.
    ///
    /// This directory contains temporary output artifacts.
    ///
    /// This is useful for V1 stores!
    fn tmp_root(&self) -> PathBuf {
        self.store_root.join("tmp")
    }

    /// The absolute `ref` root directory.
    ///
    /// This directory contains persistent reference artifacts.
    ///
    /// This is useful for V1 stores!
    fn ref_root(&self) -> PathBuf {
        self.store_root.join("ref")
    }
}

/// Non-temporary unit test directories.
impl Store {
    /// Returns an absolute path to the source directory of the unit test.
    pub fn unit_test_src_dir(&self, test: &UnitIdent) -> PathBuf {
        let mut path = self.src_root();
        path.push("unit");
        path.extend(test.path().split('/'));
        path
    }

    /// Returns an absolute path to the primary script of the unit test.
    pub fn unit_test_primary_script(&self, test: &UnitIdent) -> PathBuf {
        let mut path = self.unit_test_src_dir(test);
        path.push("test.typ");
        path
    }

    /// Returns an absolute path to the reference script of the unit test.
    pub fn unit_test_reference_script(&self, test: &UnitIdent) -> PathBuf {
        let mut path = self.unit_test_src_dir(test);
        path.push("ref.typ");
        path
    }

    /// Returns an absolute path to the persistent reference directory for the
    /// test.
    ///
    /// # Error
    /// Returns an error if this is requested for a template or doc test.
    pub fn persistent_reference_dir(&self, test: &Ident) -> Result<PathBuf, UnsupportedError> {
        match test {
            Ident::Template(_) => Err(UnsupportedError {
                ident: test.clone(),
                operation: "get template persistent reference dir".into(),
                hint: "template tests don't have persistent references".into(),
            }),
            Ident::Unit(test) => Ok(self.unit_persistent_reference_dir(test)),
            Ident::Doc(_) => Err(UnsupportedError {
                ident: test.clone(),
                operation: "get doc persistent reference dir".into(),
                hint: "doc tests don't have persistent references".into(),
            }),
        }
    }

    /// Returns an absolute path to the persistent reference directory of the
    /// unit test.
    ///
    /// This is the same as the temporary reference directory for the legacy
    /// store.
    pub fn unit_persistent_reference_dir(&self, test: &UnitIdent) -> PathBuf {
        match self.kind {
            Kind::V1 => {
                let mut path = self.ref_root();
                path.push("unit");
                path.extend(test.path().split('/'));
                path
            }
            Kind::Legacy => {
                let mut path = self.src_root();
                path.extend(test.path().split('/'));
                path.push("ref");
                path
            }
        }
    }
}

/// Artifact directories.
impl Store {
    /// Returns an absolute path to the temporary output directory for the test.
    ///
    /// If the artifact kind is given then the respective sub directory is
    /// returned. While this can return `reference` and `difference` artifact
    /// directories for doc and template tests these currently serve no purpose.
    ///
    /// # Errors
    /// Returns an error if this store is a legacy store and the test kind isn't
    /// `unit`.
    pub fn artifact_dir(
        &self,
        run_id: Uuid,
        test: &Ident,
        kind: Option<ArtifactKind>,
    ) -> Result<PathBuf, UnsupportedError> {
        match test {
            Ident::Template(test) => self.template_artifact_dir(run_id, test, kind),
            Ident::Unit(test) => Ok(self.unit_artifact_dir(run_id, test, kind)),
            Ident::Doc(test) => self.doc_artifact_dir(run_id, test, kind),
        }
    }

    fn template_artifact_prefix(
        &self,
        run_id: Uuid,
        test: &TemplateIdent,
    ) -> Result<PathBuf, UnsupportedError> {
        match self.kind {
            Kind::V1 => {
                let mut path = self.ref_root();
                path.push(run_id.to_string());
                path.push("template");
                path.push(test.as_str());
                Ok(path)
            }
            Kind::Legacy => Err(UnsupportedError {
                ident: test.clone().into_ident(),
                operation: "get template artifact dir".into(),
                hint: "legacy store doesn't support template artifacts".into(),
            }),
        }
    }

    /// Returns an absolute path to the artifact directory of the template test.
    ///
    /// If the artifact kind is given then the respective sub directory is
    /// returned. While this can return `reference` and `difference` artifact
    /// directories these currently serve no purpose.
    ///
    /// # Errors
    /// Returns an error if this store is a legacy store.
    pub fn template_artifact_dir(
        &self,
        run_id: Uuid,
        test: &TemplateIdent,
        kind: Option<ArtifactKind>,
    ) -> Result<PathBuf, UnsupportedError> {
        let mut path = self.template_artifact_prefix(run_id, test)?;
        if let Some(kind) = kind {
            path.push(kind.sub_dir());
        }
        Ok(path)
    }

    /// Returns an absolute path to the primary artifact directory of the
    /// template test.
    ///
    /// # Errors
    /// Returns an error if this store is a legacy store.
    pub fn template_primary_artifact_dir(
        &self,
        run_id: Uuid,
        test: &TemplateIdent,
    ) -> Result<PathBuf, UnsupportedError> {
        self.template_artifact_dir(run_id, test, Some(ArtifactKind::Primary))
    }

    fn unit_artifact_prefix(&self, run_id: Uuid, test: &UnitIdent) -> PathBuf {
        match self.kind {
            Kind::V1 => {
                let mut path = self.ref_root();
                path.push(run_id.to_string());
                path.push("unit");
                path.extend(test.path().split('/'));
                path
            }
            Kind::Legacy => {
                let mut path = self.src_root();
                path.extend(test.path().split('/'));
                path
            }
        }
    }

    /// Returns an absolute path to the artifact directory of the unit test.
    ///
    /// If the artifact kind is given then the respective sub directory is
    /// returned.
    pub fn unit_artifact_dir(
        &self,
        run_id: Uuid,
        test: &UnitIdent,
        kind: Option<ArtifactKind>,
    ) -> PathBuf {
        let mut path = self.unit_artifact_prefix(run_id, test);
        if let Some(kind) = kind {
            path.push(kind.sub_dir());
        }
        path
    }

    /// Returns an absolute path to the primary artifact directory of the unit
    /// test.
    pub fn unit_primary_artifact_dir(&self, run_id: Uuid, test: &UnitIdent) -> PathBuf {
        self.unit_artifact_dir(run_id, test, Some(ArtifactKind::Primary))
    }

    /// Returns an absolute path to the reference artifact directory of the unit
    /// test.
    ///
    /// This is the same as the persistent reference directory for the legacy
    /// store.
    pub fn unit_reference_artifact_dir(&self, run_id: Uuid, test: &UnitIdent) -> PathBuf {
        self.unit_artifact_dir(run_id, test, Some(ArtifactKind::Reference))
    }

    /// Returns an absolute path to the difference artifact directory of the
    /// unit test.
    pub fn unit_difference_artifact_dir(&self, run_id: Uuid, test: &UnitIdent) -> PathBuf {
        self.unit_artifact_dir(run_id, test, Some(ArtifactKind::Difference))
    }

    fn doc_artifact_prefix(
        &self,
        run_id: Uuid,
        test: &DocIdent,
    ) -> Result<PathBuf, UnsupportedError> {
        match self.kind {
            Kind::V1 => {
                let mut path = self.ref_root();
                path.push(run_id.to_string());
                path.push("doc");
                path.push(test.path());
                path.push(test.item());
                if let Some(block) = test.block() {
                    path.push(block);
                }
                Ok(path)
            }
            Kind::Legacy => Err(UnsupportedError {
                ident: test.clone().into_ident(),
                operation: "get doc artifact dir".into(),
                hint: "legacy store doesn't support doc artifacts".into(),
            }),
        }
    }

    /// Returns an absolute path to the artifact directory of the doc test.
    ///
    /// If the artifact kind is given then the respective sub directory is
    /// returned. While this can return `reference` and `difference` artifact
    /// directories these currently serve no purpose.
    ///
    /// # Errors
    /// Returns an error if this store is a legacy store.
    pub fn doc_artifact_dir(
        &self,
        run_id: Uuid,
        test: &DocIdent,
        kind: Option<ArtifactKind>,
    ) -> Result<PathBuf, UnsupportedError> {
        let mut path = self.doc_artifact_prefix(run_id, test)?;
        if let Some(kind) = kind {
            path.push(kind.sub_dir());
        }
        Ok(path)
    }

    /// Returns an absolute path to the artifact directory of the doc test.
    ///
    /// # Errors
    /// Returns an error if this store is a legacy store.
    pub fn doc_primary_artifact_dir(
        &self,
        run_id: Uuid,
        test: &DocIdent,
    ) -> Result<PathBuf, UnsupportedError> {
        self.doc_artifact_dir(run_id, test, Some(ArtifactKind::Primary))
    }
}

/// Loading tests, references, and artifacts.
impl Store {
    /// Returns the persistent reference file paths for the unit test.
    ///
    /// This will eagerly iterate over the contents of
    /// [`Store::unit_persistent_reference_dir`] and collect all paths that
    /// match `{index}.png` and check that no files are in between indices
    /// missing.
    ///
    /// The paths are returned in ascending order.
    ///
    /// # Errors
    /// Returns an error if some references are missing.
    pub fn unit_persistent_references(
        &self,
        test: &UnitIdent,
    ) -> Result<Vec<PathBuf>, PersistentReferencesError> {
        let dir = self.unit_persistent_reference_dir(test);
        let mut paths = vec![];

        for entry in dir.read_dir()? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_none_or(|ext| ext != "png") {
                continue;
            }

            let Some(stem) = path.file_stem() else {
                continue;
            };

            if stem
                .to_str()
                .is_none_or(|stem| matches!(stem.parse::<usize>(), Ok(0) | Err(_)))
            {
                tracing::warn!(
                    ?dir,
                    ?stem,
                    "found invalid file stem in persistent reference directory",
                );
                continue;
            };

            let meta = entry.metadata()?;

            if meta.is_dir() {
                tracing::warn!(
                    ?path,
                    "found directory with `{{index}}.png` name in persistent reference directory",
                );
                continue;
            }

            paths.push(path);
        }

        paths.sort_unstable();

        let mut missing = vec![];
        for window in paths.windows(2) {
            let [a, b] = window else {
                unreachable!();
            };

            let a = a
                .file_stem()
                .and_then(|p| p.to_str())
                .expect("must have a UTF-8 stem");

            let b = b
                .file_stem()
                .and_then(|p| p.to_str())
                .expect("must have a UTF-8 stem");

            let a = a.parse::<usize>().expect("must have a stem");
            let b = b.parse::<usize>().expect("must have a stem");

            if a + 1 != b {
                tracing::error!(?dir, missing = a + 1, next = b, "found missing reference");
                missing.push(a + 1);
            }
        }

        if missing.is_empty() {
            Ok(paths)
        } else {
            Err(PersistentReferencesError::MissingReferences { indices: missing })
        }
    }

    /// Collects the template test in the store.
    ///
    /// # Examples
    /// ```no_run
    /// # use tytanic_core::config::LayeredConfig;
    /// # use tytanic_core::project::store::Store;
    /// # use tytanic_core::suite;
    /// let store = Store::new(
    ///     "~/src/project",
    ///     "~/src/project/tests",
    ///     Kind::V1,
    /// )?;
    ///
    /// let test = store.collect_template_test()?;
    /// println!("{}", test.ident());
    /// Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn collect_template_test(&self) -> Result<TemplateTest, CollectTemplateTestError> {
        todo!()
    }

    /// Collects unit tests in the store.
    ///
    /// This will continue collection in the case of identifier errors, but
    /// abort if it encounters IO errors.
    ///
    /// # Examples
    /// ```no_run
    /// # use tytanic_core::config::LayeredConfig;
    /// # use tytanic_core::project::store::Store;
    /// # use tytanic_core::suite;
    /// let store = Store::new(
    ///     "~/src/project",
    ///     "~/src/project/tests",
    ///     Kind::V1,
    /// )?;
    ///
    /// let (tests, errors) = store.collect_unit_tests(None)?;
    ///
    /// for test in errors {
    ///     println!("{error:?}");
    /// }
    ///
    /// for test in tests {
    ///     println!("{}", test.ident());
    /// }
    /// Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn collect_unit_tests(
        &self,
        threads: Option<NonZeroUsize>,
    ) -> Result<(Vec<UnitTest>, Vec<PathError<ParseIdentError>>), CollectUnitTestsError> {
        #[derive(Debug, Error)]
        #[error("this should never escape this function")]
        pub enum HandleEntryError {
            Ident(#[from] PathError<ParseIdentError>),
            Walk(#[from] ignore::Error),
            Io(#[from] io::Error),
        }

        fn handle_entry(
            store: &Store,
            entry: Result<ignore::DirEntry, ignore::Error>,
        ) -> Result<Option<UnitTest>, HandleEntryError> {
            let entry = entry?;

            let meta = entry.metadata()?;
            if !meta.is_dir() {
                return Ok(None);
            }

            let path = entry.into_path();

            if !path.join("test.typ").try_exists()? {
                return Ok(None);
            }

            let trimmed = path
                .strip_prefix(store.src_root())
                .expect("`handle_entry` is only called within `unit_src_root`")
                .to_path_buf();

            let ident = UnitIdent::new_from_path(&trimmed).path_with(|| path.to_path_buf())?;

            let kind = if store.unit_test_reference_script(&ident).try_exists()? {
                UnitKind::Ephemeral
            } else if store.unit_persistent_reference_dir(&ident).try_exists()? {
                UnitKind::Persistent
            } else {
                UnitKind::CompileOnly
            };

            Ok(Some(UnitTest::new(ident, kind)))
        }

        fn inner_sequential(
            store: &Store,
            walker: ignore::Walk,
        ) -> Result<(Vec<UnitTest>, Vec<PathError<ParseIdentError>>), CollectUnitTestsError>
        {
            let mut tests: Vec<UnitTest> = vec![];
            let mut ident_errors: Vec<PathError<ParseIdentError>> = vec![];

            for entry in walker {
                match handle_entry(store, entry) {
                    Ok(Some(test)) => tests.push(test),
                    Ok(None) => {}
                    Err(HandleEntryError::Ident(error)) => ident_errors.push(error),
                    Err(HandleEntryError::Walk(error)) => Err(error)?,
                    Err(HandleEntryError::Io(error)) => Err(error)?,
                }
            }

            Ok((tests, ident_errors))
        }

        fn inner_parallel(
            store: &Store,
            walker: ignore::WalkParallel,
        ) -> Result<(Vec<UnitTest>, Vec<PathError<ParseIdentError>>), CollectUnitTestsError>
        {
            let mut tests: Vec<UnitTest> = vec![];
            let mut ident_errors: Vec<PathError<ParseIdentError>> = vec![];
            let mut outer_error: Option<CollectUnitTestsError> = None;

            let (test_tx, test_rx) = mpsc::channel();
            let (error_tx, error_rx) =
                mpsc::channel::<Result<PathError<ParseIdentError>, CollectUnitTestsError>>();

            thread::scope({
                let tests = &mut tests;
                let ident_errors = &mut ident_errors;
                let outer_error = &mut outer_error;

                move |scope| {
                    // Spawn the receiver thread before starting collection.
                    scope.spawn(move || {
                        while let Ok(val) = test_rx.recv() {
                            tests.push(val);
                        }

                        // The parallel walker does not sort its output so we do it
                        // once.
                        tests.sort_by(|a, b| Ord::cmp(&a.ident(), &b.ident()));
                    });

                    // Spawn the error thread before starting collection.
                    scope.spawn(move || {
                        while let Ok(val) = error_rx.recv() {
                            match val {
                                Ok(error) => {
                                    ident_errors.push(error);
                                }
                                Err(error) => {
                                    *outer_error = Some(error);
                                    break;
                                }
                            }
                        }
                    });

                    // Start collecting.
                    walker.run(|| {
                        let test_tx = test_tx.clone();
                        let error_tx = error_tx.clone();

                        Box::new(move |entry| {
                            match handle_entry(store, entry) {
                                Ok(Some(test)) => {
                                    if let Err(err) = test_tx.send(test) {
                                        tracing::error!(
                                            ?err,
                                            "couldn't send unit test to parent thread",
                                        );
                                        return WalkState::Quit;
                                    }
                                }
                                Ok(None) => return WalkState::Continue,
                                Err(HandleEntryError::Ident(error)) => {
                                    if let Err(error) = error_tx.send(Ok(error)) {
                                        tracing::error!(
                                            ?error,
                                            "failed to send error to parent thread"
                                        );
                                        return WalkState::Quit;
                                    }

                                    return WalkState::Continue;
                                }
                                Err(HandleEntryError::Walk(error)) => {
                                    if let Err(error) =
                                        error_tx.send(Err(CollectUnitTestsError::Walk(error)))
                                    {
                                        tracing::error!(
                                            ?error,
                                            "failed to send error to parent thread"
                                        );
                                    }

                                    return WalkState::Quit;
                                }
                                Err(HandleEntryError::Io(error)) => {
                                    if let Err(error) =
                                        error_tx.send(Err(CollectUnitTestsError::Io(error)))
                                    {
                                        tracing::error!(
                                            ?error,
                                            "failed to send error to parent thread"
                                        );
                                    }

                                    return WalkState::Quit;
                                }
                            };

                            WalkState::Skip
                        })
                    });

                    // Close the final producer to stop the receiver threads after
                    // collection has ended.
                    drop(test_tx);
                    drop(error_tx);
                }
            });

            Ok((tests, ident_errors))
        }

        fn inner(
            store: &Store,
            threads: Option<NonZeroUsize>,
        ) -> Result<(Vec<UnitTest>, Vec<PathError<ParseIdentError>>), CollectUnitTestsError>
        {
            // If we should infer the worker thread count we try to use the
            // unused cores under the assumption that the system is largely
            // idle outside of this program.
            //
            // Since we have 3 non-worker threads we subtract them and
            // fallback to 1 worker thread if there isn't enough
            // parallelism.
            let threads = if let Some(threads) = threads {
                threads.get().checked_sub(3).unwrap_or(1)
            } else if let Ok(available) = thread::available_parallelism() {
                tracing::debug!(threads = ?available, "detected available parallelism");
                available.get().checked_sub(3).unwrap_or(1)
            } else {
                1
            };

            let mut builder = WalkBuilder::new(store.src_root());

            builder
                .threads(threads)
                .hidden(true)
                .parents(false)
                .sort_by_file_name(Ord::cmp)
                .git_global(false)
                .git_ignore(false)
                .git_exclude(false)
                .require_git(false);

            if threads == 1 {
                tracing::debug!("using sequential walker");
                inner_sequential(store, builder.build())
            } else {
                tracing::debug!(threads = threads + 3, "using parallel walker");
                inner_parallel(store, builder.build_parallel())
            }
        }

        inner(self, threads)
    }
}

impl Store {
    // /// Removes the temporary artifacts for the given tests.
    // pub fn clear_artifacts<'a, I>(&self, run_id: Uuid, tests: I) -> io::Result<()>
    // where
    //     I: IntoIterator<Item = &'a Ident>,
    // {
    //     for test in tests {
    //         // self.artifact_dir(run_id, test, kind);
    //         let mut dir = store.test_root.join(test.path());

    //         tracing::debug!(?dir, "removing legacy artifacts in test directory");

    //         dir.push("out");
    //         if let Err(err) = fs::remove_dir_all(&dir) {
    //             if err.kind() != io::ErrorKind::NotFound {
    //                 Err(err)?;
    //             }
    //         }
    //         dir.pop();

    //         if !test.is_persistent() {
    //             dir.push("ref");
    //             if let Err(err) = fs::remove_dir_all(&dir) {
    //                 if err.kind() != io::ErrorKind::NotFound {
    //                     Err(err)?;
    //                 }
    //             }
    //             dir.pop();
    //         }

    //         dir.push("diff");
    //         if let Err(err) = fs::remove_dir_all(&dir) {
    //             if err.kind() != io::ErrorKind::NotFound {
    //                 Err(err)?;
    //             }
    //         }
    //     }

    //     Ok(())
    // }

    // /// Removes the temporary artifacts for all tests.
    // pub fn clear_all_artifacts(&self) -> io::Result<()> {
    //     match self.kind {
    //         Kind::V1 => fs::remove_dir_all(self.tmp_root())?,
    //         Kind::Legacy => {
    //             // TODO: Collect only paths and use those to clear the temporary
    //             // refs.
    //         }
    //     }

    //     OK(())
    // }

    /// Migrates this store from Legacy to V1.
    pub fn migrate<I>(&mut self, tests: I) -> Result<(), MigrateError>
    where
        I: IntoIterator<Item = UnitIdent>,
    {
        // TODO(tinger): Make this close to atomic by using a temporary
        // directory first and then overwriting the old root with it.
        //
        // Note that this will also remove all not explicitly moved files, make
        // sure these are copied too. Perhaps instead remove the old temporaries
        // first and then do the move.

        fn migrate_unit_test(
            old_src_root: &Path,
            new_src_root: &Path,
            new_ref_root: &Path,
            test: &UnitIdent,
        ) -> io::Result<()> {
            let mut old = old_src_root.to_path_buf();
            old.extend(test.path().split('/'));

            let old_src_primary = old.join("test.typ");
            let old_src_reference = old.join("ref.typ");
            let old_ref = old.join("ref");

            let mut new_ref = new_ref_root.to_path_buf();
            new_ref.extend(["ref", "unit"]);
            new_ref.extend(test.path().split('/'));

            let mut new_src = new_src_root.to_path_buf();
            new_src.extend(test.path().split('/'));

            let new_src_primary = new_src.join("test.typ");
            let new_src_reference = new_src.join("ref.typ");

            tracing::debug!(?old_src_primary, ?new_src_primary, "moving test script");
            fs::create_dir_all(&new_src)?;
            fs::rename(old_src_primary, new_src_primary)?;

            if old_src_reference.try_exists()? {
                tracing::debug!(
                    ?old_src_reference,
                    ?new_src_reference,
                    "moving reference script"
                );
                fs::rename(old_src_reference, new_src_reference)?;
            } else if old_ref.try_exists()? {
                tracing::debug!(?old_ref, ?new_ref, "moving persistent reference document");
                fs::create_dir_all(new_ref.parent().expect("must not be fs root"))?;
                fs::rename(old_ref, new_ref)?;
            }

            Ok(())
        }

        if self.kind == Kind::V1 {
            return Ok(());
        }

        let old_src_root = &self.store_root;
        let new_src_root = self.store_root.join("src");

        let mut new_ref_root = self.store_root.clone();
        new_ref_root.extend(["ref", "unit"]);

        for test in tests {
            migrate_unit_test(old_src_root, &new_src_root, &new_ref_root, &test)?;
            let root_child = test
                .path()
                .split_once('/')
                .map(|(first, _)| first)
                .unwrap_or(test.path());

            if let Err(error) = fs::remove_dir_all(root_child) {
                if error.kind() == io::ErrorKind::NotFound {
                    Err(error)?;
                }
            }
        }

        self.kind = Kind::V1;

        Ok(())
    }
}

/// Returned by [`Store::get_persistent_references`].
#[derive(Debug, Error)]
pub enum PersistentReferencesError {
    /// Some references were missing.
    #[error("some references were missing: {indices:?}")]
    MissingReferences {
        /// The 1-based indices that were missing.
        indices: Vec<usize>,
    },

    /// The legacy store doesn't support the requested operation.
    #[error("the legacy store doesn't support the requested operation")]
    Unsupported(#[from] UnsupportedError),

    /// An IO error occurred.
    #[error("an IO error occurred")]
    Io(#[from] io::Error),
}

/// Returned by [`Store::migrate`].
#[derive(Debug, Error)]
pub enum MigrateError {
    /// An error occurred while ignoring a directory.
    #[error("an error occurred while ignoring a directory")]
    IgnoreDirectory(#[from] IgnoreDirectoryError),

    /// An IO error occurred.
    #[error("an IO error occurred")]
    Io(#[from] io::Error),
}

/// An error returned when an invalid operation is requested from a [`Store`].
#[derive(Debug, Error)]
#[error("can't {operation} for test {ident} ({hint})")]
pub struct UnsupportedError {
    /// The identifier of the test for which the operation was requested.
    pub ident: Ident,

    /// The operation that failed.
    pub operation: String,

    /// A hint why the error occurred.
    pub hint: String,
}

/// Returned by [`Store::collect_template_test`].
#[derive(Debug, Error)]
pub enum CollectTemplateTestError {
    /// The project has no template test.
    #[error("the project has no template test")]
    NotFound {
        /// The path at which the template scaffold was searched.
        path: PathBuf,
    },

    /// An IO error occurred.
    #[error("an io error occurred")]
    Io(#[from] io::Error),
}

/// Returned by [`Store::collect_unit_tests`].
#[derive(Debug, Error)]
pub enum CollectUnitTestsError {
    /// An error occurred while walking the test root directory tree.
    #[error("an error occurred while walking the directory tree")]
    Walk(#[from] ignore::Error),

    /// An IO error occurred.
    #[error("an io error occurred")]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tytanic_utils::fs::TempTestEnv;

    use crate::project::store::Kind;

    use super::*;

    #[test]
    fn test_collect_unit_legacy() {
        TempTestEnv::run_no_check(
            |root| {
                root
                    // compile only
                    .setup_file("tests/.hidden/test.typ", "Not loaded")
                    .setup_file("tests/ignored!/test.typ", "Invalid Name")
                    .setup_file("tests/compile-only/test.typ", "Hello World")
                    // regular ephemeral
                    .setup_file("tests/compare/ephemeral/test.typ", "Hello World")
                    .setup_file("tests/compare/ephemeral/ref.typ", "Hello\nWorld")
                    // ephemeral despite ref directory
                    .setup_file("tests/compare/ephemeral-store/test.typ", "Hello World")
                    .setup_file("tests/compare/ephemeral-store/ref.typ", "Hello\nWorld")
                    .setup_dir("tests/compare/ephemeral-store/ref")
                    // persistent
                    .setup_file("tests/compare/persistent/test.typ", "Hello World")
                    .setup_file("tests/compare/persistent/ref", "Blah Blah")
                    // not a test
                    .setup_file_empty("tests/not-a-test/test.txt")
                    // ignored test
                    .setup_file("tests/ignored/test.typ", "/// [skip]\nHello World")
            },
            |root| {
                let store = Store::new(root, root.join("tests"), None, Kind::Legacy);
                let (tests, errors) = store.collect_unit_tests(None).unwrap();

                assert_eq!(tests.len(), 6);
                assert_eq!(errors.len(), 0);

                let tests: HashMap<_, _> = tests
                    .into_iter()
                    .map(|test| (test.ident().clone(), test))
                    .collect();

                assert!(tests.contains_key("compile-only"));
                assert!(tests.contains_key("compare/ephemeral"));
                assert!(tests.contains_key("compare/ephemeral-store"));
                assert!(tests.contains_key("compare/persistent"));
                assert!(tests.contains_key("ignored"));
            },
        );
    }

    #[test]
    fn test_collect_unit_v1() {
        TempTestEnv::run_no_check(
            |root| {
                root
                    // compile only
                    .setup_file("tests/src/.hidden/test.typ", "Not loaded")
                    .setup_file("tests/src/ignored!/test.typ", "Invalid Name")
                    .setup_file("tests/src/compile-only/test.typ", "Hello World")
                    // regular ephemeral
                    .setup_file("tests/src/compare/ephemeral/test.typ", "Hello World")
                    .setup_file("tests/src/compare/ephemeral/ref.typ", "Hello\nWorld")
                    // persistent
                    .setup_file("tests/src/compare/persistent/test.typ", "Hello World")
                    .setup_file("tests/store/ref/compare/persistent", "Blah Blah")
                    // not a test
                    .setup_file_empty("tests/src/not-a-test/test.txt")
                    // ignored test
                    .setup_file("tests/src/ignored/test.typ", "/// [skip]\nHello World")
            },
            |root| {
                let store = Store::new(root, root.join("tests"), None, Kind::V1);
                let (tests, errors) = store.collect_unit_tests(None).unwrap();

                assert_eq!(tests.len(), 5);
                assert_eq!(errors.len(), 0);

                let tests: HashMap<_, _> = tests
                    .into_iter()
                    .map(|test| (test.ident().clone(), test))
                    .collect();

                assert!(tests.contains_key("compile-only"));
                assert!(tests.contains_key("compare/ephemeral"));
                assert!(tests.contains_key("compare/persistent"));
                assert!(tests.contains_key("ignored"));
            },
        );
    }

    #[test]
    fn test_collect_template() {
        TempTestEnv::run_no_check(
            |root| root.setup_file("template/main.typ", "hello World"),
            |root| {
                let store = Store::new(
                    root,
                    root.join("tests"),
                    Some(root.join("template")),
                    Kind::V1,
                );

                store.collect_template_test().unwrap();
            },
        );
    }

    // #[test]
    // #[should_panic]
    // fn test_persistent_reference_dir_template() {
    //     let store = LegacyStore::new("/tests");

    //     store
    //         .get_persistent_reference_dir(Ident::new("@template").unwrap())
    //         .unwrap();
    // }

    // #[test]
    // fn test_persistent_reference_dir_unit() {
    //     let store = LegacyStore::new("/tests");

    //     assert_eq!(
    //         store
    //             .get_persistent_reference_dir(Ident::new("foo/bar").unwrap())
    //             .unwrap(),
    //         Path::new("/tests/foo/bar/ref"),
    //     );
    // }

    // #[test]
    // #[should_panic]
    // fn test_persistent_reference_dir_doc() {
    //     let store = LegacyStore::new("/tests");

    //     store
    //         .get_persistent_reference_dir(Ident::new("foo/bar#qux:zir").unwrap())
    //         .unwrap();
    // }

    // #[test]
    // #[should_panic]
    // fn test_temporary_artifact_dir_template() {
    //     let store = LegacyStore::new("/tests");

    //     // template tests are not supported by the legacy store
    //     store
    //         .get_temporary_artifact_dir(Ident::new("@template").unwrap(), ArtifactKind::Primary)
    //         .unwrap();
    // }

    // #[test]
    // fn test_temporary_artifact_dir_unit() {
    //     let store = LegacyStore::new("/tests");

    //     assert_eq!(
    //         store
    //             .get_temporary_artifact_dir(Ident::new("foo/bar").unwrap(), ArtifactKind::Primary)
    //             .unwrap(),
    //         Path::new("/tests/foo/bar/out"),
    //     );
    //     assert_eq!(
    //         store
    //             .get_temporary_artifact_dir(Ident::new("foo/bar").unwrap(), ArtifactKind::Reference)
    //             .unwrap(),
    //         Path::new("/tests/foo/bar/ref"),
    //     );
    //     assert_eq!(
    //         store
    //             .get_temporary_artifact_dir(
    //                 Ident::new("foo/bar").unwrap(),
    //                 ArtifactKind::Difference
    //             )
    //             .unwrap(),
    //         Path::new("/tests/foo/bar/diff"),
    //     );
    // }

    // #[test]
    // #[should_panic]
    // fn test_temporary_artifact_dir_doc() {
    //     let store = LegacyStore::new("/tests");

    //     // doc tests are not supported by the legacy store
    //     store
    //         .get_temporary_artifact_dir(
    //             Ident::new("foo/bar#qux:zir").unwrap(),
    //             ArtifactKind::Primary,
    //         )
    //         .unwrap();
    // }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_persistent_reference_dir_template() {
//         let store = V1Store::new("/tests", "/store");

//         assert_eq!(
//             store.get_persistent_reference_dir(Ident::new("@template").unwrap()),
//             Path::new("/store/ref/template/@template"),
//         );
//     }

//     #[test]
//     fn test_persistent_reference_dir_unit() {
//         let store = V1Store::new("/tests", "/store");

//         assert_eq!(
//             store.get_persistent_reference_dir(Ident::new("foo/bar").unwrap()),
//             Path::new("/store/ref/unit/foo/bar"),
//         );
//     }

//     #[test]
//     fn test_persistent_reference_dir_doc() {
//         let store = V1Store::new("/tests", "/store");

//         assert_eq!(
//             store.get_persistent_reference_dir(Ident::new("foo/bar#qux:zir").unwrap()),
//             Path::new("/store/ref/doc/foo/bar/qux/zir"),
//         );
//     }

//     #[test]
//     fn test_temporary_artifact_dir_template() {
//         let store = V1Store::new("/tests", "/store");

//         assert_eq!(
//             store.get_temporary_artifact_dir(
//                 Uuid::nil(),
//                 Ident::new("@template").unwrap(),
//                 ArtifactKind::Primary
//             ),
//             Path::new("/store/tmp/00000000-0000-0000-0000-000000000000/template/@template/out"),
//         );
//         assert_eq!(
//             store.get_temporary_artifact_dir(
//                 Uuid::nil(),
//                 Ident::new("@template").unwrap(),
//                 ArtifactKind::Reference
//             ),
//             Path::new("/store/tmp/00000000-0000-0000-0000-000000000000/template/@template/ref"),
//         );
//         assert_eq!(
//             store.get_temporary_artifact_dir(
//                 Uuid::nil(),
//                 Ident::new("@template").unwrap(),
//                 ArtifactKind::Difference
//             ),
//             Path::new("/store/tmp/00000000-0000-0000-0000-000000000000/template/@template/diff"),
//         );
//     }

//     #[test]
//     fn test_temporary_artifact_dir_unit() {
//         let store = V1Store::new("/tests", "/store");

//         assert_eq!(
//             store.get_temporary_artifact_dir(
//                 Uuid::nil(),
//                 Ident::new("foo/bar").unwrap(),
//                 ArtifactKind::Primary
//             ),
//             Path::new("/store/tmp/00000000-0000-0000-0000-000000000000/unit/foo/bar/out"),
//         );
//         assert_eq!(
//             store.get_temporary_artifact_dir(
//                 Uuid::nil(),
//                 Ident::new("foo/bar").unwrap(),
//                 ArtifactKind::Reference
//             ),
//             Path::new("/store/tmp/00000000-0000-0000-0000-000000000000/unit/foo/bar/ref"),
//         );
//         assert_eq!(
//             store.get_temporary_artifact_dir(
//                 Uuid::nil(),
//                 Ident::new("foo/bar").unwrap(),
//                 ArtifactKind::Difference
//             ),
//             Path::new("/store/tmp/00000000-0000-0000-0000-000000000000/unit/foo/bar/diff"),
//         );
//     }

//     #[test]
//     fn test_temporary_artifact_dir_doc() {
//         let store = V1Store::new("/tests", "/store");

//         assert_eq!(
//             store.get_temporary_artifact_dir(
//                 Uuid::nil(),
//                 Ident::new("foo/bar#qux:zir").unwrap(),
//                 ArtifactKind::Primary
//             ),
//             Path::new("/store/tmp/00000000-0000-0000-0000-000000000000/doc/foo/bar/qux/zir/out"),
//         );
//         assert_eq!(
//             store.get_temporary_artifact_dir(
//                 Uuid::nil(),
//                 Ident::new("foo/bar#qux:zir").unwrap(),
//                 ArtifactKind::Reference
//             ),
//             Path::new("/store/tmp/00000000-0000-0000-0000-000000000000/doc/foo/bar/qux/zir/ref"),
//         );
//         assert_eq!(
//             store.get_temporary_artifact_dir(
//                 Uuid::nil(),
//                 Ident::new("foo/bar#qux:zir").unwrap(),
//                 ArtifactKind::Difference
//             ),
//             Path::new("/store/tmp/00000000-0000-0000-0000-000000000000/doc/foo/bar/qux/zir/diff"),
//         );
//     }
// }
