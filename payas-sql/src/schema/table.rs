use std::collections::{HashMap, HashSet};

use crate::sql::column::PhysicalColumnType;
use crate::{PhysicalColumn, PhysicalTable};
use anyhow::Result;
use deadpool_postgres::Client;
use regex::Regex;

use super::issue::WithIssues;
use super::statement::SchemaStatement;

impl PhysicalTable {
    /// Creates a new table specification from an SQL table.
    pub async fn from_db(client: &Client, table_name: &str) -> Result<WithIssues<PhysicalTable>> {
        // Query to get a list of constraints in the table (primary key and foreign key constraints)
        let constraints_query = format!(
            "
            SELECT contype, pg_get_constraintdef(oid, true) as condef
            FROM pg_constraint
            WHERE
                conrelid = '{}'::regclass AND conparentid = 0",
            table_name
        );

        // Query to get a list of columns in the table
        let columns_query = format!(
            "SELECT column_name FROM information_schema.columns WHERE table_name = '{}'",
            table_name
        );

        let primary_key_re = Regex::new(r"PRIMARY KEY \(([^)]+)\)").unwrap();
        let foreign_key_re =
            Regex::new(r"FOREIGN KEY \(([^)]+)\) REFERENCES ([^\(]+)\(([^)]+)\)").unwrap();

        let mut issues = Vec::new();

        // Get all the constraints in the table
        let constraints = client
            .query(constraints_query.as_str(), &[])
            .await?
            .iter()
            .map(|row| {
                let contype: i8 = row.get("contype");
                let condef: String = row.get("condef");

                (contype as u8 as char, condef)
            })
            .collect::<Vec<_>>();

        // Filter out primary key constraints to find which columns are primary keys
        let primary_keys = constraints
            .iter()
            .filter(|(contype, _)| *contype == 'p')
            .map(|(_, condef)| primary_key_re.captures_iter(condef).next().unwrap()[1].to_owned())
            .collect::<HashSet<_>>();

        // Filter out foreign key constraints to find which columns require foreign key constraints
        let mut foreign_constraints = HashMap::new();
        for (_, condef) in constraints.iter().filter(|(contype, _)| *contype == 'f') {
            let matches = foreign_key_re.captures_iter(condef).next().unwrap();
            let column_name = matches[1].to_owned(); // name of the column
            let ref_table_name = matches[2].to_owned(); // name of the table the column refers to
            let ref_column_name = matches[3].to_owned(); // name of the column in the referenced table

            let mut column =
                PhysicalColumn::from_db(client, &ref_table_name, &ref_column_name, true, None)
                    .await?;
            issues.append(&mut column.issues);

            if let Some(spec) = column.value {
                foreign_constraints.insert(
                    column_name.clone(),
                    PhysicalColumnType::ColumnReference {
                        ref_table_name: ref_table_name.clone(),
                        ref_column_name: ref_column_name.clone(),
                        ref_pk_type: Box::new(spec.typ),
                    },
                );
            }
        }

        let mut columns = Vec::new();
        for row in client.query(columns_query.as_str(), &[]).await? {
            let name: String = row.get("column_name");
            let mut column = PhysicalColumn::from_db(
                client,
                table_name,
                &name,
                primary_keys.contains(&name),
                foreign_constraints.get(&name).cloned(),
            )
            .await?;
            issues.append(&mut column.issues);

            if let Some(spec) = column.value {
                columns.push(spec);
            }
        }

        Ok(WithIssues {
            value: PhysicalTable {
                name: table_name.to_string(),
                columns,
            },
            issues,
        })
    }

    /// Converts the table specification to SQL statements.
    pub fn creation_sql(&self) -> SchemaStatement {
        let mut post_statements = Vec::new();
        let column_stmts: String = self
            .columns
            .iter()
            .map(|c| {
                let mut s = c.to_sql(&self.name);
                post_statements.append(&mut s.post_statements);
                s.statement
            })
            .collect::<Vec<_>>()
            .join(",\n\t");

        for (unique_constraint_name, columns) in self.named_unique_constraints().iter() {
            let columns_part = columns
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect::<Vec<_>>()
                .join(", ");

            post_statements.push(format!(
                "ALTER TABLE \"{}\" ADD CONSTRAINT \"{}\" UNIQUE ({});",
                self.name, unique_constraint_name, columns_part
            ));
        }

        SchemaStatement {
            statement: format!("CREATE TABLE \"{}\" (\n\t{}\n);", self.name, column_stmts),
            pre_statements: vec![],
            post_statements,
        }
    }

    pub fn deletion_sql(&self) -> SchemaStatement {
        let mut pre_statements = vec![];
        for (unique_constraint_name, _) in self.named_unique_constraints().iter() {
            pre_statements.push(format!(
                "ALTER TABLE \"{}\" DROP CONSTRAINT \"{}\";",
                self.name, unique_constraint_name
            ));
        }

        SchemaStatement {
            statement: format!("DROP TABLE \"{}\";", self.name),
            pre_statements,
            post_statements: vec![],
        }
    }

    /// Get any extensions this table may depend on.
    pub fn get_required_extensions(&self) -> HashSet<String> {
        let mut required_extensions = HashSet::new();

        for col_spec in self.columns.iter() {
            if let PhysicalColumnType::Uuid = col_spec.typ {
                required_extensions.insert("pgcrypto".to_string());
            }
        }

        required_extensions
    }

    fn named_unique_constraints(&self) -> HashMap<&String, Vec<String>> {
        self.columns.iter().fold(HashMap::new(), |mut map, c| {
            {
                for name in c.unique_constraints.iter() {
                    let entry: &mut Vec<String> = map.entry(name).or_insert_with(Vec::new);
                    (*entry).push(c.column_name.clone());
                }
            }
            map
        })
    }
}
