use payas_core_model_builder::{
    ast::ast_types::AstExpr,
    builder::{
        access_builder::ResolvedAccess,
        access_utils,
        resolved_builder::{
            ResolvedCompositeType, ResolvedCompositeTypeKind, ResolvedField, ResolvedFieldType,
            ResolvedType,
        },
        type_builder::ResolvedTypeEnv,
    },
    error::ModelBuildingError,
    typechecker::Typed,
};
use payas_model::model::{
    access::Access, mapped_arena::MappedArena, relation::GqlRelation, GqlCompositeType,
    GqlCompositeTypeKind, GqlField, GqlFieldType, GqlType, GqlTypeKind,
};

use super::{resolved_builder::ResolvedMethod, system_builder::SystemContextBuilding};

pub(super) fn build_service_expanded(
    resolved_methods: &[&ResolvedMethod],
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    for (_, model_type) in resolved_env.resolved_subsystem_types.iter() {
        if let ResolvedType::Composite(c) = &model_type {
            expand_service_type_no_fields(c, resolved_env, building);
        }
    }

    for (_, model_type) in resolved_env.resolved_subsystem_types.iter() {
        if let ResolvedType::Composite(c) = &model_type {
            expand_service_type_fields(c, resolved_env, building);
        }
    }

    for (_, model_type) in resolved_env.resolved_subsystem_types.iter() {
        if let ResolvedType::Composite(c) = &model_type {
            expand_type_access(c, resolved_env, building)?;
        }
    }

    for method in resolved_methods.iter() {
        expand_method_access(method, resolved_env, building)?
    }

    Ok(())
}

pub(super) fn build_shallow(
    models: &MappedArena<ResolvedType>,
    building: &mut SystemContextBuilding,
) {
    for (_, model_type) in models.iter() {
        create_shallow_type(model_type, building);
    }
}

fn expand_service_type_no_fields(
    resolved_type: &ResolvedCompositeType,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) {
    if let ResolvedCompositeTypeKind::NonPersistent { is_input, .. } = &resolved_type.kind {
        let kind = GqlTypeKind::Composite(GqlCompositeType {
            fields: vec![],
            kind: GqlCompositeTypeKind::NonPersistent,
            access: Access::restrictive(),
        });

        let existing_type_id = building.get_id(&resolved_type.name, resolved_env);

        building.service_types.values[existing_type_id.unwrap()].kind = kind;
        building.service_types.values[existing_type_id.unwrap()].is_input = *is_input;
    }
}

fn expand_service_type_fields(
    resolved_type: &ResolvedCompositeType,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) {
    let existing_type_id = building.get_id(&resolved_type.name, resolved_env).unwrap();

    if matches!(
        &resolved_type.kind,
        ResolvedCompositeTypeKind::NonPersistent { .. }
    ) {
        let existing_type = &building.service_types[existing_type_id];

        if let GqlTypeKind::Composite(GqlCompositeType { kind, .. }) = &existing_type.kind {
            let model_fields: Vec<GqlField> = resolved_type
                .fields
                .iter()
                .map(|field| create_service_field(field, resolved_env, building))
                .collect();

            let kind = GqlTypeKind::Composite(GqlCompositeType {
                fields: model_fields,
                kind: kind.clone(),
                access: Access::restrictive(),
            });

            building.service_types.values[existing_type_id].kind = kind
        }
    }
}

fn expand_method_access(
    resolved_method: &ResolvedMethod,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    let existing_method_id = building.methods.get_id(&resolved_method.name).unwrap();
    let expr = compute_access_method(&resolved_method.access, resolved_env, building)?;
    building.methods.values[existing_method_id].access = expr;

    Ok(())
}

