---
sidebar_position: 4
---

# Life of an operation

When an Exograph server receives a query, it resolves the query according to the model defined in the exo file. There is quite a bit involved. In this section, we will examine the life of a query.

Since Exograph handles queries and mutations in a similar way, we will collectively refer to both as "operation"s (there are a few differences, but those are not relevant to this discussion). Note that an input payload may include multiple operations (for example, two queries). The following discussion applies to each such operation.

## Validation

When a request reaches the server, Exograph validates it against the GraphQL schema. If the query is invalid, the server will return an error.
Exograph performs the following validations:

- The operation is a syntactically valid GraphQL query or mutation.
- The shape of all arguments is correct. For example, if an argument is expected to be an `Int`, the value passed is an integer. Similarly, if an argument is a complex structure, the value passed is an object and matches the expected structure.
- All mandatory arguments are present, and no extra arguments.
- Each field in the selection exists in the model. For example, if the return value of the operation is `Blog` and the selection includes `temperature`, it will be invalid if `temperature` is not a field of `Blog`.
- All variables defined in the query have been supplied. If a query has a variable `$content`, the input payload must include a value for `$content`.
- The query is not too deep (to prevent denial of service attacks). Exograph provides a configuration option to set the maximum depth.

## Resolution

A valid operation is passed to each plugin's resolver in turn. Only one resolver is expected to return a result (the system ensures that to be the case at build time). The output of the resolver is returned to the client.

Each plugin can implement its resolution logic in any way it chooses. However, a typical plugin will perform the following steps:

### Pre-authorization

If the model or the query has any access rules, Exograph evaluates them against the context. We will use the following example to explain the process:

```exo
context AuthContext {
  @jwt role: String
}

@postgres
module BlogDatabase {
  @access(query = AuthContext.role == "admin" || self.published, mutation = AuthContext.role == "admin")
  type Blog {
    ...
    published: Boolean
  }
}
```

- If the result of the evaluation is `false`, Exograph rejects the operation. In our example, if the operation is a mutation (such as `createBlog`) and the request doesn't have a JWT token with the "role" attribute set to "admin", Exograph returns an authorization error.
- If the result is `true`, the operation is passed to the next stage. For example, for the same mutation, if the request has a JWT token with the role attribute as `admin`, Exograph passes the query to the next stage.
- If the result is some residual logical value, Exograph passes that as a filter to the next step. In our example, if the operation is a query (such as `blogs(...)`) and the JWT token does not have a role attribute set to `admin`, Exograph passes the residual logical value of `self.published` as a filter to the next step.

### Operation execution

The input operation, along with any residual access logical value, is mapped to an operation suitable for the underlying system. For example, The PostgreSQL plugin's resolver will map a GraphQL query to an SQL query, which it executes against the database. Similarly, the Deno plugin's resolver will map a GraphQL query to a JavaScript/TypeScript function call, which it executes in the embedded Deno runtime.
