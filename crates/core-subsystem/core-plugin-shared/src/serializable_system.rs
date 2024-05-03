// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use serde::{Deserialize, Serialize};

use crate::trusted_documents::TrustedDocuments;

use super::{
    error::ModelSerializationError, interception::InterceptionMap,
    system_serializer::SystemSerializer,
};

const PREFIX_TAG: &[u8] = b"exograph";
const PREFIX_TAG_LEN: usize = PREFIX_TAG.len();

#[derive(Serialize, Deserialize, Debug)]
pub struct SerializableSystem {
    pub subsystems: Vec<SerializableSubsystem>,
    pub query_interception_map: InterceptionMap,
    pub mutation_interception_map: InterceptionMap,
    pub trusted_documents: TrustedDocuments,
}

/// File header data for exo_ir files.
/// Used to check that the file version information matches the current version
/// of the code. The list of plugin names is also stored in the header but is
/// not checked. Deserializing a file without the necessary subsysytem loader
/// should fail at a later stage since a matching loader won't be found.
#[derive(Serialize, Deserialize, Debug)]
struct Header {
    builder_version: String,
    ir_version: String,
    plugins: Vec<String>,
}

impl Header {
    fn new(plugins: Vec<String>) -> Header {
        let version = env!("CARGO_PKG_VERSION").to_string();
        Header {
            builder_version: version.clone(),
            ir_version: version,
            plugins,
        }
    }

    fn check_header(&self, header: Header) -> Result<(), String> {
        if self.ir_version != header.ir_version {
            return Err(format!(
                "Version for this file {0} does not match current version {1}",
                header.ir_version, self.ir_version
            ));
        }
        if self.builder_version != header.builder_version {
            return Err(format!(
                "Builder version for this file {0} does not match current version {1}",
                header.builder_version, self.builder_version
            ));
        }
        Ok(())
    }
}

impl SystemSerializer for SerializableSystem {
    type Underlying = Self;

    fn serialize(&self) -> Result<Vec<u8>, ModelSerializationError> {
        serialize_header_and_system(&Header::new(vec![]), self)
    }

    fn deserialize_reader(
        mut reader: impl std::io::Read,
    ) -> Result<Self::Underlying, ModelSerializationError> {
        // TODO: ModelSerializationError should not be dependent on bincode errors since
        // it is used by subsystem serializers which may use other formats.
        fn error(msg: &str, io_error: Option<std::io::Error>) -> ModelSerializationError {
            let msg = match io_error {
                Some(e) => format!("{msg}: {e}"),
                None => msg.to_string(),
            };
            ModelSerializationError::Deserialize(bincode::ErrorKind::Custom(msg).into())
        }
        {
            // Check the file prefix
            let mut prefix = [0_u8; PREFIX_TAG_LEN];
            reader
                .read_exact(&mut prefix)
                .map_err(|e| error("Failed to read exograph prefix", Some(e)))?;

            if prefix != PREFIX_TAG {
                return Err(error("Invalid exograph file prefix", None));
            }
        }
        // Serialize header len as u64 to make exo_ir platform independent (32-bit vs 64-bit systems)
        let header_len = {
            let mut header_len = [0_u8; std::mem::size_of::<u64>()];
            reader
                .read_exact(&mut header_len)
                .map_err(|e| error("Failed to read exograph header size", Some(e)))?;
            u64::from_le_bytes(header_len)
        };
        let header_len = header_len.try_into().map_err(|_| {
            error(
                "Failed to convert the exo_ir file header size to usize",
                None,
            )
        })?;
        // Check the header
        let mut header_bytes = vec![0_u8; header_len];
        reader
            .read_exact(&mut header_bytes)
            .map_err(|e| error("Failed to read the exo_ir file header", Some(e)))?;
        let header: Header = bincode::deserialize(&header_bytes).map_err(|e| {
            error(
                &format!("Failed to deserialize the exo_ir file header: {0}", e),
                None,
            )
        })?;
        let current_header = Header::new(vec![]);
        current_header
            .check_header(header)
            .map_err(|e| error(&e, None))?;

        bincode::deserialize_from(reader).map_err(ModelSerializationError::Deserialize)
    }
}

fn serialize_header_and_system(
    header: &Header,
    system: &SerializableSystem,
) -> Result<Vec<u8>, ModelSerializationError> {
    let header: Vec<u8> = bincode::serialize(header).map_err(ModelSerializationError::Serialize)?;
    let header_len: u64 = u64::try_from(header.len()).map_err(|e| {
        ModelSerializationError::Serialize(Box::new(bincode::ErrorKind::Custom(format!(
            "Failed to convert header len to u64 {e:?}"
        ))))
    })?;

    let header_len: Vec<u8> = header_len.to_le_bytes().to_vec();
    let system = bincode::serialize(system).map_err(ModelSerializationError::Serialize)?;
    Ok([PREFIX_TAG.to_vec(), header_len, header, system].concat())
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SerializableSubsystem {
    pub id: String,
    pub subsystem_index: usize,
    pub serialized_subsystem: Vec<u8>,
}

#[cfg(test)]
mod test {
    use super::SerializableSystem;
    use crate::{interception::InterceptionMap, system_serializer::SystemSerializer};
    use multiplatform_test::multiplatform_test;
    use std::collections::HashMap;

    fn mk_system() -> SerializableSystem {
        let query_interception_map = InterceptionMap {
            map: HashMap::new(),
        };
        let mutation_interception_map = InterceptionMap {
            map: HashMap::new(),
        };
        SerializableSystem {
            subsystems: vec![],
            query_interception_map,
            mutation_interception_map,
            trusted_documents: super::TrustedDocuments::all(),
        }
    }

    #[multiplatform_test]
    fn serialize_deserialize_ok() {
        let system = mk_system();
        let bytes = system.serialize().expect("System should serialize");
        let _ = SerializableSystem::deserialize_reader(bytes.as_slice())
            .expect("Deserialization should succeed");
    }

    #[multiplatform_test]
    fn deserialize_different_version() {
        let system = mk_system();
        let mut header = super::Header::new(vec![]);
        header.builder_version = "0.0.1".to_string();
        let system_bytes =
            super::serialize_header_and_system(&header, &system).expect("Should serialize");
        let result = SerializableSystem::deserialize_reader(system_bytes.as_slice());
        assert!(
            result.is_err(),
            "Old builder_version should fail to deserialize"
        );

        let mut header = super::Header::new(vec![]);
        header.ir_version = "0.0.1".to_string();
        let system_bytes =
            super::serialize_header_and_system(&header, &system).expect("Should serialize");
        let result = SerializableSystem::deserialize_reader(system_bytes.as_slice());
        assert!(result.is_err(), "Old ir_version should fail to deserialize");
    }
}
