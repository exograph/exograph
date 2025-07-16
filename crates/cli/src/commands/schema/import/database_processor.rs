use anyhow::Result;
use std::io::Write;

use exo_sql::schema::database_spec::DatabaseSpec;
use heck::ToUpperCamelCase;

use super::{
    ImportContext,
    traits::{ImportWriter, ModelImporter},
};

#[derive(Debug)]
pub struct DatabaseImport {
    pub modules: Vec<ModuleImport>,
}

#[derive(Debug)]
pub struct ModuleImport {
    pub schema: Option<String>,
    pub module_name: String,
    pub is_fragment: bool,
    pub tables: Vec<super::table_processor::TableImport>,
    pub enums: Vec<super::enum_processor::EnumImport>,
}

impl ModelImporter<(), DatabaseImport> for DatabaseSpec {
    fn to_import(&self, _parent: &(), context: &ImportContext) -> Result<DatabaseImport> {
        let mut schemas = context.schemas.iter().collect::<Vec<_>>();
        schemas.sort(); // Sort schemas to ensure consistent output

        let mut modules = Vec::new();

        for schema in schemas {
            let schema = if schema == "public" {
                None
            } else {
                Some(schema.clone())
            };

            let module_suffix = if context.generate_fragments {
                "Fragments"
            } else {
                "Database"
            };

            let module_name = match &schema {
                Some(schema) => format!("{}{}", schema.to_upper_camel_case(), module_suffix),
                None => module_suffix.to_string(),
            };

            let matching_tables = self
                .tables
                .iter()
                .filter(|table| table.name.schema == schema)
                .collect::<Vec<_>>();

            let mut tables = Vec::new();
            for table in matching_tables {
                tables.push(table.to_import(self, context)?);
            }

            let matching_enums = self
                .enums
                .iter()
                .filter(|enum_| enum_.name.schema == schema)
                .collect::<Vec<_>>();

            let mut enums = Vec::new();
            for enum_ in matching_enums {
                enums.push(enum_.to_import(self, context)?);
            }

            modules.push(ModuleImport {
                schema: schema.clone(),
                module_name,
                is_fragment: context.generate_fragments,
                tables,
                enums,
            });
        }

        Ok(DatabaseImport { modules })
    }
}

impl ImportWriter for DatabaseImport {
    fn write_to(self, writer: &mut (dyn Write + Send)) -> Result<()> {
        let modules_len = self.modules.len();
        for (i, module) in self.modules.into_iter().enumerate() {
            module.write_to(writer)?;

            // Add newline between modules
            if i < modules_len - 1 {
                writeln!(writer)?;
            }
        }
        Ok(())
    }
}

impl ImportWriter for ModuleImport {
    fn write_to(self, writer: &mut (dyn Write + Send)) -> Result<()> {
        // Write @postgres annotation
        write!(writer, "@postgres")?;
        if let Some(schema) = &self.schema {
            if !self.is_fragment {
                write!(writer, "(schema=\"{schema}\")")?;
            }
        }
        writeln!(writer)?;

        // Write module declaration
        writeln!(writer, "module {} {{", self.module_name)?;

        // Write tables in alphabetical order
        let mut tables = self.tables;
        tables.sort_by_cached_key(|a| a.name.to_upper_camel_case());

        let table_len = tables.len();
        for (i, table) in tables.into_iter().enumerate() {
            table.write_to(writer)?;
            if i < table_len - 1 {
                writeln!(writer)?;
            }
        }

        // Write enums in alphabetical order
        let mut enums = self.enums;
        enums.sort_by_cached_key(|a| a.name.to_upper_camel_case());

        // Write enums
        if !enums.is_empty() {
            writeln!(writer)?;
        }

        let enum_len = enums.len();
        for (i, enum_) in enums.into_iter().enumerate() {
            enum_.write_to(writer)?;
            if i < enum_len - 1 {
                writeln!(writer)?;
            }
        }

        writeln!(writer, "}}")?;

        Ok(())
    }
}
