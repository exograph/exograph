use anyhow::Result;

use exo_sql::schema::column_spec::{ColumnReferenceSpec, ColumnSpec, ColumnTypeSpec};

use exo_sql::{FloatBits, IntBits};

use heck::ToLowerCamelCase;

use exo_sql::schema::issue::Issue;

use super::{ImportContext, ModelProcessor};

impl ModelProcessor for ColumnSpec {
    /// Converts the column specification to a exograph model.
    fn process(
        &self,
        context: &mut ImportContext,
        writer: &mut (dyn std::io::Write + Send),
    ) -> Result<()> {
        // [@pk] [type-annotations] [name]: [data-type] = [default-value]

        let pk_str = if self.is_pk { "@pk " } else { "" };
        write!(writer, "\t\t{}", pk_str)?;
        let (mut data_type, annots) = to_model(&self.typ, context);

        write!(writer, "{}", &annots)?;

        write!(writer, "{}: ", self.name.to_lower_camel_case())?;

        if let ColumnTypeSpec::ColumnReference(ColumnReferenceSpec {
            foreign_table_name, ..
        }) = &self.typ
        {
            // data_type = context.model_name(foreign_table_name);

            context.add_issue(Issue::Hint(format!(
                "consider adding a field to `{}` of type `[{}]` to create a one-to-many relationship",
                foreign_table_name.fully_qualified_name(), data_type,
            )));
        }

        if self.is_nullable {
            data_type += "?"
        }

        let autoinc_str = if self.is_auto_increment {
            " = autoIncrement()"
        } else {
            ""
        };

        writeln!(writer, "{}{}{}", data_type, &annots, autoinc_str)?;

        Ok(())
    }
}

fn to_model(column_type: &ColumnTypeSpec, context: &mut ImportContext) -> (String, String) {
    match column_type {
        ColumnTypeSpec::Int { bits } => (
            "Int".to_string(),
            match bits {
                IntBits::_16 => " @bits16",
                IntBits::_32 => "",
                IntBits::_64 => " @bits64",
            }
            .to_string(),
        ),

        ColumnTypeSpec::Float { bits } => (
            "Float".to_string(),
            match bits {
                FloatBits::_24 => " @singlePrecision",
                FloatBits::_53 => " @doublePrecision",
            }
            .to_owned(),
        ),

        ColumnTypeSpec::Numeric { precision, scale } => ("Numeric".to_string(), {
            let precision_part = precision
                .map(|p| format!("@precision({p})"))
                .unwrap_or_default();

            let scale_part = scale.map(|s| format!("@scale({s})")).unwrap_or_default();

            format!(" {precision_part} {scale_part}")
        }),

        ColumnTypeSpec::String { max_length } => (
            "String".to_string(),
            match max_length {
                Some(max_length) => format!(" @maxLength({max_length})"),
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
                Some(precision) => format!(" @precision({precision})"),
                None => "".to_string(),
            },
        ),

        ColumnTypeSpec::Time { precision } => (
            "LocalTime".to_string(),
            match precision {
                Some(precision) => format!(" @precision({precision})"),
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
            (format!("[{data_type}]"), annotations)
        }

        ColumnTypeSpec::ColumnReference(ColumnReferenceSpec {
            foreign_table_name, ..
        }) => (
            context.model_name(foreign_table_name).to_string(),
            "".to_string(),
        ),
    }
}
