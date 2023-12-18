---
sidebar_position: 120
---

# FAQ

## The Exograph Language

### Why doesn't Exograph use the GraphQL schema language?

At first, using the GraphQL schema definition language to describe the model, types, and queries is tempting. The initial idea seems reasonable. For example, you could describe a model like this:

```graphql
type Blog {
  id: Int!
  title: String!
  posts: [Post!]!
}
```

Soon, however, you must summon directives to describe the access rules and other metadata.

```graphql
type Blog @access("self.published || AuthContext.role == 'admin'") {
  id: Int! @pk @autoIncrement("blog_id_seq")
  published: Boolean!
  title: String!
  posts: [Post!]!
}
```

Here the `@access` directive embeds a language, but it has to be a string to fit into the GraphQL schema language.

Next, we need to describe the concept of "context". Here, too, we will have to use a directive.

```graphql
type AuthContext @context {
  role: String!
}
```

And then, there is an issue with describing modules and interceptors, which will require a bunch of new directives.

In a way, even if we were to somehow shoe-horn the GraphQL schema language to describe the model, we would still need to invent a new language in the form of directives, thus rendering that part opaque to GraphQL tooling and requiring us to write our own tooling anyway. Developers may understand the `type` part of the schema, but all the directives will overwhelm and obscure the code.

Internally, it doesn't help us avoid complexity in implementing the parser and typechecker, either. First, we would still have to parse the directives and the embedded language. Second, we still have to typecheck the content of directives.

That is why we chose a Typescript-inspired language to describe the model. Here the language focuses on describing models, their relations, access rules, and behaviors directly and concisely.

### But why not use Typescript then...?

We could have used Typescript, but it would have been a bit of a stretch. The language is not focused on describing data models. It is designed to describe programs.

If we chose Typescript, we would have to choose a subset of the language, which would have made developers keep guessing which part of the Typescript we support. We would have to also write a parser and typechecker for the subset of the language we support and that would put us back into the same problem as choosing the GraphQL schema language.
