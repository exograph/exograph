use std::any::Any;

use tokio_postgres::types::ToSql;

/// A trait to simplify our use of SQL parameters, specifically to have the [Send] and [Sync] bounds.
pub trait SQLParam: ToSql + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn eq(&self, other: &dyn SQLParam) -> bool;

    /// Create type-compatible version that we can pass to the Postgres driver.
    fn as_pg(&self) -> &(dyn ToSql + Sync);
}

impl<T: ToSql + Send + Sync + Any + PartialEq> SQLParam for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, other: &dyn SQLParam) -> bool {
        if let Some(other) = other.as_any().downcast_ref::<T>() {
            self == other
        } else {
            false
        }
    }

    fn as_pg(&self) -> &(dyn ToSql + Sync) {
        self
    }
}

impl PartialEq for dyn SQLParam {
    fn eq(&self, other: &Self) -> bool {
        SQLParam::eq(self, other)
    }
}
