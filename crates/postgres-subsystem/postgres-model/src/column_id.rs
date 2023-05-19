// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::subsystem::PostgresSubsystem;
use core_plugin_interface::core_model::mapped_arena::SerializableSlabIndex;
use exo_sql::{PhysicalColumn, PhysicalTable};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColumnId {
    pub table_id: SerializableSlabIndex<PhysicalTable>,
    column_index: usize,
}

impl ColumnId {
    pub fn new(table_id: SerializableSlabIndex<PhysicalTable>, column_index: usize) -> ColumnId {
        ColumnId {
            table_id,
            column_index,
        }
    }

    pub fn get_column<'a>(&self, system: &'a PostgresSubsystem) -> &'a PhysicalColumn {
        &system.database.tables[self.table_id].columns[self.column_index]
    }
}
