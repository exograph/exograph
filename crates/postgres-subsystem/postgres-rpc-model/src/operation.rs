// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use postgres_core_model::predicate::PredicateParameter;
use postgres_core_model::types::EntityType;
use serde::{Deserialize, Serialize};

use core_model::mapped_arena::SerializableSlabIndex;

// TODO: Share this with REST?

#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresOperation {
    pub kind: PostgresOperationKind,
    pub entity_type_id: SerializableSlabIndex<EntityType>,
    pub predicate_param: PredicateParameter,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PostgresOperationKind {
    Query,
    Create,
    Update,
    Delete,
}
