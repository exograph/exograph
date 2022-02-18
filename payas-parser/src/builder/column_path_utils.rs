use payas_model::model::{
    predicate::ColumnIdPathLink, relation::GqlRelation, GqlCompositeType, GqlField,
};

use super::system_builder::SystemContextBuilding;

pub fn column_path_link(
    container_type: &GqlCompositeType,
    field: &GqlField,
    building: &SystemContextBuilding,
) -> ColumnIdPathLink {
    let field_type_id = field.typ.type_id();
    let field_type = &building.types[*field_type_id];

    match &field.relation {
        GqlRelation::Pk { column_id, .. } | GqlRelation::Scalar { column_id, .. } => {
            ColumnIdPathLink::new(*column_id, None)
        }
        GqlRelation::ManyToOne { column_id, .. } => {
            let dependent_column_id = field_type.pk_column_id();
            ColumnIdPathLink::new(*column_id, dependent_column_id)
        }
        GqlRelation::OneToMany {
            other_type_column_id,
            ..
        } => {
            let parent_column_id = container_type.pk_column_id().unwrap();
            ColumnIdPathLink::new(parent_column_id, Some(*other_type_column_id))
        }
        GqlRelation::NonPersistent => panic!("NonPersistent is not supported"),
    }
}
