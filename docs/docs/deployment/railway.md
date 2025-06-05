---
sidebar_position: 10
---

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

# Railway

Let's deploy our application to [Railway](https://railway.app). There are a few ways to deploy an application to Railway. We use its support to deploy GitHub repositories, which allows us to deploy our application automatically whenever we publish it to GitHub.

We will explore two ways to use Postgres:

- **Railway-provided Postgres**: In this arrangement, we co-locate the app and database in Railway's infrastructure.

- **External Postgres**: In this arrangement, we use a provider specializing in Postgres and connect to it from Railway. We will illustrate using [Neon](https://neon.tech/), a "multi-cloud fully managed Postgres with a generous free tier". This free tier is perfect for this tutorial. If you want to use other Postgres providers, the basic steps remain the same: create a database and set the `EXO_POSTGRES_URL` environment variable to point to it.

## Creating a new application

import CreatingApp from './\_creating_app.md';

<CreatingApp/>

## Creating Deployment Files

The `exo deploy railway` command simplifies deploying Exograph application to Railway by creating `Dockerfile.railway` and `railway.yaml` files. The `Dockerfile.railway` file has two parts: the first part builds the exo_ir file and performs database migration, and the second part launches the Exograph server with that exo_ir file. The `railway.yaml` points to `Dockerfile.railway`. You may customize these files to suit your needs (for example, adding environment variables specific to your app). You should commit these files to your repository.

The difference between using Railway and external databases is the value we pass to the `--use-railway-db` option. If you don't specify it, Exograph will ask you the choice of database.

<Tabs groupId="database-choice">
  <TabItem value="railway" label="Railway Postgres" default>

```shell-session
# shell-command-next-line
exo deploy railway --use-railway-db=true
```

  </TabItem>
  <TabItem value="external" label="External Postgres">

```shell-session
# shell-command-next-line
exo deploy railway --use-railway-db=false
```

  </TabItem>
</Tabs>

This command creates the files described earlier and prints instructions to deploy the application to Railway.

:::note CORS
If you intend to consume the API through a web application, update `Dockerfile.railway` to set `EXO_CORS_DOMAINS=<comma-separated-domains>`.
:::

## Pushing to GitHub

Push the application to GitHub, which allows automatic deployment upon pushing to the specified GitHub branch.

```shell-session
# shell-command-next-line
git commit -am "Initial commit"
```

Now, create a new repository on GitHub and push the code to it.

## Deploying to Railway

Deploying the application to Railway is simple! Here is a video showing the process:

import railwaySelfDbVideo from './static/railway-self-db.mp4';
import railwayNeonDbVideo from './static/railway-neon.mp4';

<Tabs groupId="database-choice">
  <TabItem value="railway" label="Railway Postgres" default>
    <video controls width="100%">
      <source src={railwaySelfDbVideo}/>
    </video>
  </TabItem>
  <TabItem value="external" label="External Postgres">
    <video controls width="100%">
      <source src={railwayNeonDbVideo}/>
    </video>
  </TabItem>
</Tabs>

As shown in the video, we need to:

- Create a new Railway project.
- Add a "New Service" to the project and select "GitHub Repo" pointing to the repository we created earlier

<Tabs groupId="database-choice">
  <TabItem value="railway" label="Railway Postgres" default>

    - Provision a new database on that project
    - Add "Variable Reference" to the service for `DATABASE_URL` and `DATABASE_PRIVATE_URL` environment variables

  </TabItem>
  <TabItem value="external" label="External Postgres">

    - Sign up unless you already have an account. Create a new project by following the instructions [here](https://neon.tech/docs/get-started-with-neon/signing-up).
    - Add the `DATABASE_URL` environment variable to the service with the value set to the external database URL

  </TabItem>
</Tabs>

In either case,

- Wait for the deployment to complete
- Click on "Add a Domain" and note the server URL

## Testing the app

import TestingApp from './\_testing_app.md';

<TestingApp/>

## Updating the application

Updating the application is a simple matter of pushing the repository to GitHub. Railway will automatically deploy the new version. The first stage of `Dockerfile.railway` will attempt migration as well. The migration will succeed if you change the Exograph model without destructive changes. Otherwise, it will fail, and the deployment will fail (the older deployment will continue to serve the traffic). If that happens, you should apply migrations manually. See [Migrations](/cli-reference/development/schema/migrate.md) for more information.
