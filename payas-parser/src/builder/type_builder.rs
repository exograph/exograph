// use std::collections::HashMap;

// use id_arena::Id;
// use payas_model::{
//     model::{column_id::ColumnId, relation::ModelRelation, ModelFieldType},
//     sql::{
//         column::{ColumnReferece, PhysicalColumn, PhysicalColumnType},
//         PhysicalTable,
//     },
// };

// use super::query_builder;
// use super::system_builder::SystemContextBuilding;
// use crate::ast::ast_types::*;

// use payas_model::model::{ModelField, ModelType, ModelTypeKind};

// pub const PRIMITIVE_TYPE_NAMES: [&str; 2] = ["Int", "String"]; // TODO: Expand the list

// // pub struct SchemaType {
// //     pub name: String,
// //     pub kind: SchemaTypeKind
// // }

// // impl SchemaType {
// //     pub fn pk_field(&self) -> Option<&AstField> {
// //         self.kind.pk_field()
// //     }
// // }

// // #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
// // pub enum SchemaTypeKindKind {
// //     Int {
// //         autoincrement: bool,
// //     },
// //     Other, // For now, catch-all for other primitive types TODO: Add a variant for each supported primitive type
// //     Composite {
// //         fields: Vec<
// //     },
// // }

// // impl AstTypeKind {
// //     fn pk_field(&self) -> Option<&AstField> {
// //         match self {
// //             AstTypeKind::Composite { fields, .. } => fields
// //                 .iter()
// //                 .find(|field| matches!(&field.relation, AstRelation::Pk { .. })),
// //             _ => None,
// //         }
// //     }
// // }

// pub fn build_shallow(
//     ast_models: &HashMap<String, &AstModel>,
//     building: &mut SystemContextBuilding,
// ) {
//     for type_name in PRIMITIVE_TYPE_NAMES.iter() {
//         let typ = ModelType {
//             name: type_name.to_string(),
//             kind: ModelTypeKind::Primitive,
//             is_input: false,
//         };
//         building.types.add(type_name, typ);
//     }

//     for ast_type in ast_models.values() {
//         create_shallow_type(ast_type, ast_models, building);
//     }
// }

// pub fn build_expanded(
//     ast_types_map: &HashMap<String, &AstType>,
//     building: &mut SystemContextBuilding,
// ) {
//     for ast_type in ast_types_map.values() {
//         expand_type1(ast_type, building);
//     }
//     for ast_type in ast_types_map.values() {
//         expand_type2(ast_type, ast_types_map, building);
//     }
// }

// fn create_shallow_type(
//     ast_type: &AstModel,
//     ast_types_map: &HashMap<String, &AstModel>,
//     building: &mut SystemContextBuilding,
// ) {
//     let AstModel {
//         name: String,
//         fields: ast_fields,
//         table_name: ast_table_name,
//     } = &ast_type;

//     let table_name = ast_table_name
//         .clone()
//         .unwrap_or_else(|| ast_type.name.clone());
//     let columns = ast_fields
//         .iter()
//         .flat_map(|ast_field| create_column(ast_field, &table_name, ast_types_map))
//         .collect();

//     let table = PhysicalTable {
//         name: table_name.clone(),
//         columns,
//     };
//     building.tables.add(&table_name, table);

//     let model_type_name = ast_type.name.to_owned();
//     building.types.add(
//         &model_type_name,
//         ModelType {
//             name: model_type_name.to_owned(),
//             kind: ModelTypeKind::Primitive,
//             is_input: false,
//         },
//     );

//     let mutation_type_names = [
//         input_creation_type_name(&model_type_name),
//         input_update_type_name(&model_type_name),
//         input_reference_type_name(&model_type_name),
//     ];

//     for mutation_type_name in mutation_type_names.iter() {
//         building.mutation_types.add(
//             &mutation_type_name,
//             ModelType {
//                 name: mutation_type_name.to_string(),
//                 kind: ModelTypeKind::Primitive,
//                 is_input: true,
//             },
//         );
//     }
// }

