use crate::graphql::{
    database::{
        create_operation, database_query::DatabaseQuery, delete_operation, return_type_info,
        update_operation,
    },
    execution::system_context::SystemContext,
    execution_error::ExecutionError,
    request_context::RequestContext,
    validation::field::ValidatedField,
};
use async_trait::async_trait;

use payas_model::model::{
    operation::{DatabaseMutationKind, Interceptors, Mutation, MutationKind},
    types::GqlTypeModifier,
};
use payas_sql::{AbstractOperation, AbstractPredicate};

use crate::graphql::data::{
    operation_mapper::{DenoOperation, OperationResolverResult},
    operation_resolver::OperationResolver,
};

#[async_trait]
impl<'a> OperationResolver<'a> for Mutation {
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<OperationResolverResult<'a>, ExecutionError> {
        match &self.kind {
            MutationKind::Database { kind } => {
                let abstract_select = {
                    let (_, pk_query, collection_query) = return_type_info(self, system_context);
                    let selection_query = match &self.return_type.type_modifier {
                        GqlTypeModifier::List => collection_query,
                        GqlTypeModifier::NonNull | GqlTypeModifier::Optional => pk_query,
                    };

                    DatabaseQuery::from(selection_query)
                        .operation(
                            field,
                            AbstractPredicate::True,
                            system_context,
                            request_context,
                        )
                        .await?
                };

                Ok(OperationResolverResult::SQLOperation(match kind {
                    DatabaseMutationKind::Create(data_param) => AbstractOperation::Insert(
                        create_operation(
                            self,
                            data_param,
                            field,
                            abstract_select,
                            system_context,
                            request_context,
                        )
                        .await?,
                    ),
                    DatabaseMutationKind::Delete(predicate_param) => AbstractOperation::Delete(
                        delete_operation(
                            self,
                            predicate_param,
                            field,
                            abstract_select,
                            system_context,
                            request_context,
                        )
                        .await?,
                    ),
                    DatabaseMutationKind::Update {
                        data_param,
                        predicate_param,
                    } => AbstractOperation::Update(
                        update_operation(
                            self,
                            data_param,
                            predicate_param,
                            field,
                            abstract_select,
                            system_context,
                            request_context,
                        )
                        .await?,
                    ),
                }))
            }

            MutationKind::Service { method_id, .. } => Ok(OperationResolverResult::DenoOperation(
                DenoOperation(method_id.unwrap()),
            )),
        }
    }

    fn interceptors(&self) -> &Interceptors {
        &self.interceptors
    }

    fn name(&self) -> &str {
        &self.name
    }
}
