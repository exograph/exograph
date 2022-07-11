use payas_sql::{
    schema::{op::SchemaOp, spec::SchemaSpec},
    PhysicalTable,
};

pub fn migration_statements(
    old_schema_spec: SchemaSpec,
    new_schema_spec: SchemaSpec,
) -> Vec<(String, bool)> {
    let mut pre_statements = vec![];
    let mut statements = vec![];
    let mut post_statements = vec![];

    let diffs = diff_schema(&old_schema_spec, &new_schema_spec);

    for diff in diffs.iter() {
        let is_destructive = match diff {
            SchemaOp::DeleteColumn { .. }
            | SchemaOp::DeleteTable { .. }
            | SchemaOp::RemoveExtension { .. } => true,

            SchemaOp::CreateColumn { .. }
            | SchemaOp::CreateTable { .. }
            | SchemaOp::CreateExtension { .. } => false,
        };

        let statement = diff.to_sql();

        for constraint in statement.pre_statements.into_iter() {
            pre_statements.push((constraint, is_destructive));
        }

        statements.push((statement.statement, is_destructive));

        for constraint in statement.post_statements.into_iter() {
            post_statements.push((constraint, is_destructive));
        }
    }

    pre_statements.extend(statements);
    pre_statements.extend(post_statements);
    pre_statements
}

fn diff_schema<'a>(old: &'a SchemaSpec, new: &'a SchemaSpec) -> Vec<SchemaOp<'a>> {
    let existing_tables = &old.tables;
    let new_tables = &new.tables;
    let mut changes = vec![];

    // extension removal
    let extensions_to_drop = old.required_extensions.difference(&new.required_extensions);
    for extension in extensions_to_drop {
        changes.push(SchemaOp::RemoveExtension {
            extension: extension.clone(),
        })
    }

    // extension creation
    let extensions_to_create = new.required_extensions.difference(&old.required_extensions);
    for extension in extensions_to_create {
        changes.push(SchemaOp::CreateExtension {
            extension: extension.clone(),
        })
    }

    for old_table in old.tables.iter() {
        // try to find a table with the same name in the new spec
        match new_tables
            .iter()
            .find(|new_table| old_table.name == new_table.name)
        {
            // table exists, compare columns
            Some(new_table) => changes.extend(diff_table(old_table, new_table)),

            // table does not exist, deletion
            None => changes.push(SchemaOp::DeleteTable { table: old_table }),
        }
    }

    // try to find a table that needs to be created
    for new_table in new.tables.iter() {
        if !existing_tables
            .iter()
            .any(|old_table| new_table.name == old_table.name)
        {
            // new table
            changes.push(SchemaOp::CreateTable { table: new_table })
        }
    }

    changes
}

