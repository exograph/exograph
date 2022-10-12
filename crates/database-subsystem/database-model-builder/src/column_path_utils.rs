use core_model::mapped_arena::MappedArena;
use database_model::{
    column_path::ColumnIdPathLink,
    relation::DatabaseRelation,
    types::{DatabaseCompositeType, DatabaseField, DatabaseType},
};

pub fn column_path_link(
    container_type: &DatabaseCompositeType,
    field: &DatabaseField,
    subsystem_types: &MappedArena<DatabaseType>,
) -> ColumnIdPathLink {
    let field_type_id = field.typ.type_id();
    let field_type = &subsystem_types[*field_type_id];

    match &field.relation {
        DatabaseRelation::Pk { column_id, .. } | DatabaseRelation::Scalar { column_id, .. } => {
            ColumnIdPathLink::new(*column_id, None)
        }
        DatabaseRelation::ManyToOne { column_id, .. } => {
            let dependent_column_id = field_type.pk_column_id();
            ColumnIdPathLink::new(*column_id, dependent_column_id)
        }
        DatabaseRelation::OneToMany {
            other_type_column_id,
            ..
        } => {
            let parent_column_id = container_type.pk_column_id().unwrap();
            ColumnIdPathLink::new(parent_column_id, Some(*other_type_column_id))
        }
    }
}
