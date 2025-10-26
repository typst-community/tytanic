//! Parsing and validation of configuration.

use std::collections::HashMap;
use std::fmt::Display;
use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;

use chrono::DateTime;
use chrono::Utc;
use ecow::EcoString;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;
use typst_syntax::package::PackageManifest;
use tytanic_utils::result::ResultEx;
use tytanic_utils::result::io_not_found;

use crate::config::private::ResolveExt;
use crate::test::Annotation;
use crate::test::Kind as TestKind;
use crate::test::UnitKind;

mod private {
    use super::*;

    pub(super) trait Sealed {}
    pub(super) trait ResolveExt: Resolve {
        /// Create the source for this config option.
        fn source(source: PartialConfigSource) -> ConfigSource {
            let f = move || {
                Some(match source {
                    PartialConfigSource::EnvironmentVariable => ConfigSource::EnvironmentVariable {
                        name: Self::environment_variable()?.into(),
                    },
                    PartialConfigSource::CommandLine => {
                        if let Some((long, short)) = Self::command_line_argument() {
                            ConfigSource::CommandLineArgument {
                                long: long.into(),
                                short: short.map(Into::into),
                            }
                        } else if let Some(stem) = Self::command_line_switch() {
                            ConfigSource::CommandLineSwitch { stem: stem.into() }
                        } else {
                            return None;
                        }
                    }
                    // TODO: Separate these manifest from manifest config.
                    PartialConfigSource::ConfigFile { path, manifest } => {
                        ConfigSource::ConfigFile {
                            path,
                            key: if manifest {
                                if let Some(manifest) = Self::manifest() {
                                    manifest.into()
                                } else {
                                    format!("tool.{}.{}", crate::TOOL_NAME, Self::config_file()?)
                                }
                            } else {
                                Self::config_file()?.into()
                            },
                        }
                    }
                    PartialConfigSource::Override => ConfigSource::Override {
                        name: Self::name().into(),
                    },
                })
            };

            f().unwrap_or(ConfigSource::Override {
                name: Self::name().into(),
            })
        }
    }

    impl<R> ResolveExt for R where R: Resolve {}
}

/// A trait for config options.
#[expect(private_bounds, reason = "this trait is sealed")]
pub trait Resolve: private::Sealed {
    /// The type of the config to retrieve this from.
    type Config;

    /// The context during default retrieval.
    type DefaultContext<'c>;

    /// The context during validation.
    type ValidateContext<'c>;

    /// The type of the option's value retrieved at a config layer.
    type Get<'c>;

    /// The type of the option after all layers have been queried and the
    /// default was applied.
    type Resolved<'c>;

    /// The name of the option at the environment variable layer or `None` if
    /// this option can't be applied at this layer.
    fn environment_variable() -> Option<&'static str> {
        None
    }

    /// The names of the option at the command line layer if it isn't a switch
    /// or `None` if this option can't be applied at this layer.
    fn command_line_argument() -> Option<(&'static str, Option<char>)> {
        None
    }

    /// The name of the option at the command line layer if it is a switch or
    /// `None` if this option can't be applied at this layer.
    fn command_line_switch() -> Option<&'static str> {
        None
    }

    /// The name of the option at the config file layer or `None` if this option
    /// can't be applied at this layer.
    ///
    /// This should be mutually exclusive with `config_file` as it refers to
    /// options directly from the manifest like `template.path`.
    fn manifest() -> Option<&'static str> {
        None
    }

    /// The name of the option at the config file layer or `None` if this option
    /// can't be applied at this layer.
    ///
    /// This should be mutually exclusive with `manifest` as it refers to
    /// options in config files or the manifest config section `tool.tytanic`.
    fn config_file() -> Option<&'static str> {
        None
    }

    /// The name of the option at the annotation layer or `None` if this option
    /// can't be applied at this layer.
    fn annotation() -> Option<&'static str> {
        None
    }

    /// The name of the option at the override layer.
    ///
    /// This is used as a fallback if no other layers apply.
    fn name() -> &'static str;

    /// Get the option's value from a config.
    fn get(config: &Self::Config) -> Option<Self::Get<'_>>;

    /// Get the default value of the option according to the context.
    fn default(ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_>;

    /// Resolve the option's value into its resolved type.
    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_>;

    /// Validates the config value for the given source.
    fn validate(
        _ctx: Self::ValidateContext<'_>,
        _source: PartialConfigSource,
        _value: Self::Resolved<'_>,
    ) -> Result<(), ValidationError> {
        Ok(())
    }
}

/// The resolve type for [`ProjectConfig::template_path`].
pub struct ProjectTemplatePath;
impl private::Sealed for ProjectTemplatePath {}
impl Resolve for ProjectTemplatePath {
    type Config = ProjectConfig;
    type ValidateContext<'c> = &'c Path;
    type DefaultContext<'c> = ();
    type Get<'c> = &'c str;
    type Resolved<'c> = Option<&'c str>;

    fn manifest() -> Option<&'static str> {
        Some("template.path")
    }

    fn name() -> &'static str {
        "ProjectConfig::template_path"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.template_path.as_deref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        None
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        Some(get)
    }

    fn validate(
        ctx: Self::ValidateContext<'_>,
        source: PartialConfigSource,
        value: Self::Resolved<'_>,
    ) -> Result<(), ValidationError> {
        if let Some(value) = value {
            validate_project_path(Self::source(source), ctx, Path::new(value))?;
        }

        Ok(())
    }
}

/// The resolve type for [`ProjectConfig::template_entrypoint`].
pub struct ProjectTemplateEntrypoint;
impl private::Sealed for ProjectTemplateEntrypoint {}
impl Resolve for ProjectTemplateEntrypoint {
    type Config = ProjectConfig;
    type ValidateContext<'c> = (&'c Path, &'c Path);
    type DefaultContext<'c> = ();
    type Get<'c> = &'c str;
    type Resolved<'c> = Option<&'c str>;

