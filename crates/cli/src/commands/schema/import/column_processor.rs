use std::collections::HashSet;

use anyhow::Result;

use exo_sql::schema::column_spec::{ColumnReferenceSpec, ColumnSpec};

use exo_sql::schema::database_spec::DatabaseSpec;
use exo_sql::schema::table_spec::TableSpec;
use exo_sql::{
    FloatBits, FloatColumnType, IntBits, IntColumnType, NumericColumnType, StringColumnType,
    TimeColumnType, TimestampColumnType, VectorColumnType,
};

use super::{ImportContext, ModelProcessor};

const INDENT: &str = "    ";

impl ModelProcessor<TableSpec, HashSet<String>> for ColumnSpec {
    /// Converts the column specification to a exograph model.
    /// The HashSet<String> parameter tracks written field names to prevent duplicates
    /// when multiple foreign keys share the same column.
    fn process(
        &self,
        _parent: &TableSpec,
        context: &ImportContext,
        parent_context: &mut HashSet<String>,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<()> {
        if self.reference_specs.is_some() {
            return Ok(());
        }

        let (standard_field_name, column_annotation) =
            context.get_field_name_and_column_annotation(self);

        // Skip if we've already written this field (prevents duplicates when
        // multiple foreign keys share the same column)
        if !parent_context.insert(standard_field_name.clone()) {
            return Ok(());
        }
        // [@pk] [type-annotations] [name]: [data-type] = [default-value]
        let column_type_name = context.column_type_name(
            self,
            self.reference_specs
                .as_deref()
                .and_then(|specs| specs.first()),
        );

        let data_type = match column_type_name {
            ColumnTypeName::SelfType(data_type) => data_type,
            ColumnTypeName::ReferenceType(data_type) => data_type,
        };

        // Combine type annotations and column annotations
        let mut all_annotations = Vec::new();

        // Only add type annotations for non-reference columns
        if self.reference_specs.is_none() {
            let type_annots = type_annotation(self.typ.as_ref());
            if !type_annots.is_empty() {
                all_annotations.push(type_annots);
            }
        }

        if let Some(col_annot) = column_annotation {
            all_annotations.push(col_annot);
        }

        let combined_annotations = if all_annotations.is_empty() {
            None
        } else {
            Some(all_annotations.join(" "))
        };

        // Write the field
        let default_value = self.default_value.as_ref().and_then(|v| v.to_model());
        let field_spec = FieldSpec {
            is_pk: self.is_pk,
            is_unique: !self.unique_constraints.is_empty(),
            field_name: &standard_field_name,
            data_type: &data_type,
            is_nullable: self.is_nullable,
            annotations: combined_annotations.as_deref(),
            default_value: default_value.as_deref(),
        };
        write_field_common(writer, &field_spec)?;

        Ok(())
    }
}

pub enum ColumnTypeName {
    SelfType(String),
    ReferenceType(String),
}

fn type_annotation(physical_type: &dyn exo_sql::PhysicalColumnType) -> String {
    let inner_type = physical_type;
    if let Some(int_type) = inner_type.as_any().downcast_ref::<IntColumnType>() {
        match int_type.bits {
            IntBits::_16 => "@bits16".to_string(),
            IntBits::_32 => "".to_string(),
            IntBits::_64 => "@bits64".to_string(),
        }
    } else if let Some(float_type) = inner_type.as_any().downcast_ref::<FloatColumnType>() {
        match float_type.bits {
            FloatBits::_24 => "@singlePrecision".to_string(),
            FloatBits::_53 => "@doublePrecision".to_string(),
        }
    } else if let Some(numeric_type) = inner_type.as_any().downcast_ref::<NumericColumnType>() {
        let precision_part = numeric_type.precision.map(|p| format!("@precision({p})"));
        let scale_part = numeric_type.scale.map(|s| format!("@scale({s})"));
        match (precision_part, scale_part) {
            (Some(precision), Some(scale)) => format!("{precision} {scale}"),
            (Some(precision), None) => precision,
            (None, Some(scale)) => scale,
            (None, None) => "".to_string(),
        }
    } else if let Some(string_type) = inner_type.as_any().downcast_ref::<StringColumnType>() {
        match string_type.max_length {
            Some(max_length) => format!("@maxLength({max_length})"),
            None => "".to_string(),
        }
    } else if let Some(timestamp_type) = inner_type.as_any().downcast_ref::<TimestampColumnType>() {
        match timestamp_type.precision {
            Some(precision) => format!("@precision({precision})"),
            None => "".to_string(),
        }
    } else if let Some(time_type) = inner_type.as_any().downcast_ref::<TimeColumnType>() {
        match time_type.precision {
            Some(precision) => format!("@precision({precision})"),
            None => "".to_string(),
        }
    } else if let Some(vector_type) = inner_type.as_any().downcast_ref::<VectorColumnType>() {
        format!("@size({})", vector_type.size)
    } else {
        "".to_string()
    }
}

struct FieldSpec<'a> {
    field_name: &'a str,
    data_type: &'a str,
    is_pk: bool,
    is_unique: bool,
    is_nullable: bool,
    annotations: Option<&'a str>,
    default_value: Option<&'a str>,
}

