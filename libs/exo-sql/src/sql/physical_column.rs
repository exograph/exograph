// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    Database, ManyToOneId, OneToManyId, SchemaObjectName, TableId,
    schema::column_spec::{ColumnAutoincrement, ColumnDefault},
};

use super::{ExpressionBuilder, SQLBuilder};
use serde::{Deserialize, Serialize};

/// A column in a physical table
#[derive(Serialize, Deserialize)]
pub struct PhysicalColumn {
    /// The name of the table this column belongs to
    pub table_id: TableId,
    /// The name of the column
    pub name: String,
    /// The type of the column
    pub typ: Box<dyn PhysicalColumnType>,
    /// Is this column a part of the PK for the table
    pub is_pk: bool,
    /// should this type have a NOT NULL constraint or not?
    pub is_nullable: bool,

    /// optional names for unique constraints that this column is a part of
    pub unique_constraints: Vec<String>,

    /// optional default value for this column
    pub default_value: Option<ColumnDefault>,
    pub update_sync: bool,

    /// Names that can be used to group columns together (for example to generate a foreign key constraint name for composite primary keys)
    pub group_names: Vec<String>,
}

impl Clone for PhysicalColumn {
    fn clone(&self) -> Self {
        PhysicalColumn {
            table_id: self.table_id,
            name: self.name.clone(),
            typ: self.typ.clone(),
            is_pk: self.is_pk,
            is_nullable: self.is_nullable,
            unique_constraints: self.unique_constraints.clone(),
            default_value: self.default_value.clone(),
            update_sync: self.update_sync,
            group_names: self.group_names.clone(),
        }
    }
}

impl PartialEq for PhysicalColumn {
    fn eq(&self, other: &Self) -> bool {
        self.table_id == other.table_id
            && self.name == other.name
            && self.typ.equals(other.typ.as_ref())
            && self.is_pk == other.is_pk
            && self.is_nullable == other.is_nullable
            && self.unique_constraints == other.unique_constraints
            && self.default_value == other.default_value
            && self.update_sync == other.update_sync
            && self.group_names == other.group_names
    }
}

impl Eq for PhysicalColumn {}

/// Simpler implementation of Debug for PhysicalColumn.
///
/// The derived implementation of Debug for PhysicalColumn is not very useful, since it includes
/// every field of the struct and obscures the actual useful information. This implementation only
/// prints the table name and column name.
impl std::fmt::Debug for PhysicalColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "Column: {}.{}",
            &self.table_id.arr_idx(),
            &self.name
        ))
    }
}

impl PhysicalColumn {
    pub fn get_table_name(&self, database: &Database) -> SchemaObjectName {
        database.get_table(self.table_id).name.clone()
    }

    pub fn get_sequence_name(&self) -> Option<SchemaObjectName> {
        match &self.default_value {
            Some(ColumnDefault::Autoincrement(ColumnAutoincrement::Sequence { name })) => {
                Some(name.clone())
            }
            _ => None,
        }
    }
}

impl ExpressionBuilder for PhysicalColumn {
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        let table = database.get_table(self.table_id);
        builder.push_table_prefix(&table.name);
        builder.push_identifier(&self.name)
    }
}

// Re-export types for backward compatibility
pub use crate::sql::physical_column_type::PhysicalColumnType;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Copy, Hash)]
pub struct ColumnId {
    pub table_id: TableId,
    pub column_index: usize,
}

impl ColumnId {
    pub fn get_column<'a>(&self, database: &'a Database) -> &'a PhysicalColumn {
        &database.get_table(self.table_id).columns[self.column_index]
    }

    /// Find the many-to-one relation for the given column. The given column must be a foreign key
    /// column. For example, it could be the `concerts.venue_id` column (assuming [Concert] -> Venue).
    pub fn get_mto_relation(&self, database: &Database) -> Option<ManyToOneId> {
        database
            .relations
            .iter()
            .position(|relation| {
                relation
                    .column_pairs
                    .iter()
                    .any(|pair| &pair.self_column_id == self)
            })
            .map(ManyToOneId)
    }

    /// Find the one-to-many relation for the given column. The given column must be a foreign key
    /// column. For example, it could be the `concerts.venue_id` column (assuming [Concert] -> Venue).
    pub fn get_otm_relation(&self, database: &Database) -> Option<OneToManyId> {
        self.get_mto_relation(database).map(OneToManyId)
    }
}

impl PartialOrd for ColumnId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ColumnId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        fn tupled(a: &ColumnId) -> (usize, usize) {
            (a.table_id.arr_idx(), a.column_index)
        }
        tupled(self).cmp(&tupled(other))
    }
}