    fn manifest() -> Option<&'static str> {
        Some("template.entrypoint")
    }

    fn name() -> &'static str {
        "ProjectConfig::template_entrypoint"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.template_entrypoint.as_deref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        None
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        Some(get)
    }

    fn validate(
        ctx: Self::ValidateContext<'_>,
        source: PartialConfigSource,
        value: Self::Resolved<'_>,
    ) -> Result<(), ValidationError> {
        if let Some(value) = value {
            validate_project_path(Self::source(source), ctx.0, &ctx.1.join(value))?;
        }

        Ok(())
    }
}

/// The resolve type for [`ProjectConfig::store_root`].
pub struct ProjectStoreRoot;
impl private::Sealed for ProjectStoreRoot {}
impl Resolve for ProjectStoreRoot {
    type Config = ProjectConfig;
    type ValidateContext<'c> = &'c Path;
    type DefaultContext<'c> = ();
    type Get<'c> = &'c str;
    type Resolved<'c> = &'c str;

    fn config_file() -> Option<&'static str> {
        Some("tests")
    }

    fn name() -> &'static str {
        "ProjectConfig::store_root"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.store_root.as_deref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        "tests"
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        get
    }

    fn validate(
        ctx: Self::ValidateContext<'_>,
        source: PartialConfigSource,
        value: Self::Resolved<'_>,
    ) -> Result<(), ValidationError> {
        validate_project_path(Self::source(source), ctx, Path::new(value))
    }
}

/// The resolve type for [`ProjectConfig::font_dirs`].
pub struct ProjectFontPaths;
impl private::Sealed for ProjectFontPaths {}
impl Resolve for ProjectFontPaths {
    type Config = ProjectConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = &'c Path;
    type Get<'c> = &'c [String];
    type Resolved<'c> = &'c [String];

    fn environment_variable() -> Option<&'static str> {
        Some("TYPST_FONT_PATHS")
    }

    fn command_line_argument() -> Option<(&'static str, Option<char>)> {
        Some(("font-path", None))
    }

    fn config_file() -> Option<&'static str> {
        Some("fonts")
    }

    fn name() -> &'static str {
        "ProjectConfig::font_dirs"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.font_paths.as_deref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        &[]
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        get
    }

    fn validate(
        ctx: Self::ValidateContext<'_>,
        source: PartialConfigSource,
        value: Self::Resolved<'_>,
    ) -> Result<(), ValidationError> {
        for dir in value {
            validate_project_path(Self::source(source.clone()), ctx, Path::new(dir))?;
        }

        Ok(())
    }
}

/// The resolve type for [`ProjectConfig::optimize_refs`].
pub struct ProjectOptimizeRefs;
impl private::Sealed for ProjectOptimizeRefs {}
impl Resolve for ProjectOptimizeRefs {
    type Config = ProjectConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c bool;
    type Resolved<'c> = bool;

    fn command_line_switch() -> Option<&'static str> {
        Some("optimize-refs")
    }

    fn config_file() -> Option<&'static str> {
        Some("optimize-refs")
    }

    fn name() -> &'static str {
        "ProjectConfig::optimize_refs"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.optimize_refs.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        true
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`SettingsConfig::package_cache_path`].
pub struct SettingsPackageCachePath;
impl private::Sealed for SettingsPackageCachePath {}
impl Resolve for SettingsPackageCachePath {
    type Config = SettingsConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c str;
    type Resolved<'c> = Option<&'c str>;

    fn environment_variable() -> Option<&'static str> {
        Some("TYPST_PACKAGE_CACHE_PATH")
    }

    fn command_line_argument() -> Option<(&'static str, Option<char>)> {
        Some(("package-cache-path", None))
    }

    fn config_file() -> Option<&'static str> {
        Some("package-cache-path")
    }

    fn name() -> &'static str {
        "SettingsConfig::package_cache_path"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.package_cache_path.as_deref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        None
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        Some(get)
    }

    fn validate(
        _ctx: Self::ValidateContext<'_>,
        source: PartialConfigSource,
        value: Self::Resolved<'_>,
    ) -> Result<(), ValidationError> {
        if let Some(value) = value {
            validate_settings_path(Self::source(source), Path::new(value))?;
        }

        Ok(())
    }
}

/// The resolve type for [`SettingsConfig::jobs`].
pub struct SettingsJobs;
impl private::Sealed for SettingsJobs {}
impl Resolve for SettingsJobs {
    type Config = SettingsConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c NonZeroUsize;
    type Resolved<'c> = Option<&'c NonZeroUsize>;

    fn command_line_argument() -> Option<(&'static str, Option<char>)> {
        Some(("jobs", Some('j')))
    }

    fn config_file() -> Option<&'static str> {
        Some("jobs")
    }

    fn name() -> &'static str {
        "SettingsConfig::jobs"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.jobs.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        None
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        Some(get)
    }
}

/// The resolve type for [`SettingsConfig::package_path`].
pub struct SettingsPackagePath;
impl private::Sealed for SettingsPackagePath {}
impl Resolve for SettingsPackagePath {
    type Config = SettingsConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c str;
    type Resolved<'c> = Option<&'c str>;

    fn environment_variable() -> Option<&'static str> {
        Some("TYPST_PACKAGE_PATH")
    }

    fn command_line_argument() -> Option<(&'static str, Option<char>)> {
        Some(("package-path", None))
    }

    fn config_file() -> Option<&'static str> {
        Some("package-path")
    }

    fn name() -> &'static str {
        "SettingsConfig::package_path"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.package_path.as_deref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        None
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        Some(get)
    }

    fn validate(
        _ctx: Self::ValidateContext<'_>,
        source: PartialConfigSource,
        value: Self::Resolved<'_>,
    ) -> Result<(), ValidationError> {
        if let Some(value) = value {
            validate_settings_path(Self::source(source), Path::new(value))?;
        }

        Ok(())
    }
}

/// The resolve type for [`SettingsConfig::fail_fast`].
pub struct SettingsFailFast;
impl private::Sealed for SettingsFailFast {}
impl Resolve for SettingsFailFast {
    type Config = SettingsConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c bool;
    type Resolved<'c> = bool;

