// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use core_model::types::Named;
use core_model_builder::error::ModelBuildingError;
use lazy_static::lazy_static;
use postgres_core_model::aggregate::{
    AggregateField, AggregateFieldType, AggregateType, ScalarAggregateFieldKind,
};
use postgres_core_model::types::EntityRepresentation;

use crate::resolved_type::ResolvedType;
use crate::resolved_type::ResolvedTypeEnv;

use crate::shallow::Shallow;
use crate::system_builder::SystemContextBuilding;

pub fn aggregate_type_name(type_name: &str) -> String {
    format!("{type_name}Agg")
}

fn needs_aggregate(resolved_type: &ResolvedType) -> bool {
    if let ResolvedType::Composite(c) = resolved_type {
        if c.representation == EntityRepresentation::Json {
            return false;
        }
    }
    true
}

pub(super) fn build_shallow(resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    for (_, resolved_type) in resolved_env
        .resolved_types
        .iter()
        .filter(|(_, resolved_type)| needs_aggregate(resolved_type))
    {
        create_shallow_type(resolved_type, building);
    }
}

pub(super) fn build_expanded(
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    for (_, resolved_type) in resolved_env
        .resolved_types
        .iter()
        .filter(|(_, resolved_type)| needs_aggregate(resolved_type))
    {
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
            underlying_type: Shallow::shallow(),
        },
        ResolvedType::Enum(_) => {
            let fields = vec![AggregateField {
                // For enums, we support only the count aggregate
                name: ScalarAggregateFieldKind::Count.name().to_string(),
                typ: AggregateFieldType::Scalar {
                    type_name: "Int".to_string(),
                    kind: ScalarAggregateFieldKind::Count,
                },
                relation: None,
            }];

            AggregateType {
                name: aggregate_type_name.clone(),
                fields,
                underlying_type: Shallow::shallow(),
            }
        }
        ResolvedType::Primitive(_) => {
            let supported_kinds = AGG_MAP
                .get(resolved_type.name().as_str())
                .map(|ks| ks.iter())
                .unwrap_or_else(|| [].iter());

            let fields = supported_kinds
                .map(|(kind, agg_return_type)| AggregateField {
                    name: kind.name().to_string(),
                    typ: AggregateFieldType::Scalar {
                        type_name: agg_return_type
                            .map(|t| t.to_string())
                            .unwrap_or_else(|| resolved_type.name()),
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
                underlying_type: Shallow::shallow(),
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
                if field.typ.innermost().is_primitive || field.is_pk {
                    let type_name = aggregate_type_name(field.typ.name());

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

        let existing_type = building.aggregate_types.get_by_id_mut(existing_type_id);

        existing_type.fields = fields;

        existing_type.underlying_type =
            building.entity_types.get_id(&resolved_type.name()).unwrap();
    }
}

// TODO: Support aggregates for more types (https://github.com/exograph/exograph/issues/604)
lazy_static! {
    // An immutable map defining the aggregates allowed for each scalar type
    // We don't specify the "count" aggregate here because it is always supported (see above)
    // The second element in the tuple is the return type of the avg, if it differs from the input type
    static ref AGG_MAP: HashMap<&'static str, Vec<(ScalarAggregateFieldKind, Option<&'static str>)>> = HashMap::from([
        ("Int",
            vec![(ScalarAggregateFieldKind::Min, None), (ScalarAggregateFieldKind::Max, None),
                 (ScalarAggregateFieldKind::Sum, None), (ScalarAggregateFieldKind::Avg, Some("Float"))]),
        ("Float",
            vec![(ScalarAggregateFieldKind::Min, None), (ScalarAggregateFieldKind::Max, None),
                 (ScalarAggregateFieldKind::Sum, Some("Float")), (ScalarAggregateFieldKind::Avg, Some("Float"))]),
        ("Decimal",
            vec![(ScalarAggregateFieldKind::Min, None), (ScalarAggregateFieldKind::Max, None),
                 (ScalarAggregateFieldKind::Sum, None), (ScalarAggregateFieldKind::Avg, None)]),

        ("String",
                vec![(ScalarAggregateFieldKind::Min, None), (ScalarAggregateFieldKind::Max, None)]),

        ("Vector", vec![(ScalarAggregateFieldKind::Avg, None)])
    ]);
}
