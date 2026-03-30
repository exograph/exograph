// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Builds an `RpcSchema` from a `ModuleSubsystem` for introspection.

use std::collections::{HashMap, HashSet};

use core_model::{
    mapped_arena::{SerializableSlab, SerializableSlabIndex},
    types::{BaseOperationReturnType, FieldType, OperationReturnType},
};
use heck::ToSnakeCase;
use rpc_introspection::schema::{
    RpcMethod, RpcObjectField, RpcObjectType, RpcParameter, RpcSchema, RpcTypeSchema,
};
use subsystem_model_util::{
    module::ModuleMethod,
    subsystem::ModuleSubsystem,
    types::{
        ModuleCompositeType, ModuleFieldType, ModuleOperationReturnType, ModuleType, ModuleTypeKind,
    },
};

/// Result of building the RPC schema, including the method name mapping.
pub struct RpcSchemaWithMapping {
    pub schema: RpcSchema,
    /// Maps snake_case RPC method name → original operation name in the subsystem.
    pub method_name_map: HashMap<String, String>,
}

/// Build an `RpcSchema` from a `ModuleSubsystem`, snake_casing method names.
pub fn build_rpc_schema(subsystem: &ModuleSubsystem) -> RpcSchemaWithMapping {
    let mut schema = RpcSchema::new();
    let mut method_name_map = HashMap::new();
    let mut added_types: HashSet<String> = HashSet::new();

    // Process queries
    for (_, query) in subsystem.queries.iter() {
        if let Some(method_id) = query.method_id {
            let method = &subsystem.methods[method_id];
            build_method(
                &query.name,
                method,
                subsystem,
                &mut schema,
                &mut method_name_map,
                &mut added_types,
            );
        }
    }

    // Process mutations
    for (_, mutation) in subsystem.mutations.iter() {
        if let Some(method_id) = mutation.method_id {
            let method = &subsystem.methods[method_id];
            build_method(
                &mutation.name,
                method,
                subsystem,
                &mut schema,
                &mut method_name_map,
                &mut added_types,
            );
        }
    }

    RpcSchemaWithMapping {
        schema,
        method_name_map,
    }
}

fn build_method(
    operation_name: &str,
    method: &ModuleMethod,
    subsystem: &ModuleSubsystem,
    schema: &mut RpcSchema,
    method_name_map: &mut HashMap<String, String>,
    added_types: &mut HashSet<String>,
) {
    let snake_name = operation_name.to_snake_case();

    // Build result type
    let result_schema = return_type_to_rpc_schema(
        &method.return_type,
        &subsystem.module_types,
        schema,
        added_types,
    );

    let mut rpc_method = RpcMethod::new(snake_name.clone(), result_schema);

    if let Some(ref doc) = method.doc_comments {
        rpc_method = rpc_method.with_description(doc.clone());
    }

    // Build parameters (only non-injected arguments)
    for arg in &method.arguments {
        if arg.is_injected {
            continue;
        }

        let param_schema =
            field_type_to_rpc_schema(&arg.type_id, &subsystem.module_types, schema, added_types);

        rpc_method = rpc_method.with_param(RpcParameter::new(arg.name.clone(), param_schema));
    }

    schema.add_method(rpc_method);
    method_name_map.insert(snake_name, operation_name.to_string());
}

