use std::collections::{HashMap, HashSet};

use exo_sql::{
    ArrayColumnType, BlobColumnType, BooleanColumnType, DateColumnType, EnumColumnType,
    FloatColumnType, IntColumnType, JsonColumnType, NumericColumnType, SchemaObjectName,
    StringColumnType, TimeColumnType, TimestampColumnType, UuidColumnType, VectorColumnType,
    schema::{
        column_spec::{ColumnReferenceSpec, ColumnSpec},
        database_spec::DatabaseSpec,
    },
};

use heck::{ToLowerCamelCase, ToSnakeCase, ToUpperCamelCase};

use super::column_processor::ColumnTypeName;

pub(super) struct ImportContext<'a> {
    table_name_to_model_name: HashMap<SchemaObjectName, String>,
    pub(super) schemas: HashSet<String>,
    pub(super) database_spec: &'a DatabaseSpec,
    pub(super) query_access: bool,
    pub(super) mutation_access: bool,
    pub(super) generate_fragments: bool,
}

impl<'a> ImportContext<'a> {
    pub(super) fn new(
        database_spec: &'a DatabaseSpec,
        query_access: bool,
        mutation_access: bool,
        generate_fragments: bool,
    ) -> Self {
        Self {
            table_name_to_model_name: HashMap::new(),
            schemas: HashSet::new(),
            database_spec,
            query_access,
            mutation_access,
            generate_fragments,
        }
    }

    pub(super) fn model_name(&self, table_name: &SchemaObjectName) -> Option<&str> {
        self.table_name_to_model_name
            .get(table_name)
            .map(|name| name.as_str())
    }

    pub(super) fn has_standard_mapping(&self, table_name: &SchemaObjectName) -> bool {
        let model_name = self.model_name(table_name);

        let standard_table_name = model_name.map(|model_name| {
            postgres_core_builder::naming::ToTableName::table_name(model_name, None)
        });

        standard_table_name == Some(table_name.name.clone())
    }

    /// Given a table name, returns the standard model name.
    ///
    /// todos -> Todo
    /// news -> News
    pub(super) fn standard_model_name(&self, table_name: &SchemaObjectName) -> String {
        let singular_name = pluralizer::pluralize(&table_name.name, 1, false);
        let model_name = singular_name.to_upper_camel_case();

        // If the model name starts with a digit, prefix it with "m" (since we don't allow model names to start with a digit)
        if model_name.chars().next().unwrap().is_ascii_digit() {
            format!("m{}", model_name)
        } else {
            model_name
        }
    }

    pub fn standard_field_name(&self, column_name: &str) -> String {
        column_name.to_lower_camel_case()
    }

    pub(super) fn get_field_name_and_column_annotation(
        &self,
        column: &ColumnSpec,
    ) -> (String, Option<String>) {
        let (field_name, needs_annotation) = self.standard_field_naming(column, None);
        let annotation = needs_annotation.then(|| format!("@column(\"{}\")", column.name));
        (field_name, annotation)
    }

    pub fn get_composite_foreign_key_field_name(
        &self,
        foreign_table_name: &SchemaObjectName,
    ) -> String {
        if let Some(model_name) = self.model_name(foreign_table_name) {
            model_name.to_lower_camel_case()
        } else {
            foreign_table_name.name.to_lower_camel_case()
        }
    }

