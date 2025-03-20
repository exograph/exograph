use common::router::{PlainRequestPayload, Router};
use core_model::access::CommonAccessPrimitiveExpression;
use exo_sql::{ColumnPath, ColumnPathLink, PhysicalColumnPath};
use postgres_core_model::{
    access::{AccessPrimitiveExpressionPath, FieldPath, PrecheckAccessPrimitiveExpression},
    relation::PostgresRelation,
    types::{EntityType, PostgresField},
};
use postgres_graphql_model::subsystem::PostgresGraphQLSubsystem;

use crate::access::test_util::{context_selection, TestRouter};

#[allow(dead_code)]
pub(super) struct TestSystem {
    pub system: PostgresGraphQLSubsystem,
}

impl TestSystem {
    pub async fn new() -> TestSystem {
        let postgres_subsystem = crate::test_utils::create_postgres_system_from_str(
            r#"
                context AccessContext {
                    @test("role") role: String
                    @test("name") name: String
                    @test("id") id: Int
                }
    
                @postgres
                module ArticleModule {
                    type Article {
                        @pk id: Int = autoIncrement()
                        title: String
                        publications: Set<Publication>?
                    }

                    type Publication {
                        @pk author: User
                        @pk article: Article
                        royalty: Int
                    }
    
                    type User {
                        @pk id: Int = autoIncrement()
                        name: String
                        skill: String
                        age: Int
                        publications: Set<Publication>?
                        todos: Set<Todo>?
                    }

                    type Todo {
                        @pk id: Int = autoIncrement()
                        title: String
                        completed: Boolean
                        user: User = AccessContext.id
                    }
                }
            "#,
            "index.exo".to_string(),
        )
        .await
        .expect("Failed to create postgres subsystem");

        TestSystem {
            system: postgres_subsystem,
        }
    }

    pub fn physical_column_path(&self, entity_name: &str, path: &str) -> PhysicalColumnPath {
        let path_components = path.split('.').collect::<Vec<_>>();

        let (path, _) = path_components.into_iter().fold(
            (None::<PhysicalColumnPath>, entity_name),
            |(acc, entity_name), path_component| {
                let field = self.get_field(entity_name, path_component);
                let link = self.get_link(field);

                let new_entity_name = field.typ.innermost().type_name.as_str();

                let path = match acc {
                    None => PhysicalColumnPath::init(link),
                    Some(acc) => acc.push(link),
                };

                (Some(path), new_entity_name)
            },
        );

        path.expect("Failed to create physical column path")
    }

    pub fn column_path(&self, entity_name: &str, path: &str) -> ColumnPath {
        ColumnPath::Physical(self.physical_column_path(entity_name, path))
    }

    pub fn expr(
        &self,
        entity_name: &str,
        field_path: &str,
        function_param: Option<&str>,
    ) -> PrecheckAccessPrimitiveExpression {
        PrecheckAccessPrimitiveExpression::Path(
            self.path(entity_name, field_path),
            function_param.map(|s| s.to_string()),
        )
    }

    pub fn path(&self, entity_name: &str, path: &str) -> AccessPrimitiveExpressionPath {
        let path_components = path.split('.').collect::<Vec<_>>();

        let (_, acc_path, _, _) = path_components.into_iter().fold(
            (
                entity_name,
                None::<AccessPrimitiveExpressionPath>,
                false,
                None,
            ),
            |(acc_entity_name, acc_path, in_many_to_one, earlier_default_value), path_component| {
                let field = self.get_field(acc_entity_name, path_component);
                let link = self.get_link(field);
                let is_many_to_one = matches!(field.relation, PostgresRelation::ManyToOne { .. });

                let entity_type = self.get_entity(acc_entity_name);

                let new_entity_name = field.typ.innermost().type_name.as_str();

                let new_path = match acc_path {
                    Some(acc_path) => {
                        let field_path = match (
                            acc_path.field_path,
                            !in_many_to_one || field.relation.is_pk(),
                        ) {
                            (FieldPath::Normal(a, _), true) => {
                                let mut field_path = a.clone();
                                field_path.push(field.name.clone());

                                if field.relation.is_pk() {
                                    FieldPath::Normal(field_path, earlier_default_value)
                                } else {
                                    FieldPath::Normal(field_path, None)
                                }
                            }
                            (FieldPath::Normal(a, _), false) => FieldPath::Pk {
                                lead: a.clone(),
                                lead_default: None,
                                pk_fields: entity_type
                                    .pk_fields()
                                    .iter()
                                    .map(|f| f.name.clone())
                                    .collect(),
                            },
                            (field_path, _) => {
                                // If the field path is already a pk, we leave it as is (will lead to a database residue)
                                field_path
                            }
                        };

                        AccessPrimitiveExpressionPath {
                            column_path: acc_path.column_path.push(link),
                            field_path,
                        }
                    }
                    None => AccessPrimitiveExpressionPath::new(
                        PhysicalColumnPath::init(link),
                        FieldPath::Normal(vec![field.name.clone()], None),
                    ),
                };

                (
                    new_entity_name,
                    Some(new_path),
                    is_many_to_one,
                    field.default_value.clone(),
                )
            },
        );

        acc_path.expect("Failed to create access primitive expression path")
    }

    fn get_entity(&self, name: &str) -> &EntityType {
        self.system
            .core_subsystem
            .entity_types
            .iter()
            .find_map(|(_, entity_type)| {
                if entity_type.name == name {
                    Some(entity_type)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| panic!("Entity '{}' not found", name))
    }

    fn get_field(&self, entity_name: &str, field_name: &str) -> &PostgresField<EntityType> {
        self.get_entity(entity_name)
            .field_by_name(field_name)
            .unwrap_or_else(|| {
                panic!(
                    "Field '{}' not found in entity '{}'",
                    field_name, entity_name
                )
            })
    }

    fn get_link(&self, field: &PostgresField<EntityType>) -> ColumnPathLink {
        let database = &self.system.core_subsystem.database;

        match &field.relation {
            PostgresRelation::Scalar { column_id, .. } => ColumnPathLink::Leaf(*column_id),
            PostgresRelation::ManyToOne { relation, .. } => {
                let link = relation.column_path_link(database);
                match link {
                    ColumnPathLink::Relation(relation_link) => {
                        ColumnPathLink::Relation(relation_link.with_alias(field.name.clone()))
                    }
                    ColumnPathLink::Leaf(_) => panic!("Invalid column path link"),
                }
            }

            PostgresRelation::OneToMany(relation) => {
                let link = relation.column_path_link(database);
                match link {
                    ColumnPathLink::Relation(relation_link) => {
                        ColumnPathLink::Relation(relation_link.with_alias(field.name.clone()))
                    }
                    ColumnPathLink::Leaf(_) => panic!("Invalid column path link"),
                }
            }
            PostgresRelation::Embedded => panic!("Cannot append field to embedded relation"),
        }
    }
}

pub fn context_selection_expr(head: &str, tail: &str) -> Box<PrecheckAccessPrimitiveExpression> {
    Box::new(PrecheckAccessPrimitiveExpression::Common(
        CommonAccessPrimitiveExpression::ContextSelection(context_selection(head, tail)),
    ))
}

pub fn router() -> Box<dyn for<'request> Router<PlainRequestPayload<'request>> + Send + Sync> {
    // Create an empty Router. Since in tests we never invoke it (since we don't have @query context),
    // we don't need to populate it.
    Box::new(TestRouter {})
}
