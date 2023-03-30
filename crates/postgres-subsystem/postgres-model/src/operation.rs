//! Types to represent the operations that are generated by the postgres subsystem builder.
//!
//! Queries and mutations stem from the types defined inside each Postgres service (such as `type Todo { ... }`).
//!
//! Consider a postgres service with the following types:
//! ```exo
//! @postgres
//! service TodoService {
//!    type Todo {
//!       @pk id: Int = autoIncrement()
//!       title: String
//!       completed: Boolean
//!    }
//! }
//! ```
//!
//! Queries like `todos`, `todo`, and `todoAgg` as well as mutations like `createTodo`, `updateTodo`, and `deleteTodo` will be
//! generated by the postgres subsystem builder.

use std::fmt::Debug;

use async_graphql_parser::types::Type;
use core_plugin_interface::core_model::type_normalization::{Operation, Parameter};
use core_plugin_interface::core_model::types::OperationReturnType;

use serde::{Deserialize, Serialize};

use crate::types::EntityType;

/// An operation such as a query or mutation.
///
/// * `P` - This parameter allows differentiating between, for example,
///   [`PkQuery`](`super::query::PkQuery`), [`AggregateQuery`](super::query::AggregateQuery), and
///   [`PostgresMutation`](super::mutation::PostgresMutation).
#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresOperation<P: OperationParameters> {
    /// The name of the operation such as `todos`, `todosAgg`, createTodo`, `updateTodo`, or `deleteTodo`.
    pub name: String,
    /// The parameters (if multiple parameters are needed, they are wrapped in a struct)
    pub parameters: P,
    /// The return type such as `Todo` or `[Todo]`.
    pub return_type: OperationReturnType<EntityType>,
}

/// Supports introspection of operation parameters
pub trait OperationParameters {
    /// Create an introspection version of the parameters
    fn introspect(&self) -> Vec<&dyn Parameter>;
}

impl<P: OperationParameters> Operation for PostgresOperation<P> {
    fn name(&self) -> &String {
        &self.name
    }

    fn parameters(&self) -> Vec<&dyn Parameter> {
        self.parameters.introspect()
    }

    fn return_type(&self) -> Type {
        (&self.return_type).into()
    }
}