    fn command_line_switch() -> Option<&'static str> {
        Some("fail-fast")
    }

    fn config_file() -> Option<&'static str> {
        Some("fail-fast")
    }

    fn name() -> &'static str {
        "SettingsConfig::fail_fast"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.fail_fast.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        true
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`SettingsConfig::export_ephemeral`].
pub struct SettingsExportEphemeral;
impl private::Sealed for SettingsExportEphemeral {}
impl Resolve for SettingsExportEphemeral {
    type Config = SettingsConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c bool;
    type Resolved<'c> = bool;

    fn command_line_switch() -> Option<&'static str> {
        Some("export-ephemeral")
    }

    fn config_file() -> Option<&'static str> {
        Some("export-ephemeral")
    }

    fn name() -> &'static str {
        "SettingsConfig::export_ephemeral"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.export_ephemeral.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        true
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`SettingsConfig::template`].
pub struct SettingsTemplate;
impl private::Sealed for SettingsTemplate {}
impl Resolve for SettingsTemplate {
    type Config = SettingsConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c bool;
    type Resolved<'c> = bool;

    fn command_line_switch() -> Option<&'static str> {
        Some("template")
    }

    fn config_file() -> Option<&'static str> {
        Some("template")
    }

    fn name() -> &'static str {
        "SettingsConfig::template"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.template.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        true
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`TestConfig::skip`].
///
/// Note that this is NOT the same as the `skip` CLI option, the test CLI option
/// means to automatically exclude the `skip`-annotated tests, this option marks
/// a test as `skip` in the first place.
pub struct TestSkip;
impl private::Sealed for TestSkip {}
impl Resolve for TestSkip {
    type Config = TestConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c bool;
    type Resolved<'c> = bool;

    fn name() -> &'static str {
        "TestConfig::skip"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.skip.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        false
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`TestConfig::compare`].
pub struct TestCompare;
impl private::Sealed for TestCompare {}
impl Resolve for TestCompare {
    type Config = TestConfig;
    type DefaultContext<'c> = TestKind;
    type ValidateContext<'c> = ();
    type Get<'c> = &'c bool;
    type Resolved<'c> = bool;

    fn command_line_switch() -> Option<&'static str> {
        Some("compare")
    }

    fn config_file() -> Option<&'static str> {
        Some("compare")
    }

    fn name() -> &'static str {
        "TestConfig::compare"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.compare.as_ref()
    }

    fn default(ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        matches!(
            ctx,
            TestKind::Unit(UnitKind::Ephemeral | UnitKind::Persistent)
        )
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`TestConfig::use_system_fonts`].
pub struct TestUseSystemFonts;
impl private::Sealed for TestUseSystemFonts {}
impl Resolve for TestUseSystemFonts {
    type Config = TestConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c bool;
    type Resolved<'c> = bool;

    fn command_line_switch() -> Option<&'static str> {
        Some("use-system-fonts")
    }

    fn config_file() -> Option<&'static str> {
        Some("use-system-fonts")
    }

    fn name() -> &'static str {
        "TestConfig::use_system_fonts"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.use_system_fonts.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        false
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`TestConfig::use_embedded_fonts`].
pub struct TestUseEmbeddedFonts;
impl private::Sealed for TestUseEmbeddedFonts {}
impl Resolve for TestUseEmbeddedFonts {
    type Config = TestConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c bool;
    type Resolved<'c> = bool;

    fn command_line_switch() -> Option<&'static str> {
        Some("use-embedded-fonts")
    }

    fn config_file() -> Option<&'static str> {
        Some("use-embedded-fonts")
    }

    fn name() -> &'static str {
        "TestConfig::use_embedded_fonts"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.use_embedded_fonts.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        false
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`TestConfig::use_system_datetime`].
pub struct TestUseSystemDatetime;
impl private::Sealed for TestUseSystemDatetime {}
impl Resolve for TestUseSystemDatetime {
    type Config = TestConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c bool;
    type Resolved<'c> = bool;

    fn command_line_switch() -> Option<&'static str> {
        Some("use-system-datetime")
    }

    fn config_file() -> Option<&'static str> {
        Some("use-system-datetime")
    }

    fn name() -> &'static str {
        "TestConfig::use_system_datetime"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.use_system_datetime.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        false
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`TestConfig::use_augmented_library`].
pub struct TestUseAugmentedLibrary;
impl private::Sealed for TestUseAugmentedLibrary {}
impl Resolve for TestUseAugmentedLibrary {
    type Config = TestConfig;
    type DefaultContext<'c> = TestKind;
    type ValidateContext<'c> = ();
    type Get<'c> = &'c bool;
    type Resolved<'c> = bool;

    fn command_line_switch() -> Option<&'static str> {
        Some("use-augmented-library")
    }

    fn config_file() -> Option<&'static str> {
        Some("use-augmented-library")
    }

    fn name() -> &'static str {
        "TestConfig::use_augmented_library"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.use_augmented_library.as_ref()
    }

    fn default(ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        matches!(ctx, TestKind::Unit(_))
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`TestConfig::timestamp`].
pub struct TestTimestamp;
impl private::Sealed for TestTimestamp {}
impl Resolve for TestTimestamp {
    type Config = TestConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c DateTime<Utc>;
    type Resolved<'c> = DateTime<Utc>;

    fn command_line_argument() -> Option<(&'static str, Option<char>)> {
        Some(("timestamp", None))
    }

    fn config_file() -> Option<&'static str> {
        Some("timestamp")
    }

    fn name() -> &'static str {
        "TestConfig::timestamp"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.timestamp.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        DateTime::UNIX_EPOCH
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`TestConfig::allow_packages`].
pub struct TestAllowPackages;
impl private::Sealed for TestAllowPackages {}
impl Resolve for TestAllowPackages {
    type Config = TestConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c bool;
    type Resolved<'c> = bool;

    fn command_line_switch() -> Option<&'static str> {
        Some("allow-packages")
    }

    fn config_file() -> Option<&'static str> {
        Some("allow-packages")
    }

    fn name() -> &'static str {
        "TestConfig::allow_packages"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.allow_packages.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        true
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`TestConfig::warnings`].
pub struct TestWarnings;
impl private::Sealed for TestWarnings {}
impl Resolve for TestWarnings {
    type Config = TestConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c Warnings;
    type Resolved<'c> = Warnings;

