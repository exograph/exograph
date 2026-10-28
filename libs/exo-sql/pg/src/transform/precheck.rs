use crate::{
    Column, PgAbstractPredicate, PgAbstractSelect, PgColumnPath, PgExtension,
    core::{select::Select, table::Table},
    pg::Postgres,
    transaction::{TransactionScript, TransactionStep},
};
use exo_sql_core::{ColumnId, Database, DatabaseError, TableId};
use exo_sql_model::{
    AbstractPredicateExt, AliasedSelectionElement, ColumnPath, Selection, SelectionElement,
    selection_level::SelectionLevel,
    transformer::{PredicateTransformer, SelectTransformer},
};

pub fn add_precheck_queries(
    precheck_predicates: Vec<PgAbstractPredicate>,
    database: &Database,
    transformer: &impl SelectTransformer<PgExtension>,
    transaction_script: &mut TransactionScript,
) {
    let precheck_queries = compute_precheck_queries(precheck_predicates).unwrap();

    for precheck_query in precheck_queries {
        let precheck_select = transformer.to_select(precheck_query, database);
        transaction_script.add_step(TransactionStep::Precheck(precheck_select));
    }
}

fn compute_precheck_queries(
    predicates: Vec<PgAbstractPredicate>,
) -> Result<Vec<PgAbstractSelect>, DatabaseError> {
    let precheck_queries: Result<Vec<_>, _> =
        predicates.into_iter().map(compute_precheck_query).collect();

    let precheck_queries = precheck_queries?;

    Ok(precheck_queries.into_iter().flatten().collect())
}

fn compute_precheck_query(
    predicate: PgAbstractPredicate,
) -> Result<Option<PgAbstractSelect>, DatabaseError> {
    if predicate == PgAbstractPredicate::True {
        return Ok(None);
    }

    fn get_lead_table_ids(column_path: &PgColumnPath) -> Vec<TableId> {
        match column_path {
            ColumnPath::Physical(physical_path) => vec![physical_path.lead_table_id()],
            ColumnPath::Predicate(predicate) => predicate
                .column_paths()
                .iter()
                .flat_map(|path| get_lead_table_ids(path))
                .collect(),
            ColumnPath::Param(_) | ColumnPath::Null => vec![],
        }
    }

    let lead_table_ids: Vec<_> = predicate
        .column_paths()
        .iter()
        .flat_map(|path| get_lead_table_ids(path))
        .collect();

    let lead_table_id = match &lead_table_ids[..] {
        [table_id] => table_id,
        [lead_table_id, rest @ ..] => {
            if rest.iter().all(|table_id| table_id == lead_table_id) {
                lead_table_id
            } else {
                return Err(DatabaseError::Precheck(
                    "Access predicates with multiple lead table ids are not supported".to_string(),
                ));
            }
        }
        [] => {
            return Err(DatabaseError::Precheck(
                "Access predicates with no lead table ids are not supported".to_string(),
            ));
        }
    };

    Ok(Some(PgAbstractSelect {
        table_id: *lead_table_id,
        selection: Selection::Seq(vec![AliasedSelectionElement::new(
            "access_predicate".to_string(),
            SelectionElement::Constant("true".to_string()),
        )]),
        predicate: predicate.clone(),
        order_by: None,
        offset: None,
        limit: None,
    }))
}