fn create_service_field(
    field: &ResolvedField,
    resolved_env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> GqlField {
    fn create_field_type(
        field_type: &ResolvedFieldType,
        resolved_env: &ResolvedTypeEnv,
        building: &SystemContextBuilding,
    ) -> GqlFieldType {
        match field_type {
            ResolvedFieldType::Plain {
                type_name,
                is_primitive,
            } => {
                let type_id = if *is_primitive {
                    resolved_env
                        .base_system
                        .primitive_types
                        .get_id(type_name)
                        .unwrap()
                } else {
                    match building.service_types.get_id(type_name) {
                        Some(type_id) => type_id,
                        None => building.service_types.get_id(type_name).unwrap(),
                    }
                };

                GqlFieldType::Reference {
                    type_name: type_name.clone(),
                    is_primitive: *is_primitive,
                    type_id,
                }
            }
            ResolvedFieldType::Optional(underlying) => GqlFieldType::Optional(Box::new(
                create_field_type(underlying, resolved_env, building),
            )),
            ResolvedFieldType::List(underlying) => GqlFieldType::List(Box::new(create_field_type(
                underlying,
                resolved_env,
                building,
            ))),
        }
    }

    GqlField {
        name: field.name.to_owned(),
        typ: create_field_type(&field.typ, resolved_env, building),
        relation: GqlRelation::NonPersistent,
        has_default_value: field.default_value.is_some(),
    }
}

fn compute_access_composite_types(
    resolved: &ResolvedAccess,
    self_type_info: &GqlCompositeType,
    resolved_env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> Result<Access, ModelBuildingError> {
    let access_expr = |expr: &AstExpr<Typed>| {
        access_utils::compute_predicate_expression(
            expr,
            Some(self_type_info),
            resolved_env,
            &building.service_types,
        )
    };

    Ok(Access {
        creation: access_expr(&resolved.creation)?,
        read: access_expr(&resolved.read)?,
        update: access_expr(&resolved.update)?,
        delete: access_expr(&resolved.delete)?,
    })
}

// Expand access expressions (pre-condition: all model fields have been populated)
fn expand_type_access(
    resolved_type: &ResolvedCompositeType,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    let existing_type_id = building.get_id(&resolved_type.name, resolved_env).unwrap();

    if let ResolvedCompositeTypeKind::NonPersistent { .. } = &resolved_type.kind {
        let existing_type = &building.service_types[existing_type_id];

        if let GqlTypeKind::Composite(self_type_info) = &existing_type.kind {
            let expr = compute_access_composite_types(
                &resolved_type.access,
                self_type_info,
                resolved_env,
                building,
            )?;

            let kind = GqlTypeKind::Composite(GqlCompositeType {
                fields: self_type_info.fields.clone(),
                kind: self_type_info.kind.clone(),
                access: expr,
            });

            building.service_types.values[existing_type_id].kind = kind
        }
    };

    Ok(())
}

fn create_shallow_type(resolved_type: &ResolvedType, building: &mut SystemContextBuilding) {
    let type_name = resolved_type.name();

    // Mark every type as Primitive, since other types that may be referred haven't been processed yet
    // and we haven't build query and mutation types either
    let typ = GqlType {
        name: type_name.to_string(),
        plural_name: resolved_type.plural_name(),
        kind: GqlTypeKind::Primitive,
        is_input: false,
    };

    if let ResolvedType::Composite(composite_type) = resolved_type {
        if let ResolvedCompositeTypeKind::NonPersistent { .. } = composite_type.kind {
            building.service_types.add(&type_name, typ);
        }
    };
}

fn compute_access_method(
    resolved: &ResolvedAccess,
    resolved_env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> Result<Access, ModelBuildingError> {
    let access_expr = |expr: &AstExpr<Typed>| {
        access_utils::compute_predicate_expression(
            expr,
            None,
            resolved_env,
            &building.service_types,
        )
    };

    Ok(Access {
        creation: access_expr(&resolved.creation)?,
        read: access_expr(&resolved.read)?,
        update: access_expr(&resolved.update)?,
        delete: access_expr(&resolved.delete)?,
    })
}
