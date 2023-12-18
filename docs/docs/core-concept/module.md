---
sidebar_position: 2
---

# Modules

Modules--along with plugins--form the core concept of Exograph. Exograph defines a general structure for modules and leaves the interpretation of the module definition to plugins. Exograph requires each plugin to support a few common patterns. For example, each plugin must support expressions in an `@access` annotation consistently.

:::tip
Advanced users of Exograph can write new plugins to extend Exograph's functionality. See [Plugins](/plugin-tutorial/overview.md) for more details.
:::

A module is defined using the `module` keyword:

```exo
module TodoPersistence {
  // module definition
}
```

A module definition specifies the plugin to interpret its contents using the annotation with the plugin name. For example, the following module specifies the Postgres plugin using the `@postgres` annotation.

```exo
// highlight-next-line
@postgres
module TodoPersistence {
  // module definition
}
```

The annotation corresponding to the plugin may take arguments.

```exo
@deno("email.ts")
module EmailModule {
  // module definition
}
```

Like the content of the module definition, the plugin interprets the argument to the annotation. For example, the Deno plugin interprets the argument as the path to the file with the module implementation.

Depending on the plugin, a module definition may include types, queries, mutations, and interceptors. We will take a brief look at each of these. Please refer to [Postgres module](/postgres/overview.md) and [Deno module](/deno/overview.md) for more details on the structure of a module.
