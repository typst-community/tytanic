use typst::visualize::Color;
use typst_test_lib::render;

use super::super::run;
use super::{CliResult, Context, Global};

#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    #[command(flatten)]
    pub run_args: run::Args,

    /// Whether to save temporary output, such as ephemeral references
    #[arg(long)]
    pub no_save_temporary: bool,

    /// Whether to output raster images
    #[arg(long)]
    pub raster: bool,

    /// Whether to putput svg images
    #[arg(long)]
    pub svg: bool,

    /// Whether to output pdf documents
    #[arg(long)]
    pub pdf: bool,

    /// The pixel per inch to use for raster export
    #[arg(
        long,
        visible_alias = "ppi",
        requires = "raster",
        default_value_t = 144.0
    )]
    pub pixel_per_inch: f32,
}

pub fn run(ctx: Context, global: &Global, args: &Args) -> anyhow::Result<CliResult> {
    let strategy = render::Strategy {
        pixel_per_pt: render::ppi_to_ppp(args.pixel_per_inch),
        fill: Color::WHITE,
    };

    if args.pdf || args.svg {
        return Ok(CliResult::operation_failure(
            "PDF and SVGF export are not yet supported",
        ));
    }

    // TODO: with pdf + with svg export
    run::run(ctx, global, &args.run_args, |ctx| {
        ctx.with_render_strategy(Some(strategy))
            .with_no_save_temporary(args.no_save_temporary)
    })
}
