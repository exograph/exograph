use payas_model::model::{
    mapped_arena::MappedArena, ContextField, ContextSource, ContextType, GqlFieldType,
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
    let existing_type_id = building.contexts.get_id(&context.name).unwrap();
    let existing_context = &building.contexts[existing_type_id];

    let fields = context
        .fields
        .iter()
        .map(|field| ContextField {
            name: field.name.clone(),
            typ: create_context_field_type(&field.typ, building),
            source: match &field.source {
                ResolvedContextSource::Jwt { claim } => ContextSource::Jwt {
                    claim: claim.clone(),
                },
                ResolvedContextSource::Header { header } => ContextSource::Header {
                    header: header.clone(),
                },
                ResolvedContextSource::EnvironmentVariable { envvar } => {
                    ContextSource::EnvironmentVariable {
                        envvar: envvar.clone(),
                    }
                }
            },
        })
        .collect();

    let expanded_context = ContextType {
        name: existing_context.name.clone(),
        fields,
    };
    building.contexts[existing_type_id] = expanded_context;
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
