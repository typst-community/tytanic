//! Reading and interpreting Tytanic configuration.

use std::fs;
use std::io;

use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;
use tytanic_utils::result::ResultEx;
use tytanic_utils::result::io_not_found;

/// The key used to configure Tytanic in the manifest tool config.
pub const MANIFEST_TOOL_KEY: &str = crate::TOOL_NAME;

/// The directory name for in which the user config can be found.
pub const CONFIG_SUB_DIRECTORY: &str = crate::TOOL_NAME;

/// A system config, found in the user's `$XDG_CONFIG_HOME` or globally on the
/// system.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct SystemConfig {}

impl SystemConfig {
    /// Reads the user config at its predefined location.
    ///
    /// The location used is [`dirs::config_dir()`].
    pub fn collect_user() -> Result<Option<Self>, Error> {
        let Some(config_dir) = dirs::config_dir() else {
            tracing::warn!("couldn't retrieve user config home");
            return Ok(None);
        };

        let config = config_dir.join(CONFIG_SUB_DIRECTORY).join("config.toml");
        let Some(content) = fs::read_to_string(config).ignore(io_not_found)? else {
            return Ok(None);
        };

        Ok(toml::from_str(&content)?)
    }
}

/// A project config, read from a project's manifest.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct ProjectConfig {
    /// Custom test root directory.
    ///
    /// Defaults to `"tests"`.
    #[serde(rename = "tests", default = "default_unit_tests_root")]
    pub unit_tests_root: String,

    /// The project wide defaults.
    #[serde(rename = "default", default)]
    pub defaults: ProjectDefaults,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            unit_tests_root: default_unit_tests_root(),
            defaults: ProjectDefaults::default(),
        }
    }
}

fn default_unit_tests_root() -> String {
    String::from("tests")
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct ProjectDefaults {
    /// The default direction.
    #[serde(rename = "dir", default = "default_direction")]
    pub direction: Direction,

    /// The default pixel per inch for exporting and comparing documents.
    ///
    /// Defaults to `144.0`.
    #[serde(default = "default_ppi")]
    pub ppi: f32,

    /// The default maximum allowed delta per pixel.
    ///
    /// Defaults to `1`.
    #[serde(default = "default_max_delta")]
    pub max_delta: u8,

    /// The default maximum allowed deviating pixels for a comparison.
    ///
    /// Defaults to `0`.
    #[serde(default = "default_max_deviations")]
    pub max_deviations: usize,
}

impl Default for ProjectDefaults {
    fn default() -> Self {
        Self {
            direction: default_direction(),
            ppi: default_ppi(),
            max_delta: default_max_delta(),
            max_deviations: default_max_deviations(),
        }
    }
}

fn default_direction() -> Direction {
    Direction::Ltr
}

fn default_ppi() -> f32 {
    144.0
}

fn default_max_delta() -> u8 {
    1
}

fn default_max_deviations() -> usize {
    0
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

/// Returned by [`SystemConfig::collect_user`].
#[derive(Debug, Error)]
pub enum Error {
    /// The given key is not valid or the config.
    #[error("a toml parsing error occurred")]
    Toml(#[from] toml::de::Error),

    /// An io error occurred.
    #[error("an io error occurred")]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verify that the `tool.tytanic.default` section in `typst.toml` is optional.
    #[test]
    fn config_defaults_section_is_optional() {
        let config = r#"
        [package]
        name = "testpackage"
        version = "0.1.0"
        entrypoint = "lib.typ"

        [tool.tytanic]
        tests = "test_dir"
        "#;

        let manifest = toml::from_str::<typst::syntax::package::PackageManifest>(config).unwrap();
        let project_config = ProjectConfig::deserialize(
            manifest
                .tool
                .sections
                .get(crate::TOOL_NAME)
                .unwrap()
                .to_owned(),
        )
        .unwrap();

        assert_eq!(project_config.unit_tests_root, "test_dir");
        assert_eq!(project_config.defaults.ppi, ProjectDefaults::default().ppi);
    }
}
