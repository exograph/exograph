// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;
use std::sync::Arc;

use common::http::RequestHead;
use core_plugin_interface::interface::SubsystemLoadingError;
use core_plugin_shared::{error::ModelSerializationError, system_serializer::SystemSerializer};
use matchit::Router;
use postgres_core_model::subsystem::PostgresCoreSubsystem;
use serde::{Deserialize, Serialize};

use crate::method::Method;
use crate::operation::PostgresOperation;

#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresRestSubsystem {
    pub operations: Vec<(Method, String, PostgresOperation)>,
    #[serde(skip)]
    pub core_subsystem: Arc<PostgresCoreSubsystem>,
}

#[derive(Debug)]
pub struct PostgresRestSubsystemWithRouter {
    pub routers: HashMap<http::Method, Router<PostgresOperation>>,
    pub core_subsystem: Arc<PostgresCoreSubsystem>,
}

impl PostgresRestSubsystemWithRouter {
    pub fn new(subsystem: PostgresRestSubsystem) -> Result<Self, SubsystemLoadingError> {
        let mut routers = HashMap::new();
        for (method, path_template, operation) in subsystem.operations {
            routers
                .entry(method.into())
                .or_insert_with(Router::new)
                .insert(path_template, operation)
                .map_err(|e| SubsystemLoadingError::Config(e.to_string()))?;
        }
        Ok(Self {
            routers,
            core_subsystem: subsystem.core_subsystem.clone(),
        })
    }
}

impl PostgresRestSubsystemWithRouter {
    pub fn find_matching(
        &self,
        head: &(dyn RequestHead + Send + Sync),
        api_path_prefix: &str,
    ) -> Option<&PostgresOperation> {
        let request_path = head.get_path();

        assert!(request_path.starts_with(api_path_prefix));

        let relative_path = request_path.strip_prefix(api_path_prefix).unwrap();

        let method_router = self.routers.get(&head.get_method());

        if let Some(router) = method_router {
            return router.at(relative_path).map(|m| m.value).ok();
        }

        None
    }
}

impl SystemSerializer for PostgresRestSubsystem {
    type Underlying = Self;

    fn serialize(&self) -> Result<Vec<u8>, ModelSerializationError> {
        bincode::serde::encode_to_vec(self, bincode::config::standard())
            .map_err(ModelSerializationError::Serialize)
    }

    fn deserialize_reader(
        mut reader: impl std::io::Read,
    ) -> Result<Self::Underlying, ModelSerializationError> {
        bincode::serde::decode_from_std_read(&mut reader, bincode::config::standard())
            .map_err(ModelSerializationError::Deserialize)
    }
}