// // Expand type except for model fields. This allows types to become `Composite` and `table_id` for any type
// // can be accessed when building fields
// fn expand_type1(ast_type: &AstModel, building: &mut SystemContextBuilding) {
//     let AstModel {
//         table_name: ast_table_name,
//         ..
//     } = &ast_type;

//     let table_name = ast_table_name
//         .clone()
//         .unwrap_or_else(|| ast_type.name.clone());
//     let table_id = building.tables.get_id(&table_name).unwrap();

//     let pk_query = building
//         .queries
//         .get_id(&query_builder::pk_query_name(&ast_type.name))
//         .unwrap();
//     let collection_query = building
//         .queries
//         .get_id(&query_builder::collection_query_name(&ast_type.name))
//         .unwrap();

//     let kind = ModelTypeKind::Composite {
//         fields: vec![],
//         table_id,
//         pk_query,
//         collection_query,
//     };
//     let existing_type_id = building.types.get_id(&ast_type.name);

//     building.types.values[existing_type_id.unwrap()].kind = kind;
// }

// fn expand_type2(
//     ast_type: &AstModel,
//     ast_types_map: &HashMap<String, &AstModel>,
//     building: &mut SystemContextBuilding,
// ) {
//     let existing_type_id = building.types.get_id(&ast_type.name).unwrap();
//     let existing_type = &building.types[existing_type_id];

//     if let ModelTypeKind::Composite {
//         table_id,
//         pk_query,
//         collection_query,
//         ..
//     } = existing_type.kind
//     {
//         let AstModel {
//             fields: ast_fields,
//             ..
//         } = &ast_type;

//         let model_fields: Vec<ModelField> = ast_fields
//             .iter()
//             .map(|ast_field| create_field(ast_field, table_id, ast_types_map, building))
//             .collect();

//         let kind = ModelTypeKind::Composite {
//             fields: model_fields.clone(),
//             table_id,
//             pk_query,
//             collection_query,
//         };

//         building.types.values[existing_type_id].kind = kind;

//         {
//             let reference_type_fields = model_fields
//                 .clone()
//                 .into_iter()
//                 .flat_map(|field| match &field.relation {
//                     ModelRelation::Pk { .. } => Some(field),
//                     _ => None,
//                 })
//                 .collect();

//             let existing_type_name = input_reference_type_name(&ast_type.name);
//             let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

//             building.mutation_types[existing_type_id].kind = ModelTypeKind::Composite {
//                 fields: reference_type_fields,
//                 table_id,
//                 pk_query,
//                 collection_query,
//             }
//         }

//         {
//             let input_type_fields = compute_input_fields(&model_fields, building, false);

//             let existing_type_name = input_creation_type_name(&ast_type.name);
//             let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

//             building.mutation_types[existing_type_id].kind = ModelTypeKind::Composite {
//                 fields: input_type_fields,
//                 table_id,
//                 pk_query,
//                 collection_query,
//             }
//         }

//         {
//             let input_type_fields = compute_input_fields(&model_fields, building, true);

//             let existing_type_name = input_update_type_name(&ast_type.name);
//             let existing_type_id = building.mutation_types.get_id(&existing_type_name).unwrap();

//             building.mutation_types[existing_type_id].kind = ModelTypeKind::Composite {
//                 fields: input_type_fields,
//                 table_id,
//                 pk_query,
//                 collection_query,
//             }
//         }
//     }
// }

