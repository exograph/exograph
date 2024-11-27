mod subsystem_rest_resolver;

pub use subsystem_rest_resolver::PostgresSubsystemRestResolver;

// use std::process::id;

// fn resolve(query: RestQuery) -> Result<(), PostgresExecutionError> {
//     // if pk query, return single row

//     ValidatedQuery {
//         operation_name: None,
//         parameters: Some(PkQueryParameters {
//             id: query.id,
//         }),
//         selection_set: {
//             id,
//             title
//         }
//     }
// }
