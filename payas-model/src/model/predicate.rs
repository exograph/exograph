use serde::{Deserialize, Serialize};

use super::column_id::ColumnId;
use super::GqlType;

use super::mapped_arena::SerializableSlabIndex;
use super::types::GqlTypeModifier;

/// The two columns that link one table to another
/// These columns may be used to form a join between two tables
#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct ColumnIdPathLink {
    pub self_column_id: ColumnId,
    pub linked_column_id: Option<ColumnId>,
}

/// A list of path from that represent a relation between two tables
/// For example to reach concert -> concert_artist -> artist -> name,
/// the path would be [(concert.id, concert_artist.concert_id), (concert_artists.artists_id, artist.id), (artist.name, None)]
/// This information could be used to form a join between multiple tables
#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct ColumnIdPath {
    pub path: Vec<ColumnIdPathLink>,
}

impl ColumnIdPath {
    pub fn leaf_column(&self) -> ColumnId {
        self.path.last().expect("Empty column path").self_column_id
    }
}

impl ColumnIdPathLink {
    pub fn new(self_column_id: ColumnId, linked_column_id: Option<ColumnId>) -> Self {
        Self {
            self_column_id,
            linked_column_id,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PredicateParameter {
    /// The name of the parameter. For example, "where", "and", "id", "venue", etc.
    pub name: String,
    /// The type name of the parameter.
    /// For example, "ConcertFilter", "IntFilter". We need to keep this only for introspection, which doesn't have access to the ModelSystem.
    /// We might find a way to avoid this, since given the model system and type_id of the parameter, we can get the type name.
    pub type_name: String,
    /// The type modifier of the parameter. For parameters such as "and", this will be a list.
    pub type_modifier: GqlTypeModifier,
    /// Type id of the parameter type. For example: IntFilter, StringFilter, etc.
    pub type_id: SerializableSlabIndex<PredicateParameterType>,

    /// How does this parameter relates with the parent parameter?
    /// For example for parameter used as {where: {venue1: {id: {eq: 1}}}}, we will have following column links:
    /// eq: None
    /// id: Some((<the venues.id column>, None))
    /// venue1: Some((<the concerts.venue1_id column>, <the venues.id column>))
    /// where: None
    pub column_path_link: Option<ColumnIdPathLink>,

    /// The type this parameter is filtering on. For example, for ConcertFilter, this will be (the index of) the Concert.
    pub underlying_type_id: SerializableSlabIndex<GqlType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PredicateParameterType {
    pub name: String,
    pub kind: PredicateParameterTypeKind,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PredicateParameterTypeKind {
    ImplicitEqual,                     // {id: 3}
    Operator(Vec<PredicateParameter>), // {lt: ..,gt: ..} such as IntFilter
    Composite {
        field_params: Vec<PredicateParameter>, // {where: {id: .., name: ..}} such as AccountFilter
        logical_op_params: Vec<PredicateParameter>, // logical operator predicates like `and: [{name: ..}, {id: ..}]`
    },
}
