use std::vec;

use exo_sql::{ColumnId, VectorDistanceFunction};
use serde::{Deserialize, Serialize};

use crate::access::Access;

/// Field for a vector distance function
/// Represents:
/// ```graphql
/// document {
///    contentVector: [Float!]!
///    contentVectorDistance(to: [Float!]!): Float! <--- This is the field
/// }
/// ```
#[derive(Serialize, Deserialize, Debug)]
pub struct VectorDistanceField {
    pub name: String,
    pub column_id: ColumnId,
    pub size: usize,
    pub distance_function: VectorDistanceFunction,
    pub access: Access,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VectorDistanceType {
    pub name: String, // name of the type, currently always "VectorDistance", but we could introduce `VectorDistance64`, etc. in the future
    pub fields: Vec<VectorDistanceTypeField>,
}

impl VectorDistanceType {
    pub fn new(name: String) -> Self {
        Self {
            name,
            fields: vec![VectorDistanceTypeField {}],
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VectorDistanceTypeField {}
