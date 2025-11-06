use std::{env, io::Write, path::PathBuf};

use color_eyre::eyre;
use termcolor::Color;

use crate::{cli::commands::Switch, cwrite, cwriteln};

use super::Context;

struct About<'a> {
    build: Build,
    fonts: Fonts<'a>,
    packages: Packages,
    environment: Environment,
}

impl<'a> About<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            build: Build::new(),
            fonts: Fonts::new(ctx),
            packages: Packages::new(ctx),
            environment: Environment::new(),
        }
    }
}

struct Build {
    /// The version of tytanic.
    version: &'static str,
    /// The typst version used in this instance of tytanic.
    typst_version: &'static str,
    /// The commmit sha of the current commit when building tytanic.
    commit_sha: &'static str,
    os: &'static str,
    arch: &'static str,
}

impl Build {
    /// Retrieves build informations specified in the `build.rs`
    const fn new() -> Self {
        Self {
            version: env!("TYTANIC_VERSION"),
            typst_version: env!("TYTANIC_TYPST_VERSION"),
            commit_sha: env!("TYTANIC_COMMIT_SHA"),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        }
    }
}

struct Fonts<'a> {
    /// The font paths.
    paths: &'a [PathBuf],
    /// Whether system fonts were included in the search.
    system: bool,
    /// Whether embedded fonts were included in the search.
    embedded: bool,
}

impl<'a> Fonts<'a> {
    /// Retrieves font informations.
    fn new(ctx: &'a Context) -> Self {
        Self {
            paths: &ctx.args.font.font_paths,
            system: ctx.args.font.use_system_fonts.get_or_default(),
            embedded: ctx.args.font.use_embedded_fonts.get_or_default(),
        }
    }
}

struct Packages {
    package_path: Option<PathBuf>,
    package_cache_path: Option<PathBuf>,
}

impl Packages {
    /// Retrieves the package informations.
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
}

struct Environment {
    typst_root: Option<String>,
    typst_font_paths: Option<String>,
    typst_package_path: Option<String>,
    typst_package_cache_path: Option<String>,
    typst_cert: Option<String>,
    source_date_epoch: Option<String>,
}

impl Environment {
    /// Retrieves the environment informations.
    /// 
    /// Unset or invalid env vars will be set to `None`.
    fn new() -> Self {
        Self {
            typst_root: std::env::var("TYPST_ROOT").ok(),
            typst_font_paths: std::env::var("TYPST_FONT_PATHS").ok(),
            typst_package_path: std::env::var("TYPST_PACKAGE_PATH").ok(),
            typst_package_cache_path: std::env::var("TYPST_PACKAGE_CACHE_PATH").ok(),
            typst_cert: std::env::var("TYPST_CERT").ok(),
            source_date_epoch: std::env::var("SOURCE_DATE_EPOCH").ok(),
        }
    }

    /// Returns name-value list of the env vars.
    fn vars(&self) -> Vec<(&'static str, Option<&str>)> {
        vec![
            ("TYPST_ROOT", self.typst_root.as_deref()),
            ("TYPST_FONT_PATHS", self.typst_font_paths.as_deref()),
            ("TYPST_PACKAGE_PATH", self.typst_package_path.as_deref()),
            (
                "TYPST_PACKAGE_CACHE_PATH",
                self.typst_package_cache_path.as_deref(),
            ),
            ("TYPST_CERT", self.typst_cert.as_deref()),
            ("SOURCE_DATE_EPOCH", self.source_date_epoch.as_deref()),
        ]
    }
}

pub fn run(ctx: &mut Context) -> eyre::Result<()> {
    let about = About::new(ctx);

    let mut w = ctx.ui.stderr();

    // Write build info
    let build = about.build;
    cwrite!(colored(w, Color::Cyan), "Version ")?;
    cwrite!(colored(w, Color::Green), "{} ", build.version)?;
    write!(w, "(")?;
    cwrite!(colored(w, Color::Green), "{}", build.commit_sha)?;
    write!(w, ", ")?;
    cwrite!(colored(w, Color::Green), "{} ", build.os)?;
    write!(w, "on ")?;
    cwrite!(colored(w, Color::Green), "{}", build.arch)?;
    write!(w, ", typst: ")?;
    cwrite!(colored(w, Color::Green), "{}", build.typst_version)?;
    writeln!(w, ")")?;

    writeln!(w)?;

    // Write fonts info
    let fonts = about.fonts;
    writeln!(w, "Fonts")?;

    cwrite!(colored(w, Color::Cyan), "  Custom font paths")?;
    if fonts.paths.is_empty() {
        cwriteln!(colored(w, Color::Magenta), " <none>")?;
    } else {
        writeln!(w)?;
        for path in fonts.paths {
            write!(w, "    - ")?;
            cwriteln!(colored(w, Color::Green), "{}", path.display())?;
        }
    }

    cwrite!(colored(w, Color::Cyan), "  System fonts   ")?;
    cwriteln!(
        colored(w, Color::Magenta),
        "{}",
        if fonts.system { "on" } else { "off" }
    )?;

    cwrite!(colored(w, Color::Cyan), "  Embedded fonts ")?;
    cwriteln!(
        colored(w, Color::Magenta),
        "{}",
        if fonts.embedded { "on" } else { "off" }
    )?;

    writeln!(w)?;

    // Write packages info
    let packages = about.packages;
    writeln!(w, "Packages")?;

    cwrite!(colored(w, Color::Cyan), "  Package path       ")?;
    if let Some(package_path) = packages.package_path {
        cwriteln!(colored(w, Color::Green), "{}", package_path.display())?;
    } else {
        cwriteln!(colored(w, Color::Magenta), "<none>")?;
    }

    cwrite!(colored(w, Color::Cyan), "  Package cache path ")?;
    if let Some(package_cache_path) = packages.package_cache_path {
        cwriteln!(colored(w, Color::Green), "{}", package_cache_path.display())?;
    } else {
        cwriteln!(colored(w, Color::Magenta), "<none>")?;
    }

    writeln!(w)?;

    // Write environment info
    let envs = about.environment.vars();
    let padding = envs.iter().map(|(name, _)| name.len()).max().unwrap_or(0);

    writeln!(w, "Environment variables")?;

    for (name, value) in envs {
        cwrite!(
            colored(w, Color::Cyan),
            "  {:<width$} ",
            name,
            width = padding
        )?;
        if let Some(value) = value {
            cwriteln!(colored(w, Color::Green), "{}", value)?;
        } else {
            cwriteln!(colored(w, Color::Magenta), "<unset>")?;
        }
    }

    Ok(())
}
