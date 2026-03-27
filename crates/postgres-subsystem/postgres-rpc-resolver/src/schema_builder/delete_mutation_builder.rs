// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use postgres_rpc_model::operation::CollectionDelete;
use postgres_rpc_model::subsystem::PostgresRpcSubsystemWithRouter;
use rpc_introspection::schema::{RpcMethod, RpcParameter, RpcSchema, RpcTypeSchema};
use std::collections::HashSet;

use super::type_builder::build_return_type_schema_for_entity;
use super::{BuildRpcMethod, BuildRpcTypeSchema, build_projection_param};

impl BuildRpcMethod for CollectionDelete {
    fn build_rpc_method(
        &self,
        subsystem: &PostgresRpcSubsystemWithRouter,
        schema: &mut RpcSchema,
        added_types: &mut HashSet<String>,
    ) -> RpcMethod {
        let entity_type = self.return_type.typ(&subsystem.core_subsystem.entity_types);
        let result_schema =
            build_return_type_schema_for_entity(&self.return_type, subsystem, schema, added_types);

        let mut method = RpcMethod::new(self.name.clone(), result_schema);
        if let Some(doc) = &self.doc_comments {
            method = method.with_description(doc);
        }

        // Add `where` parameter (same filter type as collection queries)
        let where_param = RpcParameter::new(
            &self.parameters.predicate_param.name,
            RpcTypeSchema::optional(self.parameters.predicate_param.build_rpc_type_schema(
                subsystem,
                schema,
                added_types,
            )),
        )
        .with_description(format!("Filter conditions for {}", entity_type.plural_name));
        method = method.with_param(where_param);

        method = method.with_param(build_projection_param(entity_type));

        method
    }
}
