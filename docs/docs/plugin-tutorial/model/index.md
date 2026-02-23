---
sidebar_position: 2
---

# Writing the plugin model

We will now start writing the plugin's model in the `kv-model` crate. A plugin's model defines the structure it uses to keep track of everything it needs to successfully resolve operations. It is parsed from a user's `.exo` file.

1.  Add `postcard` and `serde` as dependencies to `Cargo.toml`.
    Models need to be serializable in order to be written to a `.exo_ir` file.

    ```toml
    [dependencies]
    postcard = { version = "1", features = ["use-std", "alloc"] }
    serde = "1"
    core-plugin-interface.workspace = true
    ```

2.  Clear `lib.rs`.

3.  Create a `types.rs` module in the crate. We will define the plugin's inner types here.

    _lib.rs_

    ```rust
    mod types;
    ```

    Define the following structs:

    _types.rs_

    ```rust
    use core_plugin_interface::core_model::mapped_arena::SerializableSlabIndex;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Type {
        pub name: String,
        pub kind: TypeKind,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub enum TypeKind {
        Primitive,
        Composite { fields: Vec<KvCompositeField> },
        CompositeInput { fields: Vec<KvCompositeField> },
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct KvCompositeField {
        pub name: String,
        pub type_name: String,
        pub type_id: SerializableSlabIndex<Type>,
    }
    ```

    - `Type` represents a type that the plugin can work with. It consists of a name and a kind (`TypeKind`).
    - `TypeKind` defines what kind of type a ``Type` is.

      - `Primitive`s are predefined scalar types that we've brought in from Exograph (e.g. `String`, `Int`, etc.)
      - `Composite`s are types that a user might have defined in their `.exo` file in their module declaration using a `type` block:

      ```exo
      @kv
      module KeyValueModule {
          type ExampleType {
              field: String
          }
      }
      ```

      :::info
      Although a `type` block may hold fields that are of a non-scalar type, our `Composite`s will only be allowed to be composed of primitive fields for the sake of simplicity.
      :::

    Define a `SystemSerializer` implementation for `KvModelSystem`.

    ```rust
    impl SystemSerializer for KvModelSystem {
        type Underlying = Self;

        fn serialize(&self) -> Result<Vec<u8>, ModelSerializationError> {
            postcard::to_allocvec(&self).map_err(ModelSerializationError::Serialize)
        }

        fn deserialize_reader(
            mut reader: impl std::io::Read,
        ) -> Result<Self::Underlying, ModelSerializationError> {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).map_err(|e| ModelSerializationError::Other(e.to_string()))?;
            postcard::from_bytes(&bytes).map_err(ModelSerializationError::Deserialize)
        }
    }
    ```

    `SystemSerializer` is a helper trait that defines serialization and deserialization methods for models generically. Although we use `postcard` here, the model can be serialized into any format that can be represented by a `Vec<u8>`.

4.  Create a `operations.rs` module in the crate. We will define what the plugin's queries and mutations look like in here.

    _lib.rs_

    ```rust
    mod operations;
    ```

    Define the following structs:

    _operations.rs_

    ```rust
    use core_plugin_interface::core_model::mapped_arena::SerializableSlabIndex;
    use serde::{Deserialize, Serialize};

    use crate::types::Type;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Query {
        pub name: String,
        pub arguments: Vec<Argument>,
        pub return_type: OperationReturnType,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Mutation {
        pub name: String,
        pub arguments: Vec<Argument>,
        pub return_type: OperationReturnType,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct OperationReturnType {
        pub type_name: String,
        pub type_id: SerializableSlabIndex<Type>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Argument {
        pub name: String,
        pub type_name: String,
        pub type_id: SerializableSlabIndex<Type>,
    }
    ```

5.  In `lib.rs`, define `KvModelSystem`.

    ```rust
    #[derive(Debug, Serialize, Deserialize)]
    pub struct KvModelSystem {
        pub types: MappedArena<Type>,

        pub queries: MappedArena<Query>,
        pub mutations: MappedArena<Mutation>,
    }
    ```

    `exograph-kv-plugin` will need to keep track of all `Types` declared by both Exograph and the user, as well as the necessary queries and mutations it needs to support.
