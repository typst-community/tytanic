use typst_test_lib::{compare, render};

use super::util::export;
use super::{run, CliResult, Context, Global};

#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    #[command(flatten)]
    pub export_args: export::Args,

    /// The maximum delta in each channel of a pixel
    ///
    /// If a single channel (red/green/blue/alpha component) of a pixel differs
    /// by this much between reference and output the pixel is counted as a
    /// deviation.
    #[arg(long, default_value_t = 0)]
    pub max_delta: u8,

    /// The maximum deviation per reference
    ///
    /// If a reference and output image have more than the given deviations it's
    /// counted as a failure.
    #[arg(long, default_value_t = 0)]
    pub max_deviation: usize,
}

pub fn run(ctx: Context, global: &Global, args: &Args) -> anyhow::Result<CliResult> {
    let render_strategy = render::Strategy {
        pixel_per_pt: render::ppi_to_ppp(args.export_args.pixel_per_inch),
    };

    let compare_strategy = compare::Strategy::Visual(compare::visual::Strategy::Simple {
        max_delta: args.max_delta,
        max_deviation: args.max_deviation,
    });

    // TODO: see super::export
    if args.export_args.pdf || args.export_args.svg {
        return Ok(CliResult::operation_failure(
            "PDF and SVGF export are not yet supported",
        ));
    }

    run::run(ctx, global, &args.export_args.run_args, |ctx| {
        ctx.with_compare_strategy(Some(compare_strategy))
            .with_render_strategy(Some(render_strategy))
            .with_save_temporary(args.export_args.save_temporary)
    })
}
