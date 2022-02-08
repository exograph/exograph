use payas_model::{
    model::{
        access::Access,
        column_id::ColumnId,
        mapped_arena::{MappedArena, SerializableSlabIndex},
        naming::ToGqlQueryName,
        relation::{GqlRelation, RelationCardinality},
        GqlCompositeType, GqlCompositeTypeKind, GqlFieldType,
    },
    sql::{
        column::{FloatBits, IntBits, PhysicalColumn, PhysicalColumnType},
        PhysicalTable,
    },
};

use super::{
    access_utils,
    resolved_builder::{
        ResolvedAccess, ResolvedField, ResolvedFieldDefault, ResolvedFieldKind, ResolvedFieldType,
        ResolvedMethod, ResolvedType, ResolvedTypeHint,
    },
};
use super::{resolved_builder::ResolvedCompositeType, system_builder::SystemContextBuilding};

use crate::{
    ast::ast_types::AstExpr,
    builder::resolved_builder::ResolvedCompositeTypeKind,
    error::ParserError,
    typechecker::{PrimitiveType, Typed},
};

use payas_model::model::{GqlField, GqlType, GqlTypeKind};

pub fn build_shallow(models: &MappedArena<ResolvedType>, building: &mut SystemContextBuilding) {
    for (_, model_type) in models.iter() {
        create_shallow_type(model_type, building);
    }
}

pub fn build_expanded(
    resolved_types: &MappedArena<ResolvedType>,
    resolved_methods: &[&ResolvedMethod],
    building: &mut SystemContextBuilding,
) -> Result<(), ParserError> {
    for (_, model_type) in resolved_types.iter() {
        if let ResolvedType::Composite(c) = &model_type {
            expand_type_no_fields(c, building, resolved_types);
        }
    }

    for (_, model_type) in resolved_types.iter() {
        if let ResolvedType::Composite(c) = &model_type {
            expand_type_fields(c, building, resolved_types);
        }
    }

    for (_, model_type) in resolved_types.iter() {
        if let ResolvedType::Composite(c) = &model_type {
            expand_type_access(c, building)?;
        }
    }

    for method in resolved_methods.iter() {
        expand_method_access(method, building)?
    }

    Ok(())
}

fn create_shallow_type(resolved_type: &ResolvedType, building: &mut SystemContextBuilding) {
    let type_name = resolved_type.name();

    // Mark every type as Primitive, since other types that may be referred haven't been processed yet
    // and we haven't build query and mutation types either
    building.types.add(
        &type_name,
        GqlType {
            name: type_name.to_string(),
            plural_name: resolved_type.plural_name(),
            kind: GqlTypeKind::Primitive,
            is_input: false,
        },
    );
}

/// Expand a type except for creating its fields.
///
/// Specifically:
/// 1. Create and set the table (if applicable)
/// 2. Create and set *_query members (if applicable)
/// 3. Leave fields as an empty vector
///
/// This allows types to become `Composite` and `table_id` for any type can be accessed when building fields in the next step of expansion.
/// We can't expand fields yet since creating a field requires access to columns (self as well as those in a refered field in case a relation)
/// and we may not have expanded a refered type yet.
fn expand_type_no_fields(
    resolved_type: &ResolvedCompositeType,
    building: &mut SystemContextBuilding,
    resolved_types: &MappedArena<ResolvedType>,
) {
    let kind = if let ResolvedCompositeTypeKind::Persistent { table_name } = &resolved_type.kind {
        let table_name = table_name.clone();

        let columns = resolved_type
            .fields
            .iter()
            .flat_map(|field| create_column(field, &table_name, resolved_types))
            .collect();

        let table = PhysicalTable {
            name: table_name.clone(),
            columns,
        };

        let table_id = building.tables.add(&table_name, table);

        let pk_query = building.queries.get_id(&resolved_type.pk_query()).unwrap();

        let collection_query = building
            .queries
            .get_id(&resolved_type.collection_query())
            .unwrap();

        GqlTypeKind::Composite(GqlCompositeType {
            fields: vec![],
            kind: GqlCompositeTypeKind::Persistent {
                table_id,
                pk_query,
                collection_query,
            },
            access: Access::restrictive(),
        })
    } else {
        GqlTypeKind::Composite(GqlCompositeType {
            fields: vec![],
            kind: GqlCompositeTypeKind::NonPersistent,
            access: Access::restrictive(),
        })
    };

    let existing_type_id = building.types.get_id(&resolved_type.name);

    building.types.values[existing_type_id.unwrap()].kind = kind;
}

