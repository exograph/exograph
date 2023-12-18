---
sidebar_position: 20
---

# Configuration

With JWT-based authentication, the token issuer signs JWT claims, and receivers (an Exograph server, for example) check its validity. Exograph supports two authentication methods out of the box: symmetric key and OpenID Connect.

With either form, the clients pass the `Authorization` header with the JWT token to the GraphQL endpoint with each request. On the Exograph model side, both methods work identically with the `@jwt` annotation, so nothing changes in the application code.

:::note
While Exograph has dedicated support for JWT authentication, it is possible to implement other forms of authentication using the `@query` annotation. For example, you could extract any header value, such as `X-API-Token`, decode it, and use it in access control rules. See [this blog](https://exograph.dev/blog/retrograde-mercury) for an esoteric, yet interesting, example.
:::

## Symmetric Key

This method uses the same secret key to sign and verify the JWT token. Since it involves a single key, the issuer and receiver must be the same (unless you share the key between the issuing server and the receiving servers, which can be done only in particular situations).

To enable symmetric JWT authentication, set the `EXO_JWT_SECRET` environment variable to the secret key to `exo dev` or `exo-server` commands. You can also set it with `exo yolo` to override the automatically generated secret key.

```shell-session
# shell-command-next-line
EXO_JWT_SECRET=secret exo dev
```

This helps to keep the secret key stable across multiple invocations of `exo yolo`.

With this method, your Exograph server must manage users and associated information such as passwords and roles. You can include a type to represent users in your model. You must also provide an authentication query to authenticate users and return a JWT token. You can implement a Deno module that exports a `login` function. You may also need to implement a "sign-in" mutation. Please see [a complete example](https://github.com/exograph/examples/tree/main/todo-with-nextjs-google-auth) of using symmetric JWT authentication. The example uses Google Identity, but you can easily extend it to work with other providers and email/password login. Furthermore, you will need another query to refresh the JWT token.

You can alternatively use an external authentication provider such as [Auth0](https://auth0.com) or [Clerk](https://clerk.dev), which we will explore next.

## OpenID Connect

[OpenID Connect (OIDC)](https://openid.net/developers/how-connect-works/) is a standard for authentication supported by many authentication providers such as Auth0 and Clerk. The underlying mechanism uses a public/private key pair to sign and verify the JWT token. Unlike symmetric key authentication, OIDC authentication does not require the Exograph server to manage users. Instead, it relies on the authentication provider for that.

To enable OIDC-based authentication, set the `EXO_OIDC_URL` environment variable to point to the authentication provider's URL to `exo dev` or `exo-server` commands.

By default, `exo yolo` uses symmetric JWT authentication. To use OIDC authentication, specify the `EXO_OIDC_URL` environment variable.

```shell-session
# shell-command-next-line
EXO_OIDC_URL=https://<your-authentication-provider-url> exo yolo
```

Please see a complete example [with Clerk](https://github.com/exograph/examples/tree/main/todo-with-nextjs-clerk-auth) and [with Auth0](https://github.com/exograph/examples/tree/main/todo-with-nextjs-auth0-auth) for how to use OIDC authentication.
