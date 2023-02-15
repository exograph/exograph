use crate::{
    aggregate_type_builder::aggregate_type_name, resolved_builder::ResolvedFieldTypeHelper,
    shallow::Shallow,
};

use super::{access_builder::ResolvedAccess, access_utils, resolved_builder::ResolvedFieldType};

use core_plugin_interface::{
    core_model::{
        context_type::ContextType,
        mapped_arena::{MappedArena, SerializableSlabIndex},
        primitive_type::PrimitiveType,
        types::{FieldType, Named},
    },
    core_model_builder::{ast::ast_types::AstExpr, error::ModelBuildingError, typechecker::Typed},
};

use payas_sql::{FloatBits, IntBits, PhysicalColumn, PhysicalColumnType, PhysicalTable};

use postgres_model::{
    access::Access,
    aggregate::{AggregateField, AggregateFieldType},
    column_id::ColumnId,
    relation::{PostgresRelation, RelationCardinality},
    types::{EntityType, PostgresField, PostgresFieldType, PostgresPrimitiveType, TypeIndex},
};

use super::{
    naming::ToPostgresQueryName,
    resolved_builder::{
        ResolvedCompositeType, ResolvedField, ResolvedFieldDefault, ResolvedType, ResolvedTypeHint,
    },
    system_builder::SystemContextBuilding,
};

#[derive(Debug, Clone)]
pub struct ResolvedTypeEnv<'a> {
    pub contexts: &'a MappedArena<ContextType>,
    pub resolved_types: MappedArena<ResolvedType>,
}

impl<'a> ResolvedTypeEnv<'a> {
    pub fn get_by_key(&self, key: &str) -> Option<&ResolvedType> {
        self.resolved_types.get_by_key(key)
    }
}

pub(crate) fn build_shallow(resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        create_shallow_type(resolved_type, resolved_env, building);
    }
}

pub(crate) fn build_expanded(
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            expand_type_no_fields(c, resolved_env, building);
        }
    }

    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            expand_type_fields(c, building, resolved_env);
        }
    }

    for (_, resolved_type) in resolved_env.resolved_types.iter() {
        if let ResolvedType::Composite(c) = &resolved_type {
            expand_type_access(c, resolved_env, building)?;
        }
    }

    Ok(())
}

fn create_shallow_type(
    resolved_type: &ResolvedType,
    _resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) {
    match resolved_type {
        ResolvedType::Primitive(_) => {
            building.primitive_types.add(
                &resolved_type.name(),
                PostgresPrimitiveType {
                    name: resolved_type.name(),
                },
            );
        }
        ResolvedType::Composite(_) => {
            let typ = EntityType {
                name: resolved_type.name(),
                plural_name: resolved_type.plural_name(),
                fields: vec![],
                agg_fields: vec![],
                table_id: SerializableSlabIndex::shallow(),
                pk_query: SerializableSlabIndex::shallow(),
                collection_query: SerializableSlabIndex::shallow(),
                aggregate_query: SerializableSlabIndex::shallow(),
                access: Access::restrictive(),
            };

            building.entity_types.add(&resolved_type.name(), typ);
        }
    }
}

