//! Reading and interpreting Tytanic configuration.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;

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
pub struct SystemConfig {
    #[serde(default)]
    pub term: Term,
}

/// Terminal configuration options.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct Term {
    pub osc_9_4: Option<bool>,
}

impl SystemConfig {
    /// Reads the user config at its predefined location.
    ///
    /// The location used is [`dirs::config_dir()`].
    pub fn collect_user() -> Result<Option<Self>, Error> {
        let Some(config_dir) = dirs::config_dir() else {
            tracing::warn!("couldn't retrieve user config home");
            return Ok(None);
        };

        let config_path = config_dir.join(CONFIG_SUB_DIRECTORY).join("config.toml");

        Self::collect_in(config_path)
    }

    /// Reads the system config from the given config file.
    fn collect_in(path: PathBuf) -> Result<Option<Self>, Error> {
        let Some(content) = fs::read_to_string(&path).ignore(io_not_found)? else {
            return Ok(None);
        };

        toml::from_str(&content).map_err(|error| Error::Toml { path, error })
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
    pub ppi: f64,

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

    /// Whether system fonts are used by default.
    ///
    /// Defaults to `false`.
    #[serde(default = "default_use_system_fonts")]
    pub use_system_fonts: bool,

    /// The default input key-value pairs exposed in `sys.inputs` for all tests.
    ///
    /// Defaults to an empty map.
    #[serde(default = "default_inputs")]
    pub inputs: HashMap<String, String>,
}

impl Default for ProjectDefaults {
    fn default() -> Self {
        Self {
            direction: default_direction(),
            ppi: default_ppi(),
            max_delta: default_max_delta(),
            max_deviations: default_max_deviations(),
            use_system_fonts: default_use_system_fonts(),
            inputs: default_inputs(),
        }
    }
}

fn default_direction() -> Direction {
    Direction::Ltr
}

fn default_ppi() -> f64 {
    144.0
}

fn default_max_delta() -> u8 {
    1
}

fn default_max_deviations() -> usize {
    0
}

const fn default_use_system_fonts() -> bool {
    false
}

fn default_inputs() -> HashMap<String, String> {
    HashMap::new()
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
    /// The user config file could not be parsed.
    #[error("a toml parsing error occurred at {path:?}:\n{error}")]
    Toml {
        /// The path of the config file that failed to parse.
        path: PathBuf,
        /// The error that occurred while parsing the config.
        error: toml::de::Error,
    },

    /// An io error occurred.
    #[error("an io error occurred")]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use typst::syntax::package::PackageManifest;

    use super::*;

    // Verify that the `tool.tytanic.default` section in `typst.toml` is optional.
    #[test]
    fn project_config_defaults_section_is_optional() {
        let raw_project_config = r#"
        [package]
        name = "testpackage"
        version = "0.1.0"
        entrypoint = "lib.typ"

        [tool.tytanic]
        tests = "test_dir"
        "#;

        let manifest = toml::from_str::<PackageManifest>(raw_project_config).unwrap();
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

    #[test]
    fn project_config_default_use_system_fonts_is_configurable() {
        let raw_project_config = r#"
        [package]
        name = "testpackage"
        version = "0.1.0"
        entrypoint = "lib.typ"

        [tool.tytanic.default]
        use-system-fonts = true
        "#;

        let manifest = toml::from_str::<PackageManifest>(raw_project_config).unwrap();
        let project_config = ProjectConfig::deserialize(
            manifest
                .tool
                .sections
                .get(crate::TOOL_NAME)
                .unwrap()
                .to_owned(),
        )
        .unwrap();

        assert!(project_config.defaults.use_system_fonts);
    }

    #[test]
    fn project_config_default_inputs_is_configurable() {
        let raw_project_config = r#"
        [package]
        name = "testpackage"
        version = "0.1.0"
        entrypoint = "lib.typ"

        [tool.tytanic.default]
        inputs = { key1 = "value1", key2 = "value2" }
        "#;

        let manifest = toml::from_str::<PackageManifest>(raw_project_config).unwrap();
        let project_config = ProjectConfig::deserialize(
            manifest
                .tool
                .sections
                .get(crate::TOOL_NAME)
                .unwrap()
                .to_owned(),
        )
        .unwrap();

        assert_eq!(
            project_config.defaults.inputs,
            HashMap::from([
                ("key1".to_string(), "value1".to_string()),
                ("key2".to_string(), "value2".to_string())
            ])
        );
    }

    // Verify that an absent system config is read as `None`.
    #[test]
    fn system_config_absent() {
        tytanic_utils::fs::TempTestEnv::run_no_check(
            |root| root,
            |dir| {
                let config_path: PathBuf =
                    dir.join(CONFIG_SUB_DIRECTORY).join("config.toml").into();
                assert_eq!(SystemConfig::collect_in(config_path).unwrap(), None);
            },
        );
    }

    // Verify that the system config file is loaded from the config directory.
    #[test]
    fn system_config_is_collected() {
        tytanic_utils::fs::TempTestEnv::run_no_check(
            |root| root.setup_file(format!("{}/config.toml", CONFIG_SUB_DIRECTORY), ""),
            |dir| {
                let config_path: PathBuf =
                    dir.join(CONFIG_SUB_DIRECTORY).join("config.toml").into();
                assert_eq!(
                    SystemConfig::collect_in(config_path).unwrap(),
                    Some(SystemConfig::default())
                );
            },
        );
    }

    // Verify that an invalid system config yields a parse error.
    #[test]
    fn system_config_invalid() {
        tytanic_utils::fs::TempTestEnv::run_no_check(
            |root| {
                root.setup_file(
                    format!("{}/config.toml", CONFIG_SUB_DIRECTORY),
                    "invalid-key = true\n",
                )
            },
            |dir| {
                let config_path: PathBuf =
                    dir.join(CONFIG_SUB_DIRECTORY).join("config.toml").into();
                let err = SystemConfig::collect_in(config_path).unwrap_err();
                assert!(matches!(err, Error::Toml { .. }));
            },
        );
    }

    // Verify that the `term.osc-9-4` system config option is parsed.
    #[test]
    fn system_config_term_osc_9_4_is_configurable() {
        tytanic_utils::fs::TempTestEnv::run_no_check(
            |root| {
                root.setup_file(
                    format!("{}/config.toml", CONFIG_SUB_DIRECTORY),
                    "term.osc-9-4 = true\n",
                )
            },
            |dir| {
                let config_path: PathBuf =
                    dir.join(CONFIG_SUB_DIRECTORY).join("config.toml").into();
                assert_eq!(
                    SystemConfig::collect_in(config_path).unwrap(),
                    Some(SystemConfig {
                        term: Term {
                            osc_9_4: Some(true)
                        }
                    })
                );
            },
        );
    }
}
