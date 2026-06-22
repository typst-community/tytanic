//! Manifest builders.

use std::collections::BTreeMap;

use ecow::EcoString;
use typst_syntax::package::PackageInfo;
use typst_syntax::package::PackageManifest;
use typst_syntax::package::PackageVersion;
use typst_syntax::package::TemplateInfo;
use typst_syntax::package::ToolInfo;
use typst_syntax::package::UnknownFields;
use typst_syntax::package::VersionBound;

/// A builder for [`PackageManifest`].
#[derive(Debug, Clone)]
pub struct PackageManifestBuilder {
    /// The package info builder.
    pub package: PackageInfoBuilder,

    /// The template info builder.
    pub template: Option<TemplateInfoBuilder>,

    /// The tool info builder.
    pub tool: ToolInfoBuilder,
}

impl PackageManifestBuilder {
    /// Creates a new builder with default values for its fields.
    ///
    /// Uses the default package info builder, no template info builder and an
    /// empty tool info builder.
    pub fn new() -> Self {
        Self {
            package: PackageInfoBuilder::new(),
            template: None,
            tool: ToolInfoBuilder::new(),
        }
    }

    /// Sets the package info.
    ///
    /// The input is _not_ validated.
    pub fn package<T: Into<PackageInfoBuilder>>(&mut self, value: T) -> &mut Self {
        self.package = value.into();
        self
    }

    /// Sets the template info.
    ///
    /// The input is _not_ validated.
    pub fn template<T: Into<TemplateInfoBuilder>>(&mut self, value: T) -> &mut Self {
        self.template = Some(value.into());
        self
    }

    /// Sets the tool info.
    ///
    /// The input is _not_ validated.
    pub fn tool<T: Into<ToolInfoBuilder>>(&mut self, value: T) -> &mut Self {
        self.tool = value.into();
        self
    }

    /// Creates a manifest from this builder.
    pub fn build(&self) -> PackageManifest {
        self.clone().into()
    }
}

impl Default for PackageManifestBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A builder for [`PackageInfo`].
#[derive(Debug, Clone)]
pub struct PackageInfoBuilder {
    /// The package name, must be a valid Typst identifier.
    pub name: EcoString,

    /// The package version.
    pub version: PackageVersion,

    /// The package entrypoint, must be a path relative from the root to a Typst
    /// file.
    pub entrypoint: EcoString,

    /// The authors of the package, entries must be valid authors.
    pub authors: Vec<EcoString>,

    /// The license of the, must be an OSI-compatible SPDX license identifier.
    pub license: Option<EcoString>,

    /// THe package description.
    pub description: Option<EcoString>,

    /// The homepage URL of the package.
    pub homepage: Option<EcoString>,

    /// The repository URL of the package.
    pub repository: Option<EcoString>,

    /// The keywords to find the package by on the Typst Universe.
    pub keywords: Vec<EcoString>,

    /// The categories the package is put in on the Typst Universe. Must be a
    /// valid category on the Typst Universe.
    pub categories: Vec<EcoString>,

    /// The disciplines the package is put in on the Typst Universe. Must be a
    /// valid discipline on the Typst Universe.
    pub disciplines: Vec<EcoString>,

    /// The minimum required compiler version for the package.
    pub compiler: Option<VersionBound>,

    /// The exclude patterns for packaging.
    pub exclude: Vec<EcoString>,
}

impl From<PackageManifest> for PackageManifestBuilder {
    fn from(value: PackageManifest) -> Self {
        Self {
            package: value.package.into(),
            template: value.template.map(Into::into),
            tool: value.tool.into(),
        }
    }
}

impl From<PackageManifestBuilder> for PackageManifest {
    fn from(value: PackageManifestBuilder) -> Self {
        Self {
            package: value.package.into(),
            template: value.template.map(Into::into),
            tool: value.tool.into(),
            unknown_fields: UnknownFields::new(),
        }
    }
}

