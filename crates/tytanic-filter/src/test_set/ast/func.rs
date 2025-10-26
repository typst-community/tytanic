use ecow::EcoVec;
use ecow::eco_vec;
use pest::iterators::Pair;
use pest::pratt_parser::PrattParser;

use crate::test_set::ast::Error;
use crate::test_set::ast::Expr;
use crate::test_set::ast::Id;
use crate::test_set::ast::PairExt;
use crate::test_set::ast::PairsExt;
use crate::test_set::ast::Rule;
use crate::test_set::eval;
use crate::test_set::eval::Context;
use crate::test_set::eval::Eval;
use crate::test_set::eval::Value;

/// A function call node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Func {
    /// The identifier of this function.
    pub id: Id,

    /// The arguments of this function.
    pub args: EcoVec<Expr>,
}

impl Eval for Func {
    fn eval(&self, ctx: &Context) -> Result<Value, eval::Error> {
        let func: eval::Func = ctx.resolve(&self.id)?.expect_type()?;
        let args = self
            .args
            .iter()
            .map(|e| e.eval(ctx))
            .collect::<Result<Vec<_>, _>>()?;

        func.call(ctx, &args)
    }
}

impl Func {
    pub(super) fn parse(pair: Pair<'_, Rule>, pratt: &PrattParser<Rule>) -> Result<Self, Error> {
        pair.expect_rules(&[Rule::func])?;
        let mut pairs = pair.into_inner();

        let id = pairs.expect_pair(&[Rule::id])?;
        let id = Id::parse(id)?;

        let args = pairs.expect_pair(&[Rule::func_args])?;
        let mut pairs = args.into_inner();

        let _ = pairs.expect_pair(&[Rule::func_args_delim_open])?;
        let args_or_close =
            pairs.expect_pair(&[Rule::func_args_inner, Rule::func_args_delim_close])?;
        let args = if args_or_close.as_rule() == Rule::func_args_inner {
            let _ = pairs.expect_pair(&[Rule::func_args_delim_close])?;

            let mut pairs = args_or_close.into_inner();

            let mut args = eco_vec![];
            loop {
                let Some(arg) = pairs.try_expect_pair(&[Rule::expr])? else {
                    break;
                };

                args.push(Expr::parse(arg, pratt)?);

                let Some(_) = pairs.try_expect_pair(&[Rule::func_args_sep])? else {
                    break;
                };
            }

            args
        } else {
            eco_vec![]
        };

        pairs.expect_end()?;

        Ok(Self { id, args })
    }
}