    pub(super) fn standard_field_naming(
        &self,
        column: &ColumnSpec,
        reference_spec: Option<&ColumnReferenceSpec>,
    ) -> (String, bool) {
        let column_type_name = self.column_type_name(column, reference_spec);
        let is_column_type_name_reference =
            matches!(column_type_name, ColumnTypeName::ReferenceType(_));

        let (field_name, column_name_from_field_name, field_name_from_column_name) =
            match reference_spec {
                Some(reference) if is_column_type_name_reference => {
                    let field_name = reference_field_name(&column.name, reference); // For example, `sales_region_id` -> `salesRegion`
                    let column_name_from_field_name = format!(
                        "{}_{}",
                        field_name.to_snake_case(),
                        reference.foreign_pk_column_name
                    );
                    let field_name_from_column_name =
                        reference_field_name(&column_name_from_field_name, reference);

                    (
                        field_name,
                        column_name_from_field_name,
                        field_name_from_column_name,
                    )
                }
                _ => {
                    let field_name = self.standard_field_name(&column.name);
                    let column_name_from_field_name = field_name.to_snake_case();
                    let field_name_from_column_name =
                        self.standard_field_name(&column_name_from_field_name);

                    (
                        field_name,
                        column_name_from_field_name,
                        field_name_from_column_name,
                    )
                }
            };

        // An explicit @column annotation is needed if *either* of the following is true:
        //
        // 1. The column name derived from the field name does not match the actual column name.
        //    Example: The column is named `EmployeeId`.
        //      - The importer infers the field name as `employeeId` (lowerCamelCase).
        //      - The builder infers the column name as `employee_id` (snake_case).
        //      - To avoid mismatch, annotate explicitly:
        //        @column("EmployeeId") employeeId: Int
        //
        // 2. The field name derived from the column name does not map back to the original column name.
        //    Example: The column is named `min_30d_price`.
        //      - The standard field name becomes `min30dPrice` (lowerCamelCase).
        //      - But the builder infers `min30d_price` as the column name (snake_case of field name), causing a mismatch.
        //      - This (snake_case(lowerCamelCase(column-name)) != column-name) happens when digits are involved.
        //      - To avoid mismatch, annotate explicitly:
        //        @column("min_30d_price") min30dPrice: Float

        let needs_column_annotation =
            column_name_from_field_name != column.name || field_name_from_column_name != field_name;
        (field_name, needs_column_annotation)
    }

    /// Converts the name of a SQL table to a exograph model name (for example, concert_artist -> ConcertArtist).
    pub(super) fn add_table(&mut self, table_name: &SchemaObjectName) {
        if let Some(schema) = &table_name.schema {
            self.schemas.insert(schema.clone());
        } else {
            self.schemas.insert("public".to_string());
        }

        let model_name = self.standard_model_name(table_name);

        // If the model name is already taken, try adding a number to the end.
        fn create_unique_model_name(
            table_name_to_model_name: &HashMap<SchemaObjectName, String>,
            model_name: &str,
            attempt: u32,
        ) -> String {
            let name_proposal = if attempt == 0 {
                model_name.to_string()
            } else {
                format!("{}{}", model_name, attempt)
            };

            if table_name_to_model_name
                .values()
                .any(|name| name == &name_proposal)
            {
                create_unique_model_name(table_name_to_model_name, model_name, attempt + 1)
            } else {
                name_proposal
            }
        }

        self.table_name_to_model_name.insert(
            table_name.clone(),
            create_unique_model_name(&self.table_name_to_model_name, &model_name, 0),
        );
    }