/// Now that all types have table with them (set in the earlier expand_type_no_fields phase), we can
/// expand fields
fn expand_type_fields(
    resolved_type: &ResolvedCompositeType,
    building: &mut SystemContextBuilding,
    resolved_types: &MappedArena<ResolvedType>,
) {
    let existing_type_id = building.types.get_id(&resolved_type.name).unwrap();
    let existing_type = &building.types[existing_type_id];

    if let GqlTypeKind::Composite(GqlCompositeType { kind, .. }) = &existing_type.kind {
        let table_id = match kind {
            GqlCompositeTypeKind::Persistent { table_id, .. } => Some(*table_id),
            GqlCompositeTypeKind::NonPersistent => None,
        };

        let model_fields: Vec<GqlField> = resolved_type
            .fields
            .iter()
            .map(|field| create_field(field, table_id, building, resolved_types))
            .collect();

        let kind = GqlTypeKind::Composite(GqlCompositeType {
            fields: model_fields,
            kind: kind.clone(),
            access: Access::restrictive(),
        });

        building.types.values[existing_type_id].kind = kind;
    }
}

// Expand access expressions (pre-condition: all model fields have been populated)
fn expand_type_access(
    resolved_type: &ResolvedCompositeType,
    building: &mut SystemContextBuilding,
) -> Result<(), ParserError> {
    let existing_type_id = building.types.get_id(&resolved_type.name).unwrap();
    let existing_type = &building.types[existing_type_id];

    if let GqlTypeKind::Composite(self_type_info) = &existing_type.kind {
        let expr = compute_access_composite_types(&resolved_type.access, self_type_info, building)?;

        let kind = GqlTypeKind::Composite(GqlCompositeType {
            fields: self_type_info.fields.clone(),
            kind: self_type_info.kind.clone(),
            access: expr,
        });

        building.types.values[existing_type_id].kind = kind;
    }

    Ok(())
}

fn expand_method_access(
    resolved_method: &ResolvedMethod,
    building: &mut SystemContextBuilding,
) -> Result<(), ParserError> {
    let existing_method_id = building.methods.get_id(&resolved_method.name).unwrap();
    let expr = compute_access_method(&resolved_method.access, building)?;
    building.methods.values[existing_method_id].access = expr;

    Ok(())
}

fn compute_access_composite_types(
    resolved: &ResolvedAccess,
    self_type_info: &GqlCompositeType,
    building: &SystemContextBuilding,
) -> Result<Access, ParserError> {
    let access_expr = |expr: &AstExpr<Typed>| {
        access_utils::compute_predicate_expression(expr, Some(self_type_info), building)
    };

    Ok(Access {
        creation: access_expr(&resolved.creation)?,
        read: access_expr(&resolved.read)?,
        update: access_expr(&resolved.update)?,
        delete: access_expr(&resolved.delete)?,
    })
}

fn compute_access_method(
    resolved: &ResolvedAccess,
    building: &SystemContextBuilding,
) -> Result<Access, ParserError> {
    let access_expr =
        |expr: &AstExpr<Typed>| access_utils::compute_predicate_expression(expr, None, building);

    Ok(Access {
        creation: access_expr(&resolved.creation)?,
        read: access_expr(&resolved.read)?,
        update: access_expr(&resolved.update)?,
        delete: access_expr(&resolved.delete)?,
    })
}

fn create_field(
    field: &ResolvedField,
    table_id: Option<SerializableSlabIndex<PhysicalTable>>,
    building: &SystemContextBuilding,
    env: &MappedArena<ResolvedType>,
) -> GqlField {
    fn create_field_type(
        field_type: &ResolvedFieldType,
        building: &SystemContextBuilding,
    ) -> GqlFieldType {
        match field_type {
            ResolvedFieldType::Plain(r) => GqlFieldType::Reference {
                type_name: r.clone(),
                type_id: building.types.get_id(r).unwrap(),
            },
            ResolvedFieldType::Optional(underlying) => {
                GqlFieldType::Optional(Box::new(create_field_type(underlying, building)))
            }
            ResolvedFieldType::List(underlying) => {
                GqlFieldType::List(Box::new(create_field_type(underlying, building)))
            }
        }
    }

    GqlField {
        name: field.name.to_owned(),
        typ: create_field_type(&field.typ, building),
        relation: if let Some(table_id) = table_id {
            create_relation(field, table_id, building, env)
        } else {
            GqlRelation::NonPersistent // was not provided a table id, type is not persistent in database
                                       // TODO: rethink GqlField with non-persistent models in mind
        },
        has_default_value: field.default_value.is_some(),
    }
}

