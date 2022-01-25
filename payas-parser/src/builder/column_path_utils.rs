use payas_model::model::{
    predicate::ColumnPathLink, relation::GqlRelation, GqlCompositeType, GqlField,
};

use super::system_builder::SystemContextBuilding;

pub fn column_path_link(
    container_type: &GqlCompositeType,
    field: &GqlField,
    building: &SystemContextBuilding,
) -> ColumnPathLink {
    let field_type_id = field.typ.type_id();
    let field_type = &building.types[*field_type_id];

    match &field.relation {
        GqlRelation::Pk { column_id, .. } | GqlRelation::Scalar { column_id, .. } => {
            ColumnPathLink::new(column_id.clone(), None)
        }
        GqlRelation::ManyToOne { column_id, .. } => {
            let dependent_column_id = field_type.pk_column_id();
            ColumnPathLink::new(column_id.clone(), dependent_column_id)
        }
        GqlRelation::OneToMany {
            other_type_column_id,
            ..
        } => {
            let parent_column_id = container_type.pk_column_id().unwrap();
            ColumnPathLink::new(parent_column_id, Some(other_type_column_id.clone()))
        }
        GqlRelation::NonPersistent => panic!("NonPersistent is not supported"),
    }
}
