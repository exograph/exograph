// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql::{
    schema::{op::SchemaOp, spec::SchemaSpec},
    Database,
};

pub(crate) struct Migrations {
    pub statements: Vec<MigrationStatement>,
}

pub(crate) struct MigrationStatement {
    pub statement: String,
    pub is_destructive: bool,
}

impl Migrations {
    pub async fn apply(
        &self,
        database: &Database,
        allow_destructive_changes: bool,
    ) -> Result<(), anyhow::Error> {
        let mut client = database.get_client().await?;
        let transaction = client.transaction().await?;
        for MigrationStatement {
            statement,
            is_destructive,
        } in self.statements.iter()
        {
            if !is_destructive || allow_destructive_changes {
                transaction.execute(statement, &[]).await?;
            } else {
                return Err(anyhow::anyhow!(
                    "Destructive change detected: {}",
                    statement
                ));
            }
        }
        Ok(transaction.commit().await?)
    }

    pub fn write(
        &self,
        writer: &mut dyn std::io::Write,
        allow_destructive_changes: bool,
    ) -> std::io::Result<()> {
        for MigrationStatement {
            statement,
            is_destructive,
        } in self.statements.iter()
        {
            if *is_destructive && !allow_destructive_changes {
                write!(writer, "-- ")?;
            }
            writeln!(writer, "{statement}\n")?;
        }
        Ok(())
    }
}

impl MigrationStatement {
    pub fn new(statement: String, is_destructive: bool) -> Self {
        Self {
            statement,
            is_destructive,
        }
    }
}

