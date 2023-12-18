---
sidebar_position: 1
---

# Defining Modules

You define a Postgres module using the `@postgres` annotation. The module may contain type definitions, which are mapped to tables in the database.

```exo
@postgres
module TodoDatabase {
  // Types
}
```

In the current version, the `@postgres` annotation doesn't take any parameters and the module name is for organizational purposes. In future, Exograph will allow database configuration through this annotation and use the name of the module as a namespace for the types defined in it.

In the [next section](defining-types.md), we will look at defining types in a Postgres module.
