use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{error::ModelSerializationError, system_serializer::SystemSerializer};

/// A type to represent the index of an interceptor within a subsystem.
///
/// This (instead of a simple `usize`) is used to make it intentional that the index is not
/// used for anything else than indexing into the interceptor list.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct InterceptorIndex(pub usize);

/// A type to represent the index of an interceptor across subsystems.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct InterceptorIndexWithSubsystemIndex {
    pub subsystem_index: usize,
    pub interceptor_index: InterceptorIndex,
}

impl InterceptorIndexWithSubsystemIndex {
    pub fn new(subsystem_index: usize, interceptor_index: InterceptorIndex) -> Self {
        Self {
            subsystem_index,
            interceptor_index,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum InterceptionTree {
    // before/after
    Intercepted {
        before: Vec<InterceptorIndexWithSubsystemIndex>,
        core: Box<InterceptionTree>,
        after: Vec<InterceptorIndexWithSubsystemIndex>,
    },
    Around {
        core: Box<InterceptionTree>,
        interceptor: InterceptorIndexWithSubsystemIndex,
    },
    // query/mutation
    Plain,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SerializableSubsystem {
    pub id: String,
    pub subsystem_index: usize,
    pub serialized_subsystem: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SerializableSystem {
    pub subsystems: Vec<SerializableSubsystem>,
    pub query_interception_map: InterceptionMap,
    pub mutation_interception_map: InterceptionMap,
}

impl SystemSerializer for SerializableSystem {
    type Underlying = Self;

    fn serialize(&self) -> Result<Vec<u8>, ModelSerializationError> {
        bincode::serialize(self).map_err(|e| ModelSerializationError::Serialize(e))
    }

    fn deserialize_reader(
        reader: impl std::io::Read,
    ) -> Result<Self::Underlying, ModelSerializationError> {
        bincode::deserialize_from(reader).map_err(|e| ModelSerializationError::Deserialize(e))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InterceptionMap {
    pub map: HashMap<String, InterceptionTree>,
}

impl InterceptionMap {
    pub fn get(&self, operation_name: &str) -> Option<&InterceptionTree> {
        self.map.get(operation_name)
    }
}
