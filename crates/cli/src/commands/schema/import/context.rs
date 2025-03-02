use std::collections::HashMap;

use exo_sql::{schema::database_spec::DatabaseSpec, schema::issue::Issue, PhysicalTableName};

use heck::ToUpperCamelCase;

pub(super) struct ImportContext<'a> {
    table_name_to_model_name: HashMap<PhysicalTableName, String>,
    pub(super) issues: Vec<Issue>,
    pub(super) database_spec: &'a DatabaseSpec,
    pub(super) access: bool,
}

impl<'a> ImportContext<'a> {
    pub(super) fn new(database_spec: &'a DatabaseSpec, access: bool) -> Self {
        Self {
            table_name_to_model_name: HashMap::new(),
            issues: Vec::new(),
            database_spec,
            access,
        }
    }

    pub(super) fn model_name(&self, table_name: &PhysicalTableName) -> &str {
        self.table_name_to_model_name.get(table_name).unwrap()
    }

    pub(super) fn has_standard_mapping(&self, table_name: &PhysicalTableName) -> bool {
        self.model_name(table_name) != table_name.name.to_upper_camel_case()
    }

    /// Converts the name of a SQL table to a exograph model name (for example, concert_artist -> ConcertArtist).
    pub(super) fn add_table(&mut self, table_name: &PhysicalTableName) {
        let singular_name = pluralizer::pluralize(&table_name.name, 1, false);

        // If the singular name is the same (for example, uncountable nouns such as 'news'), use the original name.
        let model_name = if singular_name == table_name.name {
            table_name.name.to_upper_camel_case()
        } else {
            singular_name.to_upper_camel_case()
        };

        self.table_name_to_model_name
            .insert(table_name.clone(), model_name.clone());
    }

    pub(super) fn add_issue(&mut self, issue: Issue) {
        self.issues.push(issue);
    }

    pub(super) fn add_issues(&mut self, issues: &mut Vec<Issue>) {
        self.issues.append(issues);
    }
}
