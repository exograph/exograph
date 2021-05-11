//#[cfg(test)] // needed until we use this code from `main`
pub mod common_test_data {
    use crate::model::system::ModelSystem;
    use crate::model::{
        ast::ast_types::AstRelation::*, ast::ast_types::AstTypeKind::*,
        ast::ast_types::AstTypeModifier::*, ast::ast_types::*,
    };

    pub fn test_system() -> ModelSystem {
        ModelSystem::build(vec![create_venue_model_type(), create_concert_model_type()])
    }

    fn create_concert_model_type() -> AstType {
        AstType {
            name: "Concert".to_string(),
            kind: Composite {
                fields: vec![
                    AstField {
                        name: "id".to_string(),
                        type_name: "Int".to_string(),
                        type_modifier: AstTypeModifier::NonNull,
                        relation: Pk { column_name: None },
                    },
                    AstField {
                        name: "title".to_string(),
                        type_name: "String".to_string(),
                        type_modifier: AstTypeModifier::NonNull,
                        relation: Scalar { column_name: None },
                    },
                    AstField {
                        name: "venue".to_string(),
                        type_name: "Venue".to_string(),
                        type_modifier: AstTypeModifier::NonNull,
                        relation: ManyToOne {
                            column_name: Some("venueid".to_string()),
                            other_type_name: "Venue".to_string(),
                            optional: true,
                        },
                    },
                ],
                table_name: Some("concerts".to_string()),
            },
        }
    }

    fn create_venue_model_type() -> AstType {
        AstType {
            name: "Venue".to_string(),
            kind: Composite {
                fields: vec![
                    AstField {
                        name: "id".to_string(),
                        type_name: "Int".to_string(),
                        type_modifier: NonNull,
                        relation: Pk { column_name: None },
                    },
                    AstField {
                        name: "name".to_string(),
                        type_name: "String".to_string(),
                        type_modifier: NonNull,
                        relation: Scalar { column_name: None },
                    },
                    AstField {
                        name: "concerts".to_string(),
                        type_name: "Concert".to_string(),
                        type_modifier: List,
                        relation: OneToMany {
                            other_type_column_name: Some("venueid".to_string()),
                            other_type_name: "Concert".to_string(),
                        },
                    },
                ],
                table_name: Some("venues".to_string()),
            },
        }
    }
}
