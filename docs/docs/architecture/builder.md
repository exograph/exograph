---
sidebar_position: 3
---

# Builder

The builder is given a typechecked AST and a base system and builds them into a subsystem. The base system consists of the context types (included in an exo file as `context` elements) and primitive types supported by Exograph such as `Int` and `String`.

In the current version of Exograph, the Postgres builder is responsible for:

- Associating types with tables and fields with columns as well as relationships between tables
- Defining queries and mutations
- Associating access control rules with queries and mutations
- Associating interceptors with queries and mutations

Similarly, the Deno subsystem builder is responsible for:

- Associating each module definition with a Deno module
- Associating each query, mutation, and interceptors with a function in the module. The builder will also process the JavaScript/TypeScript code to produce a bundle that is then embedded in the builder's output.
- Associating access control rules with queries and mutations
- Associating interceptors with queries and mutations
