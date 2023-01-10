use core_plugin_interface::core_model::mapped_arena::MappedArena;
use postgres_model::{
    column_path::ColumnIdPathLink,
    relation::PostgresRelation,
    types::{PostgresCompositeType, PostgresField},
};

pub fn column_path_link(
    container_type: &PostgresCompositeType,
    field: &PostgresField,
    subsystem_composite_types: &MappedArena<PostgresCompositeType>,
) -> ColumnIdPathLink {
    match &field.relation {
        PostgresRelation::Pk { column_id, .. } | PostgresRelation::Scalar { column_id, .. } => {
            ColumnIdPathLink::new(*column_id, None)
        }
        PostgresRelation::ManyToOne {
            column_id,
            other_type_id,
            ..
        } => {
            let other_type = &subsystem_composite_types[*other_type_id];
            ColumnIdPathLink::new(*column_id, other_type.pk_column_id())
        }
        PostgresRelation::OneToMany {
            other_type_column_id,
            ..
        } => {
            let parent_column_id = container_type.pk_column_id().unwrap();
            ColumnIdPathLink::new(parent_column_id, Some(*other_type_column_id))
        }
    }
}