/// Expand a composite type except for creating its fields.
///
/// Specifically:
/// 1. Create and set the table
/// 2. Create and set *_query members (if applicable)
/// 3. Leave fields as an empty vector
///
/// This allows the type to become `Composite` and `table_id` for any type can be accessed when building fields in the next step of expansion.
/// We can't expand fields yet since creating a field requires access to columns (self as well as those in a referred field in case a relation)
/// and we may not have expanded a referred type yet.
fn expand_type_no_fields(
    resolved_type: &ResolvedCompositeType,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) {
    let table_name = resolved_type.table_name.clone();

    let columns = resolved_type
        .fields
        .iter()
        .flat_map(|field| create_column(field, &table_name, resolved_env))
        .collect();

    let table = PhysicalTable {
        name: table_name.clone(),
        columns,
    };

    let table_id = building.tables.add(&table_name, table);

    let pk_query = building
        .pk_queries
        .get_id(&resolved_type.pk_query())
        .unwrap();

    let collection_query = building
        .collection_queries
        .get_id(&resolved_type.collection_query())
        .unwrap();

    let aggregate_query = building
        .aggregate_queries
        .get_id(&resolved_type.aggregate_query())
        .unwrap();

    let existing_type_id = building.get_entity_type_id(&resolved_type.name).unwrap();
    let mut existing_type = &mut building.entity_types[existing_type_id];
    existing_type.table_id = table_id;
    existing_type.pk_query = pk_query;
    existing_type.collection_query = collection_query;
    existing_type.aggregate_query = aggregate_query;
}

/// Now that all types have table with them (set in the earlier expand_type_no_fields phase), we can
/// expand fields
fn expand_type_fields(
    resolved_type: &ResolvedCompositeType,
    building: &mut SystemContextBuilding,
    resolved_env: &ResolvedTypeEnv,
) {
    let existing_type_id = building.get_entity_type_id(&resolved_type.name).unwrap();
    let existing_table_id = &building.entity_types[existing_type_id].table_id;

    let entity_fields = resolved_type
        .fields
        .iter()
        .map(|field| create_persistent_field(field, existing_table_id, building, resolved_env))
        .collect();

    let agg_fields = resolved_type
        .fields
        .iter()
        .flat_map(|field| create_agg_field(field, existing_table_id, building, resolved_env))
        .collect();

    let mut existing_type = &mut building.entity_types[existing_type_id];
    existing_type.fields = entity_fields;
    existing_type.agg_fields = agg_fields;
}

// Expand access expressions (pre-condition: all type fields have been populated)
fn expand_type_access(
    resolved_type: &ResolvedCompositeType,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    let existing_type_id = building.get_entity_type_id(&resolved_type.name).unwrap();

    let expr = compute_access_composite_types(
        &resolved_type.access,
        &building.entity_types[existing_type_id],
        resolved_env,
        building,
    )?;

    let mut existing_type = &mut building.entity_types[existing_type_id];

    existing_type.access = expr;

    Ok(())
}

pub fn compute_access_composite_types(
    resolved: &ResolvedAccess,
    self_type_info: &EntityType,
    resolved_env: &ResolvedTypeEnv,
    building: &SystemContextBuilding,
) -> Result<Access, ModelBuildingError> {
    let access_expr = |expr: &AstExpr<Typed>| {
        access_utils::compute_predicate_expression(
            expr,
            Some(self_type_info),
            resolved_env,
            &building.primitive_types,
            &building.entity_types,
        )
    };

    Ok(Access {
        creation: access_expr(&resolved.creation)?,
        read: access_expr(&resolved.read)?,
        update: access_expr(&resolved.update)?,
        delete: access_expr(&resolved.delete)?,
    })
}

fn create_persistent_field(
    field: &ResolvedField,
    table_id: &SerializableSlabIndex<PhysicalTable>,
    building: &SystemContextBuilding,
    env: &ResolvedTypeEnv,
) -> PostgresField<EntityType> {
    let base_field_type = {
        let ResolvedFieldType {
            type_name,
            is_primitive,
        } = field.typ.inner_most();

        if *is_primitive {
            let type_id = building.primitive_types.get_id(type_name).unwrap();
            PostgresFieldType {
                type_name: type_name.clone(),
                type_id: TypeIndex::Primitive(type_id),
            }
        } else {
            let type_id = building.entity_types.get_id(type_name).unwrap();
            PostgresFieldType {
                type_name: type_name.clone(),
                type_id: TypeIndex::Composite(type_id),
            }
        }
    };

    PostgresField {
        name: field.name.to_owned(),
        typ: field.typ.wrap(base_field_type),
        relation: create_relation(field, *table_id, building, env),
        has_default_value: field.default_value.is_some(),
    }
}

