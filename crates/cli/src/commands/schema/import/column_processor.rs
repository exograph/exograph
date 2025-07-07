use anyhow::Result;
use std::io::Write;

use exo_sql::schema::column_spec::ColumnSpec;
use exo_sql::schema::table_spec::TableSpec;
use exo_sql::{
    FloatBits, FloatColumnType, IntBits, IntColumnType, NumericColumnType, StringColumnType,
    TimeColumnType, TimestampColumnType, VectorColumnType,
};

use super::{
    ImportContext,
    traits::{ImportWriter, ModelImporter},
};

#[derive(Debug)]
pub struct FieldImport {
    pub name: String,
    pub data_type: String,
    pub is_pk: bool,
    pub is_unique: bool,
    pub is_nullable: bool,
    pub annotations: Vec<String>,
    pub default_value: Option<String>,
}

impl ModelImporter<TableSpec, FieldImport> for ColumnSpec {
    fn to_import(&self, _parent: &TableSpec, context: &ImportContext) -> Result<FieldImport> {
        let (standard_field_name, column_annotation) =
            context.get_field_name_and_column_annotation(self);

        // [@pk] [type-annotations] [name]: [data-type] = [default-value]
        let column_type_name = context.column_type_name(self, None);

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

        let default_value = self.default_value.as_ref().and_then(|v| v.to_model());

        Ok(FieldImport {
            name: standard_field_name,
            data_type,
            is_pk: self.is_pk,
            is_unique: !self.unique_constraints.is_empty(),
            is_nullable: self.is_nullable,
            annotations: all_annotations,
            default_value,
        })
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

impl ImportWriter for FieldImport {
    fn write_to(&self, writer: &mut (dyn Write + Send)) -> Result<()> {
        const INDENT: &str = "  ";

        write!(writer, "{INDENT}{INDENT}")?;

        // Write annotations
        if self.is_pk {
            write!(writer, "@pk ")?;
        }

        if self.is_unique {
            write!(writer, "@unique ")?;
        }

        for annotation in &self.annotations {
            write!(writer, "{} ", annotation)?;
        }

        // Write field name and type
        write!(writer, "{}: {}", self.name, self.data_type)?;

        if self.is_nullable {
            write!(writer, "?")?;
        }

        // Write default value
        if let Some(default) = &self.default_value {
            write!(writer, " = {}", default)?;
        }

        writeln!(writer)?;

        Ok(())
    }
}
