use payas_model::model::{
    access::Access, mapped_arena::MappedArena, relation::GqlRelation, ContextField, ContextSource,
    ContextType, GqlCompositeType, GqlCompositeTypeKind, GqlField, GqlFieldType, GqlType,
    GqlTypeKind,
};

use super::{
    resolved_builder::{ResolvedContext, ResolvedContextSource, ResolvedFieldType},
    system_builder::SystemContextBuilding,
};

// Note: The current implementation considers only simple JWT payload
// TODO: Make this a more general context
pub fn build_shallow(
    contexts: &MappedArena<ResolvedContext>,
    building: &mut SystemContextBuilding,
) {
    for (_, context) in contexts.iter() {
        create_shallow(context, building);
    }
}

fn create_shallow(context: &ResolvedContext, building: &mut SystemContextBuilding) {
    building.contexts.add(
        &context.name,
        ContextType {
            name: context.name.clone(),
            fields: vec![],
        },
    );

    building.types.add(
        &context.name,
        GqlType {
            name: context.name.clone(),
            plural_name: context.name.clone(), // TODO
            kind: GqlTypeKind::Primitive,
            is_input: false,
        },
    );
}

pub fn build_expanded(
    contexts: &MappedArena<ResolvedContext>,
    building: &mut SystemContextBuilding,
) {
    for (_, context) in contexts.iter() {
        expand(context, building);
    }
}

fn expand(context: &ResolvedContext, building: &mut SystemContextBuilding) {
    let existing_context_id = building.contexts.get_id(&context.name).unwrap();
    let existing_type_id = building.types.get_id(&context.name).unwrap();
    let existing_context = &building.contexts[existing_context_id];

    let context_fields = context
        .fields
        .iter()
        .map(|field| ContextField {
            name: field.name.clone(),
            typ: create_context_field_type(&field.typ, building),
            source: {
                let ResolvedContextSource { annotation, value } = field.source.clone();
                ContextSource {
                    annotation_name: annotation,
                    value,
                }
            },
        })
        .collect::<Vec<_>>();

    let expanded_context = ContextType {
        name: existing_context.name.clone(),
        fields: context_fields.clone(),
    };
    building.contexts[existing_context_id] = expanded_context;

    let expanded_type_kind = GqlTypeKind::Composite(GqlCompositeType {
        fields: context_fields
            .iter()
            .map(|field| GqlField {
                name: field.name.clone(),
                typ: field.typ.clone(),
                relation: GqlRelation::NonPersistent,
                has_default_value: false,
            })
            .collect(),
        kind: GqlCompositeTypeKind::NonPersistent,
        access: Access::restrictive(),
    });
    building.types[existing_type_id].kind = expanded_type_kind;
}

fn create_context_field_type(
    field_type: &ResolvedFieldType,
    building: &SystemContextBuilding,
) -> GqlFieldType {
    match field_type {
        ResolvedFieldType::Plain(type_name) => GqlFieldType::Reference {
            type_id: building.types.get_id(type_name).unwrap(),
            type_name: type_name.clone(),
        },
        ResolvedFieldType::Optional(underlying) => {
            GqlFieldType::Optional(Box::new(create_context_field_type(underlying, building)))
        }
        ResolvedFieldType::List(underlying) => {
            GqlFieldType::List(Box::new(create_context_field_type(underlying, building)))
        }
    }
}