    fn command_line_argument() -> Option<(&'static str, Option<char>)> {
        Some(("warnings", None))
    }

    fn config_file() -> Option<&'static str> {
        Some("warnings")
    }

    fn name() -> &'static str {
        "TestConfig::warnings"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.warnings.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        Warnings::Emit
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`TestConfig::direction`].
pub struct TestDirection;
impl private::Sealed for TestDirection {}
impl Resolve for TestDirection {
    type Config = TestConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c Direction;
    type Resolved<'c> = Direction;

    fn command_line_argument() -> Option<(&'static str, Option<char>)> {
        Some(("dir", None))
    }

    fn config_file() -> Option<&'static str> {
        Some("dir")
    }

    fn name() -> &'static str {
        "TestConfig::direction"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.direction.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        Direction::Ltr
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`TestConfig::pixel_per_inch`].
pub struct TestPixelPerInch;
impl private::Sealed for TestPixelPerInch {}
impl Resolve for TestPixelPerInch {
    type Config = TestConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c f32;
    type Resolved<'c> = f32;

    fn command_line_argument() -> Option<(&'static str, Option<char>)> {
        Some(("ppi", None))
    }

    fn config_file() -> Option<&'static str> {
        Some("ppi")
    }

    fn name() -> &'static str {
        "TestConfig::ppi"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.pixel_per_inch.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        144.0
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`TestConfig::max_delta`].
pub struct TestMaxDelta;
impl private::Sealed for TestMaxDelta {}
impl Resolve for TestMaxDelta {
    type Config = TestConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c u8;
    type Resolved<'c> = u8;

    fn command_line_argument() -> Option<(&'static str, Option<char>)> {
        Some(("max-delta", None))
    }

    fn config_file() -> Option<&'static str> {
        Some("max-delta")
    }

    fn name() -> &'static str {
        "TestConfig::max_delta"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.max_delta.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        1
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`TestConfig::max_deviations`].
pub struct TestMaxDeviations;
impl private::Sealed for TestMaxDeviations {}
impl Resolve for TestMaxDeviations {
    type Config = TestConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c usize;
    type Resolved<'c> = usize;

    fn command_line_argument() -> Option<(&'static str, Option<char>)> {
        Some(("max-deviations", None))
    }

    fn config_file() -> Option<&'static str> {
        Some("max-deviations")
    }

    fn name() -> &'static str {
        "TestConfig::max_deviations"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.max_deviations.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        0
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        *get
    }
}

/// The resolve type for [`TestConfig::inputs`].
pub struct TestInputs;
impl private::Sealed for TestInputs {}
impl Resolve for TestInputs {
    type Config = TestConfig;
    type DefaultContext<'c> = ();
    type ValidateContext<'c> = ();
    type Get<'c> = &'c HashMap<EcoString, EcoString>;
    type Resolved<'c> = &'c HashMap<EcoString, EcoString>;

    fn command_line_argument() -> Option<(&'static str, Option<char>)> {
        // TODO: This won't work without handling which allows multiple uses to
        // combine into a single value.
        // Some(("input", None))
        None
    }

    fn config_file() -> Option<&'static str> {
        // TODO: Without a way to unset things this is hardly useful.
        // Some("inputs")
        None
    }

    fn name() -> &'static str {
        "TestConfig::inputs"
    }

    fn get(config: &Self::Config) -> Option<Self::Get<'_>> {
        config.inputs.as_ref()
    }

    fn default(_ctx: Self::DefaultContext<'_>) -> Self::Resolved<'_> {
        static EMPTY: LazyLock<HashMap<EcoString, EcoString>> = LazyLock::new(HashMap::new);
        &*EMPTY
    }

    fn resolve(get: Self::Get<'_>) -> Self::Resolved<'_> {
        get
    }
}

/// The key used to configure Tytanic in the manifest tool config.
pub const MANIFEST_TOOL_KEY: &str = crate::TOOL_NAME;

/// The directory name for in which the user config can be found.
pub const USER_CONFIG_SUB_DIRECTORY: &str = crate::TOOL_NAME;

/// A set of configs from which values can be read.
#[derive(Debug, Default)]
pub struct LayeredConfig {
    system_settings: Option<Box<SettingsConfig>>,

    user_settings: Option<Box<SettingsConfig>>,

    project_settings: Option<Box<SettingsConfig>>,
    project_project: Option<Box<ProjectConfig>>,

    cli_settings: Option<Box<SettingsConfig>>,
    cli_project: Option<Box<ProjectConfig>>,
    cli_test: Option<Box<TestConfig>>,
}

impl LayeredConfig {
    /// Creates a new empty layered config.
    pub fn new() -> Self {
        Self {
            system_settings: None,

            user_settings: None,

            project_settings: None,
            project_project: None,

            cli_settings: None,
            cli_project: None,
            cli_test: None,
        }
    }

    /// Adds the system layer to the layered config.
    pub fn with_system_layer(&mut self, settings: Option<SettingsConfig>) -> &mut Self {
        self.system_settings = settings.map(Box::new);
        self
    }

    /// Adds the user layer to the layered config.
    pub fn with_user_layer(&mut self, settings: Option<SettingsConfig>) -> &mut Self {
        self.user_settings = settings.map(Into::into);
        self
    }

    /// Adds the project layer to the layered config.
    pub fn with_project_layer(
        &mut self,
        settings: Option<SettingsConfig>,
        project: Option<ProjectConfig>,
    ) -> &mut Self {
        self.project_settings = settings.map(Box::new);
        self.project_project = project.map(Box::new);
        self
    }

    /// Adds the CLI layer to the layered config.
    pub fn with_cli_layer(
        &mut self,
        settings: Option<SettingsConfig>,
        project: Option<ProjectConfig>,
        test: Option<TestConfig>,
    ) -> &mut Self {
        self.cli_settings = settings.map(Box::new);
        self.cli_project = project.map(Box::new);
        self.cli_test = test.map(Box::new);
        self
    }
}

impl LayeredConfig {
    /// Returns the settings config at the system layer.
    pub fn system_settings(&self) -> Option<&SettingsConfig> {
        self.system_settings.as_deref()
    }

    /// Returns the settings config at the user layer.
    pub fn user_settings(&self) -> Option<&SettingsConfig> {
        self.user_settings.as_deref()
    }

    /// Returns the settings config at the project layer.
    pub fn project_settings(&self) -> Option<&SettingsConfig> {
        self.project_settings.as_deref()
    }

    /// Returns the project config at the project layer.
    pub fn project_project(&self) -> Option<&ProjectConfig> {
        self.project_project.as_deref()
    }

    /// Returns the settings config at the CLI layer.
    pub fn cli_settings(&self) -> Option<&SettingsConfig> {
        self.cli_settings.as_deref()
    }

    /// Returns the project config at the CLI layer.
    pub fn cli_project(&self) -> Option<&ProjectConfig> {
        self.cli_project.as_deref()
    }

    /// Returns the test config at the CLI layer.
    pub fn cli_test(&self) -> Option<&TestConfig> {
        self.cli_test.as_deref()
    }

    /// The settings configs in ascending precedence.
    fn settings_configs(&self) -> impl Iterator<Item = &SettingsConfig> {
        [
            self.cli_settings.as_deref(),
            self.project_settings.as_deref(),
            self.user_settings.as_deref(),
            self.system_settings.as_deref(),
        ]
        .into_iter()
        .flatten()
    }

    /// The project configs in ascending precedence.
    fn project_configs(&self) -> impl Iterator<Item = &ProjectConfig> {
        [self.cli_project.as_deref(), self.project_project.as_deref()]
            .into_iter()
            .flatten()
    }

    /// The test configs in ascending precedence.
    ///
    /// The given test config is inserted after the CLI layer and before the
    /// project layer.
    fn test_configs<'c>(
        &'c self,
        test: Option<&'c TestConfig>,
    ) -> impl Iterator<Item = &'c TestConfig> {
        [
            self.cli_test.as_deref(),
            self.cli_project
                .as_deref()
                .and_then(|p| p.defaults.as_ref()),
            test,
            self.project_project
                .as_deref()
                .and_then(|p| p.defaults.as_ref()),
        ]
        .into_iter()
        .flatten()
    }
}

