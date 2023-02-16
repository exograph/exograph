# Services in Claytip

To allow integrating non-database functionality, we need to support services.
Services come in two flavors:

1. Services that expose queries and mutations directly to the API. Use cases
   include resetting passwords, sending concert notifications, and
   authentication.
2. Services that other services need but do not expose direct APIs. Use cases
   include a lower-level email service, rate limiting, and performance
   monitoring.

Both flavors of services require user input, a way to execute GraphQL queries
and mutations, and other services.

This document deals with services that need to be executed directly as a result
of a query or mutation (and services to support those). Services such as rate
limiting that apply to a wide range of operations or services that get invoked
as a side-effect of some "mainline" query/mutation (for example, updated
membership expiration when `updatePayment` mutation executes) will be discussed
in a separate document.

## Use case: Email service (new concepts: private service, needs initialization)

For many other use cases such as forgot password, reminders, and concert
notifications, we need a way to send emails. The `EmailService` acts as a proxy
for an email provider such as Mailgun. This service isn't meant to be exposed
through the APIs.

```clay
@external("email.ts") // or .ts or .js or .so
service EmailService {
  @construct fn construct(@inject env: Env) // the function name is insignificant; you may have at most one @construct function per service
                                            // the return type is implied to be the service type

  /* no export */ mutation send(emails: Array[String], message: String): Result<bool, String>
}
```

We define a service and specify its implementation source (a Webassembly, shared
object, or ts/js file). This is similar to how Scala.js specifies facades to
existing js code. Here we assume that parameters and result types are
de/serializable to JSON.

The inclusion of `construct` implies that the service has a non-default
constructor. Claytip will call it at the system startup time (we may later
introduce @lazy annotation to delay it to just before the first invocation) and
store away that service instance. Question: do we need a destructor spec as
well; we won't be able to guarantee to run it except for a well-executed
shutdown.

The lack of `export` means this service will not be available as a GraphQL
mutation.

The `email.ts` file will look like:

```ts
class EmailService {
  smtpSender: SmtpSender

  constructor(env: Env) {
    this.smtpSender = new SmtpSender(env.get("SMTP_URL", ....));
  }
}

async function construct(env: Env): EmailService {
  new EmailService(env);
}

async function send(emails: string[], message: string)(@inject emailSender: EmailSender): Result<boolean, string> {
  await emailSender.smtpSender.send(emails, message)
}
```

## Use case: Send email notification (new concepts: exported service, dependency on other services)

Admin wants to send a notification for an upcoming concert. The notification
itself is saved as a `ConcertNotification` model. When sending an email
notification, the admin (through UI) specifies the `ConcertNotification`'s id
and a `SubscriptionGroup`'s id. Here is a mutation that will do the job.

```graphql
mutation sendNotification(concertNotificationId: Int, subscriptionGroupId: Int): Result<bool, String>
```

Assume the following Clay model:

```clay
model ConcertNotification {
  @pk id: Int = autoIncrement()
  concert: Concert? // Allow null for sending general notification without a concert
  preBlurb: String?
  postBlurb: String?
}

model Subscription {
  @pk id: Int = autoIncrement()
  email: String
  groups: Set[SubscriptionGroup] // many-to-many
}

model SubscriptionGroup {
  @pk id: Int = autoIncrement()
  name: String // "test", "all", "admins"
  subscriptions: Set[Subscription] // many-to-many
}
```

We need a service to format the notification email and send it. The formatted
text looks like:

```
Pre-blurb

Concert Content

Post-blurb

Next concert title and link (if any)
```

In service implementation, generating the formatted email based on the input to
the mutation requires the following query:

```graphql
fragment artistInfo on ConcertArtist {
  artist {
    name
  }
  instrument
}

concertNotification(id: $id) {
  concert {
    mainArtists: concertArtists(where: {role: {eq: "main"}}, orderBy: {rank: ASC}) {
      ...artistInfo
    }
    accompanyingArtists: concertArtists(where: {role: {eq: "accompanying"}}, orderBy: {rank: ASC}) {
      ...artistInfo
    }
    description
    startTime
    endTime
    venue {
      title
      address
    }
    price
  }
  preBlurb
  postBlurb
}
```

And after receiving a response, another query to get the next concert.

```graphql
query {
  concerts(where: {startDate: {gte: <the concerts-end-time; current time if no concert is provided>}}, orderBy: {startTime: ASC}, limit: 1) {
    id # For generating the URL
    title
    mainArtists: concertArtists(where: {role: {eq: "main"}}, orderBy: {rank: ASC}) {
      ...artistInfo
    }
    startTime # to show the date
    # We don't show accompanying arists for the next concert
  }
}
```

The proposal allows writing the following service:

```clay
@external("concert-notification.wasm") // or .js or .ts or .so
service ConcertNotificationService {
  @auth(AuthContext.role == "ROLE_ADMIN")
  export mutation sendNotification(
    concertNotificationId: Int, # From GraphQL input
    subscriptionGroupId: Int) # From GraphQL input
    (@injected clay: Clay,
    @injected emailService: EmailService): Result<bool, String>
}
```

