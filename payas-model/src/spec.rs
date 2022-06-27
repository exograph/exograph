use std::collections::HashSet;

use heck::ToUpperCamelCase;

use payas_sql::spec::{Issue, SchemaSpec, WithIssues};
use payas_sql::{PhysicalColumn, PhysicalColumnType, PhysicalTable};

use crate::model::mapped_arena::SerializableSlab;

pub trait FromModel<T> {
    fn from_model(_: T) -> Self;
}

pub trait ToModel {
    fn to_model(&self) -> WithIssues<String>;
}

/// Converts the name of a SQL table to a claytip model name (for example, concert_artist -> ConcertArtist).
fn to_model_name(name: &str) -> String {
    name.to_upper_camel_case()
}

impl FromModel<SerializableSlab<PhysicalTable>> for SchemaSpec {
    /// Creates a new schema specification from the tables of a claytip model file.
    fn from_model(tables: SerializableSlab<PhysicalTable>) -> Self {
        let table_specs: Vec<_> = tables.into_iter().collect();

        let mut required_extensions = HashSet::new();
        for table_spec in table_specs.iter() {
            required_extensions = required_extensions
                .union(&table_spec.get_required_extensions())
                .cloned()
                .collect();
        }

        SchemaSpec {
            table_specs,
            required_extensions,
        }
    }
}

impl ToModel for SchemaSpec {
    /// Converts the schema specification to a claytip file.
    fn to_model(&self) -> WithIssues<String> {
        let mut issues = Vec::new();
        let stmt = self
            .table_specs
            .iter()
            .map(|table_spec| {
                let mut model = table_spec.to_model();
                issues.append(&mut model.issues);
                format!("{}\n\n", model.value)
            })
            .collect();

        WithIssues {
            value: stmt,
            issues,
        }
    }
}

impl ToModel for PhysicalTable {
    /// Converts the table specification to a claytip model.
    fn to_model(&self) -> WithIssues<String> {
        let mut issues = Vec::new();

        let table_annot = format!("@table(\"{}\")", self.name);
        let column_stmts = self
            .columns
            .iter()
            .map(|c| {
                let mut model = c.to_model();
                issues.append(&mut model.issues);
                format!("  {}\n", model.value)
            })
            .collect::<String>();

        // not a robust check
        if self.name.ends_with('s') {
            issues.push(Issue::Hint(format!(
                "model name `{}` should be changed to singular",
                to_model_name(&self.name)
            )));
        }

        WithIssues {
            value: format!(
                "{}\nmodel {} {{\n{}}}",
                table_annot,
                to_model_name(&self.name),
                column_stmts
            ),
            issues,
        }
    }
}

impl ToModel for PhysicalColumn {
    /// Converts the column specification to a claytip model.
    fn to_model(&self) -> WithIssues<String> {
        let mut issues = Vec::new();

        let pk_str = if self.is_pk { " @pk" } else { "" };
        let autoinc_str = if self.is_autoincrement {
            " = autoincrement()"
        } else {
            ""
        };

        let (mut data_type, annots) = self.typ.to_model();
        if let PhysicalColumnType::ColumnReference { ref_table_name, .. } = &self.typ {
            data_type = to_model_name(&data_type);

            issues.push(Issue::Hint(format!(
                "consider adding a field to `{}` of type `[{}]` to create a one-to-many relationship",
                ref_table_name, to_model_name(&self.table_name),
            )));
        }

        if self.is_nullable {
            data_type += "?"
        }

        WithIssues {
            value: format!(
                "{}: {}{}{}",
                self.column_name,
                data_type + &annots,
                autoinc_str,
                pk_str,
            ),
            issues: Vec::new(),
        }
    }
}
