---
sidebar_position: 1
---

# Defining Modules

Defining a Deno module comes in two parts:

- declaration of queries and mutations in an exo file
- implementation of the module in a TypeScript or JavaScript file

## Declaring a module

You may use the `@deno` annotation to define a module implemented in TypeScript or JavaScript. The annotation takes a single parameter: the TypeScript or JavaScript file path.

Here is an example of a simple module implemented in TypeScript.

```exo
@deno("math.ts")
module MathModule {
    @access(true)
    query add(a: Int, b: Int): Int

    @access(true)π
    query square(a: Int): Int
}
```

The `@deno` annotation of the `MathModule` module specifies that the implementation of the module is in the `math.ts` file. The module declares two queries: `add` and `square`. The `add` query takes two `Int` arguments and returns an `Int` value. The `square` query takes a single `Int` argument and returns an `Int` value.

Note the `@access` annotation on the queries. It specifies that the query is accessible to all users (by default, queries and mutations aren't accessible to anyone). You can specify a fine-grained access control by using the `@access` annotation as we will see [later](access-control.md).

## Implementing a module in TypeScript

For each declared query (or mutation), the corresponding TypeScript code must export a function that matches the query name. Each such function must take the same arguments as the query with each type appropriately mapped to the corresponding TypeScript type. For example, if an argument or return type is `Int`, the TypeScript type would be `number`. The function must return a value that matches the return type of the query.

```typescript
export function add(a: number, b: number): number {
  return a + b;
}

export function square(a: number): number {
  return a * a;
}
```

:::note
When you run `exo dev` (or `exo yolo`), Exograph will generate a starter TypeScript file if it doesn't already exist. Exograph doesn't update the file if it already exists, so you must manually do so.
:::

Once you have the above code, you can run `exo dev`. You can then run the `add` query in GraphQL Playground:

```graphql
query {
  add(a: 2, b: 3)
}
```

which should return:

```json
{
  "data": {
    "add": 5
  }
}
```

Similarly, you can run the `square` query:

```graphql
query {
  square(a: 2)
}
```

which should return:

```json
{
  "data": {
    "square": 4
  }
}
```

## Implementing a module in JavaScript

If you choose to use JavaScript instead of TypeScript, you need to change the name of the file to `math.js`:

```exo
@deno("math.js")
module MathModule {
    @access(true)
    query add(a: Int, b: Int): Int

    @access(true)
    query square(a: Int): Int
}
```

The `math.js` file looks pretty much the same as the TypeScript file, except that you don't need to specify the type of the arguments or the return value.

```javascript
export function add(a, b) {
  return a + b;
}

export function square(a) {
  return a * a;
}
```

You can now execute the same queries as before.

So far, we have used only primitive types (`Int`), but often you need to use more complex types. Let's see [how you can do that](custom-types.md).
