use std::collections::HashSet;

use core_model_builder::error::ModelBuildingError;
use postgres_model::{
    operation::{CreateDataParameter, PostgresMutationKind},
    types::{PostgresCompositeType, PostgresTypeKind},
};

use crate::{predicate_builder, system_builder::SystemContextBuilding};

pub(super) fn prune_unused_primitives_from_introspection(
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    let mut used_primitives = HashSet::new();
    let mut insert_if_primitive = |type_id| {
        if building.postgres_types[type_id].is_primitive() {
            used_primitives.insert(type_id);
        }
    };

    // iterate through the fields of all postgres types and add all primitives used
    for (_, typ) in building.postgres_types.iter() {
        match &typ.kind {
            PostgresTypeKind::Primitive => {}
            PostgresTypeKind::Composite(PostgresCompositeType { fields, .. }) => {
                for field in fields {
                    insert_if_primitive(*field.typ.type_id());
                }
            }
        }
    }

    // iterate through all queries and add primitives used
    for (_, query) in building.queries.iter() {
        insert_if_primitive(query.return_type.type_id);

        if let Some(limit_typ) = query.parameter.limit_param.as_ref() {
            insert_if_primitive(limit_typ.typ.type_id);
        }

        if let Some(offset_typ) = query.parameter.offset_param.as_ref() {
            insert_if_primitive(offset_typ.typ.type_id);
        }

        if let Some(predicate_param) = query.parameter.predicate_param.as_ref() {
            insert_if_primitive(predicate_param.underlying_type_id);
        }
    }

    // iterate through all mutations and add primitives used
    for (_, mutation) in building.mutations.iter() {
        insert_if_primitive(mutation.return_type.type_id);

        match &mutation.kind {
            PostgresMutationKind::Create(CreateDataParameter { typ, .. }) => {
                insert_if_primitive(typ.type_id);
            }
            PostgresMutationKind::Delete(predicate_param) => {
                insert_if_primitive(predicate_param.underlying_type_id);
            }
            PostgresMutationKind::Update {
                predicate_param, ..
            } => {
                insert_if_primitive(predicate_param.underlying_type_id);
            }
        }
    }

    for (type_id, typ) in building.postgres_types.iter_mut() {
        if typ.is_primitive() && !used_primitives.contains(&type_id) {
            // disable introspection for non-used primitives
            typ.exposed = false;

            // disable introspection for its predicate type
            building
                .predicate_types
                .get_by_key_mut(&predicate_builder::get_parameter_type_name(
                    typ.name.as_str(),
                ))
                .unwrap()
                .exposed = false;
        }
    }

    Ok(())
}