fn create_column(
    field: &ResolvedField,
    table_name: &str,
    env: &MappedArena<ResolvedType>,
) -> Option<PhysicalColumn> {
    // Check that the field holds to a self column
    let unique = match &field.kind {
        ResolvedFieldKind::Persistent {
            self_column,
            unique,
            ..
        } => {
            if !self_column {
                return None;
            }
            *unique
        }
        ResolvedFieldKind::NonPersistent => {
            panic!("Non-persistent fields are not supported")
        }
    };
    // split a Optional type into its inner type and the optional marker
    let (typ, optional) = match &field.typ {
        ResolvedFieldType::Optional(inner_typ) => (inner_typ.as_ref(), true),
        _ => (&field.typ, false),
    };

    let default_value = field
        .default_value
        .as_ref()
        .map(|default_value| match default_value {
            ResolvedFieldDefault::Value(val) => Some(match &**val {
                AstExpr::StringLiteral(string, _) => format!("'{}'", string.replace("'", "''")),
                AstExpr::BooleanLiteral(boolean, _) => format!("{}", boolean).to_ascii_uppercase(),
                AstExpr::NumberLiteral(val, _) => {
                    format!("{}", val)
                }
                _ => panic!("Invalid concrete value"),
            }),
            ResolvedFieldDefault::DatabaseFunction(string) => Some(string.to_string()),
            ResolvedFieldDefault::Autoincrement => None,
        })
        .flatten();

    match typ {
        ResolvedFieldType::Plain(type_name) => {
            // Either a scalar (primitive) or a many-to-one relationship with another table
            let field_type = env.get_by_key(type_name).unwrap();

            match field_type {
                ResolvedType::Primitive(pt) => Some(PhysicalColumn {
                    table_name: table_name.to_string(),
                    column_name: field.get_column_name().to_string(),
                    typ: determine_column_type(pt, field),
                    is_pk: field.get_is_pk(),
                    is_autoincrement: if field.get_is_autoincrement() {
                        assert!(typ.deref(env) == &ResolvedType::Primitive(PrimitiveType::Int));
                        true
                    } else {
                        false
                    },
                    is_nullable: optional,
                    is_unique: unique,
                    default_value,
                }),
                ResolvedType::Composite(ct) => {
                    // Many-to-one:
                    // Column from the current table (but of the type of the pk column of the other table)
                    // and it refers to the pk column in the other table.
                    let other_pk_field = ct.pk_field().unwrap();
                    Some(PhysicalColumn {
                        table_name: table_name.to_string(),
                        column_name: field.get_column_name().to_string(),
                        typ: PhysicalColumnType::ColumnReference {
                            ref_table_name: ct.get_table_name().to_string(),
                            ref_column_name: other_pk_field.get_column_name().to_string(),
                            ref_pk_type: Box::new(determine_column_type(
                                &other_pk_field.typ.deref(env).as_primitive(),
                                field,
                            )),
                        },
                        is_pk: false,
                        is_autoincrement: false,
                        is_nullable: optional,
                        is_unique: unique,
                        default_value,
                    })
                }
            }
        }
        ResolvedFieldType::List(typ) => {
            // unwrap list to base type
            let mut underlying_typ = typ;
            let mut depth = 1;

            while let ResolvedFieldType::List(t) = &**underlying_typ {
                underlying_typ = t;
                depth += 1;
            }

            let underlying_pt = if let ResolvedFieldType::Plain(name) = &**underlying_typ {
                if let Some(ResolvedType::Primitive(pt)) = env.get_by_key(name) {
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
                    column_name: field.get_column_name().to_string(),
                    typ: determine_column_type(&pt, field),
                    is_pk: false,
                    is_autoincrement: false,
                    is_nullable: optional,
                    is_unique: unique,
                    default_value,
                })
            } else {
                // this is a OneToMany relation, so the other side has the associated column
                None
            }
        }
        ResolvedFieldType::Optional(_) => panic!("Optional in an Optional?"),
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

    if let Some(hint) = &field.get_type_hint() {
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
            | PrimitiveType::ClaytipInjected
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
    env: &MappedArena<ResolvedType>,
) -> GqlRelation {
    fn compute_column_id(
        table: &PhysicalTable,
        table_id: SerializableSlabIndex<PhysicalTable>,
        field: &ResolvedField,
    ) -> Option<ColumnId> {
        let column_name = field.get_column_name().to_string();

        table
            .column_index(&column_name)
            .map(|index| ColumnId::new(table_id, index))
    }

    let table = &building.tables[table_id];

    if field.get_is_pk() {
        let column_id = compute_column_id(table, table_id, field);
        GqlRelation::Pk {
            column_id: column_id.unwrap(),
        }
    } else {
        fn compute_base_type(field_type: &ResolvedFieldType) -> &ResolvedFieldType {
            match field_type {
                ResolvedFieldType::Optional(inner_typ) => inner_typ.as_ref(),
                _ => field_type,
            }
        }
        // we can treat Optional fields as their inner type for the purposes of computing relations
        let field_base_typ = compute_base_type(&field.typ);

        match field_base_typ {
            ResolvedFieldType::List(underlying) => {
                if let ResolvedType::Primitive(_) = underlying.deref(env) {
                    // List of a primitive type is still a scalar from the database perspective
                    GqlRelation::Scalar {
                        column_id: compute_column_id(table, table_id, field).unwrap(),
                    }
                } else {
                    // If the field is of a list type and the underlying type is not a primitive,
                    // then it is a OneToMany relation with the self's type being the "One" side
                    // and the field's type being the "Many" side.
                    let field_type = underlying.deref(env).as_composite();

                    let other_type_id = building.types.get_id(field_type.name.as_str()).unwrap();
                    let other_type = &building.types[other_type_id];
                    let other_table_id = other_type.table_id().unwrap();
                    let other_table = &building.tables[other_table_id];

                    let other_type_column_id =
                        compute_column_id(other_table, other_table_id, field).unwrap();

                    GqlRelation::OneToMany {
                        other_type_column_id,
                        other_type_id,
                        cardinality: RelationCardinality::Unbounded,
                    }
                }
            }

            ResolvedFieldType::Plain(type_name) => {
                let field_type = env.get_by_key(type_name).unwrap();

                match field_type {
                    ResolvedType::Primitive(_) => {
                        let column_id = compute_column_id(table, table_id, field);
                        GqlRelation::Scalar {
                            column_id: column_id.unwrap(),
                        }
                    }
                    ResolvedType::Composite(ct) => {
                        // A field's type is "Plain" or "Optional" and the field type is composite,
                        // but we can't be sure if this is a ManyToOne or OneToMany unless we examine the other side's type.

                        let other_resolved_type = env.get_by_key(type_name).unwrap();
                        let other_type_field_typ = &other_resolved_type
                            .as_composite()
                            .fields
                            .iter()
                            .find(|f| f.get_column_name() == field.get_column_name())
                            .unwrap()
                            .typ;

                        let other_type_id = building.types.get_id(&ct.name).unwrap();

                        match (&field.typ, other_type_field_typ) {
                            (ResolvedFieldType::Optional(_), ResolvedFieldType::Plain(_)) => {
                                let other_type = &building.types[other_type_id];
                                let other_table_id = other_type.table_id().unwrap();
                                let other_table = &building.tables[other_table_id];
                                let other_type_column_id =
                                    compute_column_id(other_table, other_table_id, field).unwrap();

                                GqlRelation::OneToMany {
                                    other_type_column_id,
                                    other_type_id,
                                    cardinality: RelationCardinality::Optional,
                                }
                            }
                            (ResolvedFieldType::Plain(_), ResolvedFieldType::Optional(_)) => {
                                let column_id = compute_column_id(table, table_id, field);

                                GqlRelation::ManyToOne {
                                    column_id: column_id.unwrap(),
                                    other_type_id,
                                    cardinality: RelationCardinality::Optional,
                                }
                            }
                            (field_typ, other_field_type) => {
                                match (field_base_typ, compute_base_type(other_field_type)) {
                                    (ResolvedFieldType::Plain(_), ResolvedFieldType::List(_)) => {
                                        let column_id = compute_column_id(table, table_id, field);
                                        GqlRelation::ManyToOne {
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
            ResolvedFieldType::Optional(_) => panic!("Optional in an Optional?"),
        }
    }
}
