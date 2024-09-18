// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::{
    context_type::ContextType, function_defn::FunctionDefinition, mapped_arena::MappedArena,
    types::FieldType,
};
use core_model_builder::{ast::ast_types::AstExpr, error::ModelBuildingError, typechecker::Typed};
use subsystem_model_util::{
    access::Access,
    types::{ModuleCompositeType, ModuleField, ModuleFieldType, ModuleType, ModuleTypeKind},
};

use crate::builder::resolved_builder::ResolvedFieldType;

use super::{
    access_builder::ResolvedAccess,
    access_utils,
    resolved_builder::{
        ResolvedCompositeType, ResolvedField, ResolvedMethod, ResolvedModule, ResolvedType,
    },
    system_builder::SystemContextBuilding,
};

#[derive(Debug, Clone)]
pub struct ResolvedTypeEnv<'a> {
    pub contexts: &'a MappedArena<ContextType>,
    pub function_definitions: &'a MappedArena<FunctionDefinition>,
    pub resolved_types: MappedArena<ResolvedType>,
    pub resolved_modules: MappedArena<ResolvedModule>,
}

pub(super) fn build_module_expanded(
    resolved_methods: &[&ResolvedMethod],
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            expand_module_type_no_fields(c, building);
        }
    }

    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            expand_module_type_fields(c, building);
        }
    }

    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            expand_type_access(c, resolved_env, building)?;
        }
    }

    for method in resolved_methods.iter() {
        expand_method_access(method, resolved_env, building)?
    }

    Ok(())
}

pub(super) fn build_shallow(
    types: &MappedArena<ResolvedType>,
    contexts: &MappedArena<ContextType>,
    building: &mut SystemContextBuilding,
) {
    for (_, typ) in types.iter() {
        create_shallow_type(typ, building);
    }

    // For contexts, building shallow types is enough
    // (we need to have them in the SystemContextBuilding.types, so that when passed as an argument to a module method, we can resolve the type)
    for (_, context) in contexts.iter() {
        create_shallow_context(context, building);
    }
}

fn expand_module_type_no_fields(
    resolved_type: &ResolvedCompositeType,
    building: &mut SystemContextBuilding,
) {
    let existing_type_id = building.get_id(&resolved_type.name);

    building.types.get_by_id_mut(existing_type_id.unwrap()).kind =
        ModuleTypeKind::Composite(ModuleCompositeType {
            fields: vec![],
            access: Access::restrictive(),
            is_input: false,
        });
    building
        .types
        .get_by_id_mut(existing_type_id.unwrap())
        .is_input = resolved_type.is_input;
}

fn expand_module_type_fields(
    resolved_type: &ResolvedCompositeType,
    building: &mut SystemContextBuilding,
) {
    let existing_type_id = building.get_id(&resolved_type.name).unwrap();

    let model_fields: Vec<ModuleField> = resolved_type
        .fields
        .iter()
        .map(|field| create_module_field(field, building))
        .collect();

    let kind = ModuleTypeKind::Composite(ModuleCompositeType {
        fields: model_fields,
        is_input: resolved_type.is_input,
        access: Access::restrictive(),
    });

    building.types.get_by_id_mut(existing_type_id).kind = kind
}

fn expand_method_access(
    resolved_method: &ResolvedMethod,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    let existing_method_id = building.methods.get_id(&resolved_method.name).unwrap();
    let expr = compute_access_method(&resolved_method.access, resolved_env)?;
    building.methods.get_by_id_mut(existing_method_id).access = expr;

    Ok(())
}

fn create_module_field(field: &ResolvedField, building: &SystemContextBuilding) -> ModuleField {
    fn create_field_type(
        field_type: &FieldType<ResolvedFieldType>,
        building: &SystemContextBuilding,
    ) -> FieldType<ModuleFieldType> {
        match field_type {
            FieldType::Plain(ResolvedFieldType { type_name, .. }) => {
                let type_id = building.types.get_id(type_name).unwrap();

                FieldType::Plain(ModuleFieldType {
                    type_name: type_name.clone(),
                    type_id,
                })
            }
            FieldType::Optional(underlying) => {
                FieldType::Optional(Box::new(create_field_type(underlying, building)))
            }
            FieldType::List(underlying) => {
                FieldType::List(Box::new(create_field_type(underlying, building)))
            }
        }
    }

    ModuleField {
        name: field.name.to_owned(),
        typ: create_field_type(&field.typ, building),
        has_default_value: field.default_value.is_some(),
    }
}

fn compute_access_composite_types(
    resolved: &ResolvedAccess,
    resolved_env: &ResolvedTypeEnv,
) -> Result<Access, ModelBuildingError> {
    let access_expr =
        |expr: &AstExpr<Typed>| access_utils::compute_predicate_expression(expr, resolved_env);

    Ok(Access {
        value: access_expr(&resolved.value)?,
    })
}

// Expand access expressions (pre-condition: all type fields have been populated)
fn expand_type_access(
    resolved_type: &ResolvedCompositeType,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    let existing_type_id = building.get_id(&resolved_type.name).unwrap();

    let existing_type = &building.types[existing_type_id];

    if let ModuleTypeKind::Composite(self_type_info) = &existing_type.kind {
        let expr = compute_access_composite_types(&resolved_type.access, resolved_env)?;

        let kind = ModuleTypeKind::Composite(ModuleCompositeType {
            fields: self_type_info.fields.clone(),
            is_input: self_type_info.is_input,
            access: expr,
        });

        building.types.get_by_id_mut(existing_type_id).kind = kind
    };

    Ok(())
}

fn create_shallow_type(resolved_type: &ResolvedType, building: &mut SystemContextBuilding) {
    let type_name = resolved_type.name();

    // Mark every type as Primitive, since other types that may be referred haven't been processed yet
    // and we haven't build query and mutation types either
    let typ = ModuleType {
        name: type_name.to_string(),
        kind: ModuleTypeKind::Primitive,
        is_input: false,
    };

    building.types.add(&type_name, typ);
}

fn create_shallow_context(context: &ContextType, building: &mut SystemContextBuilding) {
    let type_name = &context.name;

    // Mark every type as Primitive, since other types that may be referred haven't been processed yet
    // and we haven't build query and mutation types either
    let typ = ModuleType {
        name: type_name.to_string(),
        kind: ModuleTypeKind::Primitive,
        is_input: false,
    };

    building.types.add(type_name, typ);
}

fn compute_access_method(
    resolved: &ResolvedAccess,
    resolved_env: &ResolvedTypeEnv,
) -> Result<Access, ModelBuildingError> {
    let access_expr =
        |expr: &AstExpr<Typed>| access_utils::compute_predicate_expression(expr, resolved_env);

    Ok(Access {
        value: access_expr(&resolved.value)?,
    })
}
