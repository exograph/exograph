use crate::Column;

use super::{
    physical_column::{PhysicalColumn, PhysicalColumnType},
    ExpressionBuilder, SQLBuilder,
};

#[derive(Debug, PartialEq)]
pub struct JsonObject<'a>(pub Vec<JsonObjectElement<'a>>);

#[derive(Debug, PartialEq)]
pub struct JsonObjectElement<'a> {
    pub key: String,
    pub value: Column<'a>,
}

impl<'a> JsonObjectElement<'a> {
    pub fn new(key: String, value: Column<'a>) -> Self {
        Self { key, value }
    }
}

impl<'a> ExpressionBuilder for JsonObject<'a> {
    fn build(&self, builder: &mut SQLBuilder) {
        builder.push_str("json_build_object(");
        builder.push_elems(&self.0, ", ");
        builder.push(')');
    }
}

/// Build a SQL query for an element in a JSON object. The SQL expression will be `'<key>',
/// <value>`, where `<value>` is the SQL expression for the value of the JSON object element. The
/// value of the JSON object element is encoded as base64 if it is a blob, and as text if it is a
/// numeric.
impl<'a> ExpressionBuilder for JsonObjectElement<'a> {
    fn build(&self, builder: &mut SQLBuilder) {
        builder.push_str("'");
        builder.push_str(&self.key);
        builder.push_str("', ");

        if let Column::Physical(PhysicalColumn { typ, .. }) = self.value {
            match &typ {
                // encode blob fields in JSON objects as base64
                // PostgreSQL inserts newlines into encoded base64 every 76 characters when in aligned mode
                // need to filter out using translate(...) function
                PhysicalColumnType::Blob => {
                    builder.push_str("translate(encode(");
                    self.value.build(builder);
                    builder.push_str(", \'base64\'), E'\\n', '')");
                }

                // numerics must be outputted as text to avoid any loss in precision
                PhysicalColumnType::Numeric { .. } => {
                    self.value.build(builder);
                    builder.push_str("::text");
                }

                _ => self.value.build(builder),
            }
        } else {
            self.value.build(builder)
        }
    }
}
