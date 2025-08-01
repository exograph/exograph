---
sidebar_position: 120
---

# FAQ

## License

Exograph is released under the [Business Source License 1.1 with an additional grant](https://github.com/exograph/exograph/blob/main/LICENSE). We've designed our licensing to be as permissive as possible while ensuring the sustainability of the project. The license offers:

- **Complete freedom for your applications**: Build, deploy, and monetize backends for your own products, client projects, and commercial services
- **No usage limits**: There are no restrictions on scale, number of users, or revenue generated using Exograph
- **Source code access**: The full source code is available for you to inspect, learn from, and modify
- **No vendor lock-in**: Because you have access to the source code, you're never locked into our service or support
- **Future open source**: After a specified period, Exograph will automatically become available under the [Apache 2.0 license](https://www.apache.org/licenses/LICENSE-2.0)

### Can I use Exograph to build and deploy a backend for my web and mobile application?

**Absolutely!** You can use Exograph to build and deploy production backends for your web applications, mobile apps, or any other type of application. This includes traditional APIs, MCP servers, or any other protocol that Exograph supports. There are no restrictions on this use case.

### Does my backend have to be open source?

**No!** Your backend application can be closed source, open source, or anything in between. You have complete freedom to choose how to license your own application.

### Can I build and deploy a backend for a client using Exograph?

**Absolutely!** Whether you're a freelancer, agency, or consulting company, you're free to build and deploy backends for your clients.

### Do I need to publish modifications I make to Exograph itself?

**No, you don't have to publish your modifications.** You're welcome to modify Exograph's source code for your own needs and keep those changes private. While we'd love contributions back to the project, you're under no obligation to do so.

### Can I offer Exograph as a hosted service?

**This is the only restriction in our license.** You cannot create a cloud platform where developers sign up, upload their Exograph models or connect their version control systems, and you deploy and manage their backends as a service.

If you're interested in offering Exograph as a hosted service, please [contact us](mailto:contact@exograph.dev) to discuss partnership or licensing options.

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
