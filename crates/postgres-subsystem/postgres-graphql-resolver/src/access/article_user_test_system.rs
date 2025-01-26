use common::router::{PlainRequestPayload, Router};
use exo_sql::{
    ColumnId, ColumnPathLink, PhysicalColumnPath, PhysicalTableName, RelationColumnPair, TableId,
};
use postgres_core_model::access::{
    AccessPrimitiveExpressionPath, FieldPath, PrecheckAccessPrimitiveExpression,
};
use postgres_graphql_model::subsystem::PostgresGraphQLSubsystem;

use super::test_util::TestRouter;

#[allow(dead_code)]
pub(super) struct TestSystem {
    pub system: PostgresGraphQLSubsystem,

    pub article_table_id: TableId,
    pub article_pk_column_id: ColumnId,
    pub article_title_column_id: ColumnId,

    pub user_table_id: TableId,
    pub user_id_column_id: ColumnId,
    pub user_name_column_id: ColumnId,
    pub user_skill_column_id: ColumnId,
    pub user_age_column_id: ColumnId,

    pub publication_table_id: TableId,
    pub publication_author_column_id: ColumnId,
    pub publication_article_column_id: ColumnId,
    pub publication_royalty_column_id: ColumnId,

    pub test_system_router:
        Box<dyn for<'request> Router<PlainRequestPayload<'request>> + Send + Sync>,
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
                    }
                }
            "#,
            "index.exo".to_string(),
        )
        .await
        .unwrap();

        let database = &postgres_subsystem.core_subsystem.database;

        let article_table_id = database
            .get_table_id(&PhysicalTableName::new("articles", None))
            .unwrap();
        let article_pk_column_id = database.get_column_id(article_table_id, "id").unwrap();
        let article_title_column_id = database.get_column_id(article_table_id, "title").unwrap();

        let user_table_id = database
            .get_table_id(&PhysicalTableName::new("users", None))
            .unwrap();

        let user_id_column_id = database.get_column_id(user_table_id, "id").unwrap();
        let user_name_column_id = database.get_column_id(user_table_id, "name").unwrap();
        let user_age_column_id = database.get_column_id(user_table_id, "age").unwrap();
        let user_skill_column_id = database.get_column_id(user_table_id, "skill").unwrap();

        let publication_table_id = database
            .get_table_id(&PhysicalTableName::new("publications", None))
            .unwrap();
        let publication_pk_column_author_id = database
            .get_column_id(publication_table_id, "author_id")
            .unwrap();
        let publication_pk_column_article_id = database
            .get_column_id(publication_table_id, "article_id")
            .unwrap();
        let publication_royalty_column_id = database
            .get_column_id(publication_table_id, "royalty")
            .unwrap();

        // Create an empty Router. Since in tests we never invoke it (since we don't have @query context),
        // we don't need to populate it.
        let test_system_router = Box::new(TestRouter {});

        TestSystem {
            system: postgres_subsystem,
            article_table_id,
            article_pk_column_id,
            article_title_column_id,
            user_table_id,
            user_id_column_id,
            user_name_column_id,
            user_skill_column_id,
            user_age_column_id,
            publication_table_id,
            publication_author_column_id: publication_pk_column_author_id,
            publication_article_column_id: publication_pk_column_article_id,
            publication_royalty_column_id,
            test_system_router,
        }
    }

    #[allow(dead_code)]
    pub fn article_title_column_path(&self) -> PhysicalColumnPath {
        PhysicalColumnPath::leaf(self.article_title_column_id)
    }

    #[allow(dead_code)]
    // self.title for `Article`
    pub fn article_title_expr(
        &self,
        parameter_name: Option<String>,
    ) -> PrecheckAccessPrimitiveExpression {
        PrecheckAccessPrimitiveExpression::Path(
            AccessPrimitiveExpressionPath {
                column_path: self.article_title_column_path(),
                field_path: FieldPath::Normal(vec!["title".to_string()]),
            },
            parameter_name,
        )
    }

    pub fn user_id_column_path(&self) -> PhysicalColumnPath {
        PhysicalColumnPath::leaf(self.user_id_column_id)
    }

    pub fn user_name_column_path(&self) -> PhysicalColumnPath {
        PhysicalColumnPath::leaf(self.user_name_column_id)
    }

    pub fn user_age_column_path(&self) -> PhysicalColumnPath {
        PhysicalColumnPath::leaf(self.user_age_column_id)
    }

    #[allow(dead_code)]
    pub fn user_id_expr(&self) -> PrecheckAccessPrimitiveExpression {
        PrecheckAccessPrimitiveExpression::Path(
            AccessPrimitiveExpressionPath {
                column_path: self.user_id_column_path(),
                field_path: FieldPath::Normal(vec!["id".to_string()]),
            },
            None,
        )
    }

    pub fn user_self_age_expr(&self) -> PrecheckAccessPrimitiveExpression {
        PrecheckAccessPrimitiveExpression::Path(
            AccessPrimitiveExpressionPath {
                column_path: self.user_age_column_path(),
                field_path: FieldPath::Normal(vec!["age".to_string()]),
            },
            None,
        )
    }

    pub fn user_self_name_expr(&self) -> PrecheckAccessPrimitiveExpression {
        PrecheckAccessPrimitiveExpression::Path(
            AccessPrimitiveExpressionPath {
                column_path: self.user_name_column_path(),
                field_path: FieldPath::Normal(vec!["name".to_string()]),
            },
            None,
        )
    }

    pub fn user_publications_link(&self) -> ColumnPathLink {
        ColumnPathLink::relation(
            vec![RelationColumnPair {
                self_column_id: self.user_id_column_id,
                foreign_column_id: self.publication_author_column_id,
            }],
            Some("publications".to_string()),
        )
    }

    pub fn user_publications_path(&self) -> AccessPrimitiveExpressionPath {
        AccessPrimitiveExpressionPath {
            column_path: PhysicalColumnPath::init(self.user_publications_link()),
            field_path: FieldPath::Normal(vec!["publications".to_string()]),
        }
    }

    pub fn publication_royalty_expr(
        &self,
        parameter_name: Option<String>,
    ) -> PrecheckAccessPrimitiveExpression {
        PrecheckAccessPrimitiveExpression::Path(
            AccessPrimitiveExpressionPath {
                column_path: self.publication_royalty_column_path(),
                field_path: FieldPath::Normal(vec!["royalty".to_string()]),
            },
            parameter_name,
        )
    }

    pub fn publication_royalty_column_path(&self) -> PhysicalColumnPath {
        PhysicalColumnPath::leaf(self.publication_royalty_column_id)
    }

    pub fn publication_author_age_physical_column_path(&self) -> PhysicalColumnPath {
        let path = PhysicalColumnPath::init(self.publication_author_link());
        path.push(ColumnPathLink::Leaf(self.user_age_column_id))
    }

    pub fn publication_author_age_expr(&self) -> PrecheckAccessPrimitiveExpression {
        PrecheckAccessPrimitiveExpression::Path(
            AccessPrimitiveExpressionPath {
                column_path: self.publication_author_age_physical_column_path(),
                field_path: FieldPath::Pk {
                    lead: vec!["author".to_string()],
                    pk_fields: vec!["id".to_string()],
                },
            },
            None,
        )
    }

    pub fn publication_author_link(&self) -> ColumnPathLink {
        ColumnPathLink::relation(
            vec![RelationColumnPair {
                self_column_id: self.publication_author_column_id,
                foreign_column_id: self.user_id_column_id,
            }],
            Some("author".to_string()),
        )
    }

    pub fn publication_author_id_column_path(&self) -> PhysicalColumnPath {
        let path = PhysicalColumnPath::init(self.publication_author_link());
        path.push(ColumnPathLink::Leaf(self.user_id_column_id))
    }

    // self.author.id for `Publication`
    pub fn publication_author_id_expr(&self) -> PrecheckAccessPrimitiveExpression {
        PrecheckAccessPrimitiveExpression::Path(
            AccessPrimitiveExpressionPath {
                column_path: self.publication_author_id_column_path(),
                field_path: FieldPath::Normal(vec!["author".to_string(), "id".to_string()]),
            },
            None,
        )
    }

    pub fn publication_author_name_physical_column_path(&self) -> PhysicalColumnPath {
        let path = PhysicalColumnPath::init(self.publication_author_link());
        path.push(ColumnPathLink::Leaf(self.user_name_column_id))
    }

    pub fn publication_author_skill_physical_column_path(&self) -> PhysicalColumnPath {
        let path = PhysicalColumnPath::init(self.publication_author_link());
        path.push(ColumnPathLink::Leaf(self.user_skill_column_id))
    }

    pub fn publication_author_name_expr(&self) -> PrecheckAccessPrimitiveExpression {
        PrecheckAccessPrimitiveExpression::Path(
            AccessPrimitiveExpressionPath {
                column_path: self.publication_author_name_physical_column_path(),
                field_path: FieldPath::Pk {
                    lead: vec!["author".to_string()],
                    pk_fields: vec!["id".to_string()],
                },
            },
            None,
        )
    }

    pub fn publication_author_skill_expr(&self) -> PrecheckAccessPrimitiveExpression {
        PrecheckAccessPrimitiveExpression::Path(
            AccessPrimitiveExpressionPath {
                column_path: self.publication_author_skill_physical_column_path(),
                field_path: FieldPath::Pk {
                    lead: vec!["author".to_string()],
                    pk_fields: vec!["id".to_string()],
                },
            },
            None,
        )
    }
}