// fn compute_input_fields(
//     model_fields: &Vec<ModelField>,
//     building: &SystemContextBuilding,
//     force_optional_field_modifier: bool,
// ) -> Vec<ModelField> {
//     model_fields
//         .into_iter()
//         .flat_map(|field| match &field.relation {
//             ModelRelation::Pk { .. } => None,
//             ModelRelation::Scalar { .. } => Some(ModelField {
//                 typ: field.typ.optional(),
//                 ..field.clone()
//             }),
//             ModelRelation::ManyToOne { .. } | ModelRelation::OneToMany { .. } => {
//                 let field_type_name = input_reference_type_name(&field.typ.type_name());
//                 let field_type_id = building.mutation_types.get_id(&field_type_name).unwrap();
//                 let field_plain_type = ModelFieldType::Plain {
//                     type_name: field_type_name,
//                     type_id: field_type_id,
//                 };
//                 let field_type = match field.typ {
//                     ModelFieldType::Plain { .. } => field_plain_type,
//                     ModelFieldType::Optional(_) => {
//                         ModelFieldType::Optional(Box::new(field_plain_type))
//                     }
//                     ModelFieldType::List(_) => ModelFieldType::List(Box::new(field_plain_type)),
//                 };
//                 let field_type = if force_optional_field_modifier {
//                     field_type.optional()
//                 } else {
//                     field_type
//                 };
//                 Some(ModelField {
//                     name: field.name.clone(),
//                     typ: field_type,
//                     relation: field.relation.clone(),
//                 })
//             }
//         })
//         .collect()
// }

// fn create_field(
//     ast_field: &AstField,
//     table_id: Id<PhysicalTable>,
//     ast_types_map: &HashMap<String, &AstModel>,
//     building: &SystemContextBuilding,
// ) -> ModelField {
//     fn create_model_type(
//         type_name: String,
//         ast_field_type: &AstFieldType,
//         building: &SystemContextBuilding,
//     ) -> ModelFieldType {
//         match ast_field_type {
//             AstFieldType::Plain(_) => ModelFieldType::Plain {
//                 type_name: type_name.clone(),
//                 type_id: building.types.get_id(&type_name).unwrap(),
//             },
//             AstFieldType::Optional(underlying) => ModelFieldType::Optional(Box::new(
//                 create_model_type(type_name, underlying, building),
//             )),
//             AstFieldType::List(underlying) => {
//                 ModelFieldType::List(Box::new(create_model_type(type_name, underlying, building)))
//             }
//         }
//     }

//     let type_name = ast_field.typ.name();
//     ModelField {
//         name: ast_field.name.to_owned(),
//         typ: create_model_type(type_name, &ast_field.typ, building),
//         relation: create_relation(&ast_field, table_id, ast_types_map, building),
//     }
// }

// fn create_column(
//     ast_field: &AstField,
//     table_name: &str,
//     ast_types_map: &HashMap<String, &AstModel>,
// ) -> Option<PhysicalColumn> {
//     match &ast_field.relation {
//         AstRelation::Pk => Some(PhysicalColumn {
//             table_name: table_name.to_string(),
//             column_name: ast_field
//                 .column_name
//                 .clone()
//                 .unwrap_or_else(|| ast_field.name.clone()),
//             typ: PhysicalColumnType::from_string(&ast_field.typ.name()),
//             is_pk: true,
//             is_autoincrement: match &ast_field.typ {
//                 AstFieldType::Plain(base_type) => match base_type.kind {
//                     AstTypeKind::Int { autoincrement } => autoincrement,
//                     _ => false,
//                 },
//                 _ => false,
//             },
//             references: None,
//         }),
//         AstRelation::Other { .. } => {
//             match ast_types_map.get(&ast_field.typ.name()) {
//                 Some(_) => {
//                     match ast_field.typ {
//                         AstFieldType::List(_) => None, // OneToMany, so the "many"-side type has the column

//                         _ => {
//                             let other_type = ast_types_map[&ast_field.typ.name()];
//                             let other_type_pk_field = other_type.pk_field().unwrap();
//                             let other_table_name =
//                                 if let AstTypeKind::Composite { table_name, .. } = &other_type.kind
//                                 {
//                                     table_name.clone().unwrap()
//                                 } else {
//                                     panic!("")
//                                 };

