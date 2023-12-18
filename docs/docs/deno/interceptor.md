---
sidebar_position: 6
---

# Interceptors

While the query and mutation provided by Postgres and Deno plugin will get you far, you will encounter situations where you must perform some additional logic before or after executing a query or mutation. For example, you may need to log queries or mutations executed, audit who and when users perform operations, add observability by monitoring the time taken by specific queries, or perform some business-specific validations. Exograph allows you to define interceptors to deal with such situations.

Let's look at an example before diving into the details. Imagine you want to monitor all queries to log the query. You can define an interceptor as follows:

```exo
@deno("log.ts")
module LogModule {
  @before("query *")
  interceptor logQuery(operation: Operation)
}
```

Here we intend to add some additional functionality before any query. The `log.ts` file contains the following code:

```ts
export function logQuery(operation: Operation) {
  console.log("Executing ${operation.name()} at ${Date.now()}");
}
```

Now whenever a query is executed, the `logQuery` interceptor is invoked and you will see output such as:

```
Executing add at 1676079424567
Executing square at 1676079424599
```

# Defining Interceptors

Interceptors are defined in modules and have the following structure:

```exo
@<interception-kind>("<interception-expression>")
interceptor <name>(<arguments>)
```

The `<interception-kind>` part specifies when the interceptor is invoked relative to the intercepted operation (a query or a mutation). The `<interception-expression>` part specifies which operations the interceptor intercepts. The `<name>` part specifies the name of the interceptor. The `<arguments>` part specifies the arguments to the interceptor.

On the implementation side, an interceptor is much like a query defined in a module: it has a name (which must match the name of the function in the associated implementation; for example, in a Deno module, in the JavaScript or TypeScript file), a list of arguments. Unlike query or mutation, however, it is invoked by Exograph whenever executing a matching operation.

Let's look at each part.

## Interception Kind

Exograph supports `before`, `after`, and `around` interceptors. Let's look at each of them.

### Before Interceptor

Exograph invokes each `before` interceptor before executing the intercepted operation. Such an interceptor is useful for performing logging, auditing, and rate-limiting. If the interceptor throws an exception, the intercepted operation is not executed. You may use this behavior to implement a gating logic such as validation or rate-limiting.

We have already seen an example of a `before` interceptor. Here is another example to illustrate using exception:

```exo
context IPContext {
  ip: string @clientIp
}

@deno("rate-limit.ts")
module RateLimitModule {
  @before("query *")
  interceptor rateLimit(context: IPContext, operation: Operation)
}
```

First, note the use of `@clientIp` annotation on the `ip` field of the `IPContext` context. This annotation captures the IP address of the caller. The `IPContext` context is then used as an argument to the `rateLimit` interceptor.

The `rate-limit.ts` file contains the logic to enforce rate-limiting. Here we use a simple in-memory map to keep track of the number of requests from each IP address. If the number of requests exceeds 100, we throw an exception. Production-ready implementations will use a more sophisticated data structure to keep track of the request count per sliding time window to enforce a quota per period.

```ts
let usageMap = new Map<string, number>();

export function rateLimit(context: IPContext, operation: Operation) {
  usageMap.set(context.ip, (usageMap.get(context.ip) || 0) + 1);
  if (usageMap.get(context.ip) > 100) {
    throw new ExographError("Too many requests");
  }
}
```

Now when you execute any query for the 101st time, you will see the following error:

```json
{
  "errors": [
    {
      "message": "Too many requests"
    }
  ]
}
```

Due to the behavior of the `before` interceptor, the intercepted query is not executed.

### After Interceptor

Exograph invokes the `after` interceptor after executing the intercepted operation. Such an interceptor is helpful in performing logging and auditing. Like the `before` interceptor, if the interceptor throws an exception, the exception is propagated to the caller. If any transaction is ongoing, Exograph will abandon that. However, if there isn't a transaction (say, sending an email using another Deno module), the intercepted operation's effect will be in place (the email would have been sent).

### Around Interceptor

The `around` this interceptor is the most versatile (you can think of the before/after interceptor as syntactic sugar). It surrounds the operation and can invoke the operation by calling the `proceed` function on the injected operation (of the `Operation` type). It can also modify the arguments and the result of the operation. This interceptor is useful for implementing caching (by returning a cached value and not invoking the operation), retrying the intercepted operation (by calling `proceed` multiple times), or even measuring the time spent on the operation (by taking the time before and after the invocation of `proceed`).

