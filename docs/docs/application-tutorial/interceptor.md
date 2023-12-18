---
sidebar_position: 40
---

# Monitoring Performance

Let's monitor the time taken by any query in the system. In a real-world application, you would use a telemetry system, but like the email example, we will use the console to log the time taken.

Let's write a new module with an interceptor. Since we want our interceptor to surround each query to get the before and after timestamps, we will use an `@around` interception.

```exo
@deno("time.js")
module Timing {
  @around("query *")
  interceptor time(operation: Operation)
}
```

For a change, we will implement the logic in JavaScript (TypeScript would have worked just as well). Since we want to measure each query's elapsed time, we will use the `performance` API.

```js
export async function time(operation) {
  const start = performance.now();
  const result = await operation.proceed();
  const end = performance.now();
  console.log(`'${operation.name()}' took ${end - start} ms`);
  return result;
}
```

Now rerun any query operation such as `concerts`. You will see the time taken by the query in the console.

```
'concerts' took 27.300909 ms
```

For fun, try the `sendEmail` mutation. You will see the time taken by the queries made by it.

```
'concert' took 0.9891450000031909 ms
'subscribers' took 0.8302920000023732 ms
```

It doesn't matter how the queries are invoked--directly by the user or as a part of some user-implemented logic--the interceptor will measure the time taken.
