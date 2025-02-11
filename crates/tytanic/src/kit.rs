use std::path::PathBuf;

use color_eyre::eyre;
use typst_kit::download::Downloader;
use typst_kit::fonts::{FontSearcher, Fonts};
use typst_kit::package::PackageStorage;

use crate::cli::commands::{CompileOptions, FontOptions, PackageOptions, Switch};
use crate::world::SystemWorld;

pub fn world(
    project_root: PathBuf,
    font_options: &FontOptions,
    package_options: &PackageOptions,
    compile_options: &CompileOptions,
) -> eyre::Result<SystemWorld> {
    let world = SystemWorld::new(
        project_root,
        fonts_from_args(font_options),
        package_storage_from_args(package_options),
        compile_options.timestamp,
    )?;

    Ok(world)
}

pub fn downloader_from_args(args: &PackageOptions) -> Downloader {
    let agent = format!("{}/{}", tytanic_core::TOOL_NAME, env!("CARGO_PKG_VERSION"));

    match args.certificate.clone() {
        Some(path) => Downloader::with_path(agent, path),
        None => Downloader::new(agent),
    }
}

pub fn package_storage_from_args(args: &PackageOptions) -> PackageStorage {
    PackageStorage::new(
        args.package_cache_path.clone(),
        args.package_path.clone(),
        downloader_from_args(args),
    )
}

pub fn fonts_from_args(args: &FontOptions) -> Fonts {
    let _span = tracing::debug_span!(
        "searching for fonts",
        paths = ?args.font_paths,
        use_system_fonts = ?args.use_system_fonts,
    );

    let mut searcher = FontSearcher::new();

    #[cfg(feature = "embed-fonts")]
    searcher.include_embedded_fonts(true);
    searcher.include_system_fonts(args.use_system_fonts.get_or_default());

    let fonts = searcher.search_with(args.font_paths.iter().map(PathBuf::as_path));

    tracing::debug!(fonts = ?fonts.fonts.len(), "collected fonts");
    fonts
}
