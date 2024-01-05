---
sidebar_position: 6
---

# Access Control

One of the most powerful features of Exograph is its ability to specify access control rules for Postgres types. By co-locating type definitions with access control rules, you can define a model that is both self-documenting and self-enforcing. You can examine the exo file and immediately understand the access control rules for each type.

Exograph supports access control for the Postgres module using the `@access` annotation on types. Queries and mutations associated with each type inherit the access control rules of the type. For example, if a type defines access control to limit querying only to "admin" users, all queries for that type will only be accessible to "admin" users.

The access annotation specifies rules as varied as allowing everyone to access a query, only to those with specific roles, based on a particular field's value of the accessed object, users from specific IP addresses, users with valid captcha, date and time, or any combination. This is possible because the context of a request is a flexible concept, and you can model it from various sources.

The `@access` annotation takes a boolean expression evaluated in the context of the request and the objects being accessed.

```exo
@access(expression)
```

If needed, you can specify separate rules for queries and mutations (and even specific queries and mutations). We will examine that in the [Separating queries and mutations access control](#separating-queries-and-mutations-access-control) section.

## A quick example

The typical access control rules look as follows:

```exo
@postgres
module BlogModule {
  // highlight-start
  @access(
    query = AuthContext.role == "admin" || self.published,
    mutation = AuthContext.role == "admin"
  )
  // highlight-end
  type Blog {
    @pk id: Int = autoIncrement()
    title: String
    content: String
    // highlight-next-line
    published: Boolean
    publishedOn: LocalDateTime
  }
}
```

Here, we have defined a `Blog` type along with access control rules for queries and mutations associated with the type.

Access control expressions for queries act as a gate and filter for existing data. The expressions carry additional semantics for mutations; we will examine them in the [Effect on Mutation APIs](#effect-on-mutation-apis) section.

Consider the above definition along with the following query and mutation:

```graphql
query {
  blogs(where: { publishedOn: { gt: "2022-08-18" } }) {
    id
    title
    content
  }
}
```

```graphql
mutation {
  createBlog(data: { title: "Hello", content: "World" }) {
    id
  }
}
```

If you run the above query and mutation, you will get the following result:

<table>
    <thead>
        <tr>
            <th>Operation</th>
            <th>Invoking user</th>
            <th>Result</th>
        </tr>
    </thead>
    <tbody>
        <tr>
            <td rowspan="2">Query</td>
            <td>Admin</td>
            <td>All blogs</td>
        </tr>
        <tr>
            <td>Non-admin</td>
            <td>All published blogs</td>
        </tr>
        <tr>
            <td rowspan="2">Mutation</td>
            <td>Admin</td>
            <td>Success</td>
        </tr>
        <tr>
            <td>Non-admin</td>
            <td>Authorization error</td>
        </tr>
    </tbody>
</table>

When an admin user executes the query, the access control expression `AuthContext.role == "admin" || self.published` evaluates to `true`. Thus, Exograph will not apply any further filtering. Specifically, it will return all blogs, including those not published, as long as they were published after August 18, 2022. Similarly, if an admin user executes the `createBlog` mutation, the access control expression evaluates to `true`, and the mutation will be allowed.

If a non-admin user makes a query, the access control expression evaluates to a `self.published` residue. Exograph will apply that residue to get only published blogs, as if the user had also specified `published: {eq: true}` in the where clause, effectively making the where clause as `where: {and: [publishedOn: {gt: "2022-08-18"}, {published: {eq: true}]}`. If a non-admin user makes a mutation, the access control expression evaluates to `false`, and the mutation will return an authorization error.

Access control will be in effect for all queries and mutations associated with the type--even the nested ones. For example, if we have modeled a `User` type with a `blogs` field of the `Set<Blog>` type, accessing the `blogs` field will be subject to the same access control rules as the `Blog` type. In other words, _no matter how you access a particular type, Exograph prevents unintended access_.

We will first examine the primitive elements that form the access control expression. Then, we will discuss combining these elements to form access expressions.

## Primitive Elements

An access control expression can use context objects, literals, and the special `self` object to refer to the accessed object. It can then combine these using relational and logical operations. So, let's dive a bit deeper into these elements.

### Context Objects

[Context objects](/core-concept/context.md) model certain aspects of the incoming request, such as the user's role, IP address, or other information. Let's consider an exo file with the following contexts:

```exo
context AuthContext {
  @jwt role: String
}

context IPContext {
  @clientIp ip: String
}

context CaptchaContext {
  @query("verifyCaptcha") valid: Boolean
}
```

Now, you can use `AuthContext.role` to refer to the user's role, `IPContext.ip` to refer to the user's IP address, and `CaptchaContext.valid` to refer to a captcha's validity.

### Literals

Literals specify a value directly in expressions such as `true`, `false`, `1`, and `"hello"`.

### The `self` object

Sometimes, an access control expression must refer to the accessed object. For example, you may want to allow access to a blog only if the blog is published. In this case, you can use the special `self`. For example, in the example above, you can use `self.published` to refer to the blog's `published` field. This is, of course, valid only while defining access control for the `Blog` type.

Using these elements, you can express rules to control access to the object.

## Relational and Logical Operations

You can combine these elements using relational and logical operations. Let's take a look at these operations.

### Relational Operations

To compare numeric values, you can use the relational operators: `==`, `!=`, `<`, `<=`, `>`, and `>=`. For example, you can use `AuthContext.id == 100` to check if the user's id is 100 and `self.price > 100` to check if the price of the accessed object is greater than 100.

You may use `==` and `!=` to compare boolean values. For example, you can use `CaptchaContext.isValid == true` to check captcha validity. However, usually, you will use the value as is or its negation. For example, you may write the earlier check simply as `CaptchaContext.isValid`. Similarly, you can use `!CaptchaContext.isValid` to check if the captcha is invalid.

You can use `==` and `!=` to compare strings. For example, you can use `AuthContext.role == "admin"` to check if the user's role is "admin".

You can use `in` to check if the value is in a set of values. This works for any type. For example, you can use `AuthContext.role in ["admin", "manager"]` to check if the user's role is either "admin" or "manager".

### Logical Operations

You can combine expressions with the logical operators `&&`, `||`, and `!`. For example, you can use `AuthContext.role == "admin" || AuthContext.role == "manager"` to check if the user's role is either "admin" or "manager". Similarly, you can use`EnvContext.isDevelopment && CaptchaContext.isValid` to ascertain that the captcha has been validated and that the app is in development mode.

You may use parentheses to group expressions. For example, you can use `(AuthContext.role == "admin" || AuthContext.role == "manager") && CaptchaContext.isValid` to check if the user's role is either "admin" or "manager" and that the captcha is valid.

## Examples

Equipped with the above elements, you can now form more access control expressions. Let's take a look at some examples.

### Using literals

The simplest access control expression is literal. For example, the following expression will allow access to all users:

```exo
@access(true)
```

While the following expression will deny access to all users:

```exo
@access(false)
```

:::note
The default access control expression is `false`. This secure-by-default approach forces you to think about access control for each type explicitly.
:::

When you use `access(false)` for a mutation, it has the effect of removing that API. For more details, please see the [Effect on Mutation APIs](#effect-on-mutation-apis) section.

### Using context objects

You can use context objects to express rules based on the user's role. For example, the following expression will allow access to all users with the "admin" role:

```exo
context AuthContext {
  role: String
}

@access(AuthContext.role == "admin")
```

If the context contained `roles` instead of a single `role`, you could use the `in` operator to check if the user has a particular role:

```exo
context AuthContext {
  roles: Array<String>
}

@access("admin" in AuthContext.roles)
```

### Using the `self` object

You can use the `self` object to refer to the accessed object. We've already seen an example in the [A Quick Example](#quick-example) section. Let's take a look at another example. Consider the following exo file:

```exo
context AuthContext {
  @jwt("sub") id: Int
  @jwt role: String
}

@postgres
module BlogModule {
  // highlight-next-line
  @access(self.owner.id == AuthContext.id)
  type Blog {
    @pk id: Int = autoIncrement()
    title: String
    content: String
    // highlight-next-line
    owner: AuthUser
  }

  @access(AuthContext.role == "admin")
  type AuthUser {
    @pk id: Int = autoIncrement()
    name: String
  }
}
```

Here, the access control rule on the `Blog` type specifies that the user can access a blog only if the owner is accessing it. So, not only the `self` can refer to the accessed object, but it can also refer to its fields.

### Using higher-order functions

Consider a document management system where users can create and share documents with others with read or write permission. You can model this using a `Permission` type that connects documents with users (forming a many-to-many relationship) and the permission kind (read or write). The access control for the `Document` type needs to ascertain that there is some permission such that:

- its user is the same as the user accessing the document, and
- the user can read (for queries) or write (for mutations).

You can express this rule using the `some` higher-order function. The `some` function works the same way as JavaScript's [some](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/some) function over an array. It declares a placeholder name (in the following code `permission`) and an expression, which may use this placeholder and any other values (such as `self` or any context) to create a boolean expression (in the following code `permission.user.id == AuthContext.id && permission.read` or `permission.user.id == AuthContext.id && permission.write`).

```exo
context AuthContext {
  @jwt("sub") id: Int
}

@postgres
module DocsDatabase {
  @access(
    // highlight-next-line
    query = self.permissions.some(permission => permission.user.id == AuthContext.id && permission.read),
    // highlight-next-line
    mutation = self.permissions.some(permission => permission.user.id == AuthContext.id && permission.write)
  )
  type Document {
    @pk id: Int = autoIncrement()
    content: String
    permissions: Set<Permission>
  }

  @access(
    query = self.user.id == AuthContext.id && self.read,
    mutation = self.user.id == AuthContext.id && self.write
  )
  type Permission {
    @pk id: Int = autoIncrement()
    document: Document
    user: User
    read: Boolean
    write: Boolean
  }

  @access(self.id == AuthContext.id)
  type User {
    @pk id: Int = autoIncrement()
    name: String
    permissions: Set<Permission>
  }
}
```

If you want to combine it with additional rules, such as giving admin users full access, you may do so, as we will see next.

### Combining expressions

You can combine expressions using the logical operators `&&`, `||`, and `!`. We have seen an example of this in the [A Quick Example](#a-quick-example) section, where we used `AuthContext.role == "admin" || self.published` to ensure that an "admin" user gets unfettered access to blogs. In contrast, a non-admin user can only access a published blog.

## Separating queries and mutations access control

Often, you need to provide a separate rule for queries and mutations. Furthermore, you may want to provide a different rule for each kind of mutation. For example, you may allow all users to query for a list of todos but only "admin" users to create and update todos. The `@access` annotation lets you specify a separate access control expression for queries and mutations.

### Sharing rules for queries and mutations

If you provide a single expression, it will apply to all queries and mutations associated with the type. Consider the following example, where the `@access` annotation has a single expression:

```exo
@postgres
module ProductDatabase {
  @access(AuthContext.role == "admin")
  type Product {
    @pk id: Int = autoIncrement()
    name: String
    price: Float
  }
}
```

All queries and mutations for the `Product` type will only be accessible to the "admin" user. All others will get an authorization error.

### Separating Query and Mutation Access Control

Often, queries may be allowed for a wider audience than mutations. For example, you may want all users to query for a list of products but only the "admin" users to mutate. In such cases, you can separate access control expression for queries and mutations:

```exo
@postgres
module ProductDatabase {
  @access(
    query = true,
    mutation = AuthContext.role == "admin"
  )
  type Product {
    @pk id: Int = autoIncrement()
    name: String
    price: Float
  }
}
```

All queries for the `Product` type will be accessible to all users, while all mutations will only be accessible to the "admin" user. For any non-admin user, all mutations will return an authorization error.

### Access Control for separate mutations

You can also specify access control for individual mutations:

```exo
context AuthContext {
  role: String
}

@postgres
module ProductDatabase {
  @access(
    query = true,
    create = AuthContext.role == "admin",
    update = AuthContext.role == "manager" || AuthContext.role == "admin",
    delete = AuthContext.role == "super-admin" || (self.price < 100 && AuthContext.role == "admin")
  )
  type Product {
    @pk id: Int = autoIncrement()
    name: String
    price: Float
  }
}
```

Here, anyone may query for a list of products, but only the "admin" user can create a product. Any "manager" or "admin" users can update a product. A product may be deleted by a "super-admin" user or an "admin" user if the product's price is less than 100.

### Precedence rules

Since an access control expression may specify any of the `query`, `mutation`, `create`, `update`, and `delete` attributes, Exograph uses the following precedence rules to determine which expression to use.

<table>
    <thead>
        <tr>
            <th>Lower Precedence</th>
            <th></th>
            <th>Higher Precedence</th>
        </tr>
    </thead>
    <tbody>
        <tr>
            <td rowspan="5">Anonymous expression</td>
            <td rowspan="2">query</td>
        </tr>
        <tr>
            <td>query</td>
        </tr>
        <tr>
            <td rowspan="3">mutation</td>
            <td>create</td>
        </tr>
        <tr>
            <td>update</td>
        </tr>
        <tr>
            <td rowspan="2">delete</td>
        </tr>
    </tbody>
</table>

Another way to look at access control precedence is to know that most specific rule takes precedence over the more general rule. For example, if you specify an access control expression for a particular mutation, Exograph will use that expression. If a higher precedence expression is not defined, the expression for the next lower precedence will take effect.

## Effect on Mutation APIs

Access expression of the form `@access(false)`, whether explicitly specified or the default, removes the mutation from APIs. For example, with the following access expression, none of the mutations will be available in the API.

```exo
@postgres
module ProductDatabase {
  @access(false)
  type Product {
    @pk id: Int = autoIncrement()
    name: String
    price: Float
  }
}
```

If you want to remove only a particular mutation, you can specify the access expression for that mutation. For example, the following access expression will remove the `updateProduct` mutation from the API (see [Separating queries and mutations access control](#separating-queries-and-mutations-access-control) for more details):

```exo
@postgres
module ProductDatabase {
  @access(
    query = true,
    create = true,
    delete = true,
    // highlight-next-line
    update = false // This line could be omitted since the default is false
  )
  type Product {
    @pk id: Int = autoIncrement()
    name: String
    price: Float
  }
}
```

Such an arrangement is helpful to avoid unnecessary mutations in the API.

## Access Control as Invariants

Access control expressions act as invariants, simplifying modeling your domain data access rules. The core idea is to ensure that the input and existing data satisfy the access control expression.

Consider the following example, where access control rules should be such that:

1. Query: Anyone should be able to read any blog
2. Create mutation: Users can create a blog only for themselves (it is an error to specify another user as the blog owner). Admin users can create a blog for any user.
3. Update mutation: Users can update a blog only if they own it. Admin users can update any blog.
4. Delete mutation: Users can delete a blog only if they own it. Admin users can delete any blog.

The first rule requires setting the `query` attributes to `access(true)`. Since Exograph interprets access control expressions as invariants, all other rules can be expressed using the ownership condition: `self.owner.id == AuthContext.id` combined with `AuthContext.role == "admin"`. Note the default value expressions to let create mutations skip the `owner` field.

```exo
context AuthContext {
  @jwt("sub") id: Int
  @jwt role: String
}

@postgres
module BlogModule {
  @access(query = true, mutation = self.owner.id == AuthContext.id || AuthContext.role == "admin")
  type Blog {
    @pk id: Int = autoIncrement()
    title: String
    content: String
    // highlight-next-line
    owner: AuthUser = AuthContext.id
  }

  @access(AuthContext.role == "admin")
  type AuthUser {
    @pk id: Int = autoIncrement()
    name: String
  }
}
```

Let's see how this works for each of the rules. In each case, we will assume that the user is not an admin (since the evaluation for admin users is trivial and uninteresting from the invariance point of view).

### Create mutations

For `create` mutations, access control expressions act as preconditions. The typical mutation will look as follows (due to the default value expression, Exograph will automatically set the `owner` field to the current user):

```graphql
mutation {
  createBlog(data: { title: "Hello", content: "World" }) {
    id
  }
}
```

Suppose a mutation leaves out the `owner` field (or specifies it to the same value as the accessing user's id), the `self.owner.id == AuthContext.id` expression will evaluate to `true`, and the mutation will be allowed. If the mutation specifies a different user, the access control expression will evaluate to `false`, and the mutation will return an authorization error.

Note that admin users can specify a different user as the owner, and the mutation will be allowed (due to the `AuthContext.role == "admin"` part of the expression).

### Update mutations

For `update` mutations, access control expressions act as preconditions. The typical mutation will look as follows:

```graphql
mutation {
  updateBlog(id: 1, data: { title: "Hello", content: "Updated World" }) {
    id
  }
}
```

Here, the access control expression will be evaluated twice:

1. Against input data (the same way as for `create` mutations). This prevents the user from updating the `owner` field to another user.
2. Against the existing data in the database. This prevents the user from updating a blog they do not own.

The overall effect is that the user can update only their blogs and cannot change the owner of their blog.

Like the `create` mutations, admin users can specify the `owner` field in the input to transfer blog ownership.

### Delete mutations

Delete mutations only evaluate against the existing data in the database since there is no input data. In other words, the expression evaluation is the same as for a query.

## Field-level access control

Imagine the following model where the `Product` type has two pricing fields: sales price and purchase price.

```exo
@postgres
module ProductDatabase {
  @access(query = true, mutation = AuthContext.role == "admin")
  type Product {
    @pk id: Int = autoIncrement()
    name: String
    // highlight-start
    salePrice: Float
    purchasePrice: Float
    // highlight-end
  }
}
```

Due to access control `@access(true)`, all products&mdash;and their fields&mdash;are accessible to all users. However, you likely want to restrict access to the `purchasePrice` field to users with elevated privileges. Exograph's field-level access control helps model such restrictions. For example, to limit access to the `purchasePrice` field to only "admin" users, you can specify access control for the field:

```exo
@postgres
module ProductDatabase {
  @access(query = true, mutation = AuthContext.role == "admin")
  type Product {
    @pk id: Int = autoIncrement()
    name: String
    salePrice: Float
    // highlight-start
    @access(AuthContext.role == "admin")
    purchasePrice: Float
    // highlight-end
  }
}
```

Only "admin" users can query the `purchasePrice` field. For example, if a non-admin user makes the following query, they will get the result.

```graphql title="Will return the result for any user"
query {
  products {
    id
    name
    salePrice
  }
}
```

However, if they specify the `purchasePrice` field, they will get an authorization error.

```graphql title="Will return an authorization error for non-admin users"
query {
  products {
    id
    name
    salePrice
    // highlight-next-line
    purchasePrice
  }
}
```

Note that the type-level access control will still be in effect. In this particular case, a non-admin user will get an authorization error if they try to create, update, or delete a product.

:::note Field-level access control and `self` object
Currently, Exograph imposes that field-level access control expressions must not use the `self` object. In other words, you may use context objects and literals. Please [let us know](https://github.com/exograph/exograph/issues) if you have a use case for using the `self` object.
:::

Like the type-level access control, you can specify separate access control expressions for queries and mutations. For example, you can specify that only "super-admin" users can mutate the `purchasePrice` field:

```exo
@postgres
module ProductDatabase {
  @access(query = true, mutation = AuthContext.role == "admin")
  type Product {
    @pk id: Int = autoIncrement()
    name: String
    salePrice: Float
    // highlight-start
    @access(
      query = AuthContext.role == "admin",
      mutation = AuthContext.role == "super-admin"
    )
    purchasePrice: Float
    // highlight-end
  }
}
```

Here, "admin" users can query the `purchasePrice` field, but only "super-admin" users can mutate it. You can specify separate access control expressions for creating, updating, and deleting mutations, like the access control at the type level.
