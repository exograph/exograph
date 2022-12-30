use std::collections::HashMap;

use core_plugin_interface::core_model_builder::error::ModelBuildingError;
use lazy_static::lazy_static;
use postgres_model::aggregate::{
    AggregateField, AggregateFieldType, AggregateType, ScalarAggregateFieldKind,
};

use crate::resolved_builder::ResolvedType;

use super::system_builder::SystemContextBuilding;
use super::type_builder::ResolvedTypeEnv;

pub(super) fn aggregate_type_name(type_name: &str) -> String {
    format!("{}Agg", type_name)
}

pub(super) fn build_shallow(resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        create_shallow_type(resolved_type, building);
    }
}

pub(super) fn build_expanded(
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        expand_type(resolved_type, building);
    }

    Ok(())
}

fn create_shallow_type(resolved_type: &ResolvedType, building: &mut SystemContextBuilding) {
    let aggregate_type_name = aggregate_type_name(&resolved_type.name());

    let typ = match resolved_type {
        ResolvedType::Composite(_) => AggregateType {
            name: aggregate_type_name.clone(),
            fields: vec![],
        },
        ResolvedType::Primitive(_) => {
            let supported_kinds = TYPE_OPERATORS
                .get(resolved_type.name().as_str())
                .map(|ks| ks.iter())
                .unwrap_or_else(|| [].iter());

            let fields = supported_kinds
                .map(|kind| AggregateField {
                    name: kind.name().to_string(),
                    typ: AggregateFieldType::Scalar {
                        type_name: resolved_type.name(),
                        kind: *kind,
                    },
                    relation: None,
                })
                .chain(vec![AggregateField {
                    // vec![], since extend_one() is not stable yet

                    // Always add the count aggregate
                    name: ScalarAggregateFieldKind::Count.name().to_string(),
                    typ: AggregateFieldType::Scalar {
                        type_name: "Int".to_string(),
                        kind: ScalarAggregateFieldKind::Count,
                    },
                    relation: None,
                }])
                .collect();

            AggregateType {
                name: aggregate_type_name.clone(),
                fields,
            }
        }
    };

    building.aggregate_types.add(&aggregate_type_name, typ);
}

fn expand_type(resolved_type: &ResolvedType, building: &mut SystemContextBuilding) {
    let type_name = aggregate_type_name(&resolved_type.name());

    let existing_type_id = building.aggregate_types.get_id(&type_name).unwrap();

    if let ResolvedType::Composite(c) = resolved_type {
        let fields = c
            .fields
            .iter()
            .flat_map(|field| {
                // Only include fields that are primitive types, since nested aggregations doesn't make sense.
                // For example, if we have a query like this:
                // ```graphql
                // query {
                //   venueAgg {
                //     id {
                //       min
                //       max
                //     }
                //     concerts {
                //        price {
                //          avg
                //        }
                //     }
                //   }
                // }
                // ```
                // The `price` field is a nested aggregate, which doesn't make sense.
                //
                // However, the following does make sense (get aggregate of field in a nested type, while performing a collection query):
                // ```graphql
                // query {
                //   venue {
                //     id
                //     concertAgg {
                //        price {
                //          avg
                //        }
                //     }
                //   }
                // }
                // ```
                if field.typ.get_is_underlying_primitive() {
                    let type_name = aggregate_type_name(field.typ.get_underlying_typename());

                    let type_id = building.aggregate_types.get_id(&type_name).unwrap();

                    Some(AggregateField {
                        name: field.name.clone(),
                        typ: AggregateFieldType::Composite { type_id, type_name },
                        relation: None,
                    })
                } else {
                    None
                }
            })
            .collect();

        building.aggregate_types.values[existing_type_id].fields = fields;
    }
}

// TODO: Support aggregates for more types and allow expressing the return types (for example, avg() should return a a numeric even for integer types)
lazy_static! {
    static ref NUMERIC_AGG: Vec<ScalarAggregateFieldKind> = vec![
        ScalarAggregateFieldKind::Min, ScalarAggregateFieldKind::Max,
        ScalarAggregateFieldKind::Sum, ScalarAggregateFieldKind::Avg
    ];

    // immutable map defining the aggregates allowed for each scalar type
    // We don't specify the "count" aggregate here because it is always supported (see above)
    static ref TYPE_OPERATORS: HashMap<&'static str, Vec<ScalarAggregateFieldKind>> = HashMap::from([
        ("Int", NUMERIC_AGG.clone()),
        ("Float", NUMERIC_AGG.clone()),
        ("Decimal", NUMERIC_AGG.clone()),

        ("String", vec![ScalarAggregateFieldKind::Min, ScalarAggregateFieldKind::Max]),
    ]);
}
