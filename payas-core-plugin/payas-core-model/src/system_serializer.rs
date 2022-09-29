use crate::error::ModelSerializationError;

pub trait SystemSerializer {
    type Underlying;

    fn serialize(&self) -> Result<Vec<u8>, ModelSerializationError>;

    fn deserialize(bytes: &[u8]) -> Result<Self::Underlying, ModelSerializationError>;
}
