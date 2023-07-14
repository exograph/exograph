use crate::{Database, RelationId};

/// Representation of the level of a subselection in a query.
///
/// This serve two purposes:
/// - Passing an attribute to [crate::select::Select] to indicate if it is a top-level computaiton
///   (which, in turn, forces `::text` cast for the result)
/// - Computing the alias of a selection (explained below)
///
/// We need aliasing to consider the selection level inside subqueries. For example, consider the
/// following GraphQL query:
///
/// ```graphql
/// users {
///    id
///    name
///    documents(where: {user: {id: {eq: 2}}}) {
///      id
///      content
///    }
/// }
/// ```
///
/// Here when forming the SQL, we want to user an alias when referring to the "users" table inside
/// the subquery for "documents" (note "users$users" below):
///
/// ```sql
/// SELECT COALESCE(json_agg(json_build_object(
///   'id', "users"."id",
///   'name', "users"."name",
///   'documents', (SELECT COALESCE(json_agg(json_build_object(
///     'id', "documents"."id",
///     'content', "documents"."content")), '[]'::json)
///       FROM "documents" LEFT JOIN "users" AS "users$users" ON "documents"."user_id" = "users"."id"
///         WHERE ("users$users"."id" = $1 AND "users$users"."id" = "documents"."user_id")))), '[]'::json)::text FROM "users"
/// ```
///
/// We manage aliasing by keeping track of the selection level in the [SelectionLevel::Nested]
/// variant, which is a vector of [RelationId]s, each representing the relation between the parent
/// and child selection. For example, in the above GraphQL query, the selection level for documents
/// will be `Nested(vec![RelationId::ManyToOne(<documents.user_id, users.id>)])`. Then we pick the
/// name of the self table of each relation in the vector to form the alias. For example, in the
/// above case, we pick the name of the "users" table to form the alias "users$user".
///
/// Notes:
/// - We use `$` as the separator to avoid conflicts with other table names.
/// - We could use other aliasing such as just the level number or even a mapping from the vector to
///   an arbitrary (but unique) name, but that would be less readable when debugging.
#[derive(Debug, Clone)]
pub enum SelectionLevel {
    /// Top level selection
    TopLevel,
    /// Nested sub selection, which each element representing the relation between parent and child selection
    Nested(Vec<RelationId>),
}

const ALIAS_SEPARATOR: &str = "$";

impl SelectionLevel {
    pub(super) fn is_top_level(&self) -> bool {
        matches!(self, SelectionLevel::TopLevel)
    }

    pub(super) fn with_relation_id(&self, relation_id: RelationId) -> Self {
        match self {
            SelectionLevel::TopLevel => SelectionLevel::Nested(vec![relation_id]),
            SelectionLevel::Nested(relation_ids) => {
                let mut relation_ids = relation_ids.clone();
                relation_ids.push(relation_id);
                SelectionLevel::Nested(relation_ids)
            }
        }
    }

    pub(super) fn tail_relation_id(&self) -> Option<&RelationId> {
        match self {
            SelectionLevel::TopLevel => None,
            SelectionLevel::Nested(relation_ids) => relation_ids.last(),
        }
    }

    /// Compute a suitable alias for this selection level
    pub(crate) fn alias(&self, name: String, database: &Database) -> String {
        match self {
            SelectionLevel::TopLevel => name,
            SelectionLevel::Nested(relation_ids) => {
                relation_ids.iter().rev().fold(name, |acc, relation_id| {
                    let foreign_table_id = match relation_id {
                        RelationId::ManyToOne(r) => r.deref(database).self_column_id.table_id,
                        RelationId::OneToMany(r) => r.deref(database).self_pk_column_id.table_id,
                    };
                    let table_name = &database.get_table(foreign_table_id).name;
                    format!("{table_name}{ALIAS_SEPARATOR}{acc}")
                })
            }
        }
    }
}