//                             Some(PhysicalColumn {
//                                 table_name: table_name.to_string(),
//                                 column_name: ast_field
//                                     .column_name
//                                     .clone()
//                                     .unwrap_or_else(|| format!("{}_id", ast_field.name)),
//                                 typ: PhysicalColumnType::from_string(
//                                     &other_type_pk_field.typ.name(),
//                                 ),
//                                 is_pk: false,
//                                 is_autoincrement: false,
//                                 references: Some(ColumnReferece {
//                                     table_name: other_table_name,
//                                     column_name: other_type_pk_field.column_name().to_string(),
//                                 }),
//                             })
//                         }
//                     }
//                 }
//                 None => {
//                     // Scalar type
//                     Some(PhysicalColumn {
//                         table_name: table_name.to_string(),
//                         column_name: ast_field
//                             .column_name
//                             .clone()
//                             .unwrap_or_else(|| ast_field.name.clone()),
//                         typ: PhysicalColumnType::from_string(&ast_field.typ.name()),
//                         is_pk: false,
//                         is_autoincrement: false,
//                         references: None,
//                     })
//                 }
//             }
//         }
//     }
// }

// fn create_relation(
//     ast_field: &AstField,
//     table_id: Id<PhysicalTable>,
//     ast_types_map: &HashMap<String, &AstModel>,
//     building: &SystemContextBuilding,
// ) -> ModelRelation {
//     fn compute_column_name(column_name: &Option<String>, ast_field: &AstField) -> String {
//         column_name
//             .clone()
//             .unwrap_or_else(|| ast_field.name.clone())
//     }

//     fn compute_column_id(
//         table: &PhysicalTable,
//         table_id: Id<PhysicalTable>,
//         column_name: &Option<String>,
//         ast_field: &AstField,
//     ) -> Option<ColumnId> {
//         let column_name = compute_column_name(column_name, ast_field);

//         table
//             .column_index(&column_name)
//             .map(|index| ColumnId::new(table_id, index))
//     }

//     let table = &building.tables[table_id];

//     match &ast_field.relation {
//         AstRelation::Pk { .. } => {
//             let column_id = compute_column_id(table, table_id, &ast_field.column_name, ast_field);
//             ModelRelation::Pk {
//                 column_id: column_id.unwrap(),
//             }
//         }
//         AstRelation::Other { optional } => {
//             match ast_types_map.get(&ast_field.typ.name()) {
//                 // Not primitive
//                 Some(_) => {
//                     match ast_field.typ {
//                         AstFieldType::List(_) => {
//                             let other_type_id =
//                                 building.types.get_id(&ast_field.typ.name()).unwrap();
//                             let other_type = &building.types[other_type_id];
//                             let other_table_id = other_type.table_id().unwrap();
//                             let other_table = &building.tables[other_table_id];
//                             let other_type_column_id = compute_column_id(
//                                 other_table,
//                                 other_table_id,
//                                 &ast_field.column_name,
//                                 ast_field,
//                             )
//                             .unwrap();

//                             ModelRelation::OneToMany {
//                                 other_type_column_id,
//                                 other_type_id,
//                             }
//                         }
//                         _ => {
//                             // ManyToOne
//                             let column_id = compute_column_id(
//                                 table,
//                                 table_id,
//                                 &ast_field.column_name,
//                                 ast_field,
//                             );
//                             let other_type_id =
//                                 building.types.get_id(&ast_field.typ.name()).unwrap();
//                             ModelRelation::ManyToOne {
//                                 column_id: column_id.unwrap(),
//                                 other_type_id,
//                                 optional: *optional,
//                             }
//                         }
//                     }
//                 }
//                 None => {
//                     // Primitive
//                     let column_id =
//                         compute_column_id(table, table_id, &ast_field.column_name, ast_field);
//                     ModelRelation::Scalar {
//                         column_id: column_id.unwrap(),
//                     }
//                 }
//             }
//         }
//     }
// }

// pub fn input_creation_type_name(model_type_name: &str) -> String {
//     format!("{}CreationInput", model_type_name)
// }

// pub fn input_update_type_name(model_type_name: &str) -> String {
//     format!("{}UpdateInput", model_type_name)
// }

// pub fn input_reference_type_name(model_type_name: &str) -> String {
//     format!("{}ReferenceInput", model_type_name)
// }
