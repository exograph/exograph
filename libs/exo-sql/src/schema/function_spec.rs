use std::vec;

use crate::{database_error::DatabaseError, sql::connect::database_client::DatabaseClient};

use super::{issue::WithIssues, op::SchemaOp};

#[derive(Debug, Clone)]
pub struct FunctionSpec {
    pub name: String,
    pub body: String,
    pub language: String,
}

const FUNCTIONS_QUERY: &str = r#"
SELECT routine_name, routine_definition, external_language FROM information_schema.routines WHERE routine_name like 'exograph%'
"#;

impl FunctionSpec {
    pub fn new(name: String, body: String, language: String) -> Self {
        Self {
            name,
            body,
            language,
        }
    }

    pub fn debug_print(&self, indent: usize) {
        self.debug_print_to(&mut std::io::stdout(), indent).unwrap();
    }

    pub fn debug_print_to<W: std::io::Write>(
        &self,
        writer: &mut W,
        indent: usize,
    ) -> std::io::Result<()> {
        let indent_str = " ".repeat(indent);
        writeln!(writer, "{}- Function:", indent_str)?;
        writeln!(writer, "{}  - Name: {}", indent_str, self.name)?;
        writeln!(writer, "{}  - Language: {}", indent_str, self.language)?;
        // Optionally show body preview
        let body_preview = if self.body.len() > 50 {
            // Safe UTF-8 truncation - find the last valid character boundary
            let mut truncate_at = 50;
            while !self.body.is_char_boundary(truncate_at) && truncate_at > 0 {
                truncate_at -= 1;
            }
            format!("{}...", &self.body[..truncate_at])
        } else {
            self.body.clone()
        };
        writeln!(
            writer,
            "{}  - Body: {}",
            indent_str,
            body_preview.replace('\n', " ")
        )
    }

    pub async fn from_live_db(
        client: &DatabaseClient,
    ) -> Result<WithIssues<Vec<FunctionSpec>>, DatabaseError> {
        let functions: Vec<_> = client
            .query(FUNCTIONS_QUERY, &[])
            .await?
            .iter()
            .map(|row| {
                let name: String = row.get("routine_name");
                let body: String = row.get("routine_definition");
                let language: String = row.get("external_language");

                FunctionSpec {
                    name,
                    body: body.trim().to_string(),
                    language: language.to_lowercase(),
                }
            })
            .collect();

        Ok(WithIssues {
            value: functions,
            issues: vec![],
        })
    }

    pub fn diff<'a>(&'a self, new: &'a Self) -> Vec<SchemaOp<'a>> {
        if self.body != new.body || self.language != new.language {
            vec![SchemaOp::CreateOrReplaceFunction { function: new }]
        } else {
            vec![]
        }
    }

    pub fn creation_sql(&self, replace: bool) -> String {
        // CREATE FUNCTION exograph_update_todo()
        // RETURNS TRIGGER AS $$
        // BEGIN
        //     NEW.updated_at = now();
        //     RETURN NEW;
        // END;
        // $$ language 'plpgsql';
        format!(
            "CREATE{} FUNCTION {}() RETURNS TRIGGER AS $$ {} $$ language '{}';",
            if replace { " OR REPLACE" } else { "" },
            self.name,
            self.body,
            self.language
        )
    }
}
