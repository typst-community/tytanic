//! Reading and interpreting tytanic configuration.

use std::{fs, io};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tytanic_utils::result::{io_not_found, ResultEx};

/// The key used to configure tytanic in the manifest tool config.
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
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct ProjectConfig {
    /// Custom test root directory.
    #[serde(rename = "tests")]
    pub unit_tests_root: Option<String>,
}

impl ProjectConfig {
    /// Returns the unit test root from the given config, or `"tests"`.
    pub fn unit_tests_root_or_default(config: Option<&Self>) -> &str {
        config
            .and_then(|c| c.unit_tests_root.as_deref())
            .unwrap_or("tests")
    }
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