fn create_agg_field(
    field: &ResolvedField,
    table_id: &SerializableSlabIndex<PhysicalTable>,
    building: &SystemContextBuilding,
    env: &ResolvedTypeEnv,
) -> Option<AggregateField> {
    fn is_underlying_type_list(field_type: &FieldType<ResolvedFieldType>) -> bool {
        match field_type {
            FieldType::Plain(_) => false,
            FieldType::Optional(underlying) => is_underlying_type_list(underlying),
            FieldType::List(_) => true,
        }
    }

    if field.typ.inner_most().is_primitive || !is_underlying_type_list(&field.typ) {
        None
    } else {
        let field_name = format!("{}Agg", field.name);
        let field_type_name = field.typ.name();
        let agg_type_name = aggregate_type_name(field_type_name);
        let agg_type_id = building.aggregate_types.get_id(&agg_type_name).unwrap();

        Some(AggregateField {
            name: field_name,
            typ: AggregateFieldType::Composite {
                type_name: agg_type_name,
                type_id: agg_type_id,
            },
            relation: Some(create_relation(field, *table_id, building, env)),
        })
    }
}

fn create_column(
    field: &ResolvedField,
    table_name: &str,
    env: &ResolvedTypeEnv,
) -> Option<PhysicalColumn> {
    // Check that the field holds to a self column
    let unique_constraint_name = if !field.self_column {
        return None;
    } else {
        field.unique_constraints.clone()
    };
    // split a Optional type into its inner type and the optional marker
    let (typ, optional) = match &field.typ {
        FieldType::Optional(inner_typ) => (inner_typ.as_ref(), true),
        _ => (&field.typ, false),
    };

    let default_value =
        field
            .default_value
            .as_ref()
            .and_then(|default_value| match default_value {
                ResolvedFieldDefault::Value(val) => Some(match &**val {
                    AstExpr::StringLiteral(string, _) => {
                        format!("'{}'::text", string.replace('\'', "''"))
                    }
                    AstExpr::BooleanLiteral(boolean, _) => {
                        format!("{boolean}")
                    }
                    AstExpr::NumberLiteral(val, _) => {
                        format!("{val}")
                    }
                    _ => panic!("Invalid concrete value"),
                }),
                ResolvedFieldDefault::PostgresFunction(string) => Some(string.to_string()),
                ResolvedFieldDefault::Autoincrement => None,
            });

    match typ {
        FieldType::Plain(ResolvedFieldType { type_name, .. }) => {
            // Either a scalar (primitive) or a many-to-one relationship with another table
            let field_type = env.get_by_key(type_name).unwrap();

            match field_type {
                ResolvedType::Primitive(pt) => Some(PhysicalColumn {
                    table_name: table_name.to_string(),
                    column_name: field.column_name.to_string(),
                    typ: determine_column_type(pt, field),
                    is_pk: field.is_pk,
                    is_autoincrement: if field.get_is_autoincrement() {
                        assert!(matches!(
                            typ.deref(env),
                            &ResolvedType::Primitive(PrimitiveType::Int)
                        ));
                        true
                    } else {
                        false
                    },
                    is_nullable: optional,
                    unique_constraints: unique_constraint_name,
                    default_value,
                }),
                ResolvedType::Composite(ct) => {
                    // Many-to-one:
                    // Column from the current table (but of the type of the pk column of the other table)
                    // and it refers to the pk column in the other table.
                    let other_pk_field = ct.pk_field().unwrap();
                    Some(PhysicalColumn {
                        table_name: table_name.to_string(),
                        column_name: field.column_name.to_string(),
                        typ: PhysicalColumnType::ColumnReference {
                            ref_table_name: ct.table_name.to_string(),
                            ref_column_name: other_pk_field.column_name.to_string(),
                            ref_pk_type: Box::new(determine_column_type(
                                &other_pk_field.typ.deref(env).as_primitive(),
                                field,
                            )),
                        },
                        is_pk: false,
                        is_autoincrement: false,
                        is_nullable: optional,
                        unique_constraints: unique_constraint_name,
                        default_value,
                    })
                }
            }
        }
        FieldType::List(typ) => {
            // unwrap list to base type
            let mut underlying_typ = typ;
            let mut depth = 1;

            while let FieldType::List(t) = &**underlying_typ {
                underlying_typ = t;
                depth += 1;
            }

            let underlying_pt = if let FieldType::Plain(ResolvedFieldType { type_name, .. }) =
                &**underlying_typ
            {
                if let Some(ResolvedType::Primitive(pt)) = env.resolved_types.get_by_key(type_name)
                {
                    Some(pt)
                } else {
                    None
                }
            } else {
                todo!()
            };

            // is our underlying list type a primitive or a column?
            if let Some(underlying_pt) = underlying_pt {
                // underlying type is a primitive, so treat it as an Array

                // rewrap underlying PrimitiveType
                let mut pt = underlying_pt.clone();
                for _ in 0..depth {
                    pt = PrimitiveType::Array(Box::new(pt))
                }

                Some(PhysicalColumn {
                    table_name: table_name.to_string(),
                    column_name: field.column_name.to_string(),
                    typ: determine_column_type(&pt, field),
                    is_pk: false,
                    is_autoincrement: false,
                    is_nullable: optional,
                    unique_constraints: unique_constraint_name,
                    default_value,
                })
            } else {
                // this is a OneToMany relation, so the other side has the associated column
                None
            }
        }
        FieldType::Optional(_) => panic!("Optional in an Optional?"),
    }
}