impl LayeredConfig {
    /// Retrieve a test config member with fallback.
    pub fn get_test_config_member<'c, T>(
        &'c self,
        test_config: Option<&'c TestConfig>,
        _: T,
        ctx: T::DefaultContext<'c>,
    ) -> T::Resolved<'c>
    where
        T: Resolve<Config = TestConfig>,
    {
        self.test_configs(test_config)
            .find_map(T::get)
            .map(T::resolve)
            .unwrap_or_else(|| T::default(ctx))
    }

    /// Retrieve a config member with fallback.
    pub fn get_project_config_member<'c, T>(
        &'c self,
        _: T,
        ctx: T::DefaultContext<'c>,
    ) -> T::Resolved<'c>
    where
        T: Resolve<Config = ProjectConfig>,
    {
        self.project_configs()
            .find_map(T::get)
            .map(T::resolve)
            .unwrap_or_else(|| T::default(ctx))
    }

    /// Retrieve a config member with fallback.
    pub fn get_settings_member<'c, T>(&'c self, _: T, ctx: T::DefaultContext<'c>) -> T::Resolved<'c>
    where
        T: Resolve<Config = SettingsConfig>,
    {
        self.settings_configs()
            .find_map(T::get)
            .map(T::resolve)
            .unwrap_or_else(|| T::default(ctx))
    }
}

/// A system config, found in the user's `$XDG_CONFIG_HOME` or globally on the
/// system.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct SettingsConfig {
    /// The Typst package cache directory.
    ///
    /// This is used to cache packages when they are first downloaded.
    ///
    /// Defaults to the value used by [`typst_kit`].
    pub package_cache_path: Option<String>,

    /// The Typst package directory.
    ///
    /// This is used to retrieve local packages before attempting downloads or
    /// cache accesses.
    ///
    /// Defaults to the value used by [`typst_kit`].
    pub package_path: Option<String>,

    /// THe mount of worker threads to use for various parallel operations.
    pub jobs: Option<NonZeroUsize>,

    /// Whether to abort on the first failure.
    pub fail_fast: Option<bool>,

    /// Whether to export temporary artifacts.
    pub export_ephemeral: Option<bool>,

    /// Whether to use a project's template when running `tytanic new`.
    pub template: Option<bool>,
}

impl SettingsConfig {
    /// The resolve type for [`SettingsConfig::package_path`].
    pub const PACKAGE_PATH: SettingsPackagePath = SettingsPackagePath;

    /// The resolve type for [`SettingsConfig::package_cache_path`].
    pub const PACKAGE_CACHE_PATH: SettingsPackageCachePath = SettingsPackageCachePath;

    /// The resolve type for [`SettingsConfig::jobs`].
    pub const JOBS: SettingsJobs = SettingsJobs;

    /// The resolve type for [`SettingsConfig::fail_fast`].
    pub const FAIL_FAST: SettingsFailFast = SettingsFailFast;

    /// The resolve type for [`SettingsConfig::export_ephemeral`].
    pub const EXPORT_EPHEMERAL: SettingsExportEphemeral = SettingsExportEphemeral;

    /// The resolve type for [`SettingsConfig::template`].
    pub const TEMPLATE: SettingsTemplate = SettingsTemplate;
}

impl SettingsConfig {
    /// Reads the user settings config at its predefined location.
    ///
    /// The location used is [`dirs::config_dir()`].
    pub fn collect_user() -> Result<Option<Self>, ReadError> {
        let Some(config_dir) = dirs::config_dir() else {
            tracing::warn!("couldn't retrieve user config home");
            return Ok(None);
        };

        let config = config_dir
            .join(USER_CONFIG_SUB_DIRECTORY)
            .join("config.toml");

        let Some(content) = fs::read_to_string(config).ignore(io_not_found)? else {
            return Ok(None);
        };

        Ok(toml::from_str(&content)?)
    }

