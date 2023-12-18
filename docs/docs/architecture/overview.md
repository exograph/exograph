---
sidebar_position: 1
---

# Overview

Exograph has two distinct phases: [building](builder.md) and [resolving](resolver.md). When you run `exo build`, it converts a user-defined "exo" file to an "exo_ir" file for the Exograph runtime. When you run `exo-server`, it loads the "exo_ir" file and starts a server that can resolve queries and mutations.

Exograph is a **[plugin](plugin.md)**-based architecture, where each plugin is responsible for a subsystem such as Postgres and Deno.

The Exograph core takes care of many common tasks. In the builder phase, it parses the exo file (reporting any errors), typechecks the AST, and passes it down to each plugin. The plugin then forms its subsystem and returns a serialized representation of it. The core then combines the subsystems into a single model and serializes it to an "exo_ir" file.

For the resolver phase, the Exograph core loads the exo_ir file, extracts individual subsystems, and passes them to the corresponding plugin. Each plugin then deserializes its subsystem and returns a resolver that can resolve operations for that subsystem. The core then starts an HTTP server with telemetry configured and exposes a GraphQL endpoint. When the server receives a request, it parses the query, validates it, and passes the validated operation to a matching subsystem.