fn determine_column_type<'a>(
    pt: &'a PrimitiveType,
    field: &'a ResolvedField,
) -> PhysicalColumnType {
    if let PrimitiveType::Array(underlying_pt) = pt {
        return PhysicalColumnType::Array {
            typ: Box::new(determine_column_type(underlying_pt, field)),
        };
    }

    if let Some(hint) = &field.type_hint {
        match hint {
            ResolvedTypeHint::Explicit { dbtype } => {
                PhysicalColumnType::from_string(dbtype).unwrap()
            }

            ResolvedTypeHint::Int { bits, range } => {
                assert!(matches!(pt, PrimitiveType::Int));

                // determine the proper sized type to use
                if let Some(bits) = bits {
                    PhysicalColumnType::Int {
                        bits: match bits {
                            16 => IntBits::_16,
                            32 => IntBits::_32,
                            64 => IntBits::_64,
                            _ => panic!("Invalid bits"),
                        },
                    }
                } else if let Some(range) = range {
                    let is_superset = |bound_min: i64, bound_max: i64| {
                        let range_min = range.0;
                        let range_max = range.1;
                        assert!(range_min <= range_max);
                        assert!(bound_min <= bound_max);

                        // is this bound a superset of the provided range?
                        (bound_min <= range_min && bound_min <= range_max)
                            && (bound_max >= range_max && bound_max >= range_min)
                    };

                    // determine which SQL type is appropriate for this range
                    {
                        if is_superset(i16::MIN.into(), i16::MAX.into()) {
                            PhysicalColumnType::Int { bits: IntBits::_16 }
                        } else if is_superset(i32::MIN.into(), i32::MAX.into()) {
                            PhysicalColumnType::Int { bits: IntBits::_32 }
                        } else if is_superset(i64::MIN, i64::MAX) {
                            PhysicalColumnType::Int { bits: IntBits::_64 }
                        } else {
                            // TODO: numeric type
                            panic!("Requested range is too big")
                        }
                    }
                } else {
                    // no hints provided, go with default
                    PhysicalColumnType::Int { bits: IntBits::_32 }
                }
            }

            ResolvedTypeHint::Float { bits } => {
                assert!(matches!(pt, PrimitiveType::Float));

                let bits = *bits;

                if (1..=24).contains(&bits) {
                    PhysicalColumnType::Float {
                        bits: FloatBits::_24,
                    }
                } else if bits > 24 && bits <= 53 {
                    PhysicalColumnType::Float {
                        bits: FloatBits::_53,
                    }
                } else {
                    panic!("Invalid bits")
                }
            }

            ResolvedTypeHint::Decimal { precision, scale } => {
                assert!(matches!(pt, PrimitiveType::Decimal));

                // cannot have scale and no precision specified
                if precision.is_none() {
                    assert!(scale.is_none())
                }

                PhysicalColumnType::Numeric {
                    precision: *precision,
                    scale: *scale,
                }
            }

            ResolvedTypeHint::String { length } => {
                assert!(matches!(pt, PrimitiveType::String));

                // length hint provided, use it
                PhysicalColumnType::String {
                    length: Some(*length),
                }
            }

            ResolvedTypeHint::DateTime { precision } => match pt {
                PrimitiveType::LocalTime => PhysicalColumnType::Time {
                    precision: Some(*precision),
                },
                PrimitiveType::LocalDateTime => PhysicalColumnType::Timestamp {
                    precision: Some(*precision),
                    timezone: false,
                },
                PrimitiveType::Instant => PhysicalColumnType::Timestamp {
                    precision: Some(*precision),
                    timezone: true,
                },
                _ => panic!(),
            },
        }
    } else {
        match pt {
            // choose a default SQL type
            PrimitiveType::Int => PhysicalColumnType::Int { bits: IntBits::_32 },
            PrimitiveType::Float => PhysicalColumnType::Float {
                bits: FloatBits::_24,
            },
            PrimitiveType::Decimal => PhysicalColumnType::Numeric {
                precision: None,
                scale: None,
            },
            PrimitiveType::String => PhysicalColumnType::String { length: None },
            PrimitiveType::Boolean => PhysicalColumnType::Boolean,
            PrimitiveType::LocalTime => PhysicalColumnType::Time { precision: None },
            PrimitiveType::LocalDateTime => PhysicalColumnType::Timestamp {
                precision: None,
                timezone: false,
            },
            PrimitiveType::LocalDate => PhysicalColumnType::Date,
            PrimitiveType::Instant => PhysicalColumnType::Timestamp {
                precision: None,
                timezone: true,
            },
            PrimitiveType::Json => PhysicalColumnType::Json,
            PrimitiveType::Blob => PhysicalColumnType::Blob,
            PrimitiveType::Uuid => PhysicalColumnType::Uuid,
            PrimitiveType::Array(_)
            | PrimitiveType::Claytip
            | PrimitiveType::ClaytipPriv
            | PrimitiveType::Interception(_) => {
                panic!()
            }
        }
    }
}