fn return_type_to_rpc_schema(
    return_type: &ModuleOperationReturnType,
    module_types: &SerializableSlab<ModuleType>,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcTypeSchema {
    match return_type {
        ModuleOperationReturnType::Own(op_return_type) => {
            operation_return_type_to_rpc_schema(op_return_type, module_types, schema, added_types)
        }
        ModuleOperationReturnType::Foreign(_) => {
            // Cross-subsystem types: use opaque JSON for now
            RpcTypeSchema::scalar("JSON")
        }
    }
}

fn operation_return_type_to_rpc_schema(
    op_return_type: &OperationReturnType<ModuleType>,
    module_types: &SerializableSlab<ModuleType>,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcTypeSchema {
    match op_return_type {
        FieldType::Plain(BaseOperationReturnType {
            associated_type_id, ..
        }) => {
            let module_type = &module_types[*associated_type_id];
            module_type_to_rpc_schema(module_type, module_types, schema, added_types)
        }
        FieldType::List(inner) => {
            let inner_schema =
                operation_return_type_to_rpc_schema(inner, module_types, schema, added_types);
            RpcTypeSchema::array(inner_schema)
        }
        FieldType::Optional(inner) => {
            let inner_schema =
                operation_return_type_to_rpc_schema(inner, module_types, schema, added_types);
            RpcTypeSchema::optional(inner_schema)
        }
    }
}

fn field_type_to_rpc_schema(
    field_type: &FieldType<SerializableSlabIndex<ModuleType>>,
    module_types: &SerializableSlab<ModuleType>,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcTypeSchema {
    match field_type {
        FieldType::Plain(idx) => {
            let module_type = &module_types[*idx];
            module_type_to_rpc_schema(module_type, module_types, schema, added_types)
        }
        FieldType::List(inner) => {
            let inner_schema = field_type_to_rpc_schema(inner, module_types, schema, added_types);
            RpcTypeSchema::array(inner_schema)
        }
        FieldType::Optional(inner) => {
            let inner_schema = field_type_to_rpc_schema(inner, module_types, schema, added_types);
            RpcTypeSchema::optional(inner_schema)
        }
    }
}

fn module_type_to_rpc_schema(
    module_type: &ModuleType,
    module_types: &SerializableSlab<ModuleType>,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcTypeSchema {
    match &module_type.kind {
        ModuleTypeKind::Primitive => RpcTypeSchema::scalar(&module_type.name),
        ModuleTypeKind::Injected => {
            // Injected types should not appear as parameters (filtered out earlier).
            // If somehow reached, treat as opaque JSON.
            RpcTypeSchema::scalar("JSON")
        }
        ModuleTypeKind::Composite(composite) => {
            let type_name = &module_type.name;

            // Register object type in components if not already done
            if !added_types.contains(type_name) {
                added_types.insert(type_name.clone());
                register_composite_type(type_name, composite, module_types, schema, added_types);
            }

            RpcTypeSchema::object(type_name)
        }
    }
}

fn register_composite_type(
    type_name: &str,
    composite: &ModuleCompositeType,
    module_types: &SerializableSlab<ModuleType>,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) {
    let mut object_type = RpcObjectType::new(type_name);

    if let Some(ref doc) = composite.doc_comments {
        object_type = object_type.with_description(doc.clone());
    }

    for field in &composite.fields {
        let field_schema =
            module_field_type_to_rpc_schema(&field.typ, module_types, schema, added_types);
        let mut rpc_field = RpcObjectField::new(&field.name, field_schema);
        if let Some(ref doc) = field.doc_comments {
            rpc_field = rpc_field.with_description(doc.clone());
        }
        object_type = object_type.with_field(rpc_field);
    }

    schema.add_object_type(type_name.to_string(), object_type);
}

fn module_field_type_to_rpc_schema(
    field_type: &FieldType<ModuleFieldType>,
    module_types: &SerializableSlab<ModuleType>,
    schema: &mut RpcSchema,
    added_types: &mut HashSet<String>,
) -> RpcTypeSchema {
    match field_type {
        FieldType::Plain(mft) => {
            let module_type = &module_types[mft.type_id];
            module_type_to_rpc_schema(module_type, module_types, schema, added_types)
        }
        FieldType::List(inner) => {
            let inner_schema =
                module_field_type_to_rpc_schema(inner, module_types, schema, added_types);
            RpcTypeSchema::array(inner_schema)
        }
        FieldType::Optional(inner) => {
            let inner_schema =
                module_field_type_to_rpc_schema(inner, module_types, schema, added_types);
            RpcTypeSchema::optional(inner_schema)
        }
    }
}
