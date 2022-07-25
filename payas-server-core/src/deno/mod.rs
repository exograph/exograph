pub mod deno_resolver;
pub mod interception;

macro_rules! claytip_execute_query {
    ($system_context:ident, $request_context:ident) => {
        Some(&move |query_string: String,
                    variables: Option<serde_json::Map<String, Value>>,
                    context_override: Value| {
            let new_request_context =
                RequestContext::with_override($request_context, context_override);
            async move {
                // execute query
                let result = $system_context
                    .resolve(
                        crate::OperationsPayload {
                            operation_name: None,
                            query: query_string,
                            variables,
                        },
                        &new_request_context,
                    )
                    .await?;

                // collate result into a single QueryResponse

                // since query execution results in a Vec<(String, QueryResponse)>, we want to
                // extract and collect all HTTP headers generated in QueryResponses
                let headers = result
                    .iter()
                    .flat_map(|(_, response)| response.headers.clone())
                    .collect::<Vec<_>>();

                // generate the body
                let body = result
                    .into_iter()
                    .map(|(name, response)| (name, response.body.to_json().unwrap()))
                    .collect::<Map<_, _>>();

                Ok(QueryResponse {
                    body: QueryResponseBody::Json(serde_json::Value::Object(body)),
                    headers,
                })
            }
            .boxed()
        })
    };
}

pub(crate) use claytip_execute_query;
