use std::path::PathBuf;

use color_eyre::eyre;
use typst_kit::download::Downloader;
use typst_kit::fonts::{FontSearcher, Fonts};
use typst_kit::package::PackageStorage;

use crate::cli::commands::{CompileOptions, FontOptions, PackageOptions, TypstOptions};
use crate::world::SystemWorld;

pub fn world(
    project_root: PathBuf,
    typst_options: &TypstOptions,
    compile_options: &CompileOptions,
) -> eyre::Result<SystemWorld> {
    let world = SystemWorld::new(
        project_root,
        fonts_from_args(&typst_options.font),
        package_storage_from_args(&typst_options.package),
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
        include_system_fonts = ?!args.ignore_system_fonts,
    );

    let mut searcher = FontSearcher::new();

    #[cfg(feature = "embed-fonts")]
    searcher.include_embedded_fonts(true);
    searcher.include_system_fonts(!args.ignore_system_fonts);

    let fonts = searcher.search_with(args.font_paths.iter().map(PathBuf::as_path));

    tracing::debug!(fonts = ?fonts.fonts.len(), "collected fonts");
    fonts
}
