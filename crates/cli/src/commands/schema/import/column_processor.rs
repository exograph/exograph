use anyhow::Result;

use exo_sql::schema::column_spec::{ColumnSpec, ColumnTypeSpec};

use exo_sql::schema::table_spec::TableSpec;
use exo_sql::{FloatBits, IntBits};

use super::{ImportContext, ModelProcessor};

const INDENT: &str = "    ";

impl ModelProcessor<TableSpec> for ColumnSpec {
    /// Converts the column specification to a exograph model.
    fn process(
        &self,
        parent: &TableSpec,
        context: &ImportContext,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<()> {
        // [@pk] [type-annotations] [name]: [data-type] = [default-value]

        let column_type_name = context.type_name(&self.typ);
        let is_column_type_name_reference =
            matches!(column_type_name, ColumnTypeName::ReferenceType(_));

        if let ColumnTypeSpec::ColumnReference(ref reference) = &self.typ {
            // The column was referring to a table, but that table is not in the context
            if !is_column_type_name_reference {
                writeln!(
                    writer,
                    "{INDENT}// NOTE: The table `{}` referenced by this column is not in the provided scope",
                    reference.foreign_table_name.fully_qualified_name()
                )?;
            }
        }

        write!(writer, "{INDENT}")?;

        if self.is_pk {
            write!(writer, "@pk ")?;
        }

        if let ColumnTypeSpec::ColumnReference(ref reference) = &self.typ {
            if parent.name == reference.foreign_table_name {
                let cardinality_annotation =
                    if self.unique_constraints.is_empty() || self.is_nullable {
                        "@manyToOne"
                    } else {
                        "@oneToOne"
                    };
                write!(writer, "{cardinality_annotation} ")?;
            }
        }

        if !self.unique_constraints.is_empty() {
            write!(writer, "@unique ")?;
        }

        let annots = type_annotation(&self.typ);

        if !annots.is_empty() {
            write!(writer, "{} ", annots)?;
        }

        let (standard_field_name, needs_column_annotation) = context.standard_field_naming(self);

        if needs_column_annotation {
            write!(writer, "@column(\"{}\") ", self.name)?;
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

fn type_annotation(column_type: &ColumnTypeSpec) -> String {
    match column_type {
        ColumnTypeSpec::Int { bits } => match bits {
            IntBits::_16 => "@bits16".to_string(),
            IntBits::_32 => "".to_string(),
            IntBits::_64 => "@bits64".to_string(),
        },
        ColumnTypeSpec::Float { bits } => match bits {
            FloatBits::_24 => "@singlePrecision".to_string(),
            FloatBits::_53 => "@doublePrecision".to_string(),
        },
        ColumnTypeSpec::Numeric { precision, scale } => {
            let precision_part = precision.map(|p| format!("@precision({p})"));
            let scale_part = scale.map(|s| format!("@scale({s})"));
            match (precision_part, scale_part) {
                (Some(precision), Some(scale)) => format!("{precision} {scale}"),
                (Some(precision), None) => precision,
                (None, Some(scale)) => scale,
                (None, None) => "".to_string(),
            }
        }
        ColumnTypeSpec::String {
            max_length: Some(max_length),
        } => format!("@maxLength({max_length})"),
        ColumnTypeSpec::String { max_length: None } => "".to_string(),
        ColumnTypeSpec::Timestamp {
            precision: Some(precision),
            ..
        } => format!("@precision({precision})"),
        ColumnTypeSpec::Timestamp {
            precision: None, ..
        } => "".to_string(),
        ColumnTypeSpec::Time {
            precision: Some(precision),
            ..
        } => format!("@precision({precision})"),
        ColumnTypeSpec::Vector { size } => format!("@size({size})"),
        _ => "".to_string(),
    }
}
