// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::fmt::Debug;

use async_graphql_parser::types::FieldDefinition;
use core_model::type_normalization::{
    default_positioned, default_positioned_name, InputValueProvider, Parameter, Type,
};
use core_model::{
    mapped_arena::SerializableSlabIndex, type_normalization::FieldDefinitionProvider,
};
use serde::{Deserialize, Serialize};

use crate::types::ModuleOperationReturnType;

use super::{argument::ArgumentParameter, module::ModuleMethod};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModuleQuery {
    pub name: String,
    pub method_id: Option<SerializableSlabIndex<ModuleMethod>>,
    pub argument_param: Vec<ArgumentParameter>,
    pub return_type: ModuleOperationReturnType,
    pub doc_comments: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModuleMutation {
    pub name: String,
    pub method_id: Option<SerializableSlabIndex<ModuleMethod>>,
    pub argument_param: Vec<ArgumentParameter>,
    pub return_type: ModuleOperationReturnType,
    pub doc_comments: Option<String>,
}

impl Operation for ModuleQuery {
    fn name(&self) -> &String {
        &self.name
    }

    fn parameters(&self) -> Vec<&dyn Parameter> {
        let mut params: Vec<&dyn Parameter> = vec![];

        for arg in self.argument_param.iter() {
            params.push(arg)
        }

        params
    }

    fn return_type(&self) -> Type {
        return_type(&self.return_type)
    }

    fn doc_comments(&self) -> Option<String> {
        self.doc_comments.clone()
    }
}

impl Operation for ModuleMutation {
    fn name(&self) -> &String {
        &self.name
    }

    fn parameters(&self) -> Vec<&dyn Parameter> {
        self.argument_param
            .iter()
            .map(|param| {
                let param: &dyn Parameter = param;
                param
            })
            .collect()
    }

    fn return_type(&self) -> Type {
        return_type(&self.return_type)
    }

    fn doc_comments(&self) -> Option<String> {
        self.doc_comments.clone()
    }
}

fn return_type(module_return_type: &ModuleOperationReturnType) -> Type {
    match module_return_type {
        ModuleOperationReturnType::Own(return_type) => return_type.into(),
        ModuleOperationReturnType::Foreign(return_type) => return_type.into(),
    }
}

pub trait Operation {
    fn name(&self) -> &String;
    fn parameters(&self) -> Vec<&dyn Parameter>;
    fn return_type(&self) -> Type;
    fn doc_comments(&self) -> Option<String>;
}

impl<S> FieldDefinitionProvider<S> for ModuleQuery {
    fn field_definition(&self, _system: &S) -> FieldDefinition {
        let fields = self
            .parameters()
            .iter()
            .map(|parameter| default_positioned(parameter.input_value()))
            .collect();

        FieldDefinition {
            description: self.doc_comments().map(default_positioned),
            name: default_positioned_name(self.name()),
            arguments: fields,
            directives: vec![],
            ty: default_positioned(self.return_type().to_graphql_type()),
        }
    }
}

impl<S> FieldDefinitionProvider<S> for ModuleMutation {
    fn field_definition(&self, _system: &S) -> FieldDefinition {
        let fields = self
            .parameters()
            .iter()
            .map(|parameter| default_positioned(parameter.input_value()))
            .collect();

        FieldDefinition {
            description: self.doc_comments().map(default_positioned),
            name: default_positioned_name(self.name()),
            arguments: fields,
            directives: vec![],
            ty: default_positioned(self.return_type().to_graphql_type()),
        }
    }
}
