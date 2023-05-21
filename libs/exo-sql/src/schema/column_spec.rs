use crate::{FloatBits, IntBits};

pub struct ColumnSpec {
    pub(super) name: String,
    pub(super) typ: ColumnTypeSpec,
    pub(super) is_pk: bool,
    pub(super) is_auto_increment: bool,
    pub(super) is_nullable: bool,
    pub(super) unique_constraints: Vec<String>,
    pub(super) default_value: Option<String>,
}

pub enum ColumnTypeSpec {
    Int {
        bits: IntBits,
    },
    String {
        max_length: Option<usize>,
    },
    Boolean,
    Timestamp {
        timezone: bool,
        precision: Option<usize>,
    },
    Date,
    Time {
        precision: Option<usize>,
    },
    Json,
    Blob,
    Uuid,
    Array {
        typ: Box<ColumnTypeSpec>,
    },
    ColumnReference {
        ref_table_name: String,
        ref_column_name: String,
        ref_pk_type: Box<ColumnTypeSpec>,
    },
    Float {
        bits: FloatBits,
    },
    Numeric {
        precision: Option<usize>,
        scale: Option<usize>,
    },
}
