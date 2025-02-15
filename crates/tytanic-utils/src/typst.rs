use std::collections::BTreeMap;

use ecow::EcoString;
use typst_syntax::package::{
    PackageInfo, PackageManifest, PackageVersion, TemplateInfo, ToolInfo, UnknownFields,
    VersionBound,
};

/// A builder for [`PackageManifest`].
#[derive(Debug, Clone)]
pub struct PackageManifestBuilder {
    pub package: PackageInfoBuilder,
    pub template: Option<TemplateInfoBuilder>,
    pub tool: ToolInfoBuilder,
}

impl Default for PackageManifestBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageManifestBuilder {
    pub fn new() -> Self {
        Self {
            package: PackageInfoBuilder::new(),
            template: None,
            tool: ToolInfoBuilder::new(),
        }
    }

    pub fn package<T: Into<PackageInfoBuilder>>(&mut self, value: T) -> &mut Self {
        self.package = value.into();
        self
    }

    pub fn template<T: Into<TemplateInfoBuilder>>(&mut self, value: T) -> &mut Self {
        self.template = Some(value.into());
        self
    }

    pub fn tool<T: Into<ToolInfoBuilder>>(&mut self, value: T) -> &mut Self {
        self.tool = value.into();
        self
    }

    pub fn build(&self) -> PackageManifest {
        self.clone().into()
    }
}

/// A builder for [`PackageInfo`].
#[derive(Debug, Clone)]
pub struct PackageInfoBuilder {
    pub name: EcoString,
    pub version: PackageVersion,
    pub entrypoint: EcoString,
    pub authors: Vec<EcoString>,
    pub license: Option<EcoString>,
    pub description: Option<EcoString>,
    pub homepage: Option<EcoString>,
    pub repository: Option<EcoString>,
    pub keywords: Vec<EcoString>,
    pub categories: Vec<EcoString>,
    pub disciplines: Vec<EcoString>,
    pub compiler: Option<VersionBound>,
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

const PACKAGE_INFO_VERSION_DEFAULT: PackageVersion = PackageVersion {
    major: 0,
    minor: 1,
    patch: 0,
};
const PACKAGE_INFO_NAME_DEFAULT: &str = "my-package";
const PACKAGE_INFO_ENTRYPOINT_DEFAULT: &str = "src/lib.typ";

impl Default for PackageInfoBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageInfoBuilder {
    pub fn new() -> Self {
        Self {
            name: PACKAGE_INFO_NAME_DEFAULT.into(),
            version: PACKAGE_INFO_VERSION_DEFAULT,
            entrypoint: PACKAGE_INFO_ENTRYPOINT_DEFAULT.into(),
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

    pub fn name<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.name = value.into();
        self
    }

    pub fn version<T: Into<PackageVersion>>(&mut self, value: T) -> &mut Self {
        self.version = value.into();
        self
    }

    pub fn entrypoint<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.entrypoint = value.into();
        self
    }

    pub fn authors<T, I>(&mut self, value: I) -> &mut Self
    where
        T: Into<EcoString>,
        I: IntoIterator<Item = T>,
    {
        self.authors = value.into_iter().map(Into::into).collect();
        self
    }

    pub fn license<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.license = Some(value.into());
        self
    }

    pub fn description<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.description = Some(value.into());
        self
    }

    pub fn homepage<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.homepage = Some(value.into());
        self
    }

    pub fn repository<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.repository = Some(value.into());
        self
    }

    pub fn keywords<T, I>(&mut self, value: I) -> &mut Self
    where
        T: Into<EcoString>,
        I: IntoIterator<Item = T>,
    {
        self.keywords = value.into_iter().map(Into::into).collect();
        self
    }

    pub fn categories<T, I>(&mut self, value: I) -> &mut Self
    where
        T: Into<EcoString>,
        I: IntoIterator<Item = T>,
    {
        self.categories = value.into_iter().map(Into::into).collect();
        self
    }

    pub fn disciplines<T, I>(&mut self, value: I) -> &mut Self
    where
        T: Into<EcoString>,
        I: IntoIterator<Item = T>,
    {
        self.disciplines = value.into_iter().map(Into::into).collect();
        self
    }

    pub fn compiler<T: Into<VersionBound>>(&mut self, value: T) -> &mut Self {
        self.compiler = Some(value.into());
        self
    }

    pub fn exclude<T, I>(&mut self, value: I) -> &mut Self
    where
        T: Into<EcoString>,
        I: IntoIterator<Item = T>,
    {
        self.exclude = value.into_iter().map(Into::into).collect();
        self
    }

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

const TEMPLATE_INFO_PATH_DEFAULT: &str = "template";
const TEMPLATE_INFO_ENTRYPOINT_DEFAULT: &str = "main.typ";

/// A builder for [`TemplateInfo`].
#[derive(Debug, Clone)]
pub struct TemplateInfoBuilder {
    path: EcoString,
    entrypoint: EcoString,
    thumbnail: Option<EcoString>,
}

impl Default for TemplateInfoBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateInfoBuilder {
    pub fn new() -> Self {
        Self {
            path: TEMPLATE_INFO_PATH_DEFAULT.into(),
            entrypoint: TEMPLATE_INFO_ENTRYPOINT_DEFAULT.into(),
            thumbnail: None,
        }
    }

    pub fn path<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.path = value.into();
        self
    }

    pub fn entrypoint<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.entrypoint = value.into();
        self
    }

    pub fn thumbnail<T: Into<EcoString>>(&mut self, value: T) -> &mut Self {
        self.thumbnail = Some(value.into());
        self
    }

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
    pub sections: BTreeMap<EcoString, toml::Table>,
}

impl Default for ToolInfoBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolInfoBuilder {
    pub fn new() -> Self {
        Self {
            sections: BTreeMap::new(),
        }
    }

    pub fn section<T: Into<BTreeMap<EcoString, toml::Table>>>(mut self, value: T) -> Self {
        self.sections = value.into();
        self
    }

    pub fn with_section<K: Into<EcoString>, V: Into<toml::Table>>(
        mut self,
        key: K,
        val: V,
    ) -> Self {
        self.sections.insert(key.into(), val.into());
        self
    }

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