/// The precheck for a row about to be written: a select that returns one row if `predicate`
/// holds for the row's values.
///
/// The predicate's column paths start from the row's table (`table_id`), as in any other
/// predicate. The row isn't in the table yet, so the select reads it from its values (see
/// `Table::Values`), and a relation from the row is checked through the row's own key, as a
/// sub-select (`"concerts"."venue_id" IN (SELECT "venues"."id" FROM "venues" WHERE ...)`).
/// So relations to the same table, relations to different tables, and a negation are each
/// evaluated as they would be for a stored row.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "prechecks don't yet start from the row being written"
    )
)]
pub(crate) fn compute_row_precheck_select(
    table_id: TableId,
    row: Vec<(ColumnId, Column)>,
    predicate: &PgAbstractPredicate,
    database: &Database,
    transformer: &Postgres,
) -> Result<Select, DatabaseError> {
    fn check_row_columns(
        predicate: &PgAbstractPredicate,
        table_id: TableId,
        row: &[(ColumnId, Column)],
        database: &Database,
    ) -> Result<(), DatabaseError> {
        predicate
            .column_paths()
            .into_iter()
            .try_for_each(|column_path| match column_path {
                ColumnPath::Physical(physical_path) => {
                    if physical_path.lead_table_id() != table_id {
                        return Err(DatabaseError::Precheck(format!(
                            "A precheck for a row of '{}' refers to a path from '{}'",
                            database.get_table(table_id).name.name,
                            database.get_table(physical_path.lead_table_id()).name.name
                        )));
                    }

                    // The columns of the row that the path reads (its own column, or the relation's key)
                    let (head, _) = physical_path.split_head();
                    let missing_column_id = head.self_column_ids().into_iter().find(|column_id| {
                        !row.iter()
                            .any(|(row_column_id, _)| row_column_id == column_id)
                    });
                    if let Some(missing_column_id) = missing_column_id {
                        return Err(DatabaseError::Precheck(format!(
                            "A precheck refers to the column '{}', which the row doesn't have",
                            missing_column_id.get_column(database).name
                        )));
                    }
                    Ok(())
                }
                ColumnPath::Predicate(predicate) => {
                    check_row_columns(predicate, table_id, row, database)
                }
                ColumnPath::Param(_) | ColumnPath::Null => Ok(()),
            })
    }

    check_row_columns(predicate, table_id, &row, database)?;

    let predicate = transformer.to_predicate(predicate, &SelectionLevel::TopLevel, false, database);

    Ok(Select {
        table: Table::Values { table_id, row },
        columns: vec![Column::Constant("true".to_string())],
        predicate,
        order_by: None,
        offset: None,
        limit: None,
        group_by: None,
        top_level_selection: false,
    })
}

#[cfg(test)]
mod tests {
    use exo_sql_core::{SchemaObjectName, relation::RelationColumnPair};
    use exo_sql_model::{AbstractPredicate, ColumnPathLink, PhysicalColumnPath};
    use multiplatform_test::multiplatform_test;

    use crate::{
        ExpressionBuilder, sql_param_container::SQLParamContainer, test_database_builder::*,
    };

    use super::*;

    /// A concert has two venues (two relations to one table), an organizer (a relation to another
    /// table), and artists (a one-to-many relation)
    struct Setup {
        database: Database,
        concerts_table_id: TableId,
    }

    fn setup() -> Setup {
        let database = DatabaseBuilder::new()
            .table(
                "concerts",
                vec![
                    pk("id"),
                    string("title"),
                    fk("main_venue_id", "venues", "id", "main_venue_fk"),
                    fk(
                        "alternative_venue_id",
                        "venues",
                        "id",
                        "alternative_venue_fk",
                    ),
                    fk("organizer_id", "organizers", "id", "organizer_fk"),
                ],
            )
            .table("venues", vec![pk("id"), string("name")])
            .table("organizers", vec![pk("id"), string("name")])
            .table(
                "concert_artists",
                vec![
                    pk("id"),
                    fk("concert_id", "concerts", "id", "concert_fk"),
                    string("artist"),
                ],
            )
            .build();

        let concerts_table_id = database
            .get_table_id(&SchemaObjectName::new("concerts", None))
            .unwrap();

        Setup {
            database,
            concerts_table_id,
        }
    }

    impl Setup {
        fn column(&self, table: &str, column: &str) -> ColumnId {
            let table_id = self
                .database
                .get_table_id(&SchemaObjectName::new(table, None))
                .unwrap();
            self.database.get_column_id(table_id, column).unwrap()
        }

        /// A path from `concerts` through the given columns, such as
        /// `[concerts.main_venue_id, venues.name]`
        fn path(&self, columns: &[(&str, &str)]) -> PgColumnPath {
            ColumnPath::Physical(PhysicalColumnPath::from_columns(
                columns
                    .iter()
                    .map(|(table, column)| self.column(table, column))
                    .collect(),
                &self.database,
            ))
        }

        /// `<venue>.name == name`, for the venue in the given column of `concerts` (such as
        /// `self.mainVenue.name == "v1"`)
        fn venue_name_eq(&self, venue_column: &str, name: &str) -> PgAbstractPredicate {
            AbstractPredicate::Eq(
                self.path(&[("concerts", venue_column), ("venues", "name")]),
                string_param(name),
            )
        }

        fn key(&self, column: &str, value: i32) -> (ColumnId, Column) {
            (
                self.column("concerts", column),
                Column::Param(SQLParamContainer::i32(value)),
            )
        }

