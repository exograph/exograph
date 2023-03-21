use tracing::debug;

use crate::sql::select::Select;

use super::{
    plain_join_strategy::PlainJoinStrategy, plain_subquery_strategy::PlainSubqueryStrategy,
    selection_context::SelectionContext, selection_strategy::SelectionStrategy,
    subquery_with_in_predicate_strategy::SubqueryWithInPredicateStrategy,
    unconditional_strategy::Unconditional,
};

pub(crate) struct SelectionStrategyChain<'s> {
    strategies: Vec<&'s dyn SelectionStrategy>,
}

impl<'s> SelectionStrategyChain<'s> {
    pub fn new(strategies: Vec<&'s dyn SelectionStrategy>) -> Self {
        Self { strategies }
    }

    pub fn to_select<'a>(&self, selection_context: SelectionContext<'_, 'a>) -> Option<Select<'a>> {
        let strategy = self
            .strategies
            .iter()
            .find(|s| s.suitable(&selection_context))?;

        debug!("Using selection strategy: {}", strategy.id());

        Some(strategy.to_select(selection_context))
    }
}

impl Default for SelectionStrategyChain<'_> {
    fn default() -> Self {
        Self::new(vec![
            &PlainJoinStrategy {},
            &PlainSubqueryStrategy {},
            &SubqueryWithInPredicateStrategy {},
            &Unconditional {},
        ])
    }
}
