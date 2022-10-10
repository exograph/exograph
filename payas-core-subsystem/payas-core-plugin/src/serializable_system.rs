use serde::{Deserialize, Serialize};

use super::{
    error::ModelSerializationError, interception::InterceptionMap,
    system_serializer::SystemSerializer,
};

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
pub struct SerializableSubsystem {
    pub id: String,
    pub subsystem_index: usize,
    pub serialized_subsystem: Vec<u8>,
}