        fn select(
            &self,
            row: Vec<(ColumnId, Column)>,
            predicate: &PgAbstractPredicate,
        ) -> Result<Select, DatabaseError> {
            compute_row_precheck_select(
                self.concerts_table_id,
                row,
                predicate,
                &self.database,
                &Postgres {},
            )
        }
    }

    fn string_param(value: &str) -> PgColumnPath {
        ColumnPath::Param(SQLParamContainer::string(value.to_string()))
    }

    #[multiplatform_test]
    fn two_relations_to_one_table() {
        let setup = setup();

        // self.mainVenue.name == "v1" && self.alternativeVenue.name == "v2"
        let predicate = AbstractPredicate::and(
            setup.venue_name_eq("main_venue_id", "v1"),
            setup.venue_name_eq("alternative_venue_id", "v2"),
        );

        let select = setup
            .select(
                vec![
                    setup.key("main_venue_id", 1),
                    setup.key("alternative_venue_id", 2),
                ],
                &predicate,
            )
            .unwrap();

        // Each venue is its own sub-select, through the row's own key
        assert_binding!(
            select.to_sql(&setup.database),
            r#"SELECT 'true' FROM (VALUES ($1, $2)) AS "concerts" ("main_venue_id", "alternative_venue_id") WHERE ("concerts"."main_venue_id" IN (SELECT "venues"."id" FROM "venues" WHERE "venues"."name" = $3) AND "concerts"."alternative_venue_id" IN (SELECT "venues"."id" FROM "venues" WHERE "venues"."name" = $4))"#,
            1,
            2,
            "v1".to_string(),
            "v2".to_string()
        );
    }

    #[multiplatform_test]
    fn relations_to_two_tables() {
        let setup = setup();

        // self.mainVenue.name == "v1" || self.organizer.name == "o1"
        let predicate = AbstractPredicate::or(
            setup.venue_name_eq("main_venue_id", "v1"),
            AbstractPredicate::Eq(
                setup.path(&[("concerts", "organizer_id"), ("organizers", "name")]),
                string_param("o1"),
            ),
        );

        let select = setup
            .select(
                vec![setup.key("main_venue_id", 1), setup.key("organizer_id", 2)],
                &predicate,
            )
            .unwrap();

        assert_binding!(
            select.to_sql(&setup.database),
            r#"SELECT 'true' FROM (VALUES ($1, $2)) AS "concerts" ("main_venue_id", "organizer_id") WHERE ("concerts"."main_venue_id" IN (SELECT "venues"."id" FROM "venues" WHERE "venues"."name" = $3) OR "concerts"."organizer_id" IN (SELECT "organizers"."id" FROM "organizers" WHERE "organizers"."name" = $4))"#,
            1,
            2,
            "v1".to_string(),
            "o1".to_string()
        );
    }

    #[multiplatform_test]
    fn negated_lookup() {
        let setup = setup();

        // !(self.mainVenue.name == "v1")
        let predicate =
            AbstractPredicate::Not(Box::new(setup.venue_name_eq("main_venue_id", "v1")));

        let select = setup
            .select(vec![setup.key("main_venue_id", 1)], &predicate)
            .unwrap();

        // The negation applies to the row's own venue, not to every other venue
        assert_binding!(
            select.to_sql(&setup.database),
            r#"SELECT 'true' FROM (VALUES ($1)) AS "concerts" ("main_venue_id") WHERE "concerts"."main_venue_id" IN (SELECT "venues"."id" FROM "venues" WHERE NOT("venues"."name" = $2))"#,
            1,
            "v1".to_string()
        );
    }

    #[multiplatform_test]
    fn one_to_many_relation() {
        let setup = setup();

        // self.artists.some(a => a.artist == "a1")
        let predicate = AbstractPredicate::Eq(
            setup.path(&[
                ("concert_artists", "concert_id"),
                ("concert_artists", "artist"),
            ]),
            string_param("a1"),
        );

        let select = setup.select(vec![setup.key("id", 1)], &predicate).unwrap();

        assert_binding!(
            select.to_sql(&setup.database),
            r#"SELECT 'true' FROM (VALUES ($1)) AS "concerts" ("id") WHERE "concerts"."id" IN (SELECT "concert_artists"."concert_id" FROM "concert_artists" WHERE "concert_artists"."artist" = $2)"#,
            1,
            "a1".to_string()
        );
    }