    /// Validates a parsed settings config.
    pub fn validate(&self, source: PartialConfigSource) -> Result<(), ValidationError> {
        let SettingsConfig {
            package_cache_path,
            package_path,
            jobs: _,
            fail_fast: _,
            export_ephemeral: _,
            template: _,
        } = self;

        SettingsPackageCachePath::validate((), source.clone(), package_cache_path.as_deref())?;
        SettingsPackagePath::validate((), source, package_path.as_deref())?;

        Ok(())
    }
}

/// A project config, read from a project's manifest.
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct ProjectConfig {
    /// The manifest template path.
    ///
    /// This should not be set from the anywhere other than the manifest. This
    /// is not validated, validation occurs on the manifest itself.
    #[serde(skip, default)]
    pub template_path: Option<String>,

    /// The manifest template path.
    ///
    /// This should not be set from the anywhere other than the manifest. This
    /// is not validated, validation occurs on the manifest itself.
    #[serde(skip, default)]
    pub template_entrypoint: Option<String>,

    /// The custom store root directory.
    #[serde(rename = "tests", default)]
    pub store_root: Option<String>,

    /// The font directories to use for tests.
    #[serde(rename = "fonts", default)]
    pub font_paths: Option<Vec<String>>,

    /// Whether to optimize persistent references when running `tytanic update`.
    pub optimize_refs: Option<bool>,

    /// The project wide test config defaults.
    #[serde(rename = "default", default)]
    pub defaults: Option<TestConfig>,
}

impl ProjectConfig {
    /// The resolve type for [`ProjectConfig::template_path`].
    pub const TEMPLATE_PATH: ProjectTemplatePath = ProjectTemplatePath;

    /// The resolve type for [`ProjectConfig::template_entrypoint`].
    pub const TEMPLATE_ENTRYPOINT: ProjectTemplateEntrypoint = ProjectTemplateEntrypoint;

    /// The resolve type for [`ProjectConfig::store_root`].
    pub const STORE_ROOT: ProjectStoreRoot = ProjectStoreRoot;

    /// The resolve type for [`ProjectConfig::font_paths`].
    pub const FONT_PATHS: ProjectFontPaths = ProjectFontPaths;

    /// The resolve type for [`ProjectConfig::optimize_refs`].
    pub const OPTIMIZE_REFS: ProjectOptimizeRefs = ProjectOptimizeRefs;
}

impl ProjectConfig {
    /// Attempts to deserialize the project config from the manifest tool
    /// section.
    ///
    /// Call [`ProjectConfig::validate`] to ensure all values are valid.
    ///
    /// Returns `None` if there was no appropriate tool section.
    pub fn from_manifest(manifest: &PackageManifest) -> Result<Option<Self>, toml::de::Error> {
        let mut config = manifest
            .tool
            .sections
            .get(crate::TOOL_NAME)
            .map(|section| ProjectConfig::deserialize(section.clone()))
            .transpose();

        if let (Ok(Some(config)), Some(template)) = (&mut config, &manifest.template) {
            config.template_path = Some(template.path.as_str().into());
            config.template_entrypoint = Some(template.entrypoint.as_str().into());
        }

        config
    }

    /// Validates a parsed project config within a project root.
    pub fn validate(
        &self,
        root: &Path,
        source: PartialConfigSource,
    ) -> Result<(), ValidationError> {
        let ProjectConfig {
            template_path,
            template_entrypoint,
            store_root,
            font_paths: font_dirs,
            optimize_refs: _,
            defaults: _,
        } = self;

        if let Some(template_path) = template_path {
            ProjectTemplatePath::validate(root, source.clone(), Some(template_path))?;

            if let Some(template_entrypoint) = template_entrypoint {
                ProjectTemplateEntrypoint::validate(
                    (root, Path::new(template_path)),
                    source.clone(),
                    Some(template_entrypoint),
                )?;
            }
        }

        if let Some(store_root) = store_root {
            ProjectStoreRoot::validate(root, source.clone(), store_root)?;
        }

        if let Some(font_dirs) = font_dirs {
            ProjectFontPaths::validate(root, source.clone(), font_dirs)?;
        }

        Ok(())
    }
}

/// A test config, read from a test's annotations or from the project's default
/// table.
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct TestConfig {
    /// Whether to skip the test.
    ///
    /// This is only available as an annotation and not equivalent to the `skip`
    /// CLI switch.
    #[serde(skip, default)]
    pub skip: Option<bool>,

    /// Whether to run the comparison stage for the test.
    #[serde(default)]
    pub compare: Option<bool>,

    /// Whether to use embedded fonts for the test.
    #[serde(default)]
    pub use_embedded_fonts: Option<bool>,

    /// Whether to use system fonts for the test.
    #[serde(default)]
    pub use_system_fonts: Option<bool>,

    /// Whether to use system date and time for the test.
    #[serde(default)]
    pub use_system_datetime: Option<bool>,

    /// Whether to use the Tytanic augmented library.
    #[serde(default)]
    pub use_augmented_library: Option<bool>,

    /// The timestamp to use for the test.
    ///
    /// The supported format is that of [RFC#3339][rfc].
    ///
    /// [rfc]: https://datatracker.ietf.org/doc/html/rfc3339
    #[serde(default)]
    pub timestamp: Option<DateTime<Utc>>,

    /// Whether to allow external packages for the test.
    #[serde(default)]
    pub allow_packages: Option<bool>,

    /// How to handle warnings emitted by the test.
    #[serde(default)]
    pub warnings: Option<Warnings>,

    /// The text direction.
    #[serde(rename = "dir", default)]
    pub direction: Option<Direction>,

    /// The amount of pixels per inch for exporting and comparing documents.
    #[serde(rename = "ppi", default)]
    pub pixel_per_inch: Option<f32>,

    /// The maximum allowed delta per pixel.
    #[serde(default)]
    pub max_delta: Option<u8>,

    /// The maximum allowed deviating pixels for a comparison.
    #[serde(default)]
    pub max_deviations: Option<usize>,

    /// The maximum allowed deviating pixels for a comparison.
    #[serde(default)]
    pub inputs: Option<HashMap<EcoString, EcoString>>,
}

