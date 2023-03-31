# Creating side-effects

Any non-trivial application needs to perform side-effects (additional effects to the system-generated or [user-specified mutations](./service-space.md)). Side effects come in two forms:

1. System-wide crosscutting concerns: These affect a vast range of operations and are potentially useful across many applications. Examples include rate limiting, logging, performance monitoring, and auditing.
2. Specific concerns: These affect a carefully chosen set of operations (or even a single operation) and have very application-specific behavior. Examples include notifying the shipping service upon placing an order (by publishing on a Kafka topic), updating the expiration date when a member posts a payment (a mutation in response to another mutation), and sending a welcome email upon signing up (communicating with an external service).

## Use case: Rate limiting

We want to enforce a rate limit to prevent abuse of our API. We expect the rate-limiting logic to vary a lot. For example, some systems may differentiate between anonymous users, free users, paid users, and admin users to enforce different limits. Some implementations may even consider the complexity of the query in accumulating the "usage" points (this is also considered a separate use case in itself for enforcing the complexity threshold on each query).

Developers can express this concern by writing an interceptor as follows:

```exo
@external("rate-limiting.wasm") // or .js etc.
interceptor RateLimiter {
  @around("query * || mutation *")
  intercept checkLimit(ipContext: IPContext, authContext: AuthContext, operation: Operation): OperationResult
}
```

Note, unlike user-specified queries and mutations, we don't need parameters to be marked @injected (they are implied to be @injected). Since Exograph invokes the interceptors, it supplied every parameter and we don't need the distinction between user-supplied and system-injected parameters.

The `around` interceptor surrounds the specified operations (in this case, all queries and mutations) and invokes the corresponding method, which may execute any logic and may or may not proceed with the original operation.

```rust -> wasm
fn checkLimit(ipContext: IPContext, authContext: AuthContext, operation: ProcedingOperation): OperationResult {
  let ok = RateLimitTracker::log(ipContext.address, authContext.userId)

  if ok {
    operation.proceed() // or ".run()"?
  } else {
    OperationResult::Failed("Rate limit exceeded")
  }
}


struct RateLimitTracker {
  // map to track which ip address/user is hitting the endpoints and how frequently
}

impl RateLimitTracker {
  fn log(ip: IPAddress, userId: Int) -> bool {
    ...
  }
}
```

The type `ProceedingOperation` and `Operation` has the following API.

```rust
impl Operation {
  // Question: Is there an better option than just JSON? Perhaps parsed output?
  fn params(): Vec<Json>;
  fn queryPayload(): Json; // returns the body of the opeation after resolving any fragments
}


impl ProceedingOperation /* extends Operation */ {
  fn proceed(): OperationResult
}

enum OperationResult {
  Success(Json)
  Failure(Error)
}
```

Question: Could we model this using a `before` intercept that returns a do-not-proceed value?

## Use case: Log (ideas: before intercept)

We want to log which user has caused a mutation.

```exo
@external("mutation-tracing.wasm")
interceptor Logging {
  @before("mutation *")
  intercept logMutations(authContext: AuthContext, operation: Operation): BeforeInterceptResult
}
```

```rust
fn logMutations(authContext: AuthContext, operation: Operation): : BeforeInterceptResult {
  log(operation.params, operation.queryPayload, authContext.userId);
  BeforeInterceptResult::Ok
}
```

```rust (in Exograph code) (could be just Rust's Result<()>)
enum BeforeInterceptResult {
  Ok
  Failure(Error)
}
```

If an interceptor returns `BeforeInterceptResult::Failure`, the intercepted operation isn't performed.

Note that the interface for `Operation` is such that there is no way to get the return value (which won't make sense for a before interceptor).

### Variation: log who attempted account-related mutations (ideas: patterns to select operations)

```exo
@external("mutation-tracing.wasm")
interceptor LogMutations {
  @before("mutation Account::*")
  intercept logMutations(authContext: AuthContext, operation: Operation)
}
```

Here `mutation Account::*` selects all mutation on the `Account` model: `updateAccount`, `updateAccounts`, `deleteAccount`, `deleteAccounts` etc. For user-specified queries and mutations, you would use the service name instead of the model name (for example, `LoginService::*`)

## Use case: Update member expiry upon posting a payment (ideas: after intercept, access to the return value)

```exo
@external("payment.wasm")
intercetor PaymentProcessing {
  @after("mutation Payment::*") // could also be refined as "Payment::update*" etc
  intercept updateExpiry(exo: Exo, operation: Operation, returnValue: OperationResult)
}
```

An alternative equivalent way could be:

```exo
@external("payment.wasm")
interceptor PaymentProcessing {
  intercept updateExpiry(exo: Exo, operation: Operation, returnValue: OperationResult)
}

@on(mutation="*", intercept="PaymentProcessing.updateExpiry")
model Payment {
  ...
}
```

```rust
fn updateExpiry(exo: Exo, operation: Operation, returnValue: OperationResult) {
  if /*returnValue doesn't have info needed for expiry update */ { // really an optimization to avoid an extra query
    let paymentId = returnValue.data.id; // Question: what is `id` isn't specified in the payload? Stick in "id" as a special field like Apollo?
    exo.execute(...query to get details for the payment id...)
  }

  exo.execute(...mutation to update...)
}
```

## Open questions: Priority

```
declare precedence: RateLimiter, Logging;
declare precedence: RateLimiter, PerformanceMonitoring;
```

Will lead to `RateLimiter` followed by (in arbitrary order) `Logging` and `PerformanceMonitoring`.
