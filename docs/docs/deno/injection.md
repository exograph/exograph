---
sidebar_position: 3
---

# Injecting Objects

The `MathModule` shown [earlier](defining-modules.md#declaring-a-module) doesn't need much beyond the arguments passed on to it to perform its job. However, in many cases, you may need to access context, such as authentication information, or need a way to execute Exograph queries as part of your business logic. Exograph supports such usage by injecting objects into queries and mutations.

Exograph supports three types of injectable objects: any context, the `Exograph` object, and the `ExographPriv` object. Let's take a look at each. A query or mutation can declare to have an object injected using `@inject` annotation. All injected objects are omitted from the user-facing API and are available only to query or mutation implementation.

## Context Objects

Let's look at an example where we want to return the current user's name.

```exo
context AuthContext {
  @jwt id: Int
  @jwt name: String
  @jwt email: String
  @jwt role: String
}

context IPContext {
  @clientId ip: String
}

@deno
module IdentityModule {
  @access(true)
  query whoami(@inject authContext: AuthContext, @inject ipContext: IPContext): String
}
```

Here, we define `AuthContext` that contains information sources from the JWT token. Similarly, we define `IPContext` that sources its field from the client's IP address.

The `whoami` query is declared to take a regular parameter `showIp` and two injected parameters for each context defined earlier.

On the JavaScript/TypeScript side, the injected context objects have the same shape as the corresponding `context` definition. Thus, the `authContext` object will have an `id` field of the `number` type, a `name` field of the `string` type, and so on. Using these objects, the `whoami` query simply returns the name, and if the `showIp` parameter is true, also the client's IP address.

```ts
export function whoami(
  showIp: boolean,
  authContext: AuthContext,
  ipContext: IPContext
): string {
  if (showIp) {
    return `Hi '${authContext.name}' from '${ipContext.ip}'`;
  } else {
    return `Hi '${authContext.name}'`;
  }
}
```

Now you can execute the `whoami` query as follows:

```graphql
query {
  whoami(showIp: true)
}
```

Note that even though the query took two context arguments, those are not exposed to the user making the query accept a single argument. So, as you would expect, you will get a response with the calling user's name:

```json
{
  "data": {
    "whoami": "Hi 'John Doe' from '1.1.1.1'"
  }
}
```

Injected context is more useful in [interceptors](interceptor.md), so please refer to the interceptor documentation for more details.

Injecting context, especially authentication context, can be pretty powerful for implementing business logic when it needs to access some information about the calling user. However, along with these objects, you also need a mechanism to access other queries and mutations (for example, to access the database). Exograph provides a mechanism to access the database using the `Exograph` object.

## The Exograph Object

The `Exograph` object allows you to execute queries and mutations. It also allows you to set cookies and headers. The `Exograph` type is defined as:

```ts
interface Exograph {
  executeQuery(query: string, variable?: { [key: string]: any }): Promise<any>;
  addResponseHeader(name: string, value: string): Promise<void>;
  setCookie(cookie: {
    name: string;
    value: string;
    expires: Date;
    maxAge: number;
    domain: string;
    path: string;
    secure: boolean;
    httpOnly: boolean;
    sameSite: "Lax" | "Strict" | "None";
  }): Promise<void>;
}
```

Let's implement functionality to get formatted email content for a concert (so that you can show in the UI for preview and eventually send it to subscribers). We will set up a minimal model with a single `Concert` type. The preview will include a formatted version of the concert's name and description.

```exo
@postgres
module ConcertModule {
  @access(true)
  type Concert {
    @pk id: Int = autoIncrement()
    title: String
    description: String
  }
}

@deno
module EmailModule {
  @access(true)
  query preview(concertId: Int, @inject exograph: Exograph): String
}
```

The user of the `preview` query supplies it with the `concertId` parameter (say, the one obtained from the URL). However, formatting a preview requires information about the concert itself. So we need a mechanism to get the concert object for the given concert id. That is where the `Exograph` object, which allows executing queries, comes in. In this case, we will use it to execute a query to get the concert object for the given `concertId`.

We declare the `preview` query to take an `Exograph` object as an injected parameter. The `preview` implementation returns a simple HTML of the concert name and description.

```ts
export async function preview(
  exograph: Exograph,
  concertId: number
): Promise<string> {
  const data = await exograph.executeQuery(
    `query($concertId: Int!) {
      concert(concertId: $concertId) {
        title
        description
      }
    }`,
    { concertId }
  );
  const concert = data.concert;
  return `<html><body><h1>${concert.title}</h1><p>${concert.description}</p></body></html>`;
}
```

Here, we first execute a query to get the concert object. The `executeQuery` method takes the query string and variables as the arguments. Once it gets back the concert object, it uses it to format the content.

The queries you make through the `Exograph` objects are executed with the same context as the caller of the query. So, if you are calling the `preview` query as an admin user, the queries you make through the `Exograph` object will be executed as the admin user. Let's explore a similar object that lets you execute queries with a different context.

## The ExographPriv Object

The `ExographPriv` type extends `Exograph` and augments it to allow executing queries and mutations with a different context. The `ExographPriv` type is defined as:

```ts
interface ExographPriv extends Exograph {
  executeQueryPriv(
    query: string,
    variable?: { [key: string]: any },
    contextOverride?: { [key: string]: any }
  ): Promise<any>;
}
```

Note the `contextOverride?: { [key: string]: any }` parameter. This parameter allows you to override the context for the query. The `contextOverride` object should have top-level keys that match the name of the context type and values should be a JSON object with the same shape as the context type. You don't need to provide every key for a context object; Exograph will fill any missing key with the original context value. For example, if we reconsider the context defined earlier:

```exo
context AuthContext {
  @jwt id: Int
  @jwt name: String
  @jwt email: String
  @jwt role: String
}

context IPContext {
  @clientId ip: String
}
```

You can override the `role` in `AuthContext` and `ip` in `IPContext` as follows:

```json
{
  "AuthContext": {
    "role": "admin"
  },
  "IPContext": {
    "ip": "2.2.2.2"
  }
}
```

:::note
We keep the `ExographPriv` separate that `Exograph`. This way, when you take a look at an exo file and see the use of `ExographPriv`, you know that the query could be using a different context and review the code with extra care.
:::

Let's look at an example where we will implement an authentication system. As with other examples, we will keep it to a bare minimum to focus on the core idea.

```exo
@postgres
module UserModule {
  context AuthContext {
    @jwt role: String
  }

  @access(AuthContext.role == "admin")
  type User {
    @pk id: Int = autoIncrement()
    name: String
    email: String
    password: String
  }
}

@deno
module AuthModule {
  @access(true)
  query login(email: String, password: String, @inject exograph: ExographPriv): String
}
```

Note the access rule for the `User` type. It allows access only to the admin user, which is the right thing to do to avoid non-admin users getting a list of all users. So, if you try to access the `User` type as a normal user, Exograph will issue an error. However, the `login` query has a more permissive access rule, which also makes sense because you need to be able to let any user to login!

:::note
This is why Exograph defaults to secure-by-default. If you don't specify an access rule, Exograph will assume that the type or query is not accessible to anyone. This forces you to consider the access rules for your types and queries.
:::

The first thing the login implementation needs is to get the user object from the database of the matching email. If we use just the `executeQuery` method, it will be executed with the context of the caller. Since the user is yet to authenticate, due to the access rule `AuthContext.role == "admin"`, the query will fail.

This is where the `executeQueryPriv` method of `ExographPriv` comes in handy, which allows us to execute the query with a different context. In this case, we will use the context of the admin user.

```ts
export async function login(
  email: string,
  password: string
  exograph: ExographPriv,
): Promise<String> {
  const data = await exograph.executeQueryPriv(
    `query($email: String!) {
      user(where: { email: {eq: $email}}) {
        id
        name
        email
        password
      }
    }`,
    { email },
    { AuthContext: { role: "admin" }}
  );
  const user = data.user;
  if (!user || user.password !== password) {
    // Somewhat opaque error message because we don't want it to leak if this email address is registered with us.
    throw new ExographError("User not found or invalid password");
  }

  return "TODO: generate JWT token";
}
```

Note the `{ AuthContext: { role: "admin" }}` part. It defines a new context object of type `AuthContext` with the `role` field set to `admin`. This context object will be used to execute the query. So, the query will be executed as the admin user, which has access to the `User` type.
