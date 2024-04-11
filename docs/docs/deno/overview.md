---
sidebar_position: 0
slug: /deno
---

# Overview

While modules written using the [Postgres plugin](/postgres/overview.md) offer API for your application's persistent data model, you often need to implement additional business logic. For example, you may need to implement logic to send marketing emails, sign up users, or perform some mathematical computation. Furthermore, you may need to intercept any existing query or mutation to perform additional logic. For example, you may need to impose complex validation rules on the data before persisting with the database or implementing a rate-limiting policy. Exograph offers to express such logic using TypeScript or JavaScript.

Exograph uses powerful [Deno](https://deno.land/) runtime to execute module code using TypeScript and JavaScript. The embedded Deno runtime allows the use of all the Deno features in your module.

## Setting up

To simplify the developer experience, Exograph automatically bundles JavaScript and TypeScript code during `exo build` (or commands that indirectly invoke `exo build`, such as `exo dev` and `exo yolo`).