impl TestConfig {
    /// The resolve type for [`TestConfig::skip`].
    pub const SKIP: TestSkip = TestSkip;

    /// The resolve type for [`TestConfig::compare`].
    pub const COMPARE: TestCompare = TestCompare;

    /// The resolve type for [`TestConfig::use_embedded_fonts`].
    pub const USE_EMBEDDED_FONTS: TestUseEmbeddedFonts = TestUseEmbeddedFonts;

    /// The resolve type for [`TestConfig::use_system_fonts`].
    pub const USE_SYSTEM_FONTS: TestUseSystemFonts = TestUseSystemFonts;

    /// The resolve type for [`TestConfig::use_system_datetime`].
    pub const USE_SYSTEM_DATETIME: TestUseSystemDatetime = TestUseSystemDatetime;

    /// The resolve type for [`TestConfig::use_augmented_library`].
    pub const USE_AUGMENTED_LIBRARY: TestUseAugmentedLibrary = TestUseAugmentedLibrary;

    /// The resolve type for [`TestConfig::timestamp`].
    pub const TIMESTAMP: TestTimestamp = TestTimestamp;

    /// The resolve type for [`TestConfig::allow_packages`].
    pub const ALLOW_PACKAGES: TestAllowPackages = TestAllowPackages;

    /// The resolve type for [`TestConfig::warnings`].
    pub const WARNINGS: TestWarnings = TestWarnings;

    /// The resolve type for [`TestConfig::direction`].
    pub const DIRECTION: TestDirection = TestDirection;

    /// The resolve type for [`TestConfig::pixel_per_inch`].
    pub const PIXEL_PER_INCH: TestPixelPerInch = TestPixelPerInch;

    /// The resolve type for [`TestConfig::max_delta`].
    pub const MAX_DELTA: TestMaxDelta = TestMaxDelta;

    /// The resolve type for [`TestConfig::max_deviations`].
    pub const MAX_DEVIATIONS: TestMaxDeviations = TestMaxDeviations;

    /// The resolve type for [`TestConfig::inputs`].
    pub const INPUTS: TestInputs = TestInputs;
}

impl TestConfig {
    /// Creates a test config from a list of annotations.
    pub fn from_annotations<I>(annotations: I) -> Self
    where
        I: IntoIterator<Item = Annotation>,
    {
        let mut this = Self::default();

        for annot in annotations {
            match annot {
                Annotation::Skip => this.skip = Some(true),
                Annotation::Compare(value) => this.compare = Some(value),
                Annotation::UseSystemFonts(value) => this.use_system_fonts = Some(value),
                Annotation::UseSystemDatetime(value) => this.use_system_datetime = Some(value),
                Annotation::UseAugmentedLibrary(value) => this.use_augmented_library = Some(value),
                Annotation::Timestamp(value) => this.timestamp = Some(value),
                Annotation::AllowPackages(value) => this.allow_packages = Some(value),
                Annotation::Warnings(warnings) => this.warnings = Some(warnings),
                Annotation::Dir(value) => this.direction = Some(value),
                Annotation::Ppi(value) => this.pixel_per_inch = Some(value),
                Annotation::MaxDelta(value) => this.max_delta = Some(value),
                Annotation::MaxDeviations(value) => this.max_deviations = Some(value),
                Annotation::Input { key, value } => {
                    this.inputs
                        .get_or_insert_with(Default::default)
                        .insert(key, value);
                }
            }
        }

        this
    }
}

/// The reading direction of a document.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Direction {
    /// The documents are generated left-to-right.
    #[default]
    Ltr,

    /// The documents are generated right-to-left.
    Rtl,
}

/// How to handle warnings emitted by a test.
#[derive(Debug, Default, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Warnings {
    /// Ignore all warnings.
    Ignore,

    /// Emit warnings as normal.
    #[default]
    Emit,

    /// Promote warnings to errors.
    Promote,
}

// TODO(tinger): Add span information from toml-edit instead of just the keys.

/// A config source without the primary key name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PartialConfigSource {
    /// An environment variable like `TYPST_PACKAGE_PATH` or `SOURCE_EPOCH`.
    EnvironmentVariable,

    /// A command line argument or switch.
    CommandLine,

    /// A TOML config file path and key.
    ConfigFile {
        /// The path of the config file the key was parsed from.
        path: PathBuf,

        /// Whether this was a manifest config.
        manifest: bool,
    },

    /// An override.
    Override,
}

/// Provides access to the source of a config value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConfigSource {
    /// An environment variable like `TYPST_PACKAGE_PATH` or `SOURCE_EPOCH`.
    EnvironmentVariable {
        /// The name of the environment variable.
        name: String,
    },

    /// A command line argument like `--max-delta` or `-C`.
    CommandLineArgument {
        /// The long option name stem, i.e. `max-delta` for `--max-delta`.
        long: String,

        /// The short option name stem if it exists, i.e. `C` for `-C`.
        short: Option<String>,
    },

    /// A command line switch like `--no-compare`/`--compare`.
    CommandLineSwitch {
        /// The long option name stem, i.e. `compare` for
        /// `--no-compare`/`--compare`.
        stem: String,
    },

    /// A TOML config file path and key.
    ConfigFile {
        /// The path of the config file the key was parsed from.
        path: PathBuf,

        /// The dotted TOML key path to the value.
        key: String,
    },

    /// An override.
    Override {
        /// The name of the option.
        name: String,
    },

    /// A default fallback.
    Default {
        /// The name of the option.
        name: String,
    },
}

