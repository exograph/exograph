---
sidebar_position: 20
---

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

# Fly.io

Let's deploy our application to [Fly.io](https://www.fly.io/), which has an excellent user experience. You can adapt this tutorial for other cloud providers that support Docker.

We will use two Postgres providers:

- **Fly.io**: This allows us to work with a single cloud provider. However, the Postgres provided by Fly.io is [not a managed database](https://fly.io/docs/postgres/getting-started/what-you-should-know/), which puts the onus on you to keep its image updated, etc.
- **External**: We will illustrate using [Neon](https://neon.tech/), a "multi-cloud fully managed Postgres with a generous free tier". The basic steps will remain the same if you want to use other Postgres providers.

## Installing the prerequisites

You need to install [Flyctl](https://fly.io/docs/getting-started/installing-flyctl/).

## Creating a new application

import CreatingApp from './\_creating_app.md';

<CreatingApp/>

## The `exo deploy fly` command

Exograph has a dedicated command to work with Fly.io. It will:

- Configure a `fly.toml` file
- Create two Docker files: `Dockerfile.fly` and `Dockerfile.fly.builder` (the latter is used to simplify secrets management)
- Provide deployment instructions

It offers several options to customize the deployment, and you can learn more by running `exo deploy fly --help`.

```shell-session
# shell-command-next-line
exo deploy fly --help
Deploy to Fly.io

Usage: exo deploy fly [OPTIONS] --app <app-name> [model]

Arguments:
  [model]  The path to the Exograph model file. [default: index.exo]

Options:
  -a, --app <app-name>       The name of the Fly.io application to deploy to
  -e, --env <env>            Environment variables to pass to the application (e.g. -e KEY=VALUE). May be specified multiple times.
      --env-file <env-file>  Path to a file containing environment variables to pass to the application
      --use-fly-db           Use database provided by Fly.io
  -h, --help                 Print help
```

## Setting up

From the application's directory, run the following command:

<Tabs groupId="database-choice">
  <TabItem value="fly" label="Fly Postgres" default>

We want to use the Postgres database provisioned on Fly.io, hence we pass the `--use-fly-db` option.

```shell-session
# shell-command-next-line
exo deploy fly --app todo --use-fly-db
```

  </TabItem>
  <TabItem value="external" label="External Postgres">

```shell-session
# shell-command-next-line
exo deploy fly --app todo
```

  </TabItem>
</Tabs>

:::note CORS
If you intend to consume the API through a web application, you would need to set CORS domains by passing `-e EXO_CORS_DOMAINS=<comma-separated-domains>` to `exo deploy fly` or changing the generated fly.toml file.
:::

The command creates a `fly.toml` file in the current directory. You can edit it to customize the deployment. For example, you can change the number of instances, the regions, etc. See the [Fly.io documentation](https://fly.io/docs/reference/configuration/) for more details.

Similarly, it creates a `Dockerfile.fly` in the current directory. You can edit it to customize the Docker image. For example, you can add more dependencies, set up the timezone, etc. See the [Docker documentation](https://docs.docker.com/engine/reference/builder/) for more details. It also creates `Dirverfile.fly.builder` to simplify secrets management (see [Fly secret management](https://fly.io/docs/reference/build-secrets/#automate-the-inclusion-of-build-secrets-using-an-ephemeral-machine) for more information). You are unlikely to edit it.

It also gives a step-by-step guide to deploying the application. We will follow those instructions.

## Deploying the app

Let's follow the instructions.

### Create the app

The first command will create the app in Fly.io. When presented with the option to select an organization, select an appropriate one.

```shell-session
# shell-command-next-line
fly apps create todo
? Select Organization: <your account name>
New app created: todo
```

### Set the JWT secret or the OIDC URL

Fly.io offers a secret vault for sensitive information like the JWT secret (you should _not_ use an environment variable for such values). We will use it to store authentication-related secrets. In the command below, replace `<your-jwt-secret>` with your own.

```shell-session
# shell-command-next-line
fly secrets set --app todo EXO_JWT_SECRET=<your-jwt-secret>
Secrets are staged for the first deployment
```

Alternatively, you can use an OIDC URL. In the command below, replace `<your-oidc-url>` with your own.

```shell-session
# shell-command-next-line
fly secrets set --app todo EXO_OIDC_URL=<your-oidc-url>
Secrets are staged for the first deployment
```

### Create the database

<Tabs groupId="database-choice">
  <TabItem value="fly" label="Fly Postgres" default>

Since we opted to use the Fly.io database, let's create one:

```shell-session
# shell-command-next-line
fly postgres create --name todo-db
```

  </TabItem>
  <TabItem value="external" label="External Postgres">

Create a new project by following the instructions [here](https://neon.tech/docs/get-started-with-neon/signing-up).

  </TabItem>
</Tabs>

### Attach the database to the app

The next step attaches the database to the app, which creates the database instance and the user for the app.

<Tabs groupId="database-choice">
  <TabItem value="fly" label="Fly Postgres" default>

```shell-session
# shell-command-next-line
fly postgres attach --app todo todo-db
```

  </TabItem>
  <TabItem value="external" label="External Postgres">

You must set the `DATABASE_URL` secret to point to the database. You can get the database URL from the Neon dashboard (which will look like `postgres://...neon.tech/todo-db`).

```shell-session
# shell-command-next-line
fly secrets set --app todo DATABASE_URL=<your-postgres-url>
```

  </TabItem>
</Tabs>

### Deploy the app

Finally, we follow the suggested command to deploy the app:

```shell-session
# shell-command-next-line
flyctl console --dockerfile Dockerfile.fly.builder -C "/srv/deploy.sh" --env=FLY_API_TOKEN=$(flyctl auth token)
```

This command creates an ephemeral container, which deploys the Docker image to Fly.io. See [Fly.io documentation](https://fly.io/docs/reference/build-secrets/#automate-the-inclusion-of-build-secrets-using-an-ephemeral-machine) for more details.

At the end of the deployment, you will see a message like this:

```shell-session
Visit your newly deployed app at https://exo-concerts.fly.dev/
Waiting for ephemeral machine 1781109ec04128 to be destroyed ... done.
```

## Testing the app

import TestingApp from './\_testing_app.md';

<TestingApp/>

:::tip
Run the `fly logs` command to see the app's logs. Note that the most significant factor for execution speed will be the regions in which your app and database are deployed. If they are in the same region, the latency will be smaller.
:::