fn diff_table<'a>(old: &'a PhysicalTable, new: &'a PhysicalTable) -> Vec<SchemaOp<'a>> {
    let existing_columns = &old.columns;
    let new_columns = &new.columns;
    let mut changes = vec![];

    for column in old.columns.iter() {
        if !new_columns.contains(column) {
            // column deletion
            changes.push(SchemaOp::DeleteColumn { table: new, column });
        }
    }

    for column in new.columns.iter() {
        if !existing_columns.contains(column) {
            // new column
            changes.push(SchemaOp::CreateColumn { table: new, column });
        }
    }

    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use payas_sql::schema::spec::SchemaSpec;
    use stripmargin::StripMargin;

    #[test]
    fn add_model() {
        assert_changes(
            "",
            r#"
            model Concert {
                id: Int = autoincrement() @pk
                title: String
                published: Boolean
            }
            "#,
            vec![(
                r#"CREATE TABLE "concerts" (
                   |    "id" SERIAL PRIMARY KEY,
                   |    "title" TEXT NOT NULL,
                   |    "published" BOOLEAN NOT NULL
                   |);"#,
                false,
            )],
        );
    }

    #[test]
    fn remove_model() {
        assert_changes(
            r#"
            model Concert {
                id: Int = autoincrement() @pk
                title: String
                published: Boolean
            }
            "#,
            "",
            vec![(r#"DROP TABLE "concerts" CASCADE;"#, true)],
        );
    }

    #[test]
    fn add_field() {
        assert_changes(
            r#"
            model Concert {
                id: Int = autoincrement() @pk
                title: String
            }
            "#,
            r#"
            model Concert {
                id: Int = autoincrement() @pk
                title: String
                published: Boolean
            }
            "#,
            vec![(
                r#"ALTER TABLE "concerts" ADD "published" BOOLEAN NOT NULL;"#,
                false,
            )],
        );
    }

    #[test]
    fn remove_field() {
        assert_changes(
            r#"
            model Concert {
                id: Int = autoincrement() @pk
                title: String
                published: Boolean
            }
            "#,
            r#"
            model Concert {
                id: Int = autoincrement() @pk
                title: String
            }
            "#,
            vec![(r#"ALTER TABLE "concerts" DROP COLUMN "published";"#, true)],
        );
    }

    #[test]
    fn add_relation_and_related_model() {
        assert_changes(
            r#"
            model Concert {
                id: Int = autoincrement() @pk
                title: String
            }
            "#,
            r#"
            model Concert {
                id: Int = autoincrement() @pk
                title: String
                venue: Venue
            }
            model Venue {
                id: Int = autoincrement() @pk
                name: String
                concerts: Set<Concert>?
            }
            "#,
            vec![
                (
                    r#"ALTER TABLE "concerts" ADD "venue_id" INT NOT NULL;"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "venues" (
                |    "id" SERIAL PRIMARY KEY,
                |    "name" TEXT NOT NULL
                |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";"#,
                    false,
                ),
            ],
        );
    }

    #[test]
    fn remove_relation_and_related_model() {
        assert_changes(
            r#"
            model Concert {
                id: Int = autoincrement() @pk
                title: String
                venue: Venue
            }
            model Venue {
                id: Int = autoincrement() @pk
                name: String
                concerts: Set<Concert>?
            }
            "#,
            r#"
            model Concert {
                id: Int = autoincrement() @pk
                title: String
            }
            "#,
            vec![
                (r#"ALTER TABLE "concerts" DROP COLUMN "venue_id";"#, true),
                (r#"DROP TABLE "venues" CASCADE;"#, true),
            ],
        );
    }

    #[test]
    fn add_relation_field() {
        assert_changes(
            r#"
            model Concert {
                id: Int = autoincrement() @pk
                title: String
            }
            model Venue {
                id: Int = autoincrement() @pk
                name: String
            }
            "#,
            r#"
            model Concert {
                id: Int = autoincrement() @pk
                title: String
                venue: Venue
            }
            model Venue {
                id: Int = autoincrement() @pk
                name: String
                concerts: Set<Concert>?
            }
            "#,
            vec![
                (
                    r#"ALTER TABLE "concerts" ADD "venue_id" INT NOT NULL;"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";"#,
                    false,
                ),
            ],
        );
    }

    #[test]
    fn remove_relation_field() {
        assert_changes(
            r#"
            model Concert {
                id: Int = autoincrement() @pk
                title: String
                venue: Venue
            }
            model Venue {
                id: Int = autoincrement() @pk
                name: String
                concerts: Set<Concert>?
            }
            "#,
            r#"
            model Concert {
                id: Int = autoincrement() @pk
                title: String
            }
            model Venue {
                id: Int = autoincrement() @pk
                name: String
            }
            "#,
            vec![(r#"ALTER TABLE "concerts" DROP COLUMN "venue_id";"#, true)],
        );
    }

    fn compute_spec(model: &str) -> SchemaSpec {
        let system = payas_parser::build_system_from_str(model, "test.clay".to_string()).unwrap();
        SchemaSpec::from_model(system.tables.into_iter().collect())
    }

    fn assert_changes(old_system: &str, new_system: &str, expected_changes: Vec<(&str, bool)>) {
        let old_system = compute_spec(old_system);
        let new_system = compute_spec(new_system);
        let actual = migration_statements(old_system, new_system);

        let actual_changes = actual
            .into_iter()
            .map(|(s, d)| (s.replace('\t', "    "), d))
            .collect::<Vec<_>>();
        let expected_changes = expected_changes
            .into_iter()
            .map(|(s, d)| (s.strip_margin(), d))
            .collect::<Vec<_>>();

        assert_eq!(actual_changes, expected_changes);
    }
}
