//! A world builder for short lived test worlds with shared resources.

use std::collections::HashMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

use typst::Library;
use typst::LibraryExt;
use typst::World;
use typst::diag::FileError;
use typst::diag::FileResult;
use typst::diag::PackageError;
use typst::ecow::EcoString;
use typst::ecow::eco_format;
use typst::foundations::Bytes;
use typst::syntax::FileId;
use typst::syntax::Source;
use typst::syntax::VirtualPath;
use typst::syntax::package::PackageSpec;
use typst_kit::download::Downloader;
use typst_kit::fonts::Fonts;
use typst_kit::package::PackageStorage;
use tytanic_core::config::ProjectConfig;
use tytanic_core::config::SettingsConfig;
use tytanic_core::config::TestConfig;
use tytanic_core::project::ProjectContext;
use tytanic_core::test::Test;
use tytanic_typst_library::augmented_default_library;
use tytanic_utils::forward_trait;
use tytanic_utils::typst::world::ComposedDynWorld;
use tytanic_utils::typst::world::ProvideDatetime;
use tytanic_utils::typst::world::ProvideFile;
use tytanic_utils::typst::world::ProvideFont;
use tytanic_utils::typst::world::ProvideLibrary;
use tytanic_utils::typst::world::default::FilesystemFileProvider;
use tytanic_utils::typst::world::default::FilesystemFontProvider;
use tytanic_utils::typst::world::default::FilesystemSlotCache;
use tytanic_utils::typst::world::default::FixedDateProvider;
use tytanic_utils::typst::world::default::LibraryProvider;
use tytanic_utils::typst::world::default::SystemDateProvider;

/// A trait for providing access to worlds for each test in a test run.
pub trait Provider: Debug + Send + Sync {
    /// Provides a [`World`] implementation for the given test.
    fn provide(&self, ctx: &ProjectContext, test: &Test, is_primary: bool) -> Arc<dyn World>;

    /// Reset the worlds for the next compilation.
    fn reset(&self);
}

forward_trait! {
    impl<W> Provider for [std::boxed::Box<W>, std::sync::Arc<W>, &W, &mut W] {
        fn provide(&self, ctx: &ProjectContext, test: &Test, is_primary: bool) -> Arc<dyn World> {
            W::provide(self, ctx, test, is_primary)
        }

        fn reset(&self) {
            W::reset(self);
        }
    }
}

/// A template file provider shim.
///
/// This provides template access through the preview import directly if the
/// version matches and stores access to older versions for diagnostics.
///
/// # Examples
/// For a template test in a package called `template` with the version `0.4.2`
/// this shim will provide regular file access to the.
/// ```typst
/// #import "@preview/template:0.4.2" // doesn't actually attempt a download
/// ```
#[derive(Debug)]
pub struct TemplateFileProviderShim<P, T> {
    project: P,
    template: T,
    spec: PackageSpec,
}

impl<P, T> TemplateFileProviderShim<P, T> {
    /// Creates a new template file provider shim.
    pub fn new(project: P, template: T, spec: PackageSpec) -> Self {
        Self {
            project,
            template,
            spec,
        }
    }
}

impl<P, T> TemplateFileProviderShim<P, T> {
    /// The base provider used for all other files.
    pub fn project_provider(&self) -> &P {
        &self.project
    }

    /// The target provider used for files in the spec.
    pub fn template_provider(&self) -> &T {
        &self.template
    }

    /// The spec to re-route the imports for.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }
}

