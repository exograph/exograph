---
sidebar_position: 2
---

# Plugin System

Exograph is a plugin-based architecture where each plugin represents a subsystem and comes in two parts: builder and resolver. The builder processes the AST presented by the parser and produces a serialized representation of a subsystem to resolve queries and mutations. The resolver is responsible for loading the serialized representation and resolving queries and mutations. The content of the subsystem is entirely up to the plugin; as long as the builder can produce a serialized version and the corresponding resolver can consume it, the plugin can do whatever it wants. Even the serialization format is up to each plugin (currently, plugins shipped with Exograph use the [`postcard`](https://github.com/jamesmunns/postcard) format).

The separation between builder and resolver allows to emphasize different aspects in their implementation. Builders focus on good error reporting and producing an optimized model. Resolvers, on the other hand, can assume that the model is error-free and focus on performance.

Exograph core orchestrates the overall building and resolution by delegating the bulk of work to plugins. It also provides a set of common types and utilities used by plugins.

![Overview of Exograph Plugin Architecture](/exograph-plugin-architecture.png)

Out of the box, Exograph includes plugins for Postgres, Deno, and (experimental) WebAssembly subsystems.

Each plugin participates in two phases: building and resolving. Let's examine each in more detail.

## The build phase

When you build an exo file (by directly running `exo build` or indirectly through `exo dev` or `exo yolo`), the builder produces an exo_ir file (later used by the resolver). The build phase consists of the following steps:

- **Parsing**: Exograph core parses the exograph file and produces an untyped AST. This AST reflects the content of the exo files. It includes types (for each `type` element in the exo file and primitive types supported by Exograph) and contexts (for each `context` element). Exograph core then typechecks the AST to ensure the existence and correct usage of types, resulting in a typechecked AST.

- **Subsystem Building**: For each plugin registered with the system, Exograph core calls the `build` method on the plugin's builder, passes it the typechecked AST, and gathers the `SubsystemBuild` objects returned by the builder. The `SubsystemBuild` object contains a serialized representation of the subsystem and the information needed to implement interceptions. Each plugin is free to process the AST in any way it wants. A typical plugin gathers or infers information required to form a complete model (for example, the table name associated with a database type), and compute queries and mutations supported by the model. It will also attach access control rules to queries and mutations. It may bundle external resources such as Javascript code needed by the plugin. See [builder](builder.md) for more details.

- **System Building**: Exograph core then builds the overall system. It starts by processing the `SubsystemBuild` objects and produces a `SerializableSystem` object. It also produces a mapping of query/mutation names to interceptors, so that interceptors can be applied when resolving queries and mutations. The `SerializableSystem` object contains a serialized representation of the entire system and the interception map. It also ensures that the system as a whole is valid. For example, it ensures that a query or mutation is uniquely defined.

- **Serialization**: Exograph then serializes the `SerializableSystem` object into a binary format and writes it to an "exo_ir" file.

## The resolve phase

When you execute `exo-server` (or indirectly do so through `exo dev` or `exo yolo`), it starts a server, exposes an endpoint to receive GraphQL operations, and resolves them. This phase itself has two steps: loading and resolving.

### Loading

Upon startup, the server performs the following steps:

- **Deserialization**: The Exograph core deserializes the "exo_ir" file into a `SerializableSystem` object.

- **Resolver formation**: For each subsystem in the `SerializableSystem` object, Exograph core finds a matching `SubsystemLoader` and asks it to initialize the subsystem with the serialized representation. The subsystem loader is responsible for deserializing this representation and creating any necessary objects such as a connection pool. If GraphQL introspection is enabled, it also consults each resolver to find schema-related information and forms an introspection resolver. Finally, it combines all subsystem resolvers into a `SystemResolver`.

- **Server setup**: Exograph core starts a server and exposes an endpoint to receive GraphQL queries and mutations.

### Resolving

Upon receiving an operation, Exograph core validates it and asks each subsystem resolver to resolve the validated operation. Since the builder phase has ensured that each operation is uniquely defined, it knows that only one subsystem resolver will resolve an operation.

While each subsystem resolver is free to resolve an operation in any way it wants, a typical subsystem resolver first applies the access control rules and performs the resolution logic. For example, a relational database plugin may check if the user can access the associated model and perform a database query to resolve the operation. See [resolver](resolver.md) for more details.