    #[multiplatform_test]
    fn column_of_the_row() {
        let setup = setup();

        // self.title == "t1"
        let predicate =
            AbstractPredicate::Eq(setup.path(&[("concerts", "title")]), string_param("t1"));

        let select = setup
            .select(
                vec![(
                    setup.column("concerts", "title"),
                    Column::Param(SQLParamContainer::string("t1".to_string())),
                )],
                &predicate,
            )
            .unwrap();

        assert_binding!(
            select.to_sql(&setup.database),
            r#"SELECT 'true' FROM (VALUES ($1)) AS "concerts" ("title") WHERE "concerts"."title" = $2"#,
            "t1".to_string(),
            "t1".to_string()
        );
    }

    #[multiplatform_test]
    fn null_key() {
        let setup = setup();

        let predicate = setup.venue_name_eq("main_venue_id", "v1");

        let select = setup
            .select(
                vec![(setup.column("concerts", "main_venue_id"), Column::Null)],
                &predicate,
            )
            .unwrap();

        // Typed as the column, so that it compares with the venue's id (as an unknown, as in SQL)
        assert_binding!(
            select.to_sql(&setup.database),
            r#"SELECT 'true' FROM (VALUES (NULL::int2)) AS "concerts" ("main_venue_id") WHERE "concerts"."main_venue_id" IN (SELECT "venues"."id" FROM "venues" WHERE "venues"."name" = $1)"#,
            "v1".to_string()
        );
    }

    #[multiplatform_test]
    fn empty_row() {
        let setup = setup();

        let select = setup.select(vec![], &AbstractPredicate::True).unwrap();

        assert_binding!(
            select.to_sql(&setup.database),
            r#"SELECT 'true' FROM (SELECT) AS "concerts""#
        );
    }

    #[multiplatform_test]
    fn self_relation_in_a_schema() {
        let mut database = DatabaseBuilder::new()
            .table(
                "users",
                vec![
                    pk("id"),
                    string("name"),
                    fk("manager_id", "users", "id", "manager_fk"),
                ],
            )
            .build();
        let users_table_id = database
            .get_table_id(&SchemaObjectName::new("users", None))
            .unwrap();
        database.get_table_mut(users_table_id).name = SchemaObjectName::new("users", Some("auth"));

        let column = |name| database.get_column_id(users_table_id, name).unwrap();

        // self.manager.name == "boss"
        let predicate = AbstractPredicate::Eq(
            ColumnPath::Physical(
                PhysicalColumnPath::init(ColumnPathLink::relation(
                    vec![RelationColumnPair {
                        self_column_id: column("manager_id"),
                        foreign_column_id: column("id"),
                    }],
                    None,
                ))
                .push(ColumnPathLink::Leaf(column("name"))),
            ),
            string_param("boss"),
        );

        let select = compute_row_precheck_select(
            users_table_id,
            vec![(
                column("manager_id"),
                Column::Param(SQLParamContainer::i32(1)),
            )],
            &predicate,
            &database,
            &Postgres {},
        )
        .unwrap();

        // The row's columns refer to the values, but the sub-select still reads `auth.users`
        assert_binding!(
            select.to_sql(&database),
            r#"SELECT 'true' FROM (VALUES ($1)) AS "users" ("manager_id") WHERE "users"."manager_id" IN (SELECT "users"."id" FROM "auth"."users" WHERE "users"."name" = $2)"#,
            1,
            "boss".to_string()
        );
    }

    #[multiplatform_test]
    fn column_missing_from_the_row() {
        let setup = setup();

        let predicate = setup.venue_name_eq("main_venue_id", "v1");

        let result = setup.select(vec![setup.key("organizer_id", 1)], &predicate);

        assert!(
            matches!(&result, Err(DatabaseError::Precheck(message)) if message.contains("main_venue_id")),
            "{result:?}"
        );
    }

    #[multiplatform_test]
    fn path_from_another_table() {
        let setup = setup();

        let predicate =
            AbstractPredicate::Eq(setup.path(&[("venues", "name")]), string_param("v1"));

        let result = setup.select(vec![setup.key("main_venue_id", 1)], &predicate);

        assert!(
            matches!(&result, Err(DatabaseError::Precheck(message)) if message.contains("venues")),
            "{result:?}"
        );
    }
}
