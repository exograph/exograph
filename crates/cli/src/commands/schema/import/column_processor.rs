use anyhow::Result;

use exo_sql::schema::column_spec::{ColumnReferenceSpec, ColumnSpec, ColumnTypeSpec};

use exo_sql::schema::table_spec::TableSpec;
use exo_sql::{FloatBits, IntBits};

use super::context::reference_field_name;
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

        let column_type_name = type_name(&self.typ, context);
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

        let standard_column_name = context.standard_column_name(self);

        if standard_column_name != self.name {
            write!(writer, "@column(\"{}\") ", self.name)?;
        }

        match &self.typ {
            // If the column references to a table and that table is in the context, we use the reference field name (for example `venue_id` -> `venue`)
            ColumnTypeSpec::ColumnReference(ref reference) if is_column_type_name_reference => {
                write!(writer, "{}: ", reference_field_name(self, reference))?;
            }
            _ => {
                let standard_field_name = context.standard_field_name(&self.name);
                write!(writer, "{}: ", standard_field_name)?;
            }
        }

        let data_type = match column_type_name {
            ColumnTypeName::SelfType(data_type) => data_type,
            ColumnTypeName::ReferenceType(data_type) => data_type,
        };

        write!(writer, "{}", data_type)?;

        if self.is_nullable {
            write!(writer, "?")?;
        }

        if self.is_auto_increment {
            write!(writer, " = autoIncrement()")?
        } else if let Some(default_value) = &self.model_default_value() {
            write!(writer, " = {default_value}")?;
        }

        writeln!(writer)?;

        Ok(())
    }
}

enum ColumnTypeName {
    SelfType(String),
    ReferenceType(String),
}

fn type_name(column_type: &ColumnTypeSpec, context: &ImportContext) -> ColumnTypeName {
    match column_type {
        ColumnTypeSpec::Int { .. } => ColumnTypeName::SelfType("Int".to_string()),
        ColumnTypeSpec::Float { .. } => ColumnTypeName::SelfType("Float".to_string()),
        ColumnTypeSpec::Numeric { .. } => ColumnTypeName::SelfType("Decimal".to_string()),
        ColumnTypeSpec::String { .. } => ColumnTypeName::SelfType("String".to_string()),
        ColumnTypeSpec::Boolean => ColumnTypeName::SelfType("Boolean".to_string()),
        ColumnTypeSpec::Timestamp { timezone, .. } => ColumnTypeName::SelfType(
            if *timezone {
                "Instant"
            } else {
                "LocalDateTime"
            }
            .to_string(),
        ),
        ColumnTypeSpec::Time { .. } => ColumnTypeName::SelfType("LocalTime".to_string()),
        ColumnTypeSpec::Date => ColumnTypeName::SelfType("LocalDate".to_string()),
        ColumnTypeSpec::Json => ColumnTypeName::SelfType("Json".to_string()),
        ColumnTypeSpec::Blob => ColumnTypeName::SelfType("Blob".to_string()),
        ColumnTypeSpec::Uuid => ColumnTypeName::SelfType("Uuid".to_string()),
        ColumnTypeSpec::Vector { .. } => ColumnTypeName::SelfType("Vector".to_string()),
        ColumnTypeSpec::Array { typ } => match type_name(typ, context) {
            ColumnTypeName::SelfType(data_type) => {
                ColumnTypeName::SelfType(format!("Array<{data_type}>"))
            }
            ColumnTypeName::ReferenceType(data_type) => {
                ColumnTypeName::ReferenceType(format!("Array<{data_type}>"))
            }
        },
        ColumnTypeSpec::ColumnReference(ColumnReferenceSpec {
            foreign_table_name,
            foreign_pk_type,
            ..
        }) => {
            let model_name = context.model_name(foreign_table_name);
            match model_name {
                Some(model_name) => ColumnTypeName::ReferenceType(model_name.to_string()),
                None => type_name(foreign_pk_type, context),
            }
        }
    }
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
