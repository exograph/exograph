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
select routine_name, routine_definition, external_language from information_schema.routines where routine_name like 'exograph%'
"#;

impl FunctionSpec {
    pub fn new(name: String, body: String, language: String) -> Self {
        Self {
            name,
            body,
            language,
        }
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
