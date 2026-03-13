// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Schema builder for RPC introspection.
//!
//! Builds an RpcSchema from PostgresRpcSubsystemWithRouter by iterating
//! through all collection queries, pk queries, and unique constraint queries.
//!
//! Uses two traits to keep schema building generic:
//! - `BuildRpcMethod`: Converts a query (collection or pk) into an `RpcMethod`.
//! - `BuildRpcTypeSchema`: Converts a parameter type (predicate or order-by) into an `RpcTypeSchema`.

mod create_mutation_builder;
mod delete_mutation_builder;
mod query_builder;
mod type_builder;
mod update_mutation_builder;

use core_model::types::OperationReturnType;
use postgres_core_model::types::EntityType;
use postgres_rpc_model::operation::HasPredicateParams;
use postgres_rpc_model::subsystem::PostgresRpcSubsystemWithRouter;
use rpc_introspection::schema::{RpcMethod, RpcParameter, RpcSchema, RpcTypeSchema};
use std::collections::HashSet;

use create_mutation_builder::build_create_method;
use type_builder::build_return_type_schema_with;
use update_mutation_builder::build_update_predicate_params_method;

/// Trait for converting a query into an `RpcMethod`.
pub(crate) trait BuildRpcMethod {
    fn build_rpc_method(
        &self,
        subsystem: &PostgresRpcSubsystemWithRouter,
        schema: &mut RpcSchema,
        added_types: &mut HashSet<String>,
    ) -> RpcMethod;
}

/// Trait for converting a parameter type into an `RpcTypeSchema`.
pub(crate) trait BuildRpcTypeSchema {
    fn build_rpc_type_schema(
        &self,
        subsystem: &PostgresRpcSubsystemWithRouter,
        schema: &mut RpcSchema,
        added_types: &mut HashSet<String>,
    ) -> RpcTypeSchema;
}

/// Trait to access name, return_type, and doc_comments from operation types.
pub(crate) trait HasMethodNameAndReturnType {
    fn method_name(&self) -> &str;
    fn return_type(&self) -> &OperationReturnType<EntityType>;
    fn doc_comments(&self) -> Option<&str>;
}

impl<P> HasMethodNameAndReturnType for postgres_rpc_model::operation::PostgresOperation<P> {
    fn method_name(&self) -> &str {
        &self.name
    }

    fn return_type(&self) -> &OperationReturnType<EntityType> {
        &self.return_type
    }

    fn doc_comments(&self) -> Option<&str> {
        self.doc_comments.as_deref()
    }
}

/// Build an RpcSchema from a PostgresRpcSubsystemWithRouter.
pub fn build_rpc_schema(subsystem: &PostgresRpcSubsystemWithRouter) -> RpcSchema {
    let mut schema = RpcSchema::new();
    let mut added_types: HashSet<String> = HashSet::new();

    for (_, query) in subsystem.collection_queries.iter() {
        let method = query.build_rpc_method(subsystem, &mut schema, &mut added_types);
        schema.add_method(method);
    }

    // Build PK query methods (get_<entity>)
    for (_, pk_query) in subsystem.pk_queries.iter() {
        let method = build_predicate_params_method(
            pk_query,
            ReturnTypeKind::Full,
            subsystem,
            &mut schema,
            &mut added_types,
        );
        schema.add_method(method);
    }

    // Build unique query methods (get_<entity>_by_<constraint>)
    for (_, unique_query) in subsystem.unique_queries.iter() {
        let method = build_predicate_params_method(
            unique_query,
            ReturnTypeKind::Full,
            subsystem,
            &mut schema,
            &mut added_types,
        );
        schema.add_method(method);
    }

    // Build collection delete methods (delete_<entities>)
    for (_, delete) in subsystem.collection_deletes.iter() {
        let method = delete.build_rpc_method(subsystem, &mut schema, &mut added_types);
        schema.add_method(method);
    }

    // Build PK delete methods (delete_<entity>)
    for (_, pk_delete) in subsystem.pk_deletes.iter() {
        let method = build_predicate_params_method(
            pk_delete,
            ReturnTypeKind::PkOnly,
            subsystem,
            &mut schema,
            &mut added_types,
        );
        schema.add_method(method);
    }

    // Build unique delete methods (delete_<entity>_by_<constraint>)
    for (_, unique_delete) in subsystem.unique_deletes.iter() {
        let method = build_predicate_params_method(
            unique_delete,
            ReturnTypeKind::PkOnly,
            subsystem,
            &mut schema,
            &mut added_types,
        );
        schema.add_method(method);
    }

    // Build collection update methods (update_<entities>)
    for (_, update) in subsystem.collection_updates.iter() {
        let method = update.build_rpc_method(subsystem, &mut schema, &mut added_types);
        schema.add_method(method);
    }

    // Build PK update methods (update_<entity>)
    for (_, pk_update) in subsystem.pk_updates.iter() {
        let method = build_update_predicate_params_method(
            pk_update,
            &pk_update.parameters.data_param.name,
            subsystem,
            &mut schema,
            &mut added_types,
        );
        schema.add_method(method);
    }

    // Build unique update methods (update_<entity>_by_<constraint>)
    for (_, unique_update) in subsystem.unique_updates.iter() {
        let method = build_update_predicate_params_method(
            unique_update,
            &unique_update.parameters.data_param.name,
            subsystem,
            &mut schema,
            &mut added_types,
        );
        schema.add_method(method);
    }

    // Build single create methods (create_<entity>)
    for (_, create) in subsystem.creates.iter() {
        let method = build_create_method(
            create,
            &create.parameters.data_param.name,
            false,
            subsystem,
            &mut schema,
            &mut added_types,
        );
        schema.add_method(method);
    }

    // Build collection create methods (create_<entities>)
    for (_, collection_create) in subsystem.collection_creates.iter() {
        let method = build_create_method(
            collection_create,
            &collection_create.parameters.data_param.name,
            true,
            subsystem,
            &mut schema,
            &mut added_types,
        );
        schema.add_method(method);
    }

    schema
}

#[derive(Clone, Copy)]
pub(crate) enum ReturnTypeKind {
    Full,
    PkOnly,
}

/// Build an RPC method from an operation with predicate params (PK or unique query/delete).
/// Each method gets flat parameters — no `by` wrapper.
pub(crate) fn build_predicate_params_method<T>(
    op: &T,
    return_type_kind: ReturnTypeKind,
    subsystem: &PostgresRpcSubsystemWithRouter,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcMethod
where
    T: HasPredicateParams + HasMethodNameAndReturnType,
{
    let result_schema = build_return_type_schema_with(
        op.return_type(),
        return_type_kind,
        subsystem,
        schema,
        added_types,
    );

    let mut method = RpcMethod::new(op.method_name().to_string(), result_schema);

    if let Some(doc) = op.doc_comments() {
        method = method.with_description(doc);
    }

    for param in op.predicate_params() {
        let param_schema = param.build_rpc_type_schema(subsystem, schema, added_types);
        let rpc_param = RpcParameter::new(&param.name, param_schema);
        method = method.with_param(rpc_param);
    }

    method
}