impl Default for PackageInfoBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageInfoBuilder {
    /// Creates a new builder with default values for its fields.
    ///
    /// Uses the following values for required fields:
    /// - name: `my-package`
    /// - version: `0.1.0`
    /// - entrypoint: `src/lib.typ`
    pub fn new() -> Self {
        Self {
            name: "my-package".into(),
            version: PackageVersion {
                major: 0,
                minor: 1,
                patch: 0,
            },
            entrypoint: "src/lib.typ".into(),
            authors: vec![],
            license: None,
            description: None,
            homepage: None,
            repository: None,
            keywords: vec![],
            categories: vec![],
            disciplines: vec![],
            compiler: None,
            exclude: vec![],
        }
    }

    /// Sets the name.
    ///
    /// The input is _not_ validated.
    pub fn name<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.name = value.into();
        self
    }

    /// Sets the version.
    ///
    /// The input is _not_ validated.
    pub fn version<T: Into<PackageVersion>>(&mut self, value: T) -> &mut Self {
        self.version = value.into();
        self
    }

    /// Sets the entrypoint.
    ///
    /// The input is _not_ validated.
    pub fn entrypoint<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.entrypoint = value.into();
        self
    }

    /// Sets the authors.
    ///
    /// The input is _not_ validated.
    pub fn authors<T, I>(&mut self, value: I) -> &mut Self
    where
        T: Into<EcoString>,
        I: IntoIterator<Item = T>,
    {
        self.authors = value.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the license.
    ///
    /// The input is _not_ validated.
    pub fn license<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.license = Some(value.into());
        self
    }

    /// Sets the description.
    ///
    /// The input is _not_ validated.
    pub fn description<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.description = Some(value.into());
        self
    }

    /// Sets the homepage URL.
    ///
    /// The input is _not_ validated.
    pub fn homepage<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.homepage = Some(value.into());
        self
    }

    /// Sets the repository URL.
    ///
    /// The input is _not_ validated.
    pub fn repository<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.repository = Some(value.into());
        self
    }

    /// Sets the keywords.
    ///
    /// The input is _not_ validated.
    pub fn keywords<T, I>(&mut self, value: I) -> &mut Self
    where
        T: Into<EcoString>,
        I: IntoIterator<Item = T>,
    {
        self.keywords = value.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the Typst Universe categories.
    ///
    /// The input is _not_ validated.
    pub fn categories<T, I>(&mut self, value: I) -> &mut Self
    where
        T: Into<EcoString>,
        I: IntoIterator<Item = T>,
    {
        self.categories = value.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the Typst Universe disciplines.
    ///
    /// The input is _not_ validated.
    pub fn disciplines<T, I>(&mut self, value: I) -> &mut Self
    where
        T: Into<EcoString>,
        I: IntoIterator<Item = T>,
    {
        self.disciplines = value.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the minimum required compiler version.
    ///
    /// The input is _not_ validated.
    pub fn compiler<T: Into<VersionBound>>(&mut self, value: T) -> &mut Self {
        self.compiler = Some(value.into());
        self
    }

    /// Sets the exclude patterns for packaging.
    ///
    /// The input is _not_ validated.
    pub fn exclude<T, I>(&mut self, value: I) -> &mut Self
    where
        T: Into<EcoString>,
        I: IntoIterator<Item = T>,
    {
        self.exclude = value.into_iter().map(Into::into).collect();
        self
    }

    /// Creates a package info from this builder.
    pub fn build(&self) -> PackageInfo {
        self.clone().into()
    }
}

impl From<PackageInfo> for PackageInfoBuilder {
    fn from(value: PackageInfo) -> Self {
        Self {
            name: value.name,
            version: value.version,
            entrypoint: value.entrypoint,
            authors: value.authors,
            license: value.license,
            description: value.description,
            homepage: value.homepage,
            repository: value.repository,
            keywords: value.keywords,
            categories: value.categories,
            disciplines: value.disciplines,
            compiler: value.compiler,
            exclude: value.exclude,
        }
    }
}

impl From<PackageInfoBuilder> for PackageInfo {
    fn from(value: PackageInfoBuilder) -> Self {
        Self {
            name: value.name,
            version: value.version,
            entrypoint: value.entrypoint,
            authors: value.authors,
            license: value.license,
            description: value.description,
            homepage: value.homepage,
            repository: value.repository,
            keywords: value.keywords,
            categories: value.categories,
            disciplines: value.disciplines,
            compiler: value.compiler,
            exclude: value.exclude,
            unknown_fields: UnknownFields::new(),
        }
    }
}

/// A builder for [`TemplateInfo`].
#[derive(Debug, Clone)]
pub struct TemplateInfoBuilder {
    /// The template scaffold directory. Must be relative from the root.
    pub path: EcoString,

    /// The template document entrypoint file. Must be relative form the
    /// scaffold directory.
    pub entrypoint: EcoString,

    /// The template thumbnail. Must be relative form the root.
    pub thumbnail: Option<EcoString>,
}

impl Default for TemplateInfoBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateInfoBuilder {
    /// Creates a new builder with default values for its fields.
    ///
    /// Uses the following values for required fields:
    /// - path: `template`
    /// - entrypoint: `main.typ`
    pub fn new() -> Self {
        Self {
            path: "template".into(),
            entrypoint: "main.typ".into(),
            thumbnail: None,
        }
    }

    /// Sets the path.
    ///
    /// The input is _not_ validated.
    pub fn path<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.path = value.into();
        self
    }

    /// Sets the entrypoint.
    ///
    /// The input is _not_ validated.
    pub fn entrypoint<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.entrypoint = value.into();
        self
    }

    /// Sets the thumbnail.
    ///
    /// The input is _not_ validated.
    pub fn thumbnail<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.thumbnail = Some(value.into());
        self
    }

    /// Creates a template info from this builder.
    pub fn build(&self) -> TemplateInfo {
        self.clone().into()
    }
}

