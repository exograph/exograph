---
sidebar_position: 50
---

# Local Deployment

The "yolo" mode has served us fine so far by letting us focus on the core concepts. But that mode creates a temporary database, and no data between restarts persists. So, we will proceed to the next step to explore the "dev" mode.

## Creating a database

First, let's create a new database.

```shell-session
# shell-command-next-line
createdb concerts-db
```

## Setting up the environment

Let's create a .env file and populate the environment variables.

```sh file=.env
export EXO_POSTGRES_URL=postgres://localhost/concerts-db
export EXO_CORS_DOMAINS="*"
export EXO_JWT_SECRET=your_jwt_secret # Change this to your secret
```

We set the `EXO_POSTGRES_URL` environment variable to the database we created earlier. Here, we assume the default setup of Postgres, where the username is the same as the current user without a password. If you have a different setup, you should change the URL accordingly (the format is `postgres://username:password@host:port/database`)

We also set the `EXO_CORS_DOMAINS` environment variable to allow any domain to access the server. This is useful for development, but you should restrict it to your domain in production.

Finally, we set the `EXO_JWT_SECRET` environment variable to a secret key. This key is used to sign and verify the JWT token. You can use any string as the secret key, but it is recommended to use a long, randomly generated string.

:::tip Alternative authentication
`EXO_JWT_SECRET` enables symmetric JWT authentication (the same secret key encrypts and verifies the payload). You may alternatively use OpenID Connector-based JWT authentication by specifying the `EXO_OIDC_URL` environment variable. See [the documentation](/cli-reference/environment.md) for more details.
:::

## Starting the server in development mode

Now, start the server.

```shell-session
# shell-command-next-line
source .env
# shell-command-next-line
exo dev
```

In development mode, Exograph will watch for changes to the model and automatically apply the migration to the database. Therefore, except for using the database and the JWT secret you specified, everything else is identical to the "yolo" mode.

## Starting the server in production mode

Let's deploy the production version to a local server. First, we will build a serialized version of the model and modules (the "index.exo_ir" file) so that during runtime, Exograph won't have to deal with parsing errors, etc. It is also a far leaner version of the application since it avoids parsing, typechecking, and watching for changes to the model.

```shell-session
# shell-command-next-line
exo build
```

If you wish, you can create a separate database for production (which will be the case in your real setup). In that case, you should set the `EXO_POSTGRES_URL` environment variable to the production database URL.

Unlike `exo yolo` or `exo dev`, Exograph doesn't manage the database schema in production mode. Instead, it provides a command to generate the SQL schema from the model.Let's use `exo` command to update the database schema by applying the migration. Since this is a fresh database, migration entails creating the tables and related objects.

```shell-session
# shell-command-next-line
source .env
# shell-command-next-line
exo schema migrate --apply-to-database
```

Now, you can start the server in production mode.

```shell-session
# shell-command-next-line
EXO_ENV=production exo-server
```

Compared to the development mode:

- The server will not watch for changes to the model.
- The server will not automatically apply the migration to the database.
- The server will turn off the introspection support following the [best practices for production deployment](/production/introspection.md). However, you can still turn it on by setting the `EXO_INTROSPECTION` environment variable to `true`.

Everything should work as before, except for these differences.