impl<B, T> ProvideFile for TemplateFileProviderShim<B, T>
where
    B: ProvideFile,
    T: ProvideFile,
{
    fn provide_source(&self, id: FileId) -> FileResult<Source> {
        let Some(spec) = id.package() else {
            return self.template.provide_source(id);
        };

        if spec.namespace == self.spec.namespace
            && spec.name == self.spec.name
            && spec.version == self.spec.version
        {
            let id = FileId::new(None, id.vpath().clone());
            self.project.provide_source(id)
        } else if spec.namespace == self.spec.namespace && spec.name == self.spec.name {
            Err(FileError::Package(PackageError::Other(Some(eco_format!(
                "Attempted to access {spec} in test for {}",
                self.spec,
            )))))
        } else {
            self.template.provide_source(id)
        }
    }

    fn provide_bytes(&self, id: FileId) -> FileResult<Bytes> {
        let Some(spec) = id.package() else {
            return self.template.provide_bytes(id);
        };

        if spec.namespace == self.spec.namespace
            && spec.name == self.spec.name
            && spec.version == self.spec.version
        {
            let id = FileId::new(None, id.vpath().clone());
            self.project.provide_bytes(id)
        } else if spec.namespace == self.spec.namespace && spec.name == self.spec.name {
            Err(FileError::Package(PackageError::Other(Some(eco_format!(
                "Attempted to access {spec} in test for {}",
                self.spec,
            )))))
        } else {
            self.template.provide_bytes(id)
        }
    }

    fn reset_all(&self) {
        self.project.reset_all();
        self.template.reset_all();
    }
}

type TemplateFileProvider =
    TemplateFileProviderShim<Arc<FilesystemFileProvider>, FilesystemFileProvider>;

// TODO(tinger): Optimization opportunity: Not all of these need to be `Arc`s,
// nor do the constructors always need to return `Arc` for short lived small
// providers if the composed provider has a narrower set of types. This could be
// done by using library directly instead of a provider and combining fixed and
// system time providers into an enum.

/// A default implementation of the [`ProvideWorld`] trait.
///
/// This provider builds shallow world shims using [`ComposedDynWorld`] and
/// filesystem based default providers. Providers are cached under the
/// assumption that the configurations provided by [`ProjectContext`] do not
/// change between calls.
#[derive(Debug)]
pub struct WorldProvider {
    package_storage: OnceLock<Arc<PackageStorage>>,

    project_file_provider_cache: OnceLock<FilesystemSlotCache>,
    template_file_provider_cache: OnceLock<FilesystemSlotCache>,

    system_font_provider: OnceLock<Arc<FilesystemFontProvider>>,
    hermetic_font_provider: OnceLock<Arc<FilesystemFontProvider>>,

    no_inputs_augmented_library_provider: OnceLock<Arc<LibraryProvider>>,
    no_inputs_standard_library_provider: OnceLock<Arc<LibraryProvider>>,

    system_datetime_provider: OnceLock<Arc<SystemDateProvider>>,
}

impl WorldProvider {
    /// Creates a new default world provider.
    pub fn new() -> Self {
        Self {
            package_storage: OnceLock::new(),
            project_file_provider_cache: OnceLock::new(),
            template_file_provider_cache: OnceLock::new(),
            system_font_provider: OnceLock::new(),
            hermetic_font_provider: OnceLock::new(),
            no_inputs_augmented_library_provider: OnceLock::new(),
            no_inputs_standard_library_provider: OnceLock::new(),
            system_datetime_provider: OnceLock::new(),
        }
    }
}

impl WorldProvider {
    fn package_storage(&self, ctx: &ProjectContext, test: &Test) -> Option<Arc<PackageStorage>> {
        let allow_packages = ctx.config().get_test_config_member(
            test.as_unit().and_then(|t| t.config()),
            TestConfig::ALLOW_PACKAGES,
            (),
        );

        if !allow_packages {
            return None;
        }

        Some(
            self.package_storage
                .get_or_init(|| {
                    Arc::new(PackageStorage::new(
                        ctx.config()
                            .get_settings_member(SettingsConfig::PACKAGE_CACHE_PATH, ())
                            .map(Into::into),
                        ctx.config()
                            .get_settings_member(SettingsConfig::PACKAGE_PATH, ())
                            .map(Into::into),
                        Downloader::new(format!(
                            "{}/{}",
                            tytanic_core::TOOL_NAME,
                            env!("CARGO_PKG_VERSION")
                        )),
                    ))
                })
                .clone(),
        )
    }

    fn project_file_provider(
        &self,
        ctx: &ProjectContext,
        test: &Test,
    ) -> Arc<FilesystemFileProvider> {
        let package_storage = self.package_storage(ctx, test);
        let cache = self
            .project_file_provider_cache
            .get_or_init(FilesystemSlotCache::new)
            .clone();

        Arc::new(FilesystemFileProvider::new(
            ctx.root(),
            cache,
            package_storage,
        ))
    }

