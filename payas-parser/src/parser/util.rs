use super::Rule;

use pest::{
    error::Error,
    iterators::{Pair, Pairs},
};

// Copied from async-graphql-parser
pub(super) fn next_if_rule<'a>(pairs: &mut Pairs<'a, Rule>, rule: Rule) -> Option<Pair<'a, Rule>> {
    if pairs.peek().map_or(false, |pair| pair.as_rule() == rule) {
        Some(pairs.next().unwrap())
    } else {
        None
    }
}

pub(super) fn parse_if_rule<T>(
    pairs: &mut Pairs<Rule>,
    rule: Rule,
    f: impl FnOnce(Pair<Rule>) -> Result<T, Error<Rule>>,
) -> Result<Option<T>, Error<Rule>> {
    next_if_rule(pairs, rule).map(f).transpose()
}
