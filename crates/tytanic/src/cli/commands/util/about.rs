use std::env;
use std::fmt::Display;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use clap::ValueEnum;
use color_eyre::eyre;
use serde::Serialize;
use termcolor::Color;
use termcolor::ColorSpec;
use termcolor::WriteColor;

use super::Context;
use crate::cli::commands::Switch;
use crate::cwrite;

#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    /// The format to serialize in, if it should be machine-readable.
    ///
    /// If no format is passed the output is displayed human-readable. Note that
    /// human-readable format truncates the build commit hash value.
    #[arg(long = "format", short = 'f')]
    pub format: Option<SerializationFormat>,

    /// Whether to pretty-print the serialized output.
    ///
    /// Only applies to JSON format.
    #[clap(long)]
    pub pretty: bool,
}

#[derive(ValueEnum, Clone, Default, Debug)]
pub enum SerializationFormat {
    #[default]
    Json,
    Yaml,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct About<'a> {
    /// The version of Tytanic.
    version: &'static str,

    /// The Typst version used in this build of Tytanic.
    typst_version: &'static str,

    /// Build information of Tytanic.
    build: Build,

    /// Runtime font configuration.
    fonts: Fonts<'a>,

    /// Runtime package configuration.
    packages: Packages,

    /// Runtime Environment variables.
    env: Environment,
}