Let's implement the last use case to measure the time taken by any query.

```exo
@deno("time.ts")
module TimeModule {
  @around("query *")
  interceptor measureTime(operation: Operation)
}
```

The `time.ts` file contains the following code:

```ts
export async function measureTime(operation: Operation) {
  const start = performance.now();
  const result = await operation.proceed();
  const end = performance.now();
  console.log(`Operation '${operation.name()}' took ${end - start}ms`);
  return result;
}
```

Here, we use the `performance.now()` function to get the start time. Then we call to invoke the intercepted operation. Next, we store the result of the operation in a variable (it is the value returned by the intercepted operation; we will return this result to the caller). After the operation is complete, we get the end time and print the time taken by the operation. Finally, we return the value that was returned by the `proceed` call.

Now when you execute any query, you will see the following output:

```
Operation 'add' took 1ms
Operation 'square' took 2ms
```

Other than this output, the behavior of the query is the same as before.

## Interception Expression

Interception expressions define operations to be intercepted. The expression itself follows a simple wildcard-based selection. Each operation is identified by the operation kind ("query" or "mutation") followed by the operation name. For example, the query `getUser` is identified by `query getUser`, whereas the `sendEmail` mutation is identified as `mutation sendEmail`. The expression can contain a wildcard `*` to match any operation kind or operation name. For example, the expression `query get*` matches any query whose name starts with `get`, while the expression `query *` matches all queries. Likewise, the expression `mutation sendEmail` matches the mutation `sendEmail`, while the expression `mutation *` matches all mutations.

## Interceptor Name

The name of the interceptor identifies the corresponding function in the implementation. The implementation (TypeScript or JavaScript) must export a function with the `<name>` name with matching `<arguments>` (this is similar to how you define a query or mutation).

## Interceptor Arguments

Since it is Exograph, and not an API client that invokes the interceptor, all arguments to the interceptor are implicitly injected. Thus, unlike defining a query or mutation, you do not need to mark arguments to an interceptor with the `@inject` annotations. You may pass any type of argument that you may pass to a query or mutation (specifically, `Exograph`, `ExographPriv`, and any context objects--see [injection](injection.md) for more details).

Exograph interceptor may declare an additional type of argument: `Operation`. Exograph injects this argument and represents the operation being intercepted. This is useful for getting the operation's name, the arguments, and the result. The around interceptor can also use this to invoke the operation by calling `proceed`.

The `Operation` type is defined as follows:

```ts
interface Operation {
  name(): string;
  proceed<T>(): Promise<T>;
  query(): Field;
}
```

The `name()` method returns the name of the operation. The `proceed()` method invokes the operation and returns the result. The `query()` method returns the query or mutation being executed. This is useful for getting the arguments of the operation. The `Field` and associated types are defined as follows:

```ts
type JsonObject = { [Key in string]?: JsonValue };
type JsonValue = string | number | boolean | null | JsonObject | JsonValue[];

interface Field {
  alias: string | null;
  name: string;
  arguments: JsonObject;
  subfields: Field[];
}
```

The `alias` field is the alias of the field (if any). The `name` field is the name of the field. The `arguments` field is the arguments (such as query parameters) of the field. The `subfields` field is the subfields of the field (if any). For example, if you make the following query:

```graphql
query {
  firstUser: user(id: 1) {
    name
    email
    address {
      street
      city
    }
  }
}
```

The `query()` method will return the following object:

```json
{
  "alias": "firstUser",
  "name": "user",
  "arguments": {
    "id": 1
  },
  "subfields": [
    {
      "alias": null,
      "name": "name",
      "arguments": {},
      "subfields": []
    },
    {
      "alias": null,
      "name": "email",
      "arguments": {},
      "subfields": []
    },
    {
      "alias": null,
      "name": "address",
      "arguments": {},
      "subfields": [
        {
          "alias": null,
          "name": "street",
          "arguments": {},
          "subfields": []
        },
        {
          "alias": null,
          "name": "city",
          "arguments": {},
          "subfields": []
        }
      ]
    }
  ]
}
```

You can use this information to perform precise logging. You may also use this information to enforce complex validation rules.
