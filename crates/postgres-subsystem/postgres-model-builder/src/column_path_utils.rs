use core_plugin_interface::core_model::mapped_arena::MappedArena;
use postgres_model::{
    column_path::ColumnIdPathLink,
    relation::PostgresRelation,
    types::{EntityType, PostgresField},
};

pub fn column_path_link(
    container_type: &EntityType,
    field: &PostgresField<EntityType>,
    entity_types: &MappedArena<EntityType>,
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
            let other_type = &entity_types[*other_type_id];
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
