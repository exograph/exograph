use bytes::{Buf, Bytes};

use crate::error::ModelSerializationError;

pub trait SystemSerializer {
    type Underlying;

    fn serialize(&self) -> Result<Vec<u8>, ModelSerializationError>;

    fn deserialize_reader(
        reader: impl std::io::Read,
    ) -> Result<Self::Underlying, ModelSerializationError>;

    fn deserialize(bytes: Vec<u8>) -> Result<Self::Underlying, ModelSerializationError> {
        Self::deserialize_reader(Bytes::from(bytes).reader())
    }
}
