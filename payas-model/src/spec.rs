use heck::CamelCase;
use id_arena::Arena;
use payas_sql::spec::{ColumnSpec, Issue, SchemaSpec, TableSpec, WithIssues};
use payas_sql::sql::column::{PhysicalColumn, PhysicalColumnType};
use payas_sql::sql::PhysicalTable;

pub trait FromModel<T> {
    fn from_model(_: T) -> Self;
}

pub trait ToModel {
    fn to_model(&self) -> WithIssues<String>;
}

/// Converts the name of a SQL table to a claytip model name (PascalCase).
fn to_model_name(name: &str) -> String {
    name.to_camel_case() // PascalCase is called CamelCase in heck
}

impl FromModel<Arena<PhysicalTable>> for SchemaSpec {
    /// Creates a new schema specification from the tables of a claytip model file.
    fn from_model(tables: Arena<PhysicalTable>) -> Self {
        let table_specs: Vec<_> = tables
            .iter()
            .map(|(_, table)| TableSpec::from_model(table))
            .collect();

        SchemaSpec { table_specs }
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

impl FromModel<&PhysicalTable> for TableSpec {
    /// Creates a new table specification from a claytip model.
    fn from_model(table: &PhysicalTable) -> Self {
        let column_specs: Vec<_> = table.columns.iter().map(ColumnSpec::from_model).collect();

        TableSpec {
            name: table.name.clone(),
            column_specs,
        }
    }
}

impl ToModel for TableSpec {
    /// Converts the table specification to a claytip model.
    fn to_model(&self) -> WithIssues<String> {
        let mut issues = Vec::new();

        let table_annot = format!("@table(\"{}\")", self.name);
        let column_stmts = self
            .column_specs
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

impl FromModel<&PhysicalColumn> for ColumnSpec {
    /// Creates a new column specification from a claytip model field.
    fn from_model(column: &PhysicalColumn) -> ColumnSpec {
        ColumnSpec {
            table_name: column.table_name.clone(),
            column_name: column.column_name.clone(),
            db_type: column.typ.clone(),
            is_pk: column.is_pk,
            is_autoincrement: column.is_autoincrement,
        }
    }
}

impl ToModel for ColumnSpec {
    /// Converts the column specification to a claytip model.
    fn to_model(&self) -> WithIssues<String> {
        let mut issues = Vec::new();

        let pk_str = if self.is_pk { " @pk" } else { "" };
        let autoinc_str = if self.is_autoincrement {
            " @autoincrement"
        } else {
            ""
        };

        let (mut data_type, annots) = self.db_type.to_model();
        if let PhysicalColumnType::ColumnReference { ref_table_name, .. } = &self.db_type {
            data_type = to_model_name(&data_type);

            issues.push(Issue::Hint(format!(
                "consider adding a field to `{}` of type `[{}]` to create a one-to-many relationship",
                ref_table_name, to_model_name(&self.table_name),
            )));
        }

        WithIssues {
            value: format!(
                "{}: {}{}{}",
                self.column_name,
                data_type + &annots,
                pk_str,
                autoinc_str
            ),
            issues: Vec::new(),
        }
    }
}
