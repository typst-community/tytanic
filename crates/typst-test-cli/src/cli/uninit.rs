use std::fmt::Write;

use super::{CliResult, Context, Global};
use crate::cli::bail_if_invalid_matcher_expr;
use crate::util;

pub fn run(ctx: Context, global: &Global) -> anyhow::Result<CliResult> {
    bail_if_invalid_matcher_expr!(global => matcher);
    ctx.project.collect_tests(matcher)?;
    let count = ctx.project.matched().len();

    ctx.project.uninit()?;
    writeln!(
        ctx.reporter,
        "Removed {} {}",
        count,
        util::fmt::plural(count, "test"),
    )?;

    Ok(CliResult::Ok)
}
