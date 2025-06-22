use std::collections::HashSet;

use anyhow::Result;

use exo_sql::schema::column_spec::{ColumnReferenceSpec, ColumnSpec};

use exo_sql::schema::table_spec::TableSpec;
use exo_sql::{
    FloatBits, FloatColumnType, IntBits, IntColumnType, NumericColumnType, StringColumnType,
    TimeColumnType, TimestampColumnType, VectorColumnType,
};

use super::{ImportContext, ModelProcessor};

const INDENT: &str = "    ";

impl ModelProcessor<TableSpec, HashSet<String>> for ColumnSpec {
    /// Converts the column specification to a exograph model.
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

        if !parent_context.insert(standard_field_name.clone()) {
            return Ok(());
        }
        // [@pk] [type-annotations] [name]: [data-type] = [default-value]
        let column_type_name = context.column_type_name(self, None);

        write!(writer, "{INDENT}")?;

        if self.is_pk {
            write!(writer, "@pk ")?;
        }

        if !self.unique_constraints.is_empty() {
            write!(writer, "@unique ")?;
        }

        // Only add type annotations for non-reference columns
        if self.reference_specs.is_none() {
            let annots = type_annotation(self.typ.as_ref());

            if !annots.is_empty() {
                write!(writer, "{} ", annots)?;
            }
        }

        if let Some(annotation) = column_annotation {
            write!(writer, "{} ", annotation)?;
        }

        write!(writer, "{}: ", standard_field_name)?;

        let data_type = match column_type_name {
            ColumnTypeName::SelfType(data_type) => data_type,
            ColumnTypeName::ReferenceType(data_type) => data_type,
        };

        write!(writer, "{}", data_type)?;

        if self.is_nullable {
            write!(writer, "?")?;
        }

        if let Some(default_value) = self.default_value.as_ref().and_then(|v| v.to_model()) {
            write!(writer, " = {default_value}")?;
        }

        writeln!(writer)?;

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

pub fn write_foreign_key_reference(
    writer: &mut (dyn std::io::Write + Send),
    context: &ImportContext,
    table_spec: &TableSpec,
) -> Result<()> {
    for (_, references) in table_spec.foreign_key_references() {
        let reference = references[0].1; // All references point to the same table
        let foreign_table_name = &reference.foreign_table_name;
        let field_name = context.get_composite_foreign_key_field_name(foreign_table_name);
        let column_type_name = {
            let model_name = context.model_name(foreign_table_name);
            match model_name {
                Some(model_name) => ColumnTypeName::ReferenceType(model_name.to_string()),
                None => ImportContext::physical_type_name(reference.foreign_pk_type.as_ref()),
            }
        };
        let data_type = match column_type_name {
            ColumnTypeName::SelfType(data_type) => data_type,
            ColumnTypeName::ReferenceType(data_type) => data_type,
        };

        let mapping_annotation = reference_mapping_annotation(&field_name, &references, context);

        write!(writer, "{INDENT}")?;

        if references[0].0.is_pk {
            write!(writer, "@pk ")?;
        }

        if let Some(mapping_annotation) = mapping_annotation {
            write!(writer, "{mapping_annotation} {field_name}: {data_type}")?;
        } else {
            write!(writer, "{field_name}: {data_type}")?;
        }

        if references[0].0.is_nullable {
            writeln!(writer, "?")?;
        } else {
            writeln!(writer)?;
        }
    }

    Ok(())
}

fn reference_mapping_annotation(
    field_name: &str,
    references: &Vec<(&ColumnSpec, &ColumnReferenceSpec)>,
    context: &ImportContext,
) -> Option<String> {
    let mut mapping_pairs = Vec::new();

    for (col, reference) in references {
        let reference_field_name = context.standard_field_name(&reference.foreign_pk_column_name);

        let standard_field_name = format!("{field_name}_{}", reference.foreign_pk_column_name);

        if standard_field_name != col.name || references.len() > 1 {
            mapping_pairs.push((reference_field_name, col.name.clone()));
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
