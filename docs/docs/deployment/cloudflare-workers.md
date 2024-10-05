---
sidebar_position: 40
---

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

# Cloudflare Workers

Let's deploy our application to [Cloudflare Workers](https://developers.cloudflare.com/workers/).

The application needs a Postgres database. We will use [Neon](https://neon.tech/), a "multi-cloud fully managed Postgres with a generous free tier". This free tier is perfect for this tutorial. If you want to use other Postgres providers, the basic steps remain the same: create a database and set the `EXO_POSTGRES_URL` environment variable to point to it.

:::info Only Postgres is supported
Currently, Exograph on Workers supports only Postgres modules—that is, it does not support Deno modules. We are working on utilizing the host JavaScript engine to support JavaScript/TypeScript modules instead of Deno modules to offer the same functionality.
:::

## Creating a new application

If you already have an application, you can skip this step.

import CreatingApp from './\_creating_app.md';

<CreatingApp/>

## Creating Deployment Files

Exograph CLI includes a command to simplify deploying to Cloudflare Workers.

```shell-session
# shell-command-next-line
exo deploy cf-worker
```

This command creates the WebAssembly binaries, `wrangler.toml`, and `.dev.vars` to configure the application.

The `wrangler.toml` file defines environment variables and bindings for your production deployment. You should commit this file to your repository (therefore, you should not include secrets in this file).

The `.dev.vars` file contains the environment variables for _local_ development. You must update this file to point to a development Postgres database. Typically, you don't need to commit this file to your repository.

## Trying it out locally

To run the backend locally, modify `.dev.vars` to point to your development Postgres URL. Then run the following command:

```shell-session
# shell-command-next-line
npx wrangler dev
```

It will start a local server at `http://localhost:8787` (it may use another port if 8787 isn't available). To test the application, you can launch `exo playground` and set the endpoint to `http://localhost:8787`.

```shell-session
# shell-command-next-line
exo playground --endpoint http://localhost:8787
```

You can try queries and mutation to test the application with your development Postgres database.

## Deploying to Cloudflare Workers

Before you try accessing the application, you must connect the worker to a Postgres database. You can do so by provisioning a Postgres database and setting the `EXO_POSTGRES_URL` secret:

```sh
# shell-command-next-line
exo secret set EXO_POSTGRES_URL <postgres-database-url>
```

Alternatively, for database providers like Neon and Supabase, you can use the Cloudflare Workers "Integration" tab, click "Add Integration", and follow the instructions. The result will be setting the `DATABASE_URL` secret for you.

Next, run the database migrations to create the necessary tables in the database.

```sh
exo schema migrate --database <postgres-database-url> --apply-to-database
```

Finally, let's deploy to Cloudflare.

```sh
npx wrangler deploy
```

That's it! You now have a GraphQL server connected to a Postgres database running on Cloudflare Workers.

If you wish to play with the APIs, launch the playground by running the following command:

```sh
exo playground --endpoint <cloudflare-worker-url>
```

You can try the typical queries, such as creating a new todo, as seen in the [Getting Started](../getting-started#using-the-graphiql-interface) guide.

## Improving latency

With the earlier configuration, the Exograph worker connected directly to the database. Due to the ephemeral nature of Cloudflare Workers, every request establishes a new connection to the database, which can be slow. You can improve the latency by using [Hyperdrive](https://developers.cloudflare.com/hyperdrive/).

To set up a Hyperdrive, use the `npx wrangler hyperdrive create` command or the Cloudflare Worker's dashboard. Then add the following to your `wrangler.toml` (you will get the `id` from the command output or the Hyperdrive dashboard):

```toml
EXO_HYPERDRIVE_BINDING = "<binding-name>"

[[hyperdrive]]
binding = "<binding-name>"
id = "..."
```

The worker will now use Hyperdrive to manage the database connections, significantly reducing the latency of the requests.
