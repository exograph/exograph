// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use serde::{Deserialize, Serialize};

use exo_sql::PhysicalTable;

use core_plugin_interface::core_model::mapped_arena::SerializableSlabIndex;

#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresOperation {
    pub kind: PostgresOperationKind,
    pub table_id: SerializableSlabIndex<PhysicalTable>,
    // TODO: Add parameter model
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PostgresOperationKind {
    Query,
    Mutation,
}
