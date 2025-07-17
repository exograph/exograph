---
sidebar_position: 0
slug: /
---

# Introduction

With Exograph, you can build better backends fast!

Exograph offers a declarative way to create flexible, secure, and performant backends that provide GraphQL query and mutation APIs. Exograph lets you focus on your domain model and business logic, freeing you to pursue more creative work on your application. Furthermore, it offers tooling to support all stages of the development lifecycle, from development to deployment to maintenance.

Compared to the traditional way of writing GraphQL APIs, Exograph offers several advantages. Throughout this documentation, we will dive deep into each of them, but first, let's look at some of the highlights.

## Awesome developer experience

Exograph offers a great developer experience throughout the application lifecycle.

### Simple to get started

Once you install Exograph, you can get a simple GraphQL [server ready](getting-started/local.md#creating-a-simple-application) in seconds thanks to its [yolo](cli-reference/development/yolo.md) mode. As you refine your model, Exograph helps by checking that it is error-free and [giving precise error messages](application-tutorial/model.md#creating-the-model). In its [dev](cli-reference/development/dev.md) mode, Exograph automatically reloads your code as you make changes and checks for consistency between your model and the database. It also offers a [GraphiQL playground](getting-started/local.md#using-the-graphiql-interface) to try out queries and mutations. The [Exograph VS Code extension](https://marketplace.visualstudio.com/items?itemName=exograph.exograph) makes working with Exograph model files easier.

### Easy to develop

Exograph's declarative language expresses your backend's domain model and the API precisely and concisely. This TypeScript and GraphQL IDL-inspired language will make you feel at home.

Once you define your model, Exograph will take care of the rest. For example, with Postgres modules, Exograph will infer [queries](postgres/operations/queries.md) and [mutations](postgres/operations/mutations.md). As your model evolves, Exograph offers commands to [check the consistency](cli-reference/development/schema/verify.md) of your database schema with your model and to [migrate](cli-reference/development/schema/migrate.md) your database to the latest version of your model.

Exograph's language is Git-friendly. In our experience, UI-based approaches to defining models don't scale, and it is especially tough to manage when multiple developers update the model. Since Exograph's language is just text, it is easy to integrate with Git and other version control systems. It also makes it easy to collaborate with other developers, where the standard Git workflow works just as expected.

### Effortless to deploy

You can deploy Exograph to pretty much any cloud platform. Since the executable is a single binary with no runtime dependencies, you can run it on your laptop, a server, or a serverless platform. Depending on the platform and your preferences, you can deploy it as a Docker container or a standalone binary.
To further simplify the deployment process, Exograph offers commands to create a package with your application and instructions to deploy it to a platform like [Fly.io](deployment/flyio.md) and [AWS Lambda](deployment/aws-lambda.md).

Once deployed, you can monitor your application with Exograph's built-in [OpenTelemetry integration](production/telemetry.md).

### Trivial to test

Exograph also includes a [declarative testing framework](production/testing.md) that lets you write tests with very little code. Express your operations and the expected results, and Exograph will do the rest: create a database, start an Exograph server, and run your tests in parallel.

## Secure by design

One of the biggest challenges in building a GraphQL API is ensuring data access to only authorized users; it is so easy to miss access control enforcement, for example, with nested objects. By constantly having to think about access control, developers often end up with a complex and error-prone system. Even when carefully implemented, auditing access control rules is a challenge.

Exograph language design addresses [access control](postgres/access-control.md) as a core feature. Unless explicitly allowed, Exograph prevents access to any data, thus protecting against accidental data leaks.

In Exograph, you collocate access control rules with the data model. As a result, you can express complex access control rules concisely and in an easy-to-understand manner. The collocation also makes it easy to audit the access control rules. Once you define the rules, Exograph enforces them everywhere, no matter how a particular entity is accessed--directly or nested.

Furthermore, Exograph allows expressing access control rules in a fine-grained manner: based on user roles, based on the relationship between user and entity, based on captcha verification, and even on the time of day. All this is possible through [_contexts_](core-concept/context.md), a powerful mechanism to extract values from the request and environment alongside an expressive language to define rules.

## Performant

Exograph is written in Rust, which, along with careful design, helps it provide high performance, fast startup time, and low memory usage. These characteristics make it suitable for serverless platforms (and, of course, traditional cloud deployment too).

### Fast startup time

Typical Exograph server startup time is in low milliseconds. Exograph achieves fast startup by using _ahead-of-time building_. During the build phase, it parses the model, verifies it, and compiles it to an intermediate representation. Then, during runtime, it loads that representation into memory and starts serving requests. This separation shifts the burden of parsing and verifying the model from runtime to build time. As a result, the startup time is only the duration it takes to load the intermediate representation into memory.

### Low memory usage

Typical memory usage for an Exograph backend is in tens of megabytes. The build/runtime separation helps by relying on a pre-compiled intermediate representation to avoid expending memory on building the model. Furthermore, since Exograph is written in Rust, there is no garbage collector, which reduces peak memory usage.

### Fast execution

Besides security, efficiently implementing resolvers is one of the biggest challenges in building a GraphQL API. Exograph deals with this for you, so you only need to focus on the correctness of your model and not much else.

Typical query time in Exograph involving a database operation is in milliseconds. The build/runtime separation allows Exograph to compute several aspects ahead of time. This reduces the time it takes to execute a query or mutation.

For Postgres modules, Exograph generates optimized queries that do not require additional round trips to the database. For example, suppose you execute a GraphQL query that returns a list of users along with their blogs. Exograph will generate a single SQL query to fetch all users and their associated blogs in a single round trip to the database. Furthermore, Exograph will create queries that fetch only the data needed to satisfy the query. For example, if a GraphQL query returns a list of users and only requires the user's name, Exograph will generate an SQL query with only the user's name.

## Extensible

Exograph is extensible in multiple ways: Deno modules, interceptors, and contexts. These extensions let you implement any unique backend requirements without waiting for us to implement them.

### Deno modules

While most of your Exograph model will focus on persistence, you may need to execute business logic and integrate your backend with other systems. The [Deno modules](deno/defining-modules.md) support makes this possible. For example, you can write a Deno module to perform some computation, fetch data from another server, or send an email. The queries and mutations in a Deno module can have access control rules, too. Exograph then exposes queries and mutations in Deno modules alongside other modules.

By using Deno modules, you don't need to write separate backend (micro)services (you can still do that if you want). This reduces the number of moving parts, simplifying the deployment process and making it easier to maintain your application. Furthermore, since Exograph executes Deno modules in the same process, it avoids an extra network hop and reduces latency. You don't need to deal with network issues like timeouts and retries--especially when code is simply performing some computation.

### Interceptors

Want to implement a rate-limiting logic or audit who accessed which operations? Then, write a couple of [interceptors](core-concept/interceptor.md), and you are good to go. Exograph exposes a mechanism to add interceptors to the main-line operations that help you implement cross-cutting concerns such as rate-limiting, auditing, logging, performance monitoring, etc. Over time, we will provide many of the commonly needed functionality out of the box. However, we believe that developers should be able to extend Exograph to meet their needs without waiting for us and implement a particular cross-cutting concern that fits their needs even when we provide a built-in solution.

Interceptors also help augment queries and mutations with business-specific logic. For example, if you want to email when a user buys a ticket, you can write an interceptor for that specific mutation to send the email.

### Contexts

The [context](core-concept/context.md) mechanism in Exograph allows extracting values from the request and environment: headers, cookies, JWT tokens, and environment variables. You can even write [custom extractors](core-concept/context.md#processed-value) to get, for example, the current time, a captcha verification result, customer ID from an API key header, and so on.

You can then use these values to enforce access control rules by simply referencing them in the rules. For example, you can write rules that allow access to a user only if the user ID matches the customer ID. This way, you can extend the access control rules to include business logic and maintain the separation of concerns by isolating the code to extract the value from its usage.

You can inject context values into queries and mutations in Deno modules to implement business logic. For example, you can inject the customer ID into a query to fetch only the customer's data.

## Run anywhere

Exograph can run virtually anywhere. During development, you can run the server on your laptop. Then, depending on your use case and expected traffic pattern, you can deploy as a server (on Fly.io, AWS EC2, Azure VMs, Google Compute Engine, and so on) or as a serverless function (for example, on AWS Lambda, Azure Functions or Google Cloud Functions).