fn create_relation(
    field: &ResolvedField,
    table_id: SerializableSlabIndex<PhysicalTable>,
    building: &SystemContextBuilding,
    resolved_env: &ResolvedTypeEnv,
) -> PostgresRelation {
    fn compute_column_id(
        table: &PhysicalTable,
        table_id: SerializableSlabIndex<PhysicalTable>,
        field: &ResolvedField,
    ) -> Option<ColumnId> {
        let column_name = field.column_name.to_string();

        table
            .column_index(&column_name)
            .map(|index| ColumnId::new(table_id, index))
    }

    let table = &building.tables[table_id];

    if field.is_pk {
        let column_id = compute_column_id(table, table_id, field);
        PostgresRelation::Pk {
            column_id: column_id.unwrap(),
        }
    } else {
        fn compute_base_type(
            field_type: &FieldType<ResolvedFieldType>,
        ) -> &FieldType<ResolvedFieldType> {
            match field_type {
                FieldType::Optional(inner_typ) => inner_typ.as_ref(),
                _ => field_type,
            }
        }
        // we can treat Optional fields as their inner type for the purposes of computing relations
        let field_base_typ = compute_base_type(&field.typ);

        match field_base_typ {
            FieldType::List(underlying) => {
                if let ResolvedType::Primitive(_) = underlying.deref(resolved_env) {
                    // List of a primitive type is still a scalar from the database perspective
                    PostgresRelation::Scalar {
                        column_id: compute_column_id(table, table_id, field).unwrap(),
                    }
                } else {
                    // If the field is of a list type and the underlying type is not a primitive,
                    // then it is a OneToMany relation with the self's type being the "One" side
                    // and the field's type being the "Many" side.
                    let field_type = underlying.deref(resolved_env).as_composite();

                    let other_type_id = building
                        .get_entity_type_id(field_type.name.as_str())
                        .unwrap();
                    let other_type = &building.entity_types[other_type_id];
                    let other_table_id = other_type.table_id;
                    let other_table = &building.tables[other_table_id];

                    let other_type_column_id =
                        compute_column_id(other_table, other_table_id, field).unwrap();

                    PostgresRelation::OneToMany {
                        other_type_column_id,
                        other_type_id,
                        cardinality: RelationCardinality::Unbounded,
                    }
                }
            }

            FieldType::Plain(ResolvedFieldType { type_name, .. }) => {
                let field_type = resolved_env.get_by_key(type_name).unwrap();

                match field_type {
                    ResolvedType::Primitive(_) => {
                        let column_id = compute_column_id(table, table_id, field);
                        PostgresRelation::Scalar {
                            column_id: column_id.unwrap(),
                        }
                    }
                    ResolvedType::Composite(ct) => {
                        // A field's type is "Plain" or "Optional" and the field type is composite,
                        // but we can't be sure if this is a ManyToOne or OneToMany unless we examine the other side's type.

                        let other_resolved_type = resolved_env.get_by_key(type_name).unwrap();
                        let other_type_field_typ = &other_resolved_type
                            .as_composite()
                            .fields
                            .iter()
                            .find(|f| f.column_name == field.column_name)
                            .unwrap()
                            .typ;

                        let other_type_id = building.get_entity_type_id(&ct.name).unwrap();

                        match (&field.typ, other_type_field_typ) {
                            (FieldType::Optional(_), FieldType::Plain(_)) => {
                                let other_type = &building.entity_types[other_type_id];
                                let other_table_id = other_type.table_id;
                                let other_table = &building.tables[other_table_id];
                                let other_type_column_id =
                                    compute_column_id(other_table, other_table_id, field).unwrap();

                                PostgresRelation::OneToMany {
                                    other_type_column_id,
                                    other_type_id,
                                    cardinality: RelationCardinality::Optional,
                                }
                            }
                            (FieldType::Plain { .. }, FieldType::Optional(_)) => {
                                let column_id = compute_column_id(table, table_id, field);

                                PostgresRelation::ManyToOne {
                                    column_id: column_id.unwrap(),
                                    other_type_id,
                                    cardinality: RelationCardinality::Optional,
                                }
                            }
                            (field_typ, other_field_type) => {
                                match (field_base_typ, compute_base_type(other_field_type)) {
                                    (FieldType::Plain(_), FieldType::List(_)) => {
                                        let column_id = compute_column_id(table, table_id, field);
                                        PostgresRelation::ManyToOne {
                                            column_id: column_id.unwrap(),
                                            other_type_id,
                                            cardinality: RelationCardinality::Unbounded,
                                        }
                                    }
                                    _ => {
                                        panic!(
                                            "Unexpected relation type for field `{}` of {:?} type. The matching field is {:?}",
                                            field.name, field_typ, other_field_type
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }
            FieldType::Optional(_) => panic!("Optional in an Optional?"),
        }
    }
}
