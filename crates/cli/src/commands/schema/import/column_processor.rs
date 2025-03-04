use anyhow::Result;

use exo_sql::schema::column_spec::{ColumnReferenceSpec, ColumnSpec, ColumnTypeSpec};

use exo_sql::{FloatBits, IntBits};

use heck::ToLowerCamelCase;

use super::context::reference_field_name;
use super::{ImportContext, ModelProcessor};

const INDENT: &str = "    ";

impl ModelProcessor for ColumnSpec {
    /// Converts the column specification to a exograph model.
    fn process(
        &self,
        context: &ImportContext,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<()> {
        // [@pk] [type-annotations] [name]: [data-type] = [default-value]

        write!(writer, "{INDENT}")?;

        if self.is_pk {
            write!(writer, "@pk ")?;
        }

        if !self.unique_constraints.is_empty() {
            write!(writer, "@unique ")?;
        }

        let (data_type, annots) = to_model(&self.typ, context);
        if !annots.is_empty() {
            write!(writer, "{} ", &annots)?;
        }

        if let ColumnTypeSpec::ColumnReference(ref reference) = &self.typ {
            write!(writer, "{}: ", reference_field_name(self, reference))?;
        } else {
            write!(writer, "{}: ", self.name.to_lower_camel_case())?;
        }

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

fn to_model(column_type: &ColumnTypeSpec, context: &ImportContext) -> (String, String) {
    match column_type {
        ColumnTypeSpec::Int { bits } => (
            "Int".to_string(),
            match bits {
                IntBits::_16 => "@bits16",
                IntBits::_32 => "",
                IntBits::_64 => "@bits64",
            }
            .to_string(),
        ),

        ColumnTypeSpec::Float { bits } => (
            "Float".to_string(),
            match bits {
                FloatBits::_24 => "@singlePrecision",
                FloatBits::_53 => "@doublePrecision",
            }
            .to_owned(),
        ),

        ColumnTypeSpec::Numeric { precision, scale } => ("Decimal".to_string(), {
            let precision_part = precision.map(|p| format!("@precision({p})"));

            let scale_part = scale.map(|s| format!("@scale({s})"));

            match (precision_part, scale_part) {
                (Some(precision), Some(scale)) => format!("{precision} {scale}"),
                (Some(precision), None) => precision,
                (None, Some(scale)) => scale,
                (None, None) => "".to_string(),
            }
        }),

        ColumnTypeSpec::String { max_length } => (
            "String".to_string(),
            match max_length {
                Some(max_length) => format!("@maxLength({max_length})"),
                None => "".to_string(),
            },
        ),

        ColumnTypeSpec::Boolean => ("Boolean".to_string(), "".to_string()),

        ColumnTypeSpec::Timestamp {
            timezone,
            precision,
        } => (
            if *timezone {
                "Instant"
            } else {
                "LocalDateTime"
            }
            .to_string(),
            match precision {
                Some(precision) => format!("@precision({precision})"),
                None => "".to_string(),
            },
        ),

        ColumnTypeSpec::Time { precision } => (
            "LocalTime".to_string(),
            match precision {
                Some(precision) => format!("@precision({precision})"),
                None => "".to_string(),
            },
        ),

        ColumnTypeSpec::Date => ("LocalDate".to_string(), "".to_string()),

        ColumnTypeSpec::Json => ("Json".to_string(), "".to_string()),
        ColumnTypeSpec::Blob => ("Blob".to_string(), "".to_string()),
        ColumnTypeSpec::Uuid => ("Uuid".to_string(), "".to_string()),
        ColumnTypeSpec::Vector { size } => ("Vector".to_string(), format!("@size({size})",)),

        ColumnTypeSpec::Array { typ } => {
            let (data_type, annotations) = to_model(typ, context);
            (format!("Array<{data_type}>"), annotations)
        }

        ColumnTypeSpec::ColumnReference(ColumnReferenceSpec {
            foreign_table_name, ..
        }) => (
            context.model_name(foreign_table_name).to_string(),
            "".to_string(),
        ),
    }
}
