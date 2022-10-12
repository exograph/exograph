use payas_core_model::{
    context_type::{ContextField, ContextFieldType, ContextSource, ContextType},
    mapped_arena::MappedArena,
};

use super::{
    resolved_builder::{ResolvedContext, ResolvedContextFieldType, ResolvedContextSource},
    system_builder::SystemContextBuilding,
};

pub fn build(contexts: &MappedArena<ResolvedContext>, building: &mut SystemContextBuilding) {
    // TODO: Check if we can combine this shallow-expanded building
    build_shallow(contexts, building);
    build_expanded(contexts, building);
}

// Note: The current implementation considers only simple JWT payload
// TODO: Make this a more general context
fn build_shallow(contexts: &MappedArena<ResolvedContext>, building: &mut SystemContextBuilding) {
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
    let existing_context_id = building.contexts.get_id(&context.name).unwrap();
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
        fields: context_fields,
    };
    building.contexts[existing_context_id] = expanded_context;
}

fn create_context_field_type(
    field_type: &ResolvedContextFieldType,
    building: &SystemContextBuilding,
) -> ContextFieldType {
    match field_type {
        ResolvedContextFieldType::Plain(pt) => ContextFieldType::Reference(pt.clone()),
        ResolvedContextFieldType::Optional(underlying) => {
            ContextFieldType::Optional(Box::new(create_context_field_type(underlying, building)))
        }
        ResolvedContextFieldType::List(underlying) => {
            ContextFieldType::List(Box::new(create_context_field_type(underlying, building)))
        }
    }
}