Note that `EmailService` is defined in the earlier section.

The `export` keyword implies that the query or mutation should be directly
exposed through Claytip's GraphQL API. The @injected annotation specifies
dependencies to be injected (and not supplied by the user through the GraphQL
API). To clearly separate user-supplied argument from those injected by the
system, we require that each kind be grouped together in a curried parameter
style.

The system (Claytip) supplies the `@injected` parameters based on the type. For
example, the system will pass a singleton object for the parameters of type
`Clay` and `EmailService`, whereas it will pass the current `AuthContext` based
on the information in the request's header.

Note that a service may forgo the `EmailService` dependency and manage the email
sending all by itself.

`Result` is a pre-defined type:

```
model Result<Success, Error> {
  success: Success?
  error: Error?
}
```

Here, both the `Success` and `Result` types must be model types themselves or
primitives (hence can be serialized to JSON)

This will expose a mutation that the front-end can invoke as follows:

```graphql
mutation {
  sendNotification(concertNotificationId: 55, subscriptionGroupId: 2) {
    success
    error
  }
}
```

The `concert-notification.ts` file may look like the following:

```ts
async function sendNotification(
  concertNotificationId: number,
  subscriptionGroupId: number,
  clay: Clay,
  emailService: EmailService
): Result<boolean, string> {
  let concertNotification = await clay.execute(
    "...concertNotification(id: $id) {...",
    { id: concertNotificationId }
  );
  let nextConcert = await clay.execute(
    "...concerts(where: {startDate: {gte: <the concerts-end-time...>}}",
    { startDate: concertNotification.endTime | currentTime }
  );
  let emails = await clay.execute("subscriptions(where...", {
    groupId: subscriptionGroupId,
  });

  const formatted: string = formatNotification(
    concertNotification,
    nextConcert
  );

  return await emailService.send(emails, formatted);
}
```

# Use case: Authentication (new concept: namespaced model)

Currently, we don't support authentication and let apps rely on external
authentication services such as Auth0/Supertoken or self-hosting of an
application based on code such as [next-auth](https://next-auth.js.org/). In
either case, the service returns a JWT token. Claytip has built-in support
validating JWT tokens (but "should it" is a separate discussion).

So this is how we can support flexible authentication in Claytip (in place of
next-auth; Auth0/Supertoken cases live outside of Claytip and will continue that
way).

```clay
@external("authentication.wasm") // or ".so" or ".js" or ".ts"
service Authentication {
  model LoginInput {
    provider: String // "google", "facebook", "username-password"
    code: String?
    username: String?
    password: String?
  }

  model LoginResult {
    id: String # From Google, Facebook, etc (not from our database)
    name: String
    email: String
    profilePicture: String
    refreshToken: String
  }

  model LoginError {
    kind: String # "network", "invalid-credentials", "unsupported-kind"
    info: String
  }

  export async mutation authenticate(loginInfo: LoginInput, @inject claytip: Claytip): Result<String, LoginError>;
}
```

The corresponding Rust code (which will be compiled to WASM) looks as follows:

```rust
fn authenticate(loginInfo: LoginInput, claytip: Claytip): Result<String, LoginError> {
    if (loginInfo.provider == "google") {
      match GoogleApi.verify(loginInfo.code, "secret-key") {
        Ok((id, name, email, profilePicture, refreshToken)) => {
          let userId = await claytip.execute(
            "mutation updateRefreshTokens($email: String, provider: $String) {
              updateRefreshTokens(where: and: [{email: {eq: $email}}, {provider: {eq: $provider}}], data: $data}) {
                id # doesn't matter
              }
            }",
            JSON.Map(("email", email), ("provider", provider), ("refreshToken": refreshToken))
          )[0];

          Result::success(JWT(id, email, ....).to_string())
        }
        Err(err) => Result::failure(err.to_string())
      }
    } else if (...) {

    }
  }
```

Now the UI can invoke mutation such as:

```graphql
mutation socialLogin(provider: String, code: String) {
  login(loginInfo: {provider: $provider, code: $code}) {
    result
    error  # One of result or error will be null
  }
}
```

# Use case: Connecting to other REST services and similarly for external GraphQL services (new concept: no external code)

Sometimes all you need is to connect to an external (REST) service. In that
case, we can simplify to obviate the need to write any code.

```clay
@rest(url="https://<payment-provider>.com/api/v1", headers=[CLIENT_CREDENTIALS: ${ENV{"CRED"}}])
service PaymentProcessor {
  model Payment {
    amount: Float
    currency: String
  }

  model PaymentId {
    id: String
  }

  @post
  mutation createPayment(payment: Payment): PaymentId

  @get("/{paymentId}")
  query getPaymentDetails(paymentId: PaymentId): Payment
}
```

Here, we have an service (here not exported, but by just adding `export` could
expose these queries and mutations externally).