/// Validates that a path specified in a project TOML config exists and points
/// into the project.
pub fn validate_project_path(
    source: ConfigSource,
    root: &Path,
    path: &Path,
) -> Result<(), ValidationError> {
    tracing::trace!(?source, ?path, "validating path in project config");

    if !path.is_relative() || !path.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(ValidationError::path_not_trivial(source, path));
    }

    let full = root.join(path);
    let resolved = match full.canonicalize() {
        Ok(resolved) => resolved,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(ValidationError::path_not_found(source, path, Some(&full)));
        }
        Err(error) => {
            return Err(ValidationError::io(source, error));
        }
    };

    let resolved_root = match root.canonicalize() {
        Ok(resolved) => resolved,
        Err(error) => {
            return Err(ValidationError::io(source, error));
        }
    };

    if resolved.strip_prefix(&resolved_root).is_err() {
        return Err(ValidationError::project_path_escapes(
            source, path, resolved,
        ));
    }

    Ok(())
}

/// Validates that a path specified in a user or systems TOML config exists and
/// is absolute.
pub fn validate_settings_path(source: ConfigSource, path: &Path) -> Result<(), ValidationError> {
    tracing::trace!(?source, ?path, "validating path in user/system config");

    if !path.is_absolute() || !path.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(ValidationError::path_not_trivial(source, path));
    }

    if let Err(error) = path.canonicalize() {
        if error.kind() == io::ErrorKind::NotFound {
            return Err(ValidationError::path_not_found(source, path, None));
        } else {
            return Err(ValidationError::io(source, error));
        }
    }

    Ok(())
}

/// The source of a [`ValidationError`]
#[derive(Debug, Error)]
pub enum ValidationErrorReason {
    /// The validation failed, contains the validation error message.
    #[error("{0}")]
    Simple(String),

    /// An IO error occurred.
    #[error("an IO error occurred")]
    Io(#[from] io::Error),
}

/// An error that may occur while validating a single field in a TOML document.
#[derive(Debug, Error)]
pub struct ValidationError {
    source: ConfigSource,

    #[source]
    reason: ValidationErrorReason,
}

impl ValidationError {
    /// Create a new validation error.
    pub fn new<M>(source: ConfigSource, message: M) -> Self
    where
        M: Into<String>,
    {
        Self {
            source,
            reason: ValidationErrorReason::Simple(message.into()),
        }
    }

    /// Create a new validation error.
    pub fn io(source: ConfigSource, io_error: io::Error) -> Self {
        Self {
            source,
            reason: ValidationErrorReason::Io(io_error),
        }
    }

    /// Creates a new validation error for paths that contain invalid components.
    pub fn path_not_trivial<P>(source: ConfigSource, path: P) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            source,
            reason: ValidationErrorReason::Simple(format!(
                "path contains invalid components: `{}`",
                path.as_ref().display(),
            )),
        }
    }

    /// Creates a new validation error for paths that contain aren't absolute.
    pub fn path_not_absolute<P>(source: ConfigSource, path: P) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            source,
            reason: ValidationErrorReason::Simple(format!(
                "path wasn't absolute: `{}`",
                path.as_ref().display(),
            )),
        }
    }

    /// Creates a new validation error for paths that don't exist.
    pub fn path_not_found<P>(source: ConfigSource, path: P, full: Option<&Path>) -> Self
    where
        P: AsRef<Path>,
    {
        match full {
            Some(full) => Self {
                source,
                reason: ValidationErrorReason::Simple(format!(
                    "path does not exist: `{}` (`{}`)",
                    path.as_ref().display(),
                    full.display(),
                )),
            },
            None => Self {
                source,
                reason: ValidationErrorReason::Simple(format!(
                    "path does not exist: `{}`",
                    path.as_ref().display(),
                )),
            },
        }
    }

    /// Creates a new validation error for paths that escape the project.
    pub fn project_path_escapes<P, Q>(source: ConfigSource, path: P, resolved: Q) -> Self
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        Self {
            source,
            reason: ValidationErrorReason::Simple(format!(
                "path does not point into project: `{}` (`{}`)",
                path.as_ref().display(),
                resolved.as_ref().display(),
            )),
        }
    }
}

impl ValidationError {
    /// The source of the config.
    pub fn source(&self) -> &ConfigSource {
        &self.source
    }

    /// The reason for the validation error.
    pub fn reason(&self) -> &ValidationErrorReason {
        &self.reason
    }
}

impl Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "validation failed for ")?;

        match &self.source {
            ConfigSource::EnvironmentVariable { name } => write!(f, "${name}")?,
            ConfigSource::CommandLineArgument {
                long,
                short: Some(short),
            } => write!(f, "--{long}/-{short}")?,
            ConfigSource::CommandLineArgument { long, short: None } => write!(f, "cli: --{long}")?,
            ConfigSource::CommandLineSwitch { stem } => write!(f, "--[no-]{stem}")?,
            ConfigSource::ConfigFile { path, key } => write!(f, "`{key}` in {path:?}")?,
            ConfigSource::Override { name } => write!(f, "override option: {name}")?,
            ConfigSource::Default { name } => write!(f, "default value: {name}")?,
        };

        write!(f, "): {}", self.reason)?;

        Ok(())
    }
}

/// Returned when reading a TOML document fails.
#[derive(Debug, Error)]
pub enum ReadError {
    /// The TOML document failed validation.
    #[error("the TOML document failed validation")]
    Validation(#[from] ValidationError),

    /// The TOML document could not be parsed.
    #[error("the TOML document could not be parsed")]
    Parsing(#[from] toml::de::Error),

    /// An IO error occurred.
    #[error("an IO error occurred")]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    // use super::*;

    // // Verify that the `tool.tytanic.default` section in `typst.toml` is optional.
    // #[test]
    // fn config_defaults_section_is_optional() {
    //     let config = r#"
    //     [package]
    //     name = "testpackage"
    //     version = "0.1.0"
    //     entrypoint = "lib.typ"

    //     [tool.tytanic]
    //     tests = "test_dir"
    //     "#;

    //     let manifest = toml::from_str::<typst::syntax::package::PackageManifest>(config).unwrap();
    //     let project_config = ProjectConfig::deserialize(
    //         manifest
    //             .tool
    //             .sections
    //             .get(crate::TOOL_NAME)
    //             .unwrap()
    //             .to_owned(),
    //     )
    //     .unwrap();

    //     assert_eq!(project_config.unit_tests_root, "test_dir");
    //     assert_eq!(project_config.defaults.ppi, TestConfig::default().ppi);
    // }
}