    fn template_file_provider(
        &self,
        ctx: &ProjectContext,
        test: &Test,
    ) -> Option<Arc<TemplateFileProvider>> {
        let manifest = ctx.manifest()?;
        let template = manifest.template.as_ref()?;

        let root = ctx.root().join(template.path.as_str());

        let spec = PackageSpec {
            namespace: EcoString::from("preview"),
            name: manifest.package.name.clone(),
            version: manifest.package.version,
        };

        let package_storage = self.package_storage(ctx, test);
        let project = self.project_file_provider(ctx, test);

        let cache = self
            .template_file_provider_cache
            .get_or_init(FilesystemSlotCache::new)
            .clone();

        Some(Arc::new(TemplateFileProviderShim::new(
            project,
            FilesystemFileProvider::new(root, cache, package_storage),
            spec,
        )))
    }

    /// Creates a file provider.
    ///
    /// This will respect the [`allow_packages`] test config option as well as
    /// the [`package_cache_path`] and [`package_path`] settings config
    /// options.
    ///
    /// [`allow_packages`]: crate::config::TestConfig::allow_packages
    /// [`package_cache_path`]: crate::config::SettingsConfig::package_cache_path
    /// [`package_path`]: crate::config::SettingsConfig::package_path
    pub fn file_provider(&self, ctx: &ProjectContext, test: &Test) -> Option<Arc<dyn ProvideFile>> {
        match test {
            Test::Template(_) => self.template_file_provider(ctx, test).map(|p| p as _),
            Test::Unit(_) => Some(self.project_file_provider(ctx, test)),
            Test::Doc(_) => todo!(),
        }
    }

    fn system_font_provider(&self, ctx: &ProjectContext) -> Arc<FilesystemFontProvider> {
        self.system_font_provider
            .get_or_init(|| {
                let font_dirs = ctx
                    .config()
                    .get_project_config_member(ProjectConfig::FONT_PATHS, ());

                let fonts = Fonts::searcher()
                    .include_system_fonts(true)
                    .search_with(font_dirs.iter().map(|p| ctx.root().join(p)));
                Arc::new(FilesystemFontProvider::from_searcher(fonts))
            })
            .clone()
    }

    fn hermetic_font_provider(&self, ctx: &ProjectContext) -> Arc<FilesystemFontProvider> {
        self.hermetic_font_provider
            .get_or_init(|| {
                let font_dirs = ctx
                    .config()
                    .get_project_config_member(ProjectConfig::FONT_PATHS, ());

                let fonts = Fonts::searcher()
                    .include_system_fonts(false)
                    .search_with(font_dirs.iter().map(|p| ctx.root().join(p)));
                Arc::new(FilesystemFontProvider::from_searcher(fonts))
            })
            .clone()
    }

    /// Creates a font provider.
    ///
    /// This will respect the [`font_dirs`] project config and
    /// [`use_system_fonts`] test config.
    ///
    /// [`font_dirs`]: crate::config::ProjectConfig::font_dirs
    /// [`use_system_fonts`]: crate::config::TestConfig::use_system_fonts
    fn font_provider(&self, ctx: &ProjectContext, test: &Test) -> Arc<dyn ProvideFont> {
        // TODO: embedded + system fonts -> each combination a font provider?
        let _use_embedded_fonts = ctx.config().get_test_config_member(
            test.as_unit().and_then(|t| t.config()),
            TestConfig::USE_EMBEDDED_FONTS,
            (),
        );

        let use_system_fonts = ctx.config().get_test_config_member(
            test.as_unit().and_then(|t| t.config()),
            TestConfig::USE_SYSTEM_FONTS,
            (),
        );

        // TODO(tinger): See if the font fonts loaded form the font dirs can be
        // shared. They should in theory always have the lower indices such that
        // if we know where the system fonts start we have an index after which
        // we reject font access.
        if use_system_fonts {
            self.system_font_provider(ctx)
        } else {
            self.hermetic_font_provider(ctx)
        }
    }