impl From<TemplateInfo> for TemplateInfoBuilder {
    fn from(value: TemplateInfo) -> Self {
        Self {
            path: value.path,
            entrypoint: value.entrypoint,
            thumbnail: value.thumbnail,
        }
    }
}

impl From<TemplateInfoBuilder> for TemplateInfo {
    fn from(value: TemplateInfoBuilder) -> Self {
        Self {
            path: value.path,
            entrypoint: value.entrypoint,
            thumbnail: value.thumbnail,
            unknown_fields: UnknownFields::new(),
        }
    }
}

/// A builder for [`ToolInfo`].
#[derive(Debug, Clone)]
pub struct ToolInfoBuilder {
    /// The tool sections keyed by tool name.
    pub sections: BTreeMap<EcoString, toml::Table>,
}

impl Default for ToolInfoBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolInfoBuilder {
    /// Creates a new empty builder.
    pub fn new() -> Self {
        Self {
            sections: BTreeMap::new(),
        }
    }

    /// Sets the sections.
    pub fn sections<T: Into<BTreeMap<EcoString, toml::Table>>>(mut self, value: T) -> Self {
        self.sections = value.into();
        self
    }

    /// Adds a section.
    pub fn with_section<K: Into<EcoString>, V: Into<toml::Table>>(
        mut self,
        key: K,
        val: V,
    ) -> Self {
        self.sections.insert(key.into(), val.into());
        self
    }

    /// Creates a tool info from this builder.
    pub fn build(&self) -> ToolInfo {
        self.clone().into()
    }
}

impl From<ToolInfo> for ToolInfoBuilder {
    fn from(value: ToolInfo) -> Self {
        Self {
            sections: value.sections,
        }
    }
}

impl From<ToolInfoBuilder> for ToolInfo {
    fn from(value: ToolInfoBuilder) -> Self {
        Self {
            sections: value.sections,
        }
    }
}
