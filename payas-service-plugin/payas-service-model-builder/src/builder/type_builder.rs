use payas_core_model::{context_type::ContextType, mapped_arena::MappedArena};
use payas_core_model_builder::{
    ast::ast_types::AstExpr, error::ModelBuildingError, typechecker::Typed,
};
use payas_service_model::{
    access::Access,
    types::{ServiceCompositeType, ServiceField, ServiceFieldType, ServiceType, ServiceTypeKind},
};

use crate::builder::resolved_builder::ResolvedFieldType;

use super::{
    access_builder::ResolvedAccess,
    access_utils,
    resolved_builder::{
        ResolvedCompositeType, ResolvedField, ResolvedMethod, ResolvedService, ResolvedType,
    },
    system_builder::SystemContextBuilding,
};

#[derive(Debug, Clone)]
pub struct ResolvedTypeEnv<'a> {
    pub contexts: &'a MappedArena<ContextType>,
    pub resolved_types: MappedArena<ResolvedType>,
    pub resolved_services: MappedArena<ResolvedService>,
}

impl<'a> ResolvedTypeEnv<'a> {
    pub fn get_by_key(&self, key: &str) -> Option<&ResolvedType> {
        self.resolved_types.get_by_key(key)
    }
}

pub(super) fn build_service_expanded(
    resolved_methods: &[&ResolvedMethod],
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            expand_service_type_no_fields(c, building);
        }
    }

    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            expand_service_type_fields(c, resolved_env, building);
        }
    }

    for (_, model_type) in resolved_env.resolved_types.iter() {
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
    building: &mut SystemContextBuilding,
) {
    let existing_type_id = building.get_id(&resolved_type.name);

    building.types.values[existing_type_id.unwrap()].kind =
        ServiceTypeKind::Composite(ServiceCompositeType {
            fields: vec![],
            access: Access::restrictive(),
            is_input: false,
        });
    building.types.values[existing_type_id.unwrap()].is_input = resolved_type.is_input;
}

fn expand_service_type_fields(
    resolved_type: &ResolvedCompositeType,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) {
    let existing_type_id = building.get_id(&resolved_type.name).unwrap();

    let model_fields: Vec<ServiceField> = resolved_type
        .fields
        .iter()
        .map(|field| create_service_field(field, resolved_env, building))
        .collect();

    let kind = ServiceTypeKind::Composite(ServiceCompositeType {
        fields: model_fields,
        is_input: resolved_type.is_input,
        access: Access::restrictive(),
    });

    building.types.values[existing_type_id].kind = kind
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
) -> ServiceField {
    fn create_field_type(
        field_type: &ResolvedFieldType,
        resolved_env: &ResolvedTypeEnv,
        building: &SystemContextBuilding,
    ) -> ServiceFieldType {
        match field_type {
            ResolvedFieldType::Plain {
                type_name,
                is_primitive,
            } => {
                let type_id = building.types.get_id(type_name).unwrap();

                ServiceFieldType::Reference {
                    type_name: type_name.clone(),
                    is_primitive: *is_primitive,
                    type_id,
                }
            }
            ResolvedFieldType::Optional(underlying) => ServiceFieldType::Optional(Box::new(
                create_field_type(underlying, resolved_env, building),
            )),
            ResolvedFieldType::List(underlying) => ServiceFieldType::List(Box::new(
                create_field_type(underlying, resolved_env, building),
            )),
        }
    }

    ServiceField {
        name: field.name.to_owned(),
        typ: create_field_type(&field.typ, resolved_env, building),
        has_default_value: field.default_value.is_some(),
    }
}

fn compute_access_composite_types(
    resolved: &ResolvedAccess,
    self_type_info: &ServiceCompositeType,
    resolved_env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> Result<Access, ModelBuildingError> {
    let access_expr = |expr: &AstExpr<Typed>| {
        access_utils::compute_predicate_expression(
            expr,
            Some(self_type_info),
            resolved_env,
            &building.types,
        )
    };

    Ok(Access {
        value: access_expr(&resolved.value)?,
    })
}

// Expand access expressions (pre-condition: all model fields have been populated)
fn expand_type_access(
    resolved_type: &ResolvedCompositeType,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    let existing_type_id = building.get_id(&resolved_type.name).unwrap();

    let existing_type = &building.types[existing_type_id];

    if let ServiceTypeKind::Composite(self_type_info) = &existing_type.kind {
        let expr = compute_access_composite_types(
            &resolved_type.access,
            &self_type_info,
            resolved_env,
            building,
        )?;

        let kind = ServiceTypeKind::Composite(ServiceCompositeType {
            fields: self_type_info.fields.clone(),
            is_input: self_type_info.is_input,
            access: expr,
        });

        building.types.values[existing_type_id].kind = kind
    };

    Ok(())
}

fn create_shallow_type(resolved_type: &ResolvedType, building: &mut SystemContextBuilding) {
    let type_name = resolved_type.name();

    // Mark every type as Primitive, since other types that may be referred haven't been processed yet
    // and we haven't build query and mutation types either
    let typ = ServiceType {
        name: type_name.to_string(),
        kind: ServiceTypeKind::Primitive,
        is_input: false,
    };

    building.types.add(&type_name, typ);
}

fn compute_access_method(
    resolved: &ResolvedAccess,
    resolved_env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> Result<Access, ModelBuildingError> {
    let access_expr = |expr: &AstExpr<Typed>| {
        access_utils::compute_predicate_expression(expr, None, resolved_env, &building.types)
    };

    Ok(Access {
        value: access_expr(&resolved.value)?,
    })
}
