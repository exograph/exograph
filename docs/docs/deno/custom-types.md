---
sidebar_position: 2.1
---

# Defining Custom Types

So far, we have used only primitive types (`Int`, `String`, `Boolean`, etc), but often you need queries and mutations to accept and return custom types. You can define such a type with the `type` keyword.

Let's implement a module that fetches a to-do item from [JSON Placeholder](https://jsonplaceholder.typicode.com/). We know that the response is a JSON object with the following structure:

```json
{
  "id": 1,
  "userId": 1,
  "title": "delectus aut autem",
  "completed": false
}
```

We capture this structure through the `Todo` type.

```exo
@deno("todo.ts")
module TodoModule {
    @access(true)
    type Todo {
        id: Int
        userId: Int
        title: String
        completed: Boolean
    }

    @access(true)
    query todo(id: Int): Todo
}
```

# Implementing the Module

The corresponding `todo.ts` file looks as follows (the skeleton code, including the interface types, will be generated automatically if the file doesn't exist):

```typescript
interface Todo {
  id: number;
  userId: number;
  title: string;
  completed: boolean;
}

export async function todo(id: number): Promise<Todo> {
  const r = await fetch(`https://jsonplaceholder.typicode.com/todos/${id}`);
  return r.json();
}
```

The `Todo` type will become part of the GraphQL schema and thus will offer code completion in the GraphQL Playground and facilitate query validation, etc.

Note the use of the `async` keyword. Exograph allows you to use `async` functions in your module implementation, which is often necessary to interact with external modules over the network.

If you were to implement the same module in JavaScript, the only difference would be the lack of types.

:::note
You may use a type as an argument or a return type, but not both. This is due to the GraphQL specification, which considers input and output types as different.
:::
