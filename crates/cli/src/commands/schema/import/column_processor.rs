use anyhow::Result;

use exo_sql::schema::column_spec::{ColumnSpec, ColumnTypeSpec};

use exo_sql::schema::table_spec::TableSpec;
use exo_sql::{
    FloatBits, FloatColumnType, IntBits, IntColumnType, NumericColumnType, PhysicalColumnTypeExt,
    StringColumnType, TimeColumnType, TimestampColumnType, VectorColumnType,
};

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

        if let ColumnTypeSpec::Reference(reference) = &self.typ {
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

        if let ColumnTypeSpec::Reference(reference) = &self.typ {
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
        ColumnTypeSpec::Direct(physical_type) => {
            let inner_type = physical_type.inner();
            match inner_type {
                x if x.as_any().is::<IntColumnType>() => {
                    let int_type = x.as_any().downcast_ref::<IntColumnType>().unwrap();
                    match int_type.bits {
                        IntBits::_16 => "@bits16".to_string(),
                        IntBits::_32 => "".to_string(),
                        IntBits::_64 => "@bits64".to_string(),
                    }
                }
                x if x.as_any().is::<FloatColumnType>() => {
                    let float_type = x.as_any().downcast_ref::<FloatColumnType>().unwrap();
                    match float_type.bits {
                        FloatBits::_24 => "@singlePrecision".to_string(),
                        FloatBits::_53 => "@doublePrecision".to_string(),
                    }
                }
                x if x.as_any().is::<NumericColumnType>() => {
                    let numeric_type = x.as_any().downcast_ref::<NumericColumnType>().unwrap();
                    let precision_part = numeric_type.precision.map(|p| format!("@precision({p})"));
                    let scale_part = numeric_type.scale.map(|s| format!("@scale({s})"));
                    match (precision_part, scale_part) {
                        (Some(precision), Some(scale)) => format!("{precision} {scale}"),
                        (Some(precision), None) => precision,
                        (None, Some(scale)) => scale,
                        (None, None) => "".to_string(),
                    }
                }
                x if x.as_any().is::<StringColumnType>() => {
                    let string_type = x.as_any().downcast_ref::<StringColumnType>().unwrap();
                    match string_type.max_length {
                        Some(max_length) => format!("@maxLength({max_length})"),
                        None => "".to_string(),
                    }
                }
                x if x.as_any().is::<TimestampColumnType>() => {
                    let timestamp_type = x.as_any().downcast_ref::<TimestampColumnType>().unwrap();
                    match timestamp_type.precision {
                        Some(precision) => format!("@precision({precision})"),
                        None => "".to_string(),
                    }
                }
                x if x.as_any().is::<TimeColumnType>() => {
                    let time_type = x.as_any().downcast_ref::<TimeColumnType>().unwrap();
                    match time_type.precision {
                        Some(precision) => format!("@precision({precision})"),
                        None => "".to_string(),
                    }
                }
                x if x.as_any().is::<VectorColumnType>() => {
                    let vector_type = x.as_any().downcast_ref::<VectorColumnType>().unwrap();
                    format!("@size({})", vector_type.size)
                }
                _ => "".to_string(),
            }
        }
        _ => "".to_string(),
    }
}
