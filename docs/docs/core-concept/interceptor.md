---
sidebar_position: 5
---

# Interceptors

An interceptor is a function that Exograph executes before, after, or around matching queries and mutations. Interceptors are defined using the `interceptor` keyword. Exograph defers execution of an interceptor to the plugin, however, standardizing on annotations and the expressions supplied to them.

Here is an example of an interceptor that executes around any query:

```exo
@deno("timing.ts")
module Timing {
  @around("query *")
  interceptor time(operation: Operation)
}
```

See the [Deno interceptor](deno/interceptor.md) for Deno module specifics.