impl<'a> About<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            version: env!("TYTANIC_VERSION"),
            typst_version: env!("TYTANIC_TYPST_VERSION"),
            build: Build::new(),
            // features: Features::new(),
            fonts: Fonts::new(ctx),
            packages: Packages::new(ctx),
            env: Environment::new(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct Build {
    /// The commit SHA of the current commit when building Tytanic.
    commit: Option<&'static str>,

    /// The platform of this build.
    platform: Platform,
}

impl Build {
    /// Retrieves build information specified in the `build.rs`
    const fn new() -> Self {
        Self {
            commit: option_env!("TYTANIC_COMMIT_SHA"),
            platform: Platform::new(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct Platform {
    os: &'static str,
    arch: &'static str,
}

impl Platform {
    const fn new() -> Self {
        Self {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct Fonts<'a> {
    /// The font paths.
    paths: &'a [PathBuf],

    /// Whether system fonts were included in the search.
    system: bool,

    /// Whether embedded fonts were included in the search.
    embedded: bool,
}

impl<'a> Fonts<'a> {
    /// Retrieves runtime font configuration.
    fn new(ctx: &'a Context) -> Self {
        Self {
            paths: &ctx.args.font.font_paths,
            system: ctx.args.font.use_system_fonts.get_or_default(),
            embedded: ctx.args.font.use_embedded_fonts.get_or_default(),
        }
    }

    /// Return the custom font paths.
    fn custom_paths(&self) -> impl Iterator<Item = Value<'_>> {
        self.paths.iter().map(|p| Value::Path(p))
    }

    /// Return whether system and embedded fonts are included.
    fn included(&self) -> impl Iterator<Item = (&'static str, Value<'_>)> {
        let Self {
            paths: _,
            system,
            embedded,
        } = self;

        [("System fonts", system), ("Embedded fonts", embedded)]
            .into_iter()
            .map(|(key, val)| (key, Value::Bool(*val)))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct Packages {
    /// The package path.
    package_path: Option<PathBuf>,

    /// The package cache path.
    package_cache_path: Option<PathBuf>,
}

impl Packages {
    /// Retrieves the runtime package configuration.
    fn new(ctx: &Context) -> Self {
        let package_path = match &ctx.args.package.package_path {
            Some(package_path) => Some(package_path.clone()),
            None => typst_kit::package::default_package_path(),
        };
        let package_cache_path = match &ctx.args.package.package_cache_path {
            Some(package_cache_path) => Some(package_cache_path.clone()),
            None => typst_kit::package::default_package_cache_path(),
        };

        Self {
            package_path,
            package_cache_path,
        }
    }

    /// Return the resolved package paths.
    fn paths(&self) -> impl Iterator<Item = (&'static str, Value<'_>)> {
        let Self {
            package_path,
            package_cache_path,
        } = self;

        [
            ("Package path", package_path),
            ("Package cache path", package_cache_path),
        ]
        .into_iter()
        .map(|(k, v)| (k, v.as_deref().map(Value::Path).unwrap_or(Value::Unset)))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct Environment {
    typst_cert: Option<String>,
    typst_features: Option<String>,
    typst_font_paths: Option<String>,
    typst_ignore_system_fonts: Option<String>,
    typst_ignore_embedded_fonts: Option<String>,
    typst_package_cache_path: Option<String>,
    typst_package_path: Option<String>,
    typst_root: Option<String>,
    typst_update_backup_path: Option<String>,
    source_date_epoch: Option<String>,
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios")))]
    xdg_cache_home: Option<String>,
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios")))]
    xdg_data_home: Option<String>,
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios")))]
    fontconfig_file: Option<String>,
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios")))]
    openssl_conf: Option<String>,
    no_color: Option<String>,
    no_proxy: Option<String>,
    http_proxy: Option<String>,
    https_proxy: Option<String>,
    all_proxy: Option<String>,
}

impl Environment {
    /// Retrieves the runtime environment variables.
    ///
    /// Unset or invalid env vars will be set to `None`.
    fn new() -> Self {
        Self {
            typst_cert: std::env::var("TYPST_CERT").ok(),
            typst_features: std::env::var("TYPST_FEATURES").ok(),
            typst_font_paths: std::env::var("TYPST_FONT_PATHS").ok(),
            typst_ignore_system_fonts: std::env::var("TYPST_IGNORE_SYSTEM_FONTS").ok(),
            typst_ignore_embedded_fonts: std::env::var("TYPST_IGNORE_EMBEDDED_FONTS").ok(),
            typst_package_cache_path: std::env::var("TYPST_PACKAGE_CACHE_PATH").ok(),
            typst_package_path: std::env::var("TYPST_PACKAGE_PATH").ok(),
            typst_root: std::env::var("TYPST_ROOT").ok(),
            typst_update_backup_path: std::env::var("TYPST_UPDATE_BACKUP_PATH").ok(),
            source_date_epoch: std::env::var("SOURCE_DATE_EPOCH").ok(),
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios")))]
            xdg_cache_home: std::env::var("XDG_CACHE_HOME").ok(),
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios")))]
            xdg_data_home: std::env::var("XDG_DATA_HOME").ok(),
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios")))]
            fontconfig_file: std::env::var("FONTCONFIG_FILE").ok(),
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios")))]
            openssl_conf: std::env::var("OPENSSL_CONF").ok(),
            no_color: std::env::var("NO_COLOR").ok(),
            no_proxy: std::env::var("NO_PROXY").ok(),
            http_proxy: std::env::var("HTTP_PROXY").ok(),
            https_proxy: std::env::var("HTTPS_PROXY").ok(),
            all_proxy: std::env::var("ALL_PROXY").ok(),
        }
    }

    /// Returns name-value list of the env vars.
    fn vars(&self) -> impl Iterator<Item = (&'static str, Value<'_>)> {
        let Environment {
            typst_cert,
            typst_features,
            typst_font_paths,
            typst_ignore_system_fonts,
            typst_ignore_embedded_fonts,
            typst_package_cache_path,
            typst_package_path,
            typst_root,
            typst_update_backup_path,
            source_date_epoch,
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios",)))]
            xdg_cache_home,
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios",)))]
            xdg_data_home,
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios",)))]
            fontconfig_file,
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios",)))]
            openssl_conf,
            no_color,
            no_proxy,
            http_proxy,
            https_proxy,
            all_proxy,
        } = self;

        [
            ("TYPST_CERT", typst_cert),
            ("TYPST_FEATURES", typst_features),
            ("TYPST_FONT_PATHS", typst_font_paths),
            ("TYPST_IGNORE_SYSTEM_FONTS", typst_ignore_system_fonts),
            ("TYPST_IGNORE_EMBEDDED_FONTS", typst_ignore_embedded_fonts),
            ("TYPST_PACKAGE_CACHE_PATH", typst_package_cache_path),
            ("TYPST_PACKAGE_PATH", typst_package_path),
            ("TYPST_ROOT", typst_root),
            ("TYPST_UPDATE_BACKUP_PATH", typst_update_backup_path),
            ("SOURCE_DATE_EPOCH", source_date_epoch),
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios",)))]
            ("XDG_CACHE_HOME", xdg_cache_home),
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios",)))]
            ("XDG_DATA_HOME", xdg_data_home),
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios",)))]
            ("FONTCONFIG_FILE", fontconfig_file),
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios",)))]
            ("OPENSSL_CONF", openssl_conf),
            ("NO_COLOR", no_color),
            ("NO_PROXY", no_proxy),
            ("HTTP_PROXY", http_proxy),
            ("HTTPS_PROXY", https_proxy),
            ("ALL_PROXY", all_proxy),
        ]
        .into_iter()
        .map(|(k, v)| (k, v.as_deref().map(Value::String).unwrap_or(Value::Unset)))
    }
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let about = About::new(ctx);

    if let Some(format) = &args.format {
        let w = ctx.ui.stdout();

        match (format, args.pretty) {
            (SerializationFormat::Json, true) => serde_json::to_writer_pretty(w, &about)?,
            (SerializationFormat::Json, false) => serde_json::to_writer(w, &about)?,
            (SerializationFormat::Yaml, _) => serde_yaml::to_writer(w, &about)?,
        }

        return Ok(());
    }

    let mut w = ctx.ui.stderr();

    format_human_readable(&mut w, &about)?;
    Ok(())
}

/// A value for colorful human readable formatting.
enum Value<'a> {
    Unset,
    Bool(bool),
    Path(&'a Path),
    String(&'a str),
}

impl Value<'_> {
    /// Formats this value with optional right padding.
    fn format(&self, out: &mut dyn WriteColor, pad: Option<usize>) -> io::Result<()> {
        match self {
            Value::Unset => write_value_special(out, "<unset>", pad),
            Value::Bool(true) => write_value_special(out, "on", pad),
            Value::Bool(false) => write_value_special(out, "off", pad),
            Value::Path(val) => write_value_simple(out, val.display(), pad),
            Value::String(val) => write_value_simple(out, val, pad),
        }
    }
}

/// Writes a key in cyan with optional right padding.
fn write_key(
    mut out: &mut dyn WriteColor,
    key: impl Display,
    pad: Option<usize>,
) -> io::Result<()> {
    if let Some(pad) = pad {
        cwrite!(colored(out, Color::Cyan), "{key: <pad$}")?;
    } else {
        cwrite!(colored(out, Color::Cyan), "{key}")?;
    }

    Ok(())
}

/// Writes a value in green with optional right padding.
fn write_value_simple(
    mut out: &mut dyn WriteColor,
    val: impl Display,
    pad: Option<usize>,
) -> io::Result<()> {
    if let Some(pad) = pad {
        cwrite!(colored(out, Color::Green), "{val: <pad$}")?;
    } else {
        cwrite!(colored(out, Color::Green), "{val}")?;
    }

    Ok(())
}

/// Writes a special value in blue with optional right padding.
fn write_value_special(
    out: &mut dyn WriteColor,
    val: impl Display,
    pad: Option<usize>,
) -> io::Result<()> {
    out.set_color(ColorSpec::new().set_fg(Some(Color::Blue)))?;
    if let Some(pad) = pad {
        write!(out, "{val: <pad$}")?;
    } else {
        write!(out, "{val}")?;
    }
    out.reset()?;

    Ok(())
}

fn format_human_readable(mut out: &mut dyn WriteColor, value: &About<'_>) -> io::Result<()> {
    write_key(&mut out, "Version", None)?;
    write!(out, " ")?;
    write_value_simple(&mut out, value.version, None)?;
    write!(out, " (")?;
    write_value_simple(
        &mut out,
        value
            .build
            .commit
            .map(|c| &c[..8])
            .unwrap_or("unknown commit"),
        None,
    )?;
    write!(out, ", Typst ")?;
    write_value_simple(&mut out, value.typst_version, None)?;
    write!(out, ", ")?;
    write_value_simple(&mut out, value.build.platform.os, None)?;
    write!(out, " on ")?;
    write_value_simple(&mut out, value.build.platform.arch, None)?;
    writeln!(out, ")\n")?;

    writeln!(out)?;
    writeln!(out, "Fonts")?;
    write!(out, "  ")?;
    write_key(&mut out, "Custom font paths", None)?;
    if value.fonts.paths.is_empty() {
        write!(out, " ")?;
        write_value_special(&mut out, "<none>", None)?;
        writeln!(out)?;
    } else {
        writeln!(out)?;
        for path in value.fonts.custom_paths() {
            write!(out, "    - ")?;
            path.format(&mut out, None)?;
            writeln!(out)?;
        }
    }

    let key_pad = value.fonts.included().map(|(key, _)| key.len()).max();
    for (key, val) in value.fonts.included() {
        write!(out, "  ")?;
        write_key(&mut out, key, key_pad)?;
        write!(out, " ")?;
        val.format(&mut out, None)?;
        writeln!(out)?;
    }

    writeln!(out)?;
    writeln!(out, "Packages")?;
    let key_pad = value.packages.paths().map(|(name, _)| name.len()).max();
    for (key, val) in value.packages.paths() {
        write!(out, "  ")?;
        write_key(&mut out, key, key_pad)?;
        write!(out, " ")?;
        val.format(&mut out, None)?;
        writeln!(out)?;
    }

    writeln!(out)?;
    writeln!(out, "Environment variables")?;
    let key_pad = value.env.vars().map(|(name, _)| name.len()).max();
    for (key, val) in value.env.vars() {
        write!(out, "  ")?;
        write_key(&mut out, key, key_pad)?;
        write!(out, " ")?;
        val.format(&mut out, None)?;

        writeln!(out)?;
    }

    Ok(())
}
