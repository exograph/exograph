use super::column_spec::ColumnSpec;

pub struct TableSpec {
    pub(super) name: String,
    pub(super) columns: Vec<ColumnSpec>,
}

impl TableSpec {
    pub fn new(name: impl Into<String>, columns: Vec<ColumnSpec>) -> Self {
        Self {
            name: name.into(),
            columns,
        }
    }
}
