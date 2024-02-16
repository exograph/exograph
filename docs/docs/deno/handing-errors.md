---
sidebar_position: 4
---

# Handling Errors

Things happen. During the execution of code, it may encounter errors. Also, you may want to emit an error to users of your module. Exograph provides a mechanism to process and produce exceptions.

Normally, if a module implementation throws an error, the Exograph runtime will catch it and return a generic ("Internal error") error message to the client. This avoids inadvertently exposing implementation details to the client. While this is a good default behavior, you may want to return an explicit error to the client. For example, you may wish to indicate that the argument supplied by the user is invalid. Exograph includes a special `ExographError` type that you can throw to let the client know of an error condition.

The `ExographError` type is a subclass of the `Error` type.

```typescript
class ExographError extends Error {
  constructor(message: string);
}
```

When some code throws an `ExographError`, the Exograph runtime will catch it and return the error message provided as the constructor argument to the client.

Let's look at an example. Consider the following module definition, where the `divide` query returns the quotient and remainder of the division of two integers. However, if the denominator is zero, we want to throw an error and let the client know that the denominator cannot be zero.

```exo
@deno("arithmetic.js")
module MathModule {
    @access(true)
    type DivisionResult {
        quotient: Int
        remainder: Int
    }

    @access(true)
    query divide(numerator: Int, denominator: Int): DivisionResult
}
```

The corresponding implementation is as follows:

```typescript
export function divide(numerator: number, denominator: number): DivisionResult {
  if (denominator == 0) {
    throw new ExographError("Division by zero is not allowed");
  }

  let quotient = Math.floor(numerator / denominator);
  let remainder = numerator % numerator;

  return {
    quotient: quotient,
    remainder: remainder,
  };
}
```

With this implementation, if you run the `divide` query with a denominator of zero:

```graphql
query {
  divide(numerator: 10, denominator: 0) {
    quotient
    remainder
  }
}
```

You will get the following error:

```json
{
  "errors": [
    {
      "message": "Division by zero is not allowed"
    }
  ]
}
```
