use crate::{Database, RelationId, TableId};

/// Representation of the level of a subselection in a query.
///
/// This serves two purposes:
/// - Passing an attribute to [crate::select::Select] to indicate if it is a top-level computation
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
/// Here when forming the SQL, we want to use an alias when referring to the "users" table inside
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

    pub(crate) fn tail_relation_id(&self) -> Option<&RelationId> {
        match self {
            SelectionLevel::TopLevel => None,
            SelectionLevel::Nested(relation_ids) => relation_ids.last(),
        }
    }

    /// Compute a suitable alias for this selection level
    pub(crate) fn alias(
        &self,
        leaf_table: (TableId, Option<String>),
        database: &Database,
    ) -> String {
        let prefix = self.prefix(database);

        let leaf_table_name = leaf_table.1.unwrap_or_else(|| {
            database
                .get_table(leaf_table.0)
                .name
                .fully_qualified_name_with_sep(ALIAS_SEPARATOR)
        });

        match prefix {
            None => leaf_table_name,
            Some(prefix) => format!("{prefix}{ALIAS_SEPARATOR}{leaf_table_name}"),
        }
    }

    pub(crate) fn prefix(&self, database: &Database) -> Option<String> {
        match self {
            SelectionLevel::TopLevel => None,
            SelectionLevel::Nested(relation_ids) => {
                // Collect the table and the next-table aliases in the relation chain
                let table_linking = relation_ids.iter().map(|relation_id| match relation_id {
                    RelationId::ManyToOne(r) => {
                        let many_to_one = r.deref(database);

                        (
                            many_to_one.column_pairs[0].self_column_id.table_id,
                            many_to_one.foreign_table_alias.clone(),
                        )
                    }
                    RelationId::OneToMany(r) => {
                        let one_to_many = r.deref(database);

                        (one_to_many.column_pairs[0].self_column_id.table_id, None)
                    }
                });

                // Go over the table linking and for name aliasing list If there is an alias for the
                // previous table, use it instead of the table name For example, if we have a
                // relation chain like:
                // (concert-table, None) -> (venue-table, Some("main-venue")) -> (office-table,None),
                // The alias formed will be:
                // "concert-table$main-venue$office-table" (instead of "concert-table$venue-table$office-table")
                let (_, names) = table_linking.fold(
                    (None, vec![]),
                    |(prev_alias, mut acc), cur| match prev_alias {
                        None => {
                            let (table_id, alias) = cur;
                            let table_name = database
                                .get_table(table_id)
                                .name
                                .fully_qualified_name_with_sep(ALIAS_SEPARATOR);
                            acc.push(table_name);

                            (alias, acc)
                        }
                        Some(prev_alias) => {
                            let (_, alias) = cur;
                            acc.push(prev_alias);

                            (alias, acc)
                        }
                    },
                );

                Some(names.join(ALIAS_SEPARATOR))
            }
        }
    }
}