pub(crate) fn migration_statements(
    old_schema_spec: &SchemaSpec,
    new_schema_spec: &SchemaSpec,
) -> Migrations {
    let mut pre_statements = vec![];
    let mut statements = vec![];
    let mut post_statements = vec![];

    let diffs = old_schema_spec.diff(new_schema_spec);

    for diff in diffs.iter() {
        let is_destructive = match diff {
            SchemaOp::DeleteColumn { .. }
            | SchemaOp::DeleteTable { .. }
            | SchemaOp::RemoveExtension { .. } => true,

            // Explicitly matching the other cases here to ensure that we have thought about each case
            SchemaOp::CreateColumn { .. }
            | SchemaOp::CreateTable { .. }
            | SchemaOp::CreateExtension { .. }
            | SchemaOp::CreateUniqueConstraint { .. }
            | SchemaOp::RemoveUniqueConstraint { .. }
            | SchemaOp::SetColumnDefaultValue { .. }
            | SchemaOp::UnsetColumnDefaultValue { .. }
            | SchemaOp::SetNotNull { .. }
            | SchemaOp::UnsetNotNull { .. } => false,
        };

        let statement = diff.to_sql();

        for constraint in statement.pre_statements.into_iter() {
            pre_statements.push(MigrationStatement::new(constraint, is_destructive));
        }

        statements.push(MigrationStatement::new(statement.statement, is_destructive));

        for constraint in statement.post_statements.into_iter() {
            post_statements.push(MigrationStatement::new(constraint, is_destructive));
        }
    }

    pre_statements.extend(statements);
    pre_statements.extend(post_statements);
    Migrations {
        statements: pre_statements,
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::schema::util;

    use super::*;
    use exo_sql::schema::spec::SchemaSpec;
    use stripmargin::StripMargin;

    #[test]
    fn add_model() {
        assert_changes(
            "",
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    published: Boolean
                }
            }
            "#,
            vec![],
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                   |    "id" SERIAL PRIMARY KEY,
                   |    "title" TEXT NOT NULL,
                   |    "published" BOOLEAN NOT NULL
                   |);"#,
                    false,
                ),
                ("CREATE INDEX ON \"concerts\" (\"title\");", false),
                ("CREATE INDEX ON \"concerts\" (\"published\");", false),
            ],
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                   |    "id" SERIAL PRIMARY KEY,
                   |    "title" TEXT NOT NULL,
                   |    "published" BOOLEAN NOT NULL
                   |);"#,
                    false,
                ),
                ("CREATE INDEX ON \"concerts\" (\"title\");", false),
                ("CREATE INDEX ON \"concerts\" (\"published\");", false),
            ],
            vec![(r#"DROP TABLE "concerts" CASCADE;"#, true)],
        );
    }

    #[test]
    fn add_field() {
        assert_changes(
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                }
            }
            "#,
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    published: Boolean
                }
            }
            "#,
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                ("CREATE INDEX ON \"concerts\" (\"title\");", false),
            ],
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "published" BOOLEAN NOT NULL
                    |);"#,
                    false,
                ),
                ("CREATE INDEX ON \"concerts\" (\"title\");", false),
                ("CREATE INDEX ON \"concerts\" (\"published\");", false),
            ],
            vec![
                (
                    r#"ALTER TABLE "concerts" ADD "published" BOOLEAN NOT NULL;"#,
                    false,
                ),
                ("CREATE INDEX ON \"concerts\" (\"published\");", false),
            ],
            vec![(r#"ALTER TABLE "concerts" DROP COLUMN "published";"#, true)],
        );
    }

    #[test]
    fn add_relation_and_related_model() {
        assert_changes(
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                }
            }
            "#,
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    venue: Venue
                }
                type Venue {
                    @pk id: Int = autoIncrement()
                    name: String
                    concerts: Set<Concert>?
                }
            }
            "#,
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                ("CREATE INDEX ON \"concerts\" (\"title\");", false),
            ],
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "venue_id" INT NOT NULL
                    |);"#,
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
                ("CREATE INDEX ON \"concerts\" (\"title\");", false),
                ("CREATE INDEX ON \"concerts\" (\"venue_id\");", false),
                ("CREATE INDEX ON \"venues\" (\"name\");", false),
            ],
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
                ("CREATE INDEX ON \"concerts\" (\"venue_id\");", false),
                ("CREATE INDEX ON \"venues\" (\"name\");", false),
            ],
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
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                }
                type Venue {
                    @pk id: Int = autoIncrement()
                    name: String
                }
            }
            "#,
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    venue: Venue
                }
                type Venue {
                    @pk id: Int = autoIncrement()
                    name: String
                    concerts: Set<Concert>?
                }
            }
            "#,
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "venues" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                ("CREATE INDEX ON \"concerts\" (\"title\");", false),
                ("CREATE INDEX ON \"venues\" (\"name\");", false),
            ],
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "venue_id" INT NOT NULL
                    |);"#,
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
                ("CREATE INDEX ON \"concerts\" (\"title\");", false),
                ("CREATE INDEX ON \"concerts\" (\"venue_id\");", false),
                ("CREATE INDEX ON \"venues\" (\"name\");", false),
            ],
            vec![
                (
                    r#"ALTER TABLE "concerts" ADD "venue_id" INT NOT NULL;"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venue_id_fk" FOREIGN KEY ("venue_id") REFERENCES "venues";"#,
                    false,
                ),
                ("CREATE INDEX ON \"concerts\" (\"venue_id\");", false),
            ],
            vec![(r#"ALTER TABLE "concerts" DROP COLUMN "venue_id";"#, true)],
        );
    }

    #[test]
    fn update_relation_optionality() {
        // venue: Venue <-> venue: Venue?
        assert_changes(
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    venue: Venue
                }
                type Venue {
                    @pk id: Int = autoIncrement()
                    name: String
                    concerts: Set<Concert>?
                }
            }
            "#,
            r#"
            @postgres
            module ConcertModule {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    venue: Venue?
                }
                type Venue {
                    @pk id: Int = autoIncrement()
                    name: String
                    concerts: Set<Concert>?
                }
            }
            "#,
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "venue_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "venues" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                ("ALTER TABLE \"concerts\" ADD CONSTRAINT \"concerts_venue_id_fk\" FOREIGN KEY (\"venue_id\") REFERENCES \"venues\";", false),
                ("CREATE INDEX ON \"concerts\" (\"title\");", false),
                ("CREATE INDEX ON \"concerts\" (\"venue_id\");", false),
                ("CREATE INDEX ON \"venues\" (\"name\");", false),
            ],
            vec![
                (
                    r#"CREATE TABLE "concerts" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "title" TEXT NOT NULL,
                    |    "venue_id" INT
                    |);"#,
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
                ("CREATE INDEX ON \"concerts\" (\"title\");", false),
                ("CREATE INDEX ON \"concerts\" (\"venue_id\");", false),
                ("CREATE INDEX ON \"venues\" (\"name\");", false),
            ],
            vec![
                ("ALTER TABLE \"concerts\" ALTER COLUMN \"venue_id\" DROP NOT NULL;", false),
            ],
            vec![("ALTER TABLE \"concerts\" ALTER COLUMN \"venue_id\" SET NOT NULL;", false)],
        );
    }

    #[test]
    fn one_to_one_constraints() {
        assert_changes(
            r#"
                @postgres
                module MembershipModule {
                    type Membership {
                        @pk id: Int = autoIncrement()
                    }
                    type User {
                        @pk id: Int = autoIncrement()
                        name: String
                    }
                }
            "#,
            r#"
                @postgres
                module MembershipModule {
                    type Membership {
                        @pk id: Int = autoIncrement()
                        user: User
                    }
                    type User {
                        @pk id: Int = autoIncrement()
                        name: String
                        membership: Membership?
                    }
                }
            "#,
            vec![
                (
                    r#"CREATE TABLE "memberships" (
                    |    "id" SERIAL PRIMARY KEY
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "users" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                ("CREATE INDEX ON \"users\" (\"name\");", false),
            ],
            vec![
                (
                    r#"CREATE TABLE "memberships" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "user_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"CREATE TABLE "users" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "name" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "memberships" ADD CONSTRAINT "memberships_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "users";"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "memberships" ADD CONSTRAINT "unique_constraint_membership_user" UNIQUE ("user_id");"#,
                    false,
                ),
                ("CREATE INDEX ON \"memberships\" (\"user_id\");", false),
                ("CREATE INDEX ON \"users\" (\"name\");", false),
            ],
            vec![
                (
                    r#"ALTER TABLE "memberships" ADD "user_id" INT NOT NULL;"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "memberships" ADD CONSTRAINT "unique_constraint_membership_user" UNIQUE ("user_id");"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "memberships" ADD CONSTRAINT "memberships_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "users";"#,
                    false,
                ),
                ("CREATE INDEX ON \"memberships\" (\"user_id\");", false),
            ],
            vec![
                (r#"ALTER TABLE "memberships" DROP COLUMN "user_id";"#, true),
                (
                    r#"ALTER TABLE "memberships" DROP CONSTRAINT "unique_constraint_membership_user";"#,
                    false,
                ),
            ],
        )
    }

    #[test]
    fn multi_column_unique_constraint() {
        assert_changes(
            r#"
                @postgres
                module RsvpModule {
                    type Rsvp {
                        @pk id: Int = autoIncrement()
                        email: String
                        event_id: Int
                    }
                }
            "#,
            r#"
                @postgres
                module RsvpModule {
                    type Rsvp {
                        @pk id: Int = autoIncrement()
                        @unique("email_event_id") email: String 
                        @unique("email_event_id") event_id: Int 
                    }
                }
            "#,
            vec![
                (
                    r#"CREATE TABLE "rsvps" (
                |    "id" SERIAL PRIMARY KEY,
                |    "email" TEXT NOT NULL,
                |    "event_id" INT NOT NULL
                |);"#,
                    false,
                ),
                ("CREATE INDEX ON \"rsvps\" (\"email\");", false),
                ("CREATE INDEX ON \"rsvps\" (\"event_id\");", false),
            ],
            vec![
                (
                    r#"CREATE TABLE "rsvps" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "email" TEXT NOT NULL,
                    |    "event_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "rsvps" ADD CONSTRAINT "email_event_id" UNIQUE ("email", "event_id");"#,
                    false,
                ),
                ("CREATE INDEX ON \"rsvps\" (\"email\");", false),
                ("CREATE INDEX ON \"rsvps\" (\"event_id\");", false),
            ],
            vec![(
                r#"ALTER TABLE "rsvps" ADD CONSTRAINT "email_event_id" UNIQUE ("email", "event_id");"#,
                false,
            )],
            vec![(
                r#"ALTER TABLE "rsvps" DROP CONSTRAINT "email_event_id";"#,
                false,
            )],
        )
    }

    #[test]
    fn multi_column_unique_constraint_participation_change() {
        assert_changes(
            r#"
                @postgres
                module RsvpModule {
                    type Rsvp {
                        @pk id: Int = autoIncrement()
                        @unique("email_event_id") email: String 
                        event_id: Int
                    }
                }
            "#,
            r#"
                @postgres
                module RsvpModule {
                    type Rsvp {
                        @pk id: Int = autoIncrement()
                        @unique("email_event_id") email: String 
                        @unique("email_event_id") event_id: Int 
                    }
                }
            "#,
            vec![
                (
                    r#"CREATE TABLE "rsvps" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "email" TEXT NOT NULL,
                    |    "event_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "rsvps" ADD CONSTRAINT "email_event_id" UNIQUE ("email");"#,
                    false,
                ),
                ("CREATE INDEX ON \"rsvps\" (\"email\");", false),
                ("CREATE INDEX ON \"rsvps\" (\"event_id\");", false),
            ],
            vec![
                (
                    r#"CREATE TABLE "rsvps" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "email" TEXT NOT NULL,
                    |    "event_id" INT NOT NULL
                    |);"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "rsvps" ADD CONSTRAINT "email_event_id" UNIQUE ("email", "event_id");"#,
                    false,
                ),
                ("CREATE INDEX ON \"rsvps\" (\"email\");", false),
                ("CREATE INDEX ON \"rsvps\" (\"event_id\");", false),
            ],
            vec![
                (
                    r#"ALTER TABLE "rsvps" DROP CONSTRAINT "email_event_id";"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "rsvps" ADD CONSTRAINT "email_event_id" UNIQUE ("email", "event_id");"#,
                    false,
                ),
            ],
            vec![
                (
                    r#"ALTER TABLE "rsvps" DROP CONSTRAINT "email_event_id";"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "rsvps" ADD CONSTRAINT "email_event_id" UNIQUE ("email");"#,
                    false,
                ),
            ],
        )
    }

    #[test]
    fn default_value_change() {
        assert_changes(
            r#"
                @postgres
                module UserModule {
                    type User {
                        @pk id: Int = autoIncrement()
                        role: String
                        verified: Boolean = false
                        enabled: Boolean = true
                    }
                }
            "#,
            r#"
                @postgres
                module UserModule {
                    type User {
                        @pk id: Int = autoIncrement()
                        role: String = "USER" // Set default value
                        verified: Boolean = true // Change default value
                        enabled: Boolean // Drop default value
                    }
                }
            "#,
            vec![
                (
                    r#"CREATE TABLE "users" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "role" TEXT NOT NULL,
                    |    "verified" BOOLEAN NOT NULL DEFAULT false,
                    |    "enabled" BOOLEAN NOT NULL DEFAULT true
                    |);"#,
                    false,
                ),
                ("CREATE INDEX ON \"users\" (\"role\");", false),
                ("CREATE INDEX ON \"users\" (\"verified\");", false),
                ("CREATE INDEX ON \"users\" (\"enabled\");", false),
            ],
            vec![
                (
                    r#"CREATE TABLE "users" (
                    |    "id" SERIAL PRIMARY KEY,
                    |    "role" TEXT NOT NULL DEFAULT 'USER'::text,
                    |    "verified" BOOLEAN NOT NULL DEFAULT true,
                    |    "enabled" BOOLEAN NOT NULL
                    |);"#,
                    false,
                ),
                ("CREATE INDEX ON \"users\" (\"role\");", false),
                ("CREATE INDEX ON \"users\" (\"verified\");", false),
                ("CREATE INDEX ON \"users\" (\"enabled\");", false),
            ],
            vec![
                (
                    r#"ALTER TABLE "users" ALTER COLUMN "role" SET DEFAULT 'USER'::text;"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "users" ALTER COLUMN "verified" SET DEFAULT true;"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "users" ALTER COLUMN "enabled" DROP DEFAULT;"#,
                    false,
                ),
            ],
            vec![
                (
                    r#"ALTER TABLE "users" ALTER COLUMN "role" DROP DEFAULT;"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "users" ALTER COLUMN "verified" SET DEFAULT false;"#,
                    false,
                ),
                (
                    r#"ALTER TABLE "users" ALTER COLUMN "enabled" SET DEFAULT true;"#,
                    false,
                ),
            ],
        )
    }

    #[test]
    fn not_null() {
        assert_changes(
            r#"
                @postgres
                module LogModule {
                    type Log {
                        @pk id: Int
                        level: String?
                        message: String
                    }
                }
            "#,
            r#"
                @postgres
                module LogModule {
                    type Log {
                        @pk id: Int
                        level: String
                        message: String
                    }
                } 
            "#,
            vec![
                (
                    r#"CREATE TABLE "logs" (
                    |    "id" INT PRIMARY KEY,
                    |    "level" TEXT,
                    |    "message" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                ("CREATE INDEX ON \"logs\" (\"level\");", false),
                ("CREATE INDEX ON \"logs\" (\"message\");", false),
            ],
            vec![
                (
                    r#"CREATE TABLE "logs" (
                    |    "id" INT PRIMARY KEY,
                    |    "level" TEXT NOT NULL,
                    |    "message" TEXT NOT NULL
                    |);"#,
                    false,
                ),
                ("CREATE INDEX ON \"logs\" (\"level\");", false),
                ("CREATE INDEX ON \"logs\" (\"message\");", false),
            ],
            vec![(
                r#"ALTER TABLE "logs" ALTER COLUMN "level" SET NOT NULL;"#,
                false,
            )],
            vec![(
                r#"ALTER TABLE "logs" ALTER COLUMN "level" DROP NOT NULL;"#,
                false,
            )],
        )
    }

    fn compute_spec(model: &str) -> SchemaSpec {
        let postgres_subsystem =
            util::create_postgres_system_from_str(model, "test.exo".to_string()).unwrap();

        SchemaSpec::from_model(postgres_subsystem.tables.into_iter().collect())
    }

    fn assert_changes(
        old_system: &str,
        new_system: &str,
        old_create: Vec<(&str, bool)>,
        new_create: Vec<(&str, bool)>,
        up_migration: Vec<(&str, bool)>,
        down_migration: Vec<(&str, bool)>,
    ) {
        let old_system = compute_spec(old_system);
        let new_system = compute_spec(new_system);

        assert_change(
            &SchemaSpec::default(),
            &old_system,
            old_create,
            "Create old system schema",
        );
        assert_change(
            &SchemaSpec::default(),
            &new_system,
            new_create,
            "Create new system schema",
        );

        // Check that migration is idempotent by checking that re-migrating yield no changes
        assert_change(
            &old_system,
            &old_system,
            vec![],
            "Idempotent with old model",
        );

        assert_change(
            &new_system,
            &new_system,
            vec![],
            "Idempotent with new model",
        );

        // Up changes old -> new
        assert_change(&old_system, &new_system, up_migration, "Up migration");
        // Down changes new -> old
        assert_change(&new_system, &old_system, down_migration, "Down migration");
    }

    fn assert_change(
        old_system: &SchemaSpec,
        new_system: &SchemaSpec,
        expected: Vec<(&str, bool)>,
        message: &str,
    ) {
        fn clean_actual(actual: Migrations) -> Vec<(String, bool)> {
            actual
                .statements
                .into_iter()
                .map(
                    |MigrationStatement {
                         statement: s,
                         is_destructive: d,
                     }| (s.replace('\t', "    "), d),
                )
                .collect()
        }
        fn clean_expected(expected: Vec<(&str, bool)>) -> Vec<(String, bool)> {
            expected
                .into_iter()
                .map(|(s, d)| (s.strip_margin(), d))
                .collect()
        }

        let actual = migration_statements(old_system, new_system);

        let actual_changes = clean_actual(actual);
        let expected_changes = clean_expected(expected);

        assert_eq!(actual_changes, expected_changes, "{message}");
    }
}