fn write_field_common(writer: &mut (dyn std::io::Write + Send), spec: &FieldSpec) -> Result<()> {
    write!(writer, "{INDENT}")?;

    if spec.is_pk {
        write!(writer, "@pk ")?;
    }

    if spec.is_unique {
        write!(writer, "@unique ")?;
    }

    if let Some(annots) = spec.annotations {
        write!(writer, "{annots} ")?;
    }

    write!(writer, "{}: {}", spec.field_name, spec.data_type)?;

    if spec.is_nullable {
        write!(writer, "?")?;
    }

    if let Some(default) = spec.default_value {
        write!(writer, " = {default}")?;
    }

    writeln!(writer)?;
    Ok(())
}

pub fn write_foreign_key_reference(
    writer: &mut (dyn std::io::Write + Send),
    context: &ImportContext,
    database_spec: &DatabaseSpec,
    table_spec: &TableSpec,
    processed_fields: &mut HashSet<String>,
    filter: &dyn Fn(&ColumnSpec) -> bool,
) -> Result<()> {
    for (_, references) in table_spec.foreign_key_references() {
        if references.is_empty() {
            continue;
        }

        if !filter(references[0].0) {
            continue;
        }

        if !processed_fields.insert(references[0].0.name.clone()) {
            continue;
        }

        let reference = references[0].1; // All references point to the same table
        let foreign_table_name = &reference.foreign_table_name;
        let field_name = context.get_composite_foreign_key_field_name(foreign_table_name);
        let data_type = context
            .model_name(foreign_table_name)
            .ok_or(anyhow::anyhow!(
                "No model name found for foreign table name: {:?}",
                foreign_table_name
            ))?;

        let mapping_annotation =
            reference_mapping_annotation(&field_name, &references, database_spec, context);

        let is_pk = references.iter().all(|(col, _)| col.is_pk);
        let is_unique = references
            .iter()
            .all(|(col, _)| !col.unique_constraints.is_empty());
        let is_nullable = references.iter().any(|(col, _)| col.is_nullable);

        let field_spec = FieldSpec {
            is_pk,
            is_unique,
            field_name: &field_name,
            data_type,
            is_nullable,
            annotations: mapping_annotation.as_deref(),
            default_value: None, // Foreign key references don't have default values
        };
        write_field_common(writer, &field_spec)?;
    }

    Ok(())
}

fn reference_mapping_annotation(
    field_name: &str,
    references: &Vec<(&ColumnSpec, &ColumnReferenceSpec)>,
    database_spec: &DatabaseSpec,
    context: &ImportContext,
) -> Option<String> {
    let mut mapping_pairs = Vec::new();

    for (col, reference) in references {
        let foreign_table = database_spec
            .tables
            .iter()
            .find(|t| t.name == reference.foreign_table_name)
            .unwrap();
        let foreign_column_spec = foreign_table
            .columns
            .iter()
            .find(|c| c.name == reference.foreign_pk_column_name)
            .unwrap();

        let (foreign_field_name, needs_mapping) = match &foreign_column_spec.reference_specs {
            Some(foreign_reference_specs) => {
                let name = context.get_composite_foreign_key_field_name(
                    &foreign_reference_specs[0].foreign_table_name,
                );
                let needs_mapping = name != col.name;
                (name, needs_mapping)
            }
            None => {
                let name = context.standard_field_name(&reference.foreign_pk_column_name);
                let default_field_name =
                    format!("{field_name}_{}", reference.foreign_pk_column_name);

                let needs_mapping = default_field_name != col.name;
                (name, needs_mapping)
            }
        };

        if needs_mapping {
            mapping_pairs.push((foreign_field_name, col.name.clone()));
        }
    }

    match &mapping_pairs[..] {
        [] => None,
        [mapping_pair] => {
            let mapping_annotation = format!("@column(\"{}\")", mapping_pair.1);
            Some(mapping_annotation)
        }
        _ => {
            let mapping_annotation = format!(
                "@column(mapping={{{}}})",
                mapping_pairs
                    .iter()
                    .map(|(k, v)| format!("{}: \"{}\"", k, v))
                    .collect::<Vec<String>>()
                    .join(", ")
            );
            Some(mapping_annotation)
        }
    }
}
