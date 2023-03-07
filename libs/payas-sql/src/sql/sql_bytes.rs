use std::fmt::{Debug, Display};

use bytes::Bytes;
use tokio_postgres::types::{to_sql_checked, ToSql, Type};

// Wrapper type for bytes::Bytes for use with BYTEA, since [`Bytes`] does not implement ToSql.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SQLBytes(pub Bytes);

impl SQLBytes {
    pub fn new(vec: Vec<u8>) -> Self {
        Self(Bytes::from(vec))
    }
}

impl ToSql for SQLBytes {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut bytes::BytesMut,
    ) -> Result<tokio_postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>>
    where
        Self: Sized,
    {
        (&self.0[..]).to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool
    where
        Self: Sized,
    {
        matches!(*ty, Type::BYTEA)
    }

    to_sql_checked!();
}

impl Display for SQLBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