    fn augmented_library_provider(
        &self,
        inputs: Option<&HashMap<EcoString, EcoString>>,
    ) -> Arc<LibraryProvider> {
        if let Some(inputs) = inputs.filter(|i| !i.is_empty()) {
            Arc::new(LibraryProvider::with_library(augmented_default_library()))
        } else {
            self.no_inputs_augmented_library_provider
                .get_or_init(|| {
                    Arc::new(LibraryProvider::with_library(augmented_default_library()))
                })
                .clone()
        }
    }

    fn standard_library_provider(
        &self,
        inputs: Option<&HashMap<EcoString, EcoString>>,
    ) -> Arc<LibraryProvider> {
        if let Some(inputs) = inputs.filter(|i| !i.is_empty()) {
            Arc::new(LibraryProvider::with_library(Library::builder()))
        } else {
            self.no_inputs_standard_library_provider
                .get_or_init(|| Arc::new(LibraryProvider::with_library(Library::default())))
                .clone()
        }
    }

    /// Creates a library provider.
    pub fn library_provider(&self, ctx: &ProjectContext, test: &Test) -> Arc<dyn ProvideLibrary> {
        let use_augmented_library = ctx.config().get_test_config_member(
            test.as_unit().and_then(|t| t.config()),
            TestConfig::USE_AUGMENTED_LIBRARY,
            test.kind(),
        );

        let inputs = ctx.config().get_test_config_member(
            test.as_unit().and_then(|t| t.config()),
            TestConfig::INPUTS,
            test.kind(),
        );

        if use_augmented_library {
            self.augmented_library_provider(test)
        } else {
            self.standard_library_provider(test)
        }
    }

    fn fixed_datetime_provider(&self, ctx: &ProjectContext, test: &Test) -> Arc<FixedDateProvider> {
        let timestamp = ctx.config().get_test_config_member(
            test.as_unit().and_then(|t| t.config()),
            TestConfig::TIMESTAMP,
            (),
        );

        Arc::new(FixedDateProvider::new(timestamp))
    }

    fn system_datetime_provider(&self) -> Arc<SystemDateProvider> {
        self.system_datetime_provider
            .get_or_init(|| Arc::new(SystemDateProvider::new()))
            .clone()
    }

    /// Creates a datetime provider.
    ///
    /// This will respect the [`use_system_datetime`] and [`timestamp`] test
    /// config.
    ///
    /// [`use_system_datetime`]: crate::config::TestConfig::use_system_datetime
    /// [`timestamp`]: crate::config::TestConfig::timestamp
    pub fn datetime_provider(&self, ctx: &ProjectContext, test: &Test) -> Arc<dyn ProvideDatetime> {
        let use_system_datetime = ctx.config().get_test_config_member(
            test.as_unit().and_then(|t| t.config()),
            TestConfig::USE_SYSTEM_DATETIME,
            (),
        );

        if use_system_datetime {
            self.system_datetime_provider()
        } else {
            self.fixed_datetime_provider(ctx, test)
        }
    }
}

impl WorldProvider {
    /// Creates a new world for this template test.
    ///
    /// Returns `None` if it isn't a template test.
    fn template_world(&self, ctx: &ProjectContext, test: &Test) -> Option<Arc<ComposedDynWorld>> {
        tracing::trace!(test = %test.ident(), "creating template world");

        let file_id = FileId::new(
            None,
            VirtualPath::new(
                ctx.manifest()
                    .and_then(|m| m.template.as_ref())?
                    .entrypoint
                    .as_str(),
            ),
        );

        let files = self.template_file_provider(ctx, test)?;
        let fonts = self.font_provider(ctx, test);
        let library = self.library_provider(ctx, test);
        let datetime = self.datetime_provider(ctx, test);

        let world = Arc::new(
            ComposedDynWorld::builder()
                .file_provider(files)
                .font_provider(fonts)
                .library_provider(library)
                .datetime_provider(datetime)
                .build(file_id),
        );

        tracing::trace!(?file_id, "created template world");
        Some(world)
    }

