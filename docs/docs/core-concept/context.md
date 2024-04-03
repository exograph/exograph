---
sidebar_position: 1
---

# Context

A context is a representation of the incoming request and the environment. Context types define the fields and how to initialize them. A context is defined using the `context` keyword followed by the name of the context. Contexts, like types, include fields. For example, the following context defines `AuthContext` with three fields: `id`, `email`, and `captcha`.

```exo
context AuthContext {
  @jwt("sub") id: Int
  @jwt email: String
  @query("checkCaptcha") captcha: Boolean
}
```

Each field in a context type carries an annotation that denotes the source of the value. Above, we use the `@jwt` annotation to specify how to initialize those. Exograph tries to coerce the value of the payload to the field's type and abandons the request if it fails.

You can use context types in a few ways:

- In [access control expressions](/postgres/access-control.md): You can refer to context fields in access control expressions.
- As [injected dependencies](/deno/injection.md): You can declare injected arguments of the context types in queries and mutations defined in [modules](/deno/defining-modules.md) and in [interceptors](/deno/interceptor.md).
- As [default values](/postgres/customizing-types.md#default-value): You can use context fields as default values for fields in types.

Exograph supports several annotations to specify the source of the value of a field in a context type.

## JWT Token

You may use the `@jwt` annotation to extract value from the JWT token specified in the `Authentication` header (of the form `Authentication: Bearer <token>`). Exograph will decode and verify the JWT token and extract the value specified in the annotation parameter from the decoded token. The incoming request will fail if the JWT token is invalid or expired.

The `@jwt` annotation takes a single optional argument, which denotes the key in the decoded token. In the above example, for the `id` field, we specify the `"sub"` argument to extract the `"sub"` key from the JWT payload. Exograph uses the field name as the key if the annotation parameter is absent. Therefore, we didn't provide the argument for the `role` field since the field's name matches the key in the JWT payload. In other words, the following two context fields are equivalent:

```exo
@jwt role: String
@jwt("role") role: String
```

The field will be set to ' null ' if the JWT token is not present in the request.

To make the `@jwt` annotation work, you must configure JWT authentication. See [authentication](/authentication/overview.md) for more details.

## Request Header

You can use the `@header` annotation to specify a field in the context derived from the request header. The annotation parameter is the name of the header.

```exo
@header("X-Forwarded-For") connectingIp: String
```

A typical use of the `@header` annotation is to extract a header value, such as the API key or a captcha code. Then, coupled with the [query](#processed-value) annotation, you can use the header value to invoke a query and use the result as a context field.

## Cookie Value

You can use the `@cookie` annotation to extract the value of a cookie. The annotation parameter specifies the name of the cookie.

```exo
@cookie("token") token: String
```

Usages of the `@cookie` annotation are similar to the `@header` annotation.

## Environment Variable

You can use the `@env` annotation to extract an environment variable. The annotation parameter specifies the name of the environment variable.

```exo
@env("MODE") isProduction: Boolean
```

Typically, you will use the `@env` context fields to implement environment-specific authorization. For example, you can use the `CUSTOMER_ID` environment variable to specify the customer ID for the current environment and use it with some JWT token value.

```exo
context AuthContext {
  @jwt role: String
  @jwt("sub") id: Int
  @jwt customerId: Int
}

context CustomerContext {
  @env("CUSTOMER_ID") customerId: Int
}

@access(AuthContext.customerId == CustomerContext.customerId)
...

```

Here, we define an access control rule that allows access only to the customer ID specified in the environment variable.

## Processed Value

So far, we have seen how to extract raw values from the request and environment. However, you may want to process those values before using them in access control expressions or injected dependencies. For example, you may want to extract a header carrying an API key and decode it to get the customer ID, resulting in modularization of the logic to map the API key to the customer ID.

Exograph offers the `@query` annotation to process values from other contexts. The annotation takes a query name as an argument. The associated query declares other contexts as injected arguments so that the associated implementation can compute the result using those values.

Suppose you want to ensure that the user has cleared a captcha challenge. You can use the `@header` annotation to extract the captcha code. Let's first define the `CaptchaValidatorContext` context that grabs a couple of headers needed to perform the captcha validation logic.

```exo
context CaptchaValidatorContext {
    @header("X-Captcha-Id") uuid: Uuid
    @header("X-Captcha-Response")  response: String
}
```

Let's also define a `Captcha` module with a `verifyCaptcha` query, which takes the `CaptchaValidatorContext` as an injected argument.

```exo
@deno("captcha.ts")
module Captcha {
    @access(true) query verifyCaptcha(@inject context: CaptchaValidatorContext): Boolean
}
```

The associated TypeScript code in `captcha.ts` uses the header values to verify the captcha and return a boolean value:

```typescript
function verifyCaptcha(context: CaptchaValidatorContext): boolean {
  const { uuid, response } = context;
  // verify the captcha
  return true;
}
```

We now have all the ingredients to implement the captcha validation logic. We define the `CaptchaContext` context that uses the `@query` annotation to invoke the `verifyCaptcha` query and use the result as a context field.

```exo
context CaptchaContext {
    @query("verifyCaptcha") isValid: Boolean
}
```

With this setup, it is easier to express access control expressions. For example, we can use the `CaptchaContext` in the `@access` annotation to specify that the `Todo` model is only accessible if the captcha is valid.

```exo
@access(CaptchaContext.isValid)
type Todo {
  ...
}
```

Currently, the `@query` annotation is limited to queries that only take other contexts as injected arguments and return a single primitive value. We will expand this support in the future.
