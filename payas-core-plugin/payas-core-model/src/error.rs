use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelSerializationError {
    #[error("{0}")]
    IO(#[from] std::io::Error),

    #[error("Unable to serialize model {0}")]
    Serialize(#[source] bincode::Error),

    #[error("Unable to deserialize model {0}")]
    Deserialize(#[source] bincode::Error),
}
