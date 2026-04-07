// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::Result;
use async_trait::async_trait;
use clap::Command;
use core_plugin_shared::system_serializer::SystemSerializer;
use exo_env::Environment;
use postgres_core_model::{
    projection::ProjectionElement,
    relation::{PostgresRelation, RelationCardinality},
    subsystem::PostgresCoreSubsystem,
    types::{
        EntityRepresentation, EntityType, PostgresFieldDefaultValue, PostgresPrimitiveType,
        PostgresPrimitiveTypeKind, TypeIndex,
    },
};

use core_model::{mapped_arena::SerializableSlab, types::FieldType};

use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    path::PathBuf,
    sync::Arc,
};

use serde::Serialize;

use crate::commands::{
    command::{CommandDefinition, default_model_file, output_arg, resolve_output_path},
    schema::util::create_system,
    util::use_ir_arg,
};
use crate::config::Config;

// --- Output types ---

#[derive(Serialize)]
pub struct ReflectionOutput {
    pub types: BTreeMap<String, ReflectedType>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflectedType {
    pub plural_name: String,
    pub fields: BTreeMap<String, ReflectedField>,
    pub projections: BTreeMap<String, ReflectedProjection>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub unique_constraints: BTreeMap<String, ReflectedUniqueConstraint>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflectedField {
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub pk: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub auto_increment: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub list: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation: Option<ReflectedRelation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflectedRelation {
    pub kind: RelationKind,
    pub target_type: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RelationKind {
    ManyToOne,
    OneToMany,
    OneToOne,
}

#[derive(Serialize)]
pub struct ReflectedProjection {
    pub elements: Vec<ReflectedProjectionElement>,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum ReflectedProjectionElement {
    ScalarField(String),
    RelationProjection {
        field: String,
        projections: Vec<String>,
    },
    SelfProjection {
        projection: String,
    },
}

#[derive(Serialize)]
pub struct ReflectedUniqueConstraint {
    pub fields: Vec<String>,
}

// --- Command ---

pub(crate) struct ReflectCommandDefinition {}

#[async_trait]
impl CommandDefinition for ReflectCommandDefinition {
    fn command(&self) -> Command {
        Command::new("reflect")
            .about("Emit model metadata JSON for code generation and tooling")
            .arg(
                output_arg()
                    .long_help("Output file for the model metadata. Default: generated/model.json"),
            )
            .arg(use_ir_arg())
    }

    async fn execute(
        &self,
        matches: &clap::ArgMatches,
        _config: &Config,
        _env: Arc<dyn Environment>,
    ) -> Result<()> {
        let use_ir: bool = matches.get_flag("use-ir");
        let model_path: PathBuf = default_model_file();

        // TODO: Currently works only with the Postgres subsystem, but should be extended to support others.

        let serialized_system = create_system(&model_path, None, use_ir).await?;

        let mut all_types: BTreeMap<String, ReflectedType> = BTreeMap::new();

        for subsystem in &serialized_system.subsystems {
            let Ok(core_subsystem) =
                PostgresCoreSubsystem::deserialize_reader(subsystem.core.0.as_slice())
            else {
                continue;
            };

            let entity_types = &core_subsystem.entity_types;
            let primitive_types = &core_subsystem.primitive_types;
            let database = &core_subsystem.database;

            for (_, entity_type) in entity_types.iter() {
                if entity_type.representation != EntityRepresentation::Managed {
                    continue;
                }

                let table = database.get_table(entity_type.table_id);

                // Build unique constraints from physical columns
                let unique_constraints = build_unique_constraints(table, entity_type, database);

                // Build fields
                let fields = build_fields(entity_type, entity_types, primitive_types);

                // Build projections
                let projections = build_projections(entity_type);

                all_types.insert(
                    entity_type.name.clone(),
                    ReflectedType {
                        plural_name: entity_type.plural_name.clone(),
                        fields,
                        projections,
                        unique_constraints,
                    },
                );
            }
        }

        let output = ReflectionOutput { types: all_types };

        let output_path = resolve_output_path(matches, "model.json")?;

        serde_json::to_writer_pretty(&mut File::create(&output_path)?, &output)?;

        println!("Model metadata written to {}", output_path.display());

        Ok(())
    }
}

fn build_fields(
    entity_type: &EntityType,
    entity_types: &SerializableSlab<EntityType>,
    primitive_types: &SerializableSlab<PostgresPrimitiveType>,
) -> BTreeMap<String, ReflectedField> {
    let mut fields = BTreeMap::new();

    for field in &entity_type.fields {
        let is_pk = field.relation.is_pk();

        let auto_increment = matches!(
            &field.default_value,
            Some(PostgresFieldDefaultValue::AutoIncrement(_))
        );

        let is_optional = matches!(&field.typ, FieldType::Optional(_));
        let is_list = matches!(field.typ.base_type(), FieldType::List(_));
        let inner_type = field.typ.innermost();

        let type_name = inner_type.type_name.clone();

        let relation = match &field.relation {
            PostgresRelation::ManyToOne { relation, .. } => {
                let target = &entity_types[relation.foreign_entity_id];
                Some(ReflectedRelation {
                    kind: RelationKind::ManyToOne,
                    target_type: target.name.clone(),
                })
            }
            PostgresRelation::OneToMany(relation) => {
                let target = &entity_types[relation.foreign_entity_id];
                let kind = match relation.cardinality {
                    RelationCardinality::Optional => RelationKind::OneToOne,
                    RelationCardinality::Unbounded => RelationKind::OneToMany,
                };
                Some(ReflectedRelation {
                    kind,
                    target_type: target.name.clone(),
                })
            }
            _ => None,
        };

        let enum_values = match &inner_type.type_id {
            TypeIndex::Primitive(idx) => {
                let prim = &primitive_types[*idx];
                match &prim.kind {
                    PostgresPrimitiveTypeKind::Enum(values) => Some(values.clone()),
                    PostgresPrimitiveTypeKind::Builtin => None,
                }
            }
            TypeIndex::Composite(_) => None,
        };

        fields.insert(
            field.name.clone(),
            ReflectedField {
                type_name,
                pk: is_pk,
                auto_increment,
                optional: is_optional,
                list: is_list,
                relation,
                enum_values,
            },
        );
    }

    fields
}

fn build_projections(entity_type: &EntityType) -> BTreeMap<String, ReflectedProjection> {
    let mut projections = BTreeMap::new();

    for proj in &entity_type.projections {
        let elements = proj
            .elements
            .iter()
            .map(|elem| match elem {
                ProjectionElement::ScalarField(name) => {
                    ReflectedProjectionElement::ScalarField(name.clone())
                }
                ProjectionElement::RelationProjection {
                    relation_field_name,
                    projection_names,
                } => ReflectedProjectionElement::RelationProjection {
                    field: relation_field_name.clone(),
                    projections: projection_names.clone(),
                },
                ProjectionElement::SelfProjection(name) => {
                    ReflectedProjectionElement::SelfProjection {
                        projection: name.clone(),
                    }
                }
            })
            .collect();

        projections.insert(proj.name.clone(), ReflectedProjection { elements });
    }

    projections
}

fn build_unique_constraints(
    table: &exo_sql_pg::PhysicalTable,
    entity_type: &EntityType,
    database: &exo_sql_pg::Database,
) -> BTreeMap<String, ReflectedUniqueConstraint> {
    // Build a mapping from column name → entity field name
    let mut column_to_field: HashMap<String, String> = HashMap::new();

    for field in &entity_type.fields {
        match &field.relation {
            PostgresRelation::Scalar { column_id, .. } => {
                if let Some(col) = table.columns.get(column_id.column_index) {
                    column_to_field.insert(col.name.clone(), field.name.clone());
                }
            }
            PostgresRelation::ManyToOne { relation, .. } => {
                // ManyToOne FK columns map to the relation field name
                let mto = relation.relation_id.deref(database);
                for pair in &mto.column_pairs {
                    if let Some(col) = table.columns.get(pair.self_column_id.column_index) {
                        column_to_field.insert(col.name.clone(), field.name.clone());
                    }
                }
            }
            _ => {}
        }
    }

    // Aggregate unique constraints from columns
    let mut constraints: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for col in &table.columns {
        for constraint_name in &col.unique_constraints {
            let field_name = column_to_field
                .get(&col.name)
                .cloned()
                .unwrap_or_else(|| col.name.clone());
            constraints
                .entry(constraint_name.clone())
                .or_default()
                .push(field_name);
        }
    }

    constraints
        .into_iter()
        .map(|(name, fields)| (name, ReflectedUniqueConstraint { fields }))
        .collect()
}
