use core_plugin_interface::core_resolver::access_solver::AccessPredicate;
use exo_sql::AbstractPredicate;

// Only to get around the orphan rule while implementing AccessSolver
#[derive(Debug)]
pub struct AbstractPredicateWrapper(pub AbstractPredicate);

impl std::ops::Not for AbstractPredicateWrapper {
    type Output = AbstractPredicateWrapper;

    fn not(self) -> Self::Output {
        AbstractPredicateWrapper(self.0.not())
    }
}

impl From<bool> for AbstractPredicateWrapper {
    fn from(value: bool) -> Self {
        AbstractPredicateWrapper(AbstractPredicate::from(value))
    }
}

impl AccessPredicate for AbstractPredicateWrapper {
    fn and(self, other: Self) -> Self {
        AbstractPredicateWrapper(AbstractPredicate::and(self.0, other.0))
    }

    fn or(self, other: Self) -> Self {
        AbstractPredicateWrapper(AbstractPredicate::or(self.0, other.0))
    }
}