    /// Creates a new unit world for this template test.
    ///
    /// Returns `None` if it isn't a template test.
    fn unit_world(
        &self,
        ctx: &ProjectContext,
        test: &Test,
        is_primary: bool,
    ) -> Option<Arc<ComposedDynWorld>> {
        tracing::trace!(test = %test.ident(), "creating unit world");

        let unit_test = test.as_unit()?;
        let mut path = PathBuf::new();

        path.push(
            ctx.config()
                .get_project_config_member(ProjectConfig::UNIT_TESTS_ROOT, ctx.store().kind()),
        );
        path.push(unit_test.ident().path());
        path.push(if is_primary { "test.typ" } else { "ref.typ" });

        let file_id = FileId::new(None, VirtualPath::new(path));

        let files = self.project_file_provider(ctx, test);
        let fonts = self.font_provider(ctx, test);
        let library = self.library_provider(ctx, test);
        let datetime = self.datetime_provider(ctx, test);

        let world = Arc::new(
            ComposedDynWorld::builder()
                .file_provider(files)
                .font_provider(fonts)
                .library_provider(library)
                .datetime_provider(datetime)
                .build(file_id),
        );

        tracing::trace!(?file_id, "created unit world");
        Some(world)
    }
}

impl Provider for WorldProvider {
    fn provide(&self, ctx: &ProjectContext, test: &Test, is_primary: bool) -> Arc<dyn World> {
        match test {
            Test::Template(_) => self.template_world(ctx, test).unwrap(),
            Test::Unit(_) => self.unit_world(ctx, test, is_primary).unwrap(),
            Test::Doc(_) => todo!(),
        }
    }

    fn reset(&self) {
        if let Some(project_file_provider_cache) = self.project_file_provider_cache.get() {
            project_file_provider_cache.reset_slots();
        }

        if let Some(template_file_provider_cache) = self.template_file_provider_cache.get() {
            template_file_provider_cache.reset_slots();
        }

        if let Some(system_datetime_provider) = self.system_datetime_provider.get() {
            system_datetime_provider.reset_today();
        }
    }
}

impl Default for WorldProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use typst::syntax::Source;
    use typst::syntax::VirtualPath;
    use typst::syntax::package::PackageVersion;

    use super::*;
    use tytanic_utils::typst::world::ProvideFile;
    use tytanic_utils::typst::world::default::VirtualFileProvider;
    use tytanic_utils::typst::world::default::VirtualSlot;

    #[test]
    fn test_template_file_provider_shim() {
        let spec = PackageSpec {
            namespace: "preview".into(),
            name: "self".into(),
            version: PackageVersion {
                major: 0,
                minor: 1,
                patch: 0,
            },
        };

        let project_lib_id = FileId::new(None, VirtualPath::new("lib.typ"));
        let template_lib_id = FileId::new(None, VirtualPath::new("lib.typ"));
        let template_main_id = FileId::new(None, VirtualPath::new("main.typ"));

        let project_lib = Source::new(project_lib_id, "#let foo(bar) = bar".into());
        let template_lib = Source::new(template_lib_id, "#let bar = [qux]".into());
        let template_main = Source::new(
            template_main_id,
            "#import \"@preview/self:0.1.0\"\n#show foo".into(),
        );

        let mut project = HashMap::new();
        let mut template = HashMap::new();

        project.insert(
            project_lib.id(),
            VirtualSlot::from_source(project_lib.clone()),
        );
        template.insert(
            template_lib.id(),
            VirtualSlot::from_source(template_lib.clone()),
        );
        template.insert(
            template_main.id(),
            VirtualSlot::from_source(template_main.clone()),
        );

        let project = VirtualFileProvider::from_slots(project);
        let template = VirtualFileProvider::from_slots(template);

        let shim = TemplateFileProviderShim::new(project, template, spec.clone());

        // lib.typ is available inside the template
        assert_eq!(
            shim.provide_source(FileId::new(None, VirtualPath::new("lib.typ")),)
                .unwrap()
                .text(),
            template_lib.text()
        );

        // main.typ is available inside the template
        assert_eq!(
            shim.provide_source(FileId::new(None, VirtualPath::new("main.typ")),)
                .unwrap()
                .text(),
            template_main.text()
        );

        // lib.typ is also available from the project
        assert_eq!(
            shim.provide_source(FileId::new(Some(spec), VirtualPath::new("lib.typ")),)
                .unwrap()
                .text(),
            project_lib.text()
        );
    }
}