    pub(super) fn referenced_columns(
        &self,
        table_name: &SchemaObjectName,
    ) -> Vec<(SchemaObjectName, &ColumnSpec)> {
        let other_tables = self
            .database_spec
            .tables
            .iter()
            .filter(|table| &table.name != table_name);

        other_tables
            .map(|other_table| (other_table.name.clone(), &other_table.columns))
            .flat_map(|(other_table_name, other_table_columns)| {
                other_table_columns
                    .iter()
                    .flat_map(move |other_table_column| {
                        if let Some(reference_specs) = &other_table_column.reference_specs {
                            if reference_specs.iter().any(|reference_spec| {
                                &reference_spec.foreign_table_name == table_name
                            }) {
                                Some((other_table_name.clone(), other_table_column))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
            })
            .collect()
    }

    pub(super) fn column_type_name(
        &self,
        column: &ColumnSpec,
        reference_spec: Option<&ColumnReferenceSpec>,
    ) -> ColumnTypeName {
        if let Some(ColumnReferenceSpec {
            foreign_table_name,
            foreign_pk_type,
            ..
        }) = &reference_spec
        {
            let model_name = self.model_name(foreign_table_name);
            match model_name {
                Some(model_name) => ColumnTypeName::ReferenceType(model_name.to_string()),
                None => Self::physical_type_name(foreign_pk_type.as_ref()),
            }
        } else {
            Self::physical_type_name(column.typ.as_ref())
        }
    }

    pub(super) fn physical_type_name(
        physical_type: &dyn exo_sql::PhysicalColumnType,
    ) -> ColumnTypeName {
        let inner_type = physical_type;
        if inner_type.as_any().is::<IntColumnType>() {
            ColumnTypeName::SelfType("Int".to_string())
        } else if inner_type.as_any().is::<FloatColumnType>() {
            ColumnTypeName::SelfType("Float".to_string())
        } else if inner_type.as_any().is::<NumericColumnType>() {
            ColumnTypeName::SelfType("Decimal".to_string())
        } else if inner_type.as_any().is::<StringColumnType>() {
            ColumnTypeName::SelfType("String".to_string())
        } else if inner_type.as_any().is::<BooleanColumnType>() {
            ColumnTypeName::SelfType("Boolean".to_string())
        } else if let Some(timestamp_type) =
            inner_type.as_any().downcast_ref::<TimestampColumnType>()
        {
            ColumnTypeName::SelfType(
                if timestamp_type.timezone {
                    "Instant"
                } else {
                    "LocalDateTime"
                }
                .to_string(),
            )
        } else if inner_type.as_any().is::<TimeColumnType>() {
            ColumnTypeName::SelfType("LocalTime".to_string())
        } else if inner_type.as_any().is::<DateColumnType>() {
            ColumnTypeName::SelfType("LocalDate".to_string())
        } else if inner_type.as_any().is::<JsonColumnType>() {
            ColumnTypeName::SelfType("Json".to_string())
        } else if inner_type.as_any().is::<BlobColumnType>() {
            ColumnTypeName::SelfType("Blob".to_string())
        } else if inner_type.as_any().is::<UuidColumnType>() {
            ColumnTypeName::SelfType("Uuid".to_string())
        } else if inner_type.as_any().is::<VectorColumnType>() {
            ColumnTypeName::SelfType("Vector".to_string())
        } else if let Some(array_type) = inner_type.as_any().downcast_ref::<ArrayColumnType>() {
            match Self::physical_type_name(array_type.typ.as_ref()) {
                ColumnTypeName::SelfType(data_type) => {
                    ColumnTypeName::SelfType(format!("Array<{data_type}>"))
                }
                ColumnTypeName::ReferenceType(data_type) => {
                    ColumnTypeName::ReferenceType(format!("Array<{data_type}>"))
                }
            }
        } else if let Some(enum_type) = inner_type.as_any().downcast_ref::<EnumColumnType>() {
            ColumnTypeName::SelfType(enum_type.enum_name.name.to_upper_camel_case())
        } else {
            ColumnTypeName::SelfType("Unknown".to_string())
        }
    }
}

fn reference_field_name(column_name: &str, reference: &ColumnReferenceSpec) -> String {
    if column_name.ends_with(&format!("_{}", reference.foreign_pk_column_name)) {
        // Drop the trailing underscore and the foreign key column name
        column_name[..column_name.len() - reference.foreign_pk_column_name.len() - 1].to_string()
    } else if column_name.ends_with("id") || column_name.ends_with("Id") {
        // Some databases (for example, a version of "chinook") uses `ArtistId` as the primary key column name and `ArtistId` to refer to this column.
        // Drop the trailing "id" or "Id"
        column_name[..column_name.len() - 2].to_string()
    } else {
        column_name.to_string()
    }
    .to_lower_camel_case()
}
