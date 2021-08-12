use id_arena::Id;

use super::column_id::ColumnId;

use super::types::GqlTypeModifier;

#[derive(Debug, Clone)]
pub struct PredicateParameter {
    pub name: String,
    pub type_name: String,
    pub type_modifier: GqlTypeModifier,
    pub type_id: Id<PredicateParameterType>,
    pub column_id: Option<ColumnId>,
}

#[derive(Debug, Clone)]
pub struct PredicateParameterType {
    pub name: String,
    pub kind: PredicateParameterTypeKind,
}

type ModelPredicateParameters = Vec<PredicateParameter>;
type BooleanPredicateParameters = Vec<PredicateParameter>;

#[derive(Debug, Clone)]
pub enum PredicateParameterTypeKind {
    ImplicitEqual,                     // {id: 3}
    Opeartor(Vec<PredicateParameter>), // {lt: ..,gt: ..} such as IntFilter
    Composite(ModelPredicateParameters, BooleanPredicateParameters), // {where: {id: .., name: ..}} such as AccountFilter
                                                                     // also includes comparators like
                                                                     // {where: {
                                                                     //   and: [{name: ..}, {id: ..}]
                                                                     // }}
}
