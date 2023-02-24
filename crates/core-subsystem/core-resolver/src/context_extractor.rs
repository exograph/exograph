use async_trait::async_trait;
use core_model::{access::AccessContextSelection, context_type::ContextContainer};
use serde_json::{Map, Value};

use crate::request_context::RequestContext;

/// Extract context objects from the request context.
#[async_trait]
pub trait ContextExtractor {
    /// Extract the context object.
    ///
    /// If the context type is defined as:
    ///
    /// ```clay
    /// context AuthContext {
    ///   id: Int
    ///   name: String
    ///   role: String
    /// }
    /// ```
    ///
    /// Then calling this with `context_name` set to `"AuthContext"` will return an object
    /// such as:
    ///
    /// ```json
    /// {
    ///   id: 1,
    ///   name: "John",
    ///   role: "admin",
    /// }
    /// ```
    async fn extract_context(
        &self,
        request_context: &RequestContext,
        context_name: &str,
    ) -> Option<Map<String, Value>>;

    /// Extract the context object selection.
    ///
    /// This method is similar to `extract_context` but it allows to select a specific field from
    /// the context object. For example, consider the context type and the context object in the
    /// documentation of [`extract_context`](AccessSolver::extract_context). Calling this method
    /// with `context_selection` set to
    /// `AccessContextSelection::Select(AccessContextSelection("AuthContext"), "role")` will return
    /// the value `"admin"`.
    async fn extract_context_selection(
        &self,
        request_context: &RequestContext,
        context_selection: &AccessContextSelection,
    ) -> Option<Value> {
        fn extract_path<'a>(value: &'a Value, path: &[String]) -> Option<&'a Value> {
            match path.split_first() {
                Some((key, tail)) => value.get(key).and_then(|value| extract_path(value, tail)),
                None => Some(value),
            }
        }

        let context = self
            .extract_context(request_context, &context_selection.context_name)
            .await?;
        context
            .get(&context_selection.path.0)
            .and_then(|head_selection| extract_path(head_selection, &context_selection.path.1))
            .cloned()
    }
}

#[async_trait]
impl<T: ContextContainer + std::marker::Sync> ContextExtractor for T {
    async fn extract_context(
        &self,
        request_context: &RequestContext,
        context_name: &str,
    ) -> Option<Map<String, Value>> {
        let contexts = self.contexts();
        let context_type = contexts.get_by_key(context_name).unwrap();
        request_context.extract_context(context_type).await.ok()
    }
}
